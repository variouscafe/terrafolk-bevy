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
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 데모 철거+라이브 지형 스폰 자원들
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

    // 라이브 데이터로 교체 — GID 그리드, 평면 고도
    *world = WorldData::from_map(&map.layers[0].data, map.width, map.height);

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

    // map은 1회 소비(이후 프레임에서 중복 파싱 방지)
    net.frame = Some(LiveFrame {
        map: None,
        ..frame.clone()
    });
}

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
                "FRAME_READY" => (PropKind::Hut, 3.4 * 0.5),
                "ENCLOSURE_READY" => (PropKind::Hut, 3.4 * 0.75),
                _ => (PropKind::Hut, 3.4),
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
pub fn sync_resources(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut q_live: Query<(
        Entity,
        &LiveResourceTag,
        Option<&LiveResMeta>,
        Option<&crate::world::models::PropRoot>,
    )>,
    mut first_log: Local<u32>,
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
        for (_, _, _, prop) in &q_live {
            total += 1;
            if let Some(p) = prop
                && matches!(
                    p.kind,
                    PropKind::AnimalBoar
                        | PropKind::AnimalDeer
                        | PropKind::AnimalWaterDeer
                        | PropKind::AnimalSquirrel
                        | PropKind::AnimalPheasant
                )
            {
                animals += 1;
            }
        }
        info!("[net] 라이브 자원 실체 {total}개 (동물 {animals})");
    }
    for r in &frame.resources {
        if q_live.iter().any(|(_, t, _, _)| t.0 == r.id) {
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
    }
}

/// 주민 동기화 — 신규 스폰 / 목적지·이동상태 갱신 / 사라진 것 제거.
pub fn sync_agents(
    mut commands: Commands,
    net: Res<NetState>,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut q_live: Query<(
        Entity,
        &LiveAgentTag,
        &Transform,
        &mut crate::world::villager::Villager,
    )>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else {
        return;
    };

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
    if !keys.just_pressed(KeyCode::KeyG) {
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
