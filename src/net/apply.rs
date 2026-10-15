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

/// 라이브 전환 — 데모 지형·물·소품·주민 제거 후 라이브 지형 스폰(1회).
pub fn prime_live_world(
    mut commands: Commands,
    mut net: ResMut<NetState>,
    mut primed: Local<bool>,
    q_terrain: Query<Entity, With<crate::world::terrain::TerrainMesh>>,
    q_water: Query<Entity, With<crate::world::water::WaterMesh>>,
    q_props: Query<Entity, With<crate::world::models::PropRoot>>,
    q_villagers: Query<Entity, With<crate::world::villager::Villager>>,
    mut world: ResMut<WorldData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut water_mats: ResMut<Assets<crate::world::water::WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut assets: Res<AssetServer>,
    mut terrain_mats: ResMut<Assets<crate::world::splat::TerrainMaterial>>,
) {
    if *primed || net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else { return };
    let Some(map_json) = frame.map.as_ref() else { return };
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

    // 라이브 데이터로 교체 — GID 그리드, 평면 고도
    *world = WorldData::from_map(&map.layers[0].data, map.width, map.height);

    crate::world::terrain::spawn_live_terrain(&mut commands, &world, &assets, &mut meshes, &mut terrain_mats);
    crate::world::water::spawn_live_water(&mut commands, &world, &mut meshes, &mut water_mats, &mut images);

    // map은 1회 소비(이후 프레임에서 중복 파싱 방지)
    net.frame = Some(LiveFrame { map: None, ..frame.clone() });
}

/// 서버 시간 → DayTime(로컬 진행 정지 — 서버가 정본).
pub fn sync_daynight(net: Res<NetState>, mut day: ResMut<crate::world::daynight::DayTime>) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(f) = net.frame.as_ref() else { return };
    day.hour = f.hour as f32 + f.minute as f32 / 60.0;
    day.time_scale = 0.0;
}

/// 서버 kind → 소품 종(미지원 동물은 2b).
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
        "hut" => Some(PropKind::Hut),
        "stash" => Some(PropKind::Stash),
        "grave" => Some(PropKind::Grave),
        "relic" => Some(PropKind::Relic),
        _ => None,
    }
}

/// 자원 동기화 — 신규 스폰 / 사라진 것 제거(자원은 배치 고정 — 좌표 이동 없음).
pub fn sync_resources(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    q_live: Query<(Entity, &LiveResourceTag)>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else { return };
    let mut seen: Vec<u32> = Vec::new();

    for r in &frame.resources {
        let Some(kind) = live_prop(&r.kind, &r.species) else {
            continue;
        };
        seen.push(r.id);
        if q_live.iter().any(|(_, t)| t.0 == r.id) {
            continue; // 이미 스폰됨
        }
        if world.terrain_at(r.x as i32, r.y as i32) == "water" {
            continue;
        }
        let ground = world.height_at(r.x as i32, r.y as i32);
        let (target_h, variants) = crate::world::models::prop_model(kind);
        commands.spawn((
            crate::world::models::PropRoot {
                kind,
                target_h,
                ground_y: ground,
                rot_y: (r.id % 360) as f32 / 360.0 * std::f32::consts::TAU,
                scale_jitter: 1.0,
                variants,
            },
            bevy::world_serialization::WorldAssetRoot(crate::world::models::prop_scene(kind, &assets)),
            LiveResourceTag(r.id),
            Transform::from_translation(Vec3::new(r.x as f32 + 0.5, ground, r.y as f32 + 0.5)),
        ));
    }
    for (e, t) in &q_live {
        if !seen.contains(&t.0) {
            commands.entity(e).despawn();
        }
    }
}

/// 주민 동기화 — 신규 스폰 / 목적지·이동상태 갱신 / 사라진 것 제거.
pub fn sync_agents(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut q_live: Query<(Entity, &LiveAgentTag, &Transform, &mut crate::world::villager::Villager)>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else { return };

    // 갱신·제거
    for (e, tag, tr, mut v) in &mut q_live {
        let Some(a) = frame.agents.iter().find(|a| a.id == tag.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let target = Vec3::new(
            a.x,
            world.height_at(a.x.floor() as i32, a.y.floor() as i32),
            a.y,
        );
        v.to = target;
        v.moving = tr.translation.distance(target) > 0.05;
    }

    // 신규
    for a in &frame.agents {
        if q_live.iter().any(|(_, t, _, _)| t.0 == a.id) {
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
    mut q: Query<(&LiveAgentTag, &mut Transform, &mut crate::world::villager::Villager)>,
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
