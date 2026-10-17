//! 라이브 자원 동기 — 자원 엔티티 생성·갱신·제거와 동물 배위·건축 발판·
//! 성장 단계 적용(apply.rs에서 기능별 분리, 파일 500줄 규칙). 서버
//! resources 행을 소품 GLB·절차 발판으로 투영한다.

use bevy::prelude::*;

use super::{NetState, NetStatus};
use crate::world::data::{PropKind, WorldData};
use super::visual_map::{ResState, bld_size, live_visual};

#[derive(Component)]
pub struct LiveResourceTag(pub u32);

/// 구조물 kind — 타일을 선점해 나무·장식이 같은 타일에 뚫고 서지 못하게 한다.
pub(crate) fn structural_kind(kind: &str) -> bool {
    matches!(kind, "hut" | "campfire" | "stash" | "grave" | "relic")
}

/// 구조물 청소 반경(타일, 체비셰프) — 주변까지 포함해 나무·덤불 스폰을 양보한다.
/// 에셋이 늦게 로딩돼도(나무 GLB가 마지막에 뜸) 기본 뷰에서 집이 숲에 묻히지
/// 않게 하는 클라이언트 투영. 구조물 집합의 3×3 확장 좌표를 반환.
pub(crate) fn cleared_tiles(
    structural: &std::collections::HashSet<(u16, u16)>,
) -> std::collections::HashSet<(i32, i32)> {
    let mut out = std::collections::HashSet::new();
    for &(x, y) in structural {
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                out.insert((x as i32 + dx, y as i32 + dy));
            }
        }
    }
    out
}

/// 자원 상태 서명 — 변화(사망·건축 단계 진행) 감지용.
/// hut은 정체성 필드(stage·type·w·h)만 — work/progress는 스냅숏마다 변해
/// 재스폰 깜빡임을 일으키므로 제외하고 진행률은 SiteMeta를 통해 갱신한다.
/// (동물 x/y는 행 필드라 애초에 서명에 없다 — 배회는 RoamTo로 부드럽게.)
fn res_sig(r: &super::LiveResource, vis: bool) -> u64 {
    let mut h = 1469598103934665603u64;
    let mut feed = String::new();
    if r.kind == "hut" {
        let st: ResState = r
            .state
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        feed.push_str(st.stage.as_deref().unwrap_or(""));
        feed.push('|');
        feed.push_str(st.bld_type.as_deref().unwrap_or(""));
        feed.push_str(&format!("|{}|{}", st.w.unwrap_or(0), st.h.unwrap_or(0)));
    } else {
        feed.push_str(r.state.as_deref().unwrap_or(""));
    }
    for b in r
        .kind
        .bytes()
        .chain(r.species.bytes())
        .chain(feed.bytes())
        .chain(std::iter::once(if vis { 1 } else { 0 }))
    {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// 자원 상태 서명 컴포넌트 — 변화 시 재스폰 트리거.
#[derive(Component)]
pub struct LiveResMeta(pub u64);

/// 동물 배위 목적지(§17.64) — 서버 x/y diff를 재스폰 없이 부드러운 이동으로.
#[derive(Component)]
pub struct RoamTo(pub Vec3);

/// 자원 동기화 — 신규 스폰 / 사라진 것·상태 변화(사망·건축 단계) 재스폰.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 메시·재질은 스폰 직접 주입용
#[allow(clippy::type_complexity)] // 동기 루프 쿼리 — 배위·발판 메타까지 읽는 6튜플
pub fn sync_resources(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_mats: ResMut<Assets<bevy::pbr::StandardMaterial>>,
    mut q_live: Query<(
        Entity,
        &LiveResourceTag,
        Option<&LiveResMeta>,
        Option<&crate::world::models::PropRoot>,
        Option<&mut RoamTo>,
        Option<&mut crate::world::bld::SiteMeta>,
    )>,
    mut first_log: Local<u32>,
    // 구조물 청소 집합 캐시 — (서명, 3×3 확장 좌표). 매 프레임 재구축(구조물×9
    // 삽입+할당) 대신 자원 목록 서명이 바뀔 때만 다시 계산한다.
    mut cleared_cache: Local<(u64, std::collections::HashSet<(i32, i32)>)>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else {
        return;
    };

    // 갱신·제거 — 서명이 바뀌면 despawn(다음 프레임 새 시각으로 재스폰).
    // 동물 좌표 이동(§17.64 배회)은 서명과 무관 — RoamTo 목적지만 갱신하고,
    // 공사 진행률은 SiteMeta에 흘려 발판을 재스폰 없이 바만 움직인다.
    for (e, tag, meta, _, roam, site) in &mut q_live {
        let Some(r) = frame.resources.iter().find(|r| r.id == tag.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let st: ResState = r
            .state
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let vis = live_visual(&r.kind, &r.species, &st).is_some();
        let sig = res_sig(r, vis);
        match meta {
            Some(m) if m.0 == sig => {
                if r.kind == "animal" && st.alive != Some(false) {
                    let ground = world.height_at(r.x as i32, r.y as i32);
                    let to = Vec3::new(r.x as f32 + 0.5, ground, r.y as f32 + 0.5);
                    match roam {
                        Some(mut rt) => rt.0 = to,
                        None => {
                            commands.entity(e).insert(RoamTo(to));
                        }
                    }
                }
                if r.kind == "hut"
                    && let Some(mut m) = site
                {
                    m.progress = st.progress.unwrap_or(0.0).clamp(0.0, 1.0);
                }
            }
            _ => commands.entity(e).despawn(),
        }
    }

    // 신규 스폰
    *first_log += 1;
    if *first_log == 2 {
        let mut total = 0;
        let mut animals = 0;
        let mut buildings = 0;
        for (_, _, _, prop, _, _) in &q_live {
            total += 1;
            if let Some(p) = prop {
                if matches!(
                    p.kind,
                    PropKind::AnimalBoar
                        | PropKind::AnimalDeer
                        | PropKind::AnimalWaterDeer
                        | PropKind::AnimalSquirrel
                        | PropKind::AnimalPheasant
                ) {
                    animals += 1;
                }
                if matches!(
                    p.kind,
                    PropKind::Hut | PropKind::StorageHut | PropKind::Tent
                ) {
                    buildings += 1;
                }
            }
        }
        info!("[net] 라이브 자원 실체 {total}개 (동물 {animals}·건물 {buildings})");
    }
    // 구조물 청소 반경 집합(3×3) — 나무·덤불이 구조물을 뒤덮는 것 방지.
    // 서버가 같은 타일·인접 타일에 나무와 오두막을 함께 두는 데이터에서도 건물이
    // 이긴다. 나무 GLB가 가장 늦게 로딩되므로 기본 뷰의 집이 숲에 묻히지 않는다.
    // 서명(구조물 좌표 폴드)이 바뀔 때만 확장 집합을 재계산 — 프레임당 할당 제거.
    let sig = frame
        .resources
        .iter()
        .filter(|r| structural_kind(&r.kind))
        .fold(frame.resources.len() as u64, |a, r| {
            a.rotate_left(5) ^ (r.x as u64).wrapping_mul(31) ^ (r.y as u64).wrapping_mul(97)
        });
    if cleared_cache.0 != sig {
        let structural: std::collections::HashSet<(u16, u16)> = frame
            .resources
            .iter()
            .filter(|r| structural_kind(&r.kind))
            .map(|r| (r.x, r.y))
            .collect();
        cleared_cache.1 = cleared_tiles(&structural);
        cleared_cache.0 = sig;
    }
    let cleared = &cleared_cache.1;
    // 기존 실체 id 사전 — 신규 판정을 O(N×M) 순회 대신 해시 조회로.
    let live_ids: std::collections::HashSet<u32> =
        q_live.iter().map(|(_, t, _, _, _, _)| t.0).collect();
    // 스폰 예산 — 수천 자원이 한 프레임에 몰리면 씬 인스턴스화가 메인 스레드를
    // 수십 초 묶는다(2026-10-17 프리즈). 64/프레임 분산 — 잔여분은 다음 프레임
    // 호출에서 자동 이어진다(태그 없음 = 신규 판정).
    let mut budget: usize = 64;
    for r in &frame.resources {
        if live_ids.contains(&r.id) {
            continue;
        }
        if budget == 0 {
            break;
        }
        budget -= 1;
        let st: ResState = r
            .state
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        if world.terrain_at(r.x as i32, r.y as i32) == "water" {
            continue;
        }
        // ── hut — 다중 타일 발판(§17.75)·창불(§17.78). 서버 앵커=좌상단 타일,
        //    표현은 발판 중심. 공사 중=절차 발판(말뚝·밧줄·링·진행 바·재료),
        //    완공=타입별 GLB + 발판 안 주민 점등 창불.
        if r.kind == "hut" {
            let bld_type = st.bld_type.clone().unwrap_or_else(|| "HUT".into());
            let (w, h) = bld_size(&st, &bld_type);
            let stage = st.stage.clone().unwrap_or_else(|| "COMPLETED".into());
            let cx = r.x as f32 + w as f32 / 2.0;
            let cz = r.y as f32 + h as f32 / 2.0;
            let ground = world.height_at(r.x as i32 + w as i32 / 2, r.y as i32 + h as i32 / 2);
            let sig = res_sig(r, true);
            let rect = crate::world::bld::SiteRect {
                x: r.x as i32,
                y: r.y as i32,
                w,
                h,
            };
            if stage != "COMPLETED" {
                let site = if super::dbg_gate::off("no_scaffold") {
                    commands.spawn(Transform::default()).id() // 진단 게이트 — 빈 발판
                } else {
                crate::world::bld::spawn_site(
                    &mut commands,
                    &mut meshes,
                    &mut std_mats,
                    r.x as i32,
                    r.y as i32,
                    w,
                    h,
                    ground,
                    st.progress.unwrap_or(0.0).clamp(0.0, 1.0),
                )
                };
                commands
                    .entity(site)
                    .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
                // 재료 더미 — 부지 준비 단계의 장작(FE SITE_PREPARED 재료 표현)
                let log_h = crate::world::models::prop_model(PropKind::WoodLog).0 * 0.9;
                let pile = commands
                    .spawn((
                        crate::world::models::PropRoot {
                            kind: PropKind::WoodLog,
                            target_h: log_h,
                            ground_y: ground,
                            rot_y: (r.id % 360) as f32 / 360.0 * std::f32::consts::TAU,
                            scale_jitter: 1.0,
                            variants: crate::world::models::prop_model(PropKind::WoodLog).1,
                        },
                        bevy::world_serialization::WorldAssetRoot(
                            crate::world::models::prop_scene(PropKind::WoodLog, &assets),
                        ),
                        Transform::from_translation(Vec3::new(
                            r.x as f32 + 0.35,
                            ground,
                            r.y as f32 + 0.35,
                        )),
                    ))
                    .id();
                commands
                    .entity(pile)
                    .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
            } else if let Some((kind, target_h)) = live_visual(&r.kind, &r.species, &st) {
                let b = commands
                    .spawn((
                        crate::world::models::PropRoot {
                            kind,
                            target_h,
                            ground_y: ground,
                            rot_y: (r.id % 4) as f32 * std::f32::consts::FRAC_PI_2,
                            scale_jitter: 1.0,
                            variants: crate::world::models::prop_model(kind).1,
                        },
                        bevy::world_serialization::WorldAssetRoot(
                            crate::world::models::prop_scene(kind, &assets),
                        ),
                        LiveResourceTag(r.id),
                        LiveResMeta(sig),
                        Transform::from_translation(Vec3::new(cx, ground, cz)),
                    ))
                    .id();
                let glow = crate::world::bld::spawn_window_glow(
                    &mut commands,
                    &mut meshes,
                    &mut std_mats,
                    rect,
                    cx,
                    cz,
                    ground,
                    target_h,
                );
                commands
                    .entity(glow)
                    .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
                let _ = b;
            }
            continue;
        }
        let Some((kind, target_h)) = live_visual(&r.kind, &r.species, &st) else {
            continue;
        };
        if matches!(
            kind,
            PropKind::TreeBroadleaf
                | PropKind::TreePine
                | PropKind::TreePalm
                | PropKind::TreeSapling
                | PropKind::BerryBush
        ) && cleared.contains(&(r.x as i32, r.y as i32))
        {
            continue; // 구조물 청소 반경 양보 — 지붕 뚫기·집 가림 방지
        }
        let vis = true;
        let sig = res_sig(r, vis);
        let ground = world.height_at(r.x as i32, r.y as i32);
        let variants = crate::world::models::prop_model(kind).1;
        let e = commands
            .spawn((
                crate::world::models::PropRoot {
                    kind,
                    target_h,
                    ground_y: ground,
                    rot_y: (r.id % 360) as f32 / 360.0 * std::f32::consts::TAU,
                    scale_jitter: 1.0,
                    variants,
                },
                bevy::world_serialization::WorldAssetRoot(crate::world::models::prop_scene(
                    kind, &assets,
                )),
                LiveResourceTag(r.id),
                LiveResMeta(sig),
                Transform::from_translation(Vec3::new(r.x as f32 + 0.5, ground, r.y as f32 + 0.5)),
            ))
            .id();
        // 유물 — 지면 위 부유+자전(주민 시선 유도, FE 아이콘 sin 트윈)
        if kind == PropKind::Relic {
            commands.entity(e).insert(crate::world::propsfx::FloatBob {
                base_y: ground + 0.22,
                phase: (r.id % 628) as f32 / 100.0,
            });
        }
        // 광맥 — 귀한 자원 점멸 스파크(FE spark, 위상은 id로 결정론 분산)
        if kind == PropKind::GoldVein {
            let spark = crate::world::propsfx::spawn_vein_spark(
                &mut commands,
                &mut meshes,
                &mut std_mats,
                r.x as f32 + 0.5,
                ground,
                r.y as f32 + 0.5,
                (r.id % 628) as f32 / 100.0,
            );
            commands
                .entity(spark)
                .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
        }
        // 모닥불 — 발광 불꽃·라이트는 PropRoot와 별도 엔티티(데모 경로와 공통 헬퍼).
        // 동일 동기화 태그+서명 부여로 제거·상태변화 시 모델과 함께 철거·재생성.
        if kind == crate::world::data::PropKind::Campfire {
            let (flame, glow) = crate::world::models::spawn_campfire_fire(
                &mut commands,
                &mut meshes,
                &mut std_mats,
                r.x as f32 + 0.5,
                r.y as f32 + 0.5,
                ground,
            );
            commands
                .entity(flame)
                .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
            commands
                .entity(glow)
                .insert((LiveResourceTag(r.id), LiveResMeta(sig)));
        }
    }
}

/// 동물 배위 이동(§17.64) — RoamTo 목적지로 등속 보간 + 이동 방향 바라보기.
/// FE의 좌우 반환 이동에 해당하는 3D 환 — 재스폰 깜빡임 없이 자연스럽게 걷는다.
pub fn live_res_move(time: Res<Time>, mut q: Query<(&RoamTo, &mut Transform)>) {
    const SPEED: f32 = 0.6; // 타일/초 — 배위 동물은 주민보다 느리게
    let dt = time.delta_secs();
    for (to, mut tr) in &mut q {
        let dx = to.0.x - tr.translation.x;
        let dz = to.0.z - tr.translation.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < 1e-4 {
            continue;
        }
        let step = SPEED * dt;
        if dist <= step {
            tr.translation.x = to.0.x;
            tr.translation.z = to.0.z;
        } else {
            tr.translation.x += dx / dist * step;
            tr.translation.z += dz / dist * step;
            tr.rotation = Quat::from_rotation_y(dx.atan2(dz));
        }
    }
}
