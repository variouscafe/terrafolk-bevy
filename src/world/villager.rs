//! 주민 — Quaternius CC0 리깅 캐릭터(GLB) + 보행/대기 애니메이션 + 배회 FSM.
//! Phase 2에서 서버 agents 스키마 좌표로 구동된다(구조 동일).
//! 스폰 → WorldInstanceReady 옵저버에서 오토펏(족장 지면 착지)·애니메이션
//! 그래프(Walk/Idle 2클립) 연결. 이동 시 캐릭터가 이동 방향으로 회전.

use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use std::time::Duration;

use super::data::{SIZE, WorldData};
use crate::net::apply::LiveAgentTag;

const WALK_TIME: f32 = 0.85;
/// 주민 키 목표(미터)
pub const VILLAGER_H: f32 = 1.62;

/// 모델 전방 보정 — Quaternius 리깅는 +Z 전방(실측 후 조정)
const MODEL_YAW: f32 = 0.0;

#[derive(Component)]
pub struct Villager {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub moving: bool,
    pub idle: f32,
    pub target_h: f32,
}

#[derive(Component)]
pub struct VillagerAnims {
    pub walk: AnimationNodeIndex,
    pub idle_anim: AnimationNodeIndex,
    pub player_e: Entity,
    pub current_walk: bool,
}

#[derive(Component)]
pub struct VillagerModel(pub Handle<Gltf>, pub Option<String>);

const MODELS: [&str; 2] = ["models/VillagerMan.glb", "models/VillagerWoman.glb"];

/// 여성 모델 헤어 변형 프리미티브(VillagerWoman Casual_Head 공존 파츠) —
/// 룩 해시로 1택. 남성(Sam 단일 메시)엔 해당 없다.
const HAIR_VARIANTS: [&str; 3] = ["Hair_Blond", "Hair_Brown", "Brown"];

// ── 착의 팔레트 — 프리미티브 재질 베이스컬러 틴트(코지·페인터리 톤) ──
// 슬롯 매칭: "Casual_Body.White"=셔츠 · "Casual_Legs.Orange"=바지 ·
// "Casual_Feet.Grey"=신발(엔티티명 "메시.재질" 규칙). 피부·머리 슬롯은 불간섭.
fn shirt_colors() -> [Color; 8] {
    [
        Color::srgb(0.62, 0.28, 0.22), // 벽돌
        Color::srgb(0.36, 0.46, 0.33), // 세이지
        Color::srgb(0.24, 0.32, 0.46), // 네이비
        Color::srgb(0.72, 0.58, 0.26), // 머스터드
        Color::srgb(0.66, 0.36, 0.42), // 로즈
        Color::srgb(0.24, 0.46, 0.44), // 청록
        Color::srgb(0.42, 0.34, 0.50), // 플럼
        Color::srgb(0.80, 0.74, 0.62), // 크림
    ]
}
fn pants_colors() -> [Color; 5] {
    [
        Color::srgb(0.32, 0.24, 0.18), // 갈색
        Color::srgb(0.22, 0.28, 0.38), // 짙은 청
        Color::srgb(0.40, 0.36, 0.26), // 카키
        Color::srgb(0.20, 0.20, 0.22), // 차콜
        Color::srgb(0.38, 0.24, 0.26), // 버건디
    ]
}
fn shoe_colors() -> [Color; 3] {
    [
        Color::srgb(0.14, 0.13, 0.14), // 흑
        Color::srgb(0.34, 0.24, 0.16), // 갈
        Color::srgb(0.24, 0.20, 0.18), // 암갈
    ]
}

/// 라이브 에이전트용 — lookJson.sex(있으면) 또는 id 해시로 남/여 선택.
pub fn model_path_for(id: &str, look: Option<&str>) -> &'static str {
    if let Some(look) = look
        && look.contains("\"sex\":\"")
    {
        // lookJson은 {uuid,sex,age} — sex 값 문자로 판별(여성 F/f)
        if let Some(i) = look.find("\"sex\":\"") {
            let c = look[i + 7..].chars().next().unwrap_or('M');
            if c == 'F' || c == 'f' {
                return MODELS[1];
            }
        }
    }
    let h = id.bytes().fold(0u64, |a, b| a.wrapping_mul(31) + b as u64);
    MODELS[(h % 2) as usize]
}

pub fn spawn_villagers(mut commands: Commands, world: Res<WorldData>, assets: Res<AssetServer>) {
    let mut rng = super::data::Rng::new(99);
    let c = SIZE as i32 / 2;
    let mut placed = 0;
    let mut attempts = 0;
    while placed < 6 && attempts < 200 {
        attempts += 1;
        let x = rng.range_i(c - 7, c + 7);
        let y = rng.range_i(c - 7, c + 7);
        if !world.walkable(x, y) {
            continue;
        }
        let path = MODELS[placed % MODELS.len()];
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let gltf = assets.load(path);
        let ground = world.height_at(x, y);
        let _ = &path;
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        commands
            .spawn((
                VillagerModel(gltf, None),
                Villager {
                    from: Vec3::new(x as f32 + 0.5, ground, y as f32 + 0.5),
                    to: Vec3::new(x as f32 + 0.5, ground, y as f32 + 0.5),
                    t: 0.0,
                    moving: false,
                    idle: rng.range(0.0, 2.0),
                    target_h: VILLAGER_H,
                },
                WorldAssetRoot(scene),
                Transform::from_translation(Vec3::new(x as f32 + 0.5, ground, y as f32 + 0.5))
                    .with_rotation(Quat::from_rotation_y(yaw)),
            ))
            .observe(villager_ready);
        placed += 1;
    }
}

/// 스폰 완료 보장 마커 — finish_villager_setup이 아직 못 끝낸 주민.
/// 무음 early-return이 애님+오토핏을 영구 누락시켜 원시 스케일 유령(2026-10-17
/// 신규 합류 주민 '테린' 사례 — 화면에 아예 안 보임)을 낳는다. 재시도로 보장한다.
#[derive(Component)]
pub struct VillagerPending(pub f32);

/// 씬 스폰 완료 — 애니메이션 그래프 연결 + 오토펏. 자산 비동기 로드로 아직
/// 못 끝내면 VillagerPending을 남기고, retry_villager_setup이 완결한다.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — finish_villager_setup과 동일 세트
pub fn villager_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    q_self: Query<(&Villager, &VillagerModel, &Transform)>,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer)>,
    q_walk: super::models::FitQuery,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    meshes: Res<Assets<Mesh>>,
    mut aabb_cache: ResMut<super::models::MeshAabbCache>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mat_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    world: Res<super::data::WorldData>,
) {
    let Ok((villager, model, tr)) = q_self.get(trigger.entity) else {
        return;
    };
    let done = finish_villager_setup(
        trigger.entity,
        villager,
        model,
        tr,
        &mut commands,
        &children,
        &mut players,
        &q_walk,
        &gltfs,
        &mut graphs,
        &meshes,
        &mut aabb_cache,
        &mut std_mats,
        &mat_handles,
        &world,
    );
    if !done {
        commands.entity(trigger.entity).insert(VillagerPending(0.0));
    }
}

/// 스폰 완료 재시도 — Gltf·클립·AnimationPlayer가 비동기로 늦게 도착하는
/// 경합을 매 프레임 재시도로 흡수(성공 시 마커 제거, 60s 포기).
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — finish_villager_setup과 동일 세트
pub fn retry_villager_setup(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &Villager,
        &VillagerModel,
        &Transform,
        &mut VillagerPending,
    )>,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer)>,
    q_walk: super::models::FitQuery,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    meshes: Res<Assets<Mesh>>,
    mut aabb_cache: ResMut<super::models::MeshAabbCache>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mat_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    world: Res<super::data::WorldData>,
) {
    for (entity, villager, model, tr, mut pending) in &mut q {
        pending.0 += time.delta_secs();
        if pending.0 > 60.0 {
            warn!("[villager] 스폰 완료 실패(60s) — 포기 {:?}", model.0.path());
            commands.entity(entity).remove::<VillagerPending>();
            continue;
        }
        let done = finish_villager_setup(
            entity,
            villager,
            model,
            tr,
            &mut commands,
            &children,
            &mut players,
            &q_walk,
            &gltfs,
            &mut graphs,
            &meshes,
            &mut aabb_cache,
            &mut std_mats,
            &mat_handles,
            &world,
        );
        if done {
            info!("[villager] 스폰 완료 재시도 성공 (+{:.1}s)", pending.0);
            commands.entity(entity).remove::<VillagerPending>();
        }
    }
}

/// 주민 씬 완결 공용 본체 — 애님 그래프·크로스페이드 시작·오토펏.
/// Gltf/클립/플레이어 미준비면 false(호출처가 재시도 마커 처리).
#[allow(clippy::too_many_arguments)] // 옵저버·재시도 양쪽이 같은 자산 집합을 주입
fn finish_villager_setup(
    entity: Entity,
    villager: &Villager,
    model: &VillagerModel,
    tr: &Transform,
    commands: &mut Commands,
    children: &Query<&Children>,
    players: &mut Query<(Entity, &mut AnimationPlayer)>,
    q_walk: &super::models::FitQuery,
    gltfs: &Assets<Gltf>,
    graphs: &mut Assets<AnimationGraph>,
    meshes: &Assets<Mesh>,
    aabb_cache: &mut super::models::MeshAabbCache,
    std_mats: &mut Assets<StandardMaterial>,
    mat_handles: &Query<&MeshMaterial3d<StandardMaterial>>,
    world: &super::data::WorldData,
) -> bool {
    let Some(gltf) = gltfs.get(&model.0) else {
        return false; // Gltf 아직 로딩 — 재시도로 흡수
    };
    // ── 룩 파츠 선택 — GLB는 변형 프리미티브가 '공존'한다(여성 머리 3색).
    // 엔티티명 "Casual_Head.Hair_Blond" 규칙(bevy_gltf primitive_name)으로
    // lookJson 해시가 고른 하나만 남기고 나머지 파기 — 주민별 외형 분리.
    {
        let seed = model.1.as_deref().unwrap_or("");
        let h = seed.bytes().fold(0u64, |a, b| a.wrapping_mul(31) + b as u64);
        let pick = HAIR_VARIANTS[(h % HAIR_VARIANTS.len() as u64) as usize];
        for child in children.iter_descendants(entity) {
            if let Ok((Some(name), _, _, _, _)) = q_walk.get(child)
                && let Some(mat) = name.rsplit('.').next()
                && name.starts_with("Casual_Head.")
                && HAIR_VARIANTS.contains(&mat)
                && mat != pick
            {
                commands.entity(child).despawn();
            }
        }
    }
    // ── 착의 틴트 — 옷 슬롯 재질을 주민별 고유 복제로 교체(lookJson 해시 색).
    // 주민별 고유 재질은 오토 인스턴싱 배칭을 의도적으로 분리한다 — 모바일
    // WebGL2에서 다중 스키닝 메시 배칩이 조인트 행렬을 섞어 '뼈'처럼 뭉개지는
    // 회귀의 1차 회피책(주민 수십 개 드로우콜 — 비용 무시 가능).
    {
        let seed = model.1.as_deref().unwrap_or("");
        let h = seed.bytes().fold(0u64, |a, b| a.wrapping_mul(31) + b as u64);
        let slots: [(&str, Color); 3] = [
            ("Casual_Body.White", shirt_colors()[(h.wrapping_mul(0x9E37) % 8) as usize]),
            ("Casual_Legs.Orange", pants_colors()[(h.wrapping_mul(0x85EB) % 5) as usize]),
            ("Casual_Feet.Grey", shoe_colors()[(h.wrapping_mul(0xC2B2) % 3) as usize]),
        ];
        for child in children.iter_descendants(entity) {
            let Ok((Some(name), _, _, _, _)) = q_walk.get(child) else {
                continue;
            };
            let Some(tint) = slots.iter().find(|(slot, _)| name.as_str() == *slot).map(|(_, c)| *c)
            else {
                continue;
            };
            let Ok(handle) = mat_handles.get(child) else {
                continue;
            };
            let Some(m) = std_mats.get(&handle.0) else {
                continue;
            };
            let mut clone = m.clone();
            clone.base_color = tint; // 단색 슬롯 — 직접 지정(피부·머리 불간섭)
            commands
                .entity(child)
                .insert(MeshMaterial3d(std_mats.add(clone)));
        }
    }
    // Walk/Idle 클립 검색 — 접미 매칭(프리픽스는 모델별 상이)
    let find = |suffix: &str| {
        gltf.named_animations
            .iter()
            .find(|(name, _)| name.ends_with(suffix))
            .map(|(_, h)| h.clone())
    };
    let (Some(walk_h), Some(idle_h)) = (find("Walk"), find("Idle")) else {
        return false;
    };
    let mut graph = AnimationGraph::new();
    let walk_i = graph.add_clip(walk_h, 1.0, graph.root);
    let idle_i = graph.add_clip(idle_h, 1.0, graph.root);
    let graph_h = graphs.add(graph);

    // 하위 AnimationPlayer 탐색(bevy_gltf가 자동 스폰)
    let mut player_e = None;
    for child in children.iter_descendants(entity) {
        if players.get(child).is_ok() {
            player_e = Some(child);
            break;
        }
    }
    let Some(player_e) = player_e else {
        return false; // 플레이어 아직 스폰 전 — 재시도로 흡수
    };
    if let Ok((_, player)) = players.get_mut(player_e) {
        let mut player = player.clone();
        // AnimationTransitions — 크로스페이드 주체(재생은 이 컴포넌트 경유로만).
        let mut transitions = AnimationTransitions::default();
        transitions
            .play(&mut player, idle_i, Duration::ZERO)
            .repeat();
        commands.entity(player_e).insert((player, transitions));
    }
    commands
        .entity(player_e)
        .insert(AnimationGraphHandle(graph_h));

    // 오토펏 — 서브트리 전체 메시의 결합 AABB(조상 체인 행렬)로 스케일·착지.
    // GLB 루트 직속 자식엔 메시가 없고 Armature 아래 깊이 있으므로 재귀 수집.
    let mut measure: Vec<Entity> = Vec::new();
    for child in children.iter_descendants(entity) {
        if q_walk
            .get(child)
            .map(|(_, _, m, _, _)| m.is_some())
            .unwrap_or(false)
        {
            measure.push(child);
        }
    }
    if let Some((mn, mx)) =
        super::models::meshes_aabb(&measure, entity, q_walk, meshes, aabb_cache)
    {
        let size = (mx - mn).max(Vec3::splat(1e-4));
        let s = (villager.target_h / size.y).clamp(0.05, 50.0);
        // 착지 기준은 현재 위치의 지면 — 씬 로딩 사이 배회로 스폰점에서
        // 벗어났어도 되돌아가지 않는다(발밑 고정, 회전·수평 위치 보존).
        let ground = world.height_at(
            tr.translation.x.floor() as i32,
            tr.translation.z.floor() as i32,
        );
        let mut fitted = *tr;
        fitted.translation.y = ground - mn.y * s;
        fitted.scale = Vec3::splat(s);
        commands.entity(entity).insert(fitted);
    }
    commands.entity(entity).insert(VillagerAnims {
        walk: walk_i,
        idle_anim: idle_i,
        player_e,
        current_walk: false,
    });
    true
}

/// 진단 — 주민 렌더 상태 덤프(유령 디버깅): 스폰 8초 후 1회.
/// 애님 연결·스킨·재질 알파 모드를 콘솔에 기록한다.
pub fn villager_render_probe(
    time: Res<Time>,
    q_root: Query<(Entity, &LiveAgentTag, Has<VillagerAnims>), With<Villager>>,
    children: Query<&Children>,
    players: Query<(Entity, &AnimationPlayer)>,
    skinned: Query<(Entity, &bevy::mesh::skinning::SkinnedMesh)>,
    mats: Res<Assets<StandardMaterial>>,
    mat_handles: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    #[allow(clippy::collapsible_if)]
    if time.elapsed_secs() < 8.0 || time.elapsed_secs() > 8.2 {
        return;
    }
    for (e, tag, anims) in &q_root {
        let mut n_mesh = 0;
        let mut n_skinned = 0;
        let mut alpha_modes = std::collections::BTreeMap::new();
        for child in children.iter_descendants(e) {
            if let Ok((se, _)) = skinned.get(child) {
                n_skinned += 1;
                let _ = se;
            }
            if let Ok(h) = mat_handles.get(child) {
                n_mesh += 1;
                if let Some(m) = mats.get(h) {
                    let key = format!("{:?}", m.alpha_mode);
                    *alpha_modes.entry(key).or_insert(0) += 1;
                }
            }
        }
        let playing = children
            .iter_descendants(e)
            .any(|c| players.get(c).is_ok_and(|(_, p)| !p.all_paused()));
        info!(
            "[probe] {} anims={} playing={} meshes={} skinned={} alpha={:?}",
            tag.0, anims, playing, n_mesh, n_skinned, alpha_modes
        );
    }
}

/// 시간 기반 자유 난수(배회 휴식 길이) — 결정론 대상 아님(연출).
fn rand_free(seed: u64, scale: f32) -> f32 {
    super::data::Rng::new(seed.wrapping_mul(2654435761)).f32() * scale
}

pub fn wander(
    time: Res<Time>,
    world: Res<WorldData>,
    mut q: Query<(&mut Villager, &mut Transform), Without<crate::net::apply::LiveAgentTag>>,
) {
    for (mut v, mut tr) in &mut q {
        if v.moving {
            v.t += time.delta_secs() / WALK_TIME;
            if v.t >= 1.0 {
                v.t = 0.0;
                v.moving = false;
                v.idle = 0.4
                    + rand_free(
                        (time.elapsed_secs() * 1000.0) as u64
                            ^ (tr.translation.x as u64).wrapping_mul(31),
                        2.0,
                    );
                tr.translation = v.to;
            } else {
                let s = v.t * v.t * (3.0 - 2.0 * v.t); // smoothstep
                tr.translation = v.from.lerp(v.to, s);
                // 고도 보간 — 계단 절벽에서 직선 관통 방지(낮은 쪽 따라감)
                tr.translation.y = tr.translation.y.max(v.from.y.min(v.to.y));
            }
            continue;
        }
        v.idle -= time.delta_secs();
        if v.idle > 0.0 {
            continue;
        }
        let here = (
            tr.translation.x.floor() as i32,
            tr.translation.z.floor() as i32,
        );
        let mut rng = super::data::Rng::new(
            (here.0 as u64 * 31 + here.1 as u64).wrapping_mul(2654435761)
                ^ (time.elapsed_secs() as u64),
        );
        let dirs = [
            [0, 1],
            [0, -1],
            [1, 0],
            [-1, 0],
            [1, 1],
            [-1, 1],
            [1, -1],
            [-1, -1],
        ];
        let start = (rng.next_u64() % 8) as usize;
        for i in 0..8 {
            let d = dirs[(start + i) % 8];
            let (nx, ny) = (here.0 + d[0], here.1 + d[1]);
            if world.walkable(nx, ny) {
                let target = Vec3::new(nx as f32 + 0.5, world.height_at(nx, ny), ny as f32 + 0.5);
                // 이동 방향 회전(발밑 고정 — 스케일 보존)
                let yaw = (d[0] as f32).atan2(d[1] as f32) + MODEL_YAW;
                tr.rotation = Quat::from_rotation_y(yaw);
                v.from = tr.translation;
                v.to = target;
                v.moving = true;
                v.t = 0.0;
                break;
            }
        }
        if !v.moving {
            v.idle = rand_free(
                (time.elapsed_secs() * 1000.0) as u64 ^ (here.0 as u64 * 31 + here.1 as u64),
                1.5,
            );
        }
    }
}

/// 보행↔대기 애니메이션 전환(플레이어는 하위 엔티티에 있음).
/// 0.25초 크로스페이드 — 하드 컷은 주민을 기계적으로 보이게 한다(생명감 직결).
pub fn animate_walk_idle(
    mut q: Query<(&Villager, &mut VillagerAnims)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (v, mut anims) in &mut q {
        if v.moving == anims.current_walk {
            continue;
        }
        anims.current_walk = v.moving;
        if let Ok((mut player, mut transitions)) = players.get_mut(anims.player_e) {
            let idx = if v.moving {
                anims.walk
            } else {
                anims.idle_anim
            };
            transitions
                .play(&mut player, idx, Duration::from_millis(250))
                .repeat();
        }
    }
}
