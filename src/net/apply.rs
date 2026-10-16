//! 라이브 프레임 → 렌더 적용 — 지형 프라임(최초 1회)·시간·자원·주민.
//! 데모 WorldData를 라이브 지형으로 교체(평면 — 실 맵은 절벽 없음)하고
//! 서버 id 단위로 자원/주민 엔티티를 생성·갱신·제거한다.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use super::{LiveFrame, NetState, NetStatus, TiledMap};
use crate::world::data::{PropKind, WorldData};

#[derive(Component)]
pub struct LiveResourceTag(pub u32);

#[derive(Component)]
pub struct LiveAgentTag(pub String);

/// 프레임 공백 유예(초) — 재접속·드랍 프레임 사이 주민 엔티티를 즉시 철거하지
/// 않아 선택 링·카메라 추적이 튕기지 않게 한다. 프레임에 있으면 0으로 리셋.
#[derive(Component, Default)]
pub struct AgentGrace(pub f32);

/// 라이브 전환 — 데모 지형·물·소품·주민 제거 후 라이브 지형 스폰(1회).
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 데모 철거+라이브 지형 스폰 자원들
pub fn prime_live_world(
    mut commands: Commands,
    mut net: ResMut<NetState>,
    mut primed: Local<bool>,
    q_terrain: Query<Entity, With<crate::world::terrain::TerrainMesh>>,
    q_water: Query<Entity, With<crate::world::water::WaterMesh>>,
    q_props: Query<Entity, With<crate::world::models::PropRoot>>,
    q_villagers: Query<
        Entity,
        (
            With<crate::world::villager::Villager>,
            Without<LiveAgentTag>,
        ),
    >,
    mut world: ResMut<WorldData>,
    mut cam_rig: ResMut<crate::world::camera_rig::CameraRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_mats: ResMut<Assets<bevy::pbr::StandardMaterial>>,
    mut water_mats: ResMut<Assets<crate::world::water::WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut terrain_mats: ResMut<Assets<crate::world::splat::TerrainMaterial>>,
) {
    if *primed || net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else {
        return;
    };
    let Some(map_json) = frame.map.as_ref() else {
        return;
    };
    let Ok(map) = serde_json::from_str::<TiledMap>(map_json) else {
        warn!("[net] mapJson 파싱 실패");
        return;
    };
    *primed = true;
    info!("[net] 라이브 지형 프라임 {}×{}", map.width, map.height);

    // 데모 세계 철거
    for e in &q_terrain {
        commands.entity(e).despawn();
    }
    for e in &q_water {
        commands.entity(e).despawn();
    }
    for e in &q_props {
        commands.entity(e).despawn();
    }
    for e in &q_villagers {
        commands.entity(e).despawn();
    }

    // 라이브 데이터로 교체 — GID 그리드, 릴리프 합성
    *world = WorldData::from_map(&map.layers[0].data, map.width, map.height);
    // 카메라 재중심 — 맵 중심이 아니라 '맵 중심에 가장 가까운 주민'에게.
    // 주민들은 각자 멀리서 활동 중(채집·이동)이라 광장을 비추면 첫 화면에
    // 캐릭터가 안 보인다 — 관전 부팅 = 사람이 보이는 화면부터.
    let center = Vec2::splat(world.size as f32 / 2.0);
    cam_rig.target = frame
        .agents
        .iter()
        .filter(|a| a.kind == "bot" || a.kind == "user")
        .map(|a| Vec2::new(a.x, a.y))
        .min_by(|a, b| {
            a.distance(center)
                .partial_cmp(&b.distance(center))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(center);

    crate::world::terrain::spawn_live_terrain(
        &mut commands,
        &world,
        &assets,
        &mut meshes,
        &mut terrain_mats,
    );
    crate::world::water::spawn_live_water(
        &mut commands,
        &world,
        &mut meshes,
        &mut water_mats,
        &mut images,
    );
    // 장식 산포 — GID 기반 결정론(꽃·풀·자갈). 서버 자원 아님(순수 클라이언트 투영).
    // 구조물 청소 반경(3×3)은 제외 — 지붕 위 바위·꽃과 집을 덮는 덤불 방지.
    let structural: std::collections::HashSet<(u16, u16)> = frame
        .resources
        .iter()
        .filter(|r| structural_kind(&r.kind))
        .map(|r| (r.x, r.y))
        .collect();
    let cleared = cleared_tiles(&structural);
    let mut scene_cache = std::collections::HashMap::new();
    for p in world
        .decorative_props()
        .into_iter()
        .filter(|p| !cleared.contains(&(p.x as i32, p.y as i32)))
    {
        crate::world::models::spawn_one_prop(
            &mut commands,
            &world,
            &assets,
            &mut meshes,
            &mut std_mats,
            &mut scene_cache,
            &p,
        );
    }

    // map은 1회 소비(이후 프레임에서 중복 파싱 방지)
    net.frame = Some(LiveFrame {
        map: None,
        ..frame.clone()
    });
    notify_prime();
}

/// 라이브 프라임 완료 JS 통보 — tf.uiPrime() 로딩 오버레이 해제(데모 플래시 차폐).
#[cfg(target_arch = "wasm32")]
fn notify_prime() {
    use wasm_bindgen::prelude::*;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tf) = js_sys::Reflect::get(&window.into(), &"terrafolk".into()) else {
        return;
    };
    let Ok(f) = js_sys::Reflect::get(&tf, &"uiPrime".into()) else {
        return;
    };
    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
        let _ = f.call0(&tf);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn notify_prime() {}

/// 서버 시간 → DayTime(로컬 진행 정지 — 서버가 정본) + 일출/일몰·날씨.
pub fn sync_daynight(
    net: Res<NetState>,
    mut day: ResMut<crate::world::daynight::DayTime>,
    mut weather: ResMut<crate::world::precip::WeatherState>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(f) = net.frame.as_ref() else { return };
    day.hour = f.hour as f32 + f.minute as f32 / 60.0;
    day.time_scale = 0.0;
    if let Some(sr) = f.sunrise {
        day.sunrise_min = sr as f32;
    }
    if let Some(ss) = f.sunset {
        day.sunset_min = ss as f32;
    }
    if let Some(w) = f.weather.as_deref() {
        weather.kind = w.to_string();
    }
}

/// 구조물 kind — 타일을 선점해 나무·장식이 같은 타일에 뚫고 서지 못하게 한다.
fn structural_kind(kind: &str) -> bool {
    matches!(kind, "hut" | "campfire" | "stash" | "grave" | "relic")
}

/// 구조물 청소 반경(타일, 체비셰프) — 주변까지 포함해 나무·덤불 스폰을 양보한다.
/// 에셋이 늦게 로딩돼도(나무 GLB가 마지막에 뜸) 기본 뷰에서 집이 숲에 묻히지
/// 않게 하는 클라이언트 투영. 구조물 집합의 3×3 확장 좌표를 반환.
fn cleared_tiles(
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

/// 자원 stateJson 최소 파싱 — 동물 {alive}·건축 {stage}.
#[derive(Default, serde::Deserialize)]
struct ResState {
    #[serde(default)]
    alive: Option<bool>,
    #[serde(default)]
    stage: Option<String>,
}

/// 서버 kind → (소품 종, 목표 높이). None = 비표시.
/// 동물은 사냥 사망(alive=false) 시 소멸, 오두막은 건축 단계별 가시화.
fn live_visual(kind: &str, species: &str, state: &ResState) -> Option<(PropKind, f32)> {
    match kind {
        "animal" => {
            if state.alive == Some(false) {
                return None;
            }
            let k = match species {
                "boar" => PropKind::AnimalBoar,
                "deer" => PropKind::AnimalDeer,
                "water_deer" => PropKind::AnimalWaterDeer,
                "squirrel" => PropKind::AnimalSquirrel,
                _ => PropKind::AnimalPheasant, // pheasant·기타 조류
            };
            Some((k, crate::world::models::prop_model(k).0))
        }
        "hut" => {
            // 단계별 시각화 — 부지/기초=재료, 골조·벽=성장 스케일, 완공=전체
            let stage = state.stage.as_deref().unwrap_or("COMPLETED");
            Some(match stage {
                "SITE_PREPARED" | "FOUNDATION_READY" => (PropKind::WoodLog, 0.9),
                "FRAME_READY" => (PropKind::Hut, 4.2 * 0.5),
                "ENCLOSURE_READY" => (PropKind::Hut, 4.2 * 0.75),
                _ => (PropKind::Hut, 4.2),
            })
        }
        _ => {
            let k = live_prop(kind, species)?;
            let (h, _) = crate::world::models::prop_model(k);
            Some((k, h))
        }
    }
}

/// 서버 kind → 소품 종(정적 자원).
fn live_prop(kind: &str, species: &str) -> Option<PropKind> {
    match kind {
        "tree" => Some(if species.contains("pine") {
            PropKind::TreePine
        } else {
            PropKind::TreeBroadleaf
        }),
        "plant" => Some(PropKind::BerryBush),
        "gold_vein" => Some(PropKind::GoldVein),
        "campfire" => Some(PropKind::Campfire),
        "stash" => Some(PropKind::Stash),
        "grave" => Some(PropKind::Grave),
        "relic" => Some(PropKind::Relic),
        _ => None,
    }
}

/// 자원 상태 서명 — 변화(사망·건축 단계 진행) 감지용.
fn res_sig(r: &super::LiveResource, vis: bool) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in r.kind.bytes().chain(r.species.bytes()).chain(
        r.state
            .as_deref()
            .unwrap_or("")
            .bytes()
            .chain(std::iter::once(if vis { 1 } else { 0 })),
    ) {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// 자원 상태 서명 컴포넌트 — 변화 시 재스폰 트리거.
#[derive(Component)]
pub struct LiveResMeta(pub u64);

/// 자원 동기화 — 신규 스폰 / 사라진 것·상태 변화(사망·건축 단계) 재스폰.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 메시·재질은 스폰 직접 주입용
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

    // 갱신·제거 — 서명이 바뀌면 despawn(다음 프레임 새 시각으로 재스폰)
    for (e, tag, meta, _) in &mut q_live {
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
            Some(m) if m.0 == sig => {}
            _ => commands.entity(e).despawn(),
        }
    }

    // 신규 스폰
    *first_log += 1;
    if *first_log == 2 {
        let mut total = 0;
        let mut animals = 0;
        let mut buildings = 0;
        for (_, _, _, prop) in &q_live {
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
        q_live.iter().map(|(_, t, _, _)| t.0).collect();
    for r in &frame.resources {
        if live_ids.contains(&r.id) {
            continue;
        }
        let st: ResState = r
            .state
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
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
        if world.terrain_at(r.x as i32, r.y as i32) == "water" {
            continue;
        }
        let ground = world.height_at(r.x as i32, r.y as i32);
        let variants = crate::world::models::prop_model(kind).1;
        commands.spawn((
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
        ));
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

/// 주민 동기화 — 신규 스폰 / 목적지·이동상태 갱신 / 사라진 것 제거.
/// 1프레임 공백(재접속·드랍)은 AgentGrace 유예로 버티고, 유예 초과분만 철거.
#[allow(clippy::type_complexity)] // Bevy 쿼리 타입 — 유예 필드까지 읽는 동기 루프
pub fn sync_agents(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    mut q_live: Query<(
        Entity,
        &LiveAgentTag,
        &Transform,
        &mut crate::world::villager::Villager,
        Option<&mut AgentGrace>,
    )>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else {
        return;
    };
    const GRACE_SECS: f32 = 5.0;

    // 갱신·제거
    for (e, tag, tr, mut v, grace) in &mut q_live {
        let Some(a) = frame.agents.iter().find(|a| a.id == tag.0) else {
            match grace {
                Some(mut g) => {
                    g.0 += time.delta_secs();
                    if g.0 >= GRACE_SECS {
                        commands.entity(e).despawn();
                    }
                }
                None => {
                    commands.entity(e).insert(AgentGrace(time.delta_secs()));
                }
            }
            continue;
        };
        if let Some(mut g) = grace {
            g.0 = 0.0;
        }
        let target = Vec3::new(
            a.x,
            world.height_at(a.x.floor() as i32, a.y.floor() as i32),
            a.y,
        );
        v.to = target;
        v.moving = tr.translation.distance(target) > 0.05;
    }

    // 신규 — 기존 id 사전 조회(O(1)), 프레임당 O(N×M) 순회 제거
    let live_ids: std::collections::HashSet<&str> =
        q_live.iter().map(|(_, t, _, _, _)| t.0.as_str()).collect();
    for a in &frame.agents {
        if live_ids.contains(a.id.as_str()) {
            continue;
        }
        if a.kind != "bot" && a.kind != "user" {
            continue;
        }
        let path = crate::world::villager::model_path_for(&a.id, a.look.as_deref());
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let gltf = assets.load(path);
        let ground = world.height_at(a.x.floor() as i32, a.y.floor() as i32);
        commands
            .spawn((
                crate::world::villager::VillagerModel(gltf),
                crate::world::villager::Villager {
                    from: Vec3::new(a.x, ground, a.y),
                    to: Vec3::new(a.x, ground, a.y),
                    t: 0.0,
                    moving: false,
                    idle: 0.0,
                    target_h: crate::world::villager::VILLAGER_H,
                },
                LiveAgentTag(a.id.clone()),
                bevy::world_serialization::WorldAssetRoot(scene),
                Transform::from_translation(Vec3::new(a.x, ground, a.y)),
            ))
            .observe(crate::world::villager::villager_ready);
    }
}

/// 라이브 주민 이동 — 서버 목적지로 등속 이동 + 이동 방향 회전.
pub fn live_move(
    time: Res<Time>,
    mut q: Query<(
        &LiveAgentTag,
        &mut Transform,
        &mut crate::world::villager::Villager,
    )>,
) {
    const SPEED: f32 = 1.2; // 타일/초(서버 배회 속도 근사)
    for (_, mut tr, mut v) in &mut q {
        if !v.moving {
            continue;
        }
        let d = v.to - tr.translation;
        let dist = d.length();
        let step = SPEED * time.delta_secs();
        if dist <= step || dist < 1e-4 {
            tr.translation = v.to;
            v.moving = false;
        } else {
            tr.translation += d / dist * step;
            // 이동 방향 회전(발밑 스케일 보존)
            let yaw = d.x.atan2(d.z);
            tr.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

/// 검증 디버그 — G키: 카메라를 첫 라이브 동물로 점프.
pub fn debug_goto_animal(
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<crate::world::camera_rig::CameraRig>,
    q: Query<(&Transform, &crate::world::models::PropRoot), With<LiveResourceTag>>,
) {
    if !keys.just_pressed(KeyCode::KeyG) || crate::world::pick::ui_typing() {
        return;
    }
    for (tr, prop) in &q {
        if matches!(
            prop.kind,
            PropKind::AnimalBoar
                | PropKind::AnimalDeer
                | PropKind::AnimalWaterDeer
                | PropKind::AnimalSquirrel
                | PropKind::AnimalPheasant
        ) {
            rig.target = Vec2::new(tr.translation.x, tr.translation.z);
            rig.zoom_goal = 0.8;
            info!("[net] G키 — 라이브 동물 위치로 점프");
            break;
        }
    }
}
