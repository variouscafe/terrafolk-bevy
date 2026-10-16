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
pub struct VillagerModel(pub Handle<Gltf>);

const MODELS: [&str; 2] = ["models/VillagerMan.glb", "models/VillagerWoman.glb"];

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
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        commands
            .spawn((
                VillagerModel(gltf),
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

/// 씬 스폰 완료 — 애니메이션 그래프 연결 + 오토펏 + 틴트.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 준비 관찰자 인자
pub fn villager_ready(
    trigger: On<WorldInstanceReady>,
    mut commands: Commands,
    q_self: Query<(&Villager, &VillagerModel, &Transform)>,
    children: Query<&Children>,
    players: Query<(Entity, &mut AnimationPlayer)>,
    q_walk: super::models::FitQuery,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    meshes: Res<Assets<Mesh>>,
    world: Res<super::data::WorldData>,
) {
    let Ok((villager, model, tr)) = q_self.get(trigger.entity) else {
        return;
    };
    let Some(gltf) = gltfs.get(&model.0) else {
        return; // 아직 로딩 — WorldAsset은 Gltf 처리 후 준비되므로 실제로는 도달 드묾
    };
    // Walk/Idle 클립 검색 — 접미 매칭(프리픽스는 모델별 상이)
    let find = |suffix: &str| {
        gltf.named_animations
            .iter()
            .find(|(name, _)| name.ends_with(suffix))
            .map(|(_, h)| h.clone())
    };
    let (Some(walk_h), Some(idle_h)) = (find("Walk"), find("Idle")) else {
        return;
    };
    let mut graph = AnimationGraph::new();
    let walk_i = graph.add_clip(walk_h, 1.0, graph.root);
    let idle_i = graph.add_clip(idle_h, 1.0, graph.root);
    let graph_h = graphs.add(graph);

    // 하위 AnimationPlayer 탐색(bevy_gltf가 자동 스폰)
    let mut player_e = None;
    for child in children.iter_descendants(trigger.entity) {
        if players.get(child).is_ok() {
            player_e = Some(child);
            break;
        }
    }
    let Some(player_e) = player_e else {
        return;
    };
    if let Ok((_, player)) = players.get(player_e) {
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
    for child in children.iter_descendants(trigger.entity) {
        if q_walk
            .get(child)
            .map(|(_, _, m, _, _)| m.is_some())
            .unwrap_or(false)
        {
            measure.push(child);
        }
    }
    if let Some((mn, mx)) = super::models::meshes_aabb(&measure, trigger.entity, &q_walk, &meshes) {
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
        commands.entity(trigger.entity).insert(fitted);
    }
    commands.entity(trigger.entity).insert(VillagerAnims {
        walk: walk_i,
        idle_anim: idle_i,
        player_e,
        current_walk: false,
    });
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
