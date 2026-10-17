//! 라이브 프레임 → 렌더 적용 — 지형 프라임(최초 1회)·시간·자원·주민.
//! 데모 WorldData를 라이브 지형으로 교체(평면 — 실 맵은 절벽 없음)하고
//! 서버 id 단위로 자원/주민 엔티티를 생성·갱신·제거한다.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use super::resources::{LiveResourceTag, cleared_tiles, structural_kind};
use super::{LiveFrame, NetState, NetStatus, TiledMap};
use crate::world::data::{PropKind, WorldData};

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

    if !super::dbg_gate::off("no_terrain") {
        crate::world::terrain::spawn_live_terrain(
            &mut commands,
            &world,
            &assets,
            &mut meshes,
            &mut terrain_mats,
        );
    }
    if !super::dbg_gate::off("no_water") {
        crate::world::water::spawn_live_water(
            &mut commands,
            &world,
            &mut meshes,
            &mut water_mats,
            &mut images,
        );
    }
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
    let deco: Vec<_> = if super::dbg_gate::off("no_deco") {
        vec![] // 진단 게이트 — 장식 산포 스킵
    } else {
        world
            .decorative_props()
            .into_iter()
            .filter(|p| !cleared.contains(&(p.x as i32, p.y as i32)))
            .collect()
    };
    for p in deco
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
    day.season = f.season.clone();
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
    mut first_log: Local<u32>,
) {
    if net.status != NetStatus::Connected {
        return;
    }
    let Some(frame) = net.frame.as_ref() else {
        return;
    };
    const GRACE_SECS: f32 = 5.0;
    if *first_log == 0 {
        *first_log = 1;
        let eligible = frame
            .agents
            .iter()
            .filter(|a| a.kind == "bot" || a.kind == "user")
            .count();
        info!(
            "[net] 주민 동기 시작 — 상태 {:?} · 프레임 에이전트 {}명(대상 {}명)",
            net.status,
            frame.agents.len(),
            eligible
        );
    }

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
            // 착지 클램프 — 물 타일 위에서도 수면(-0.18) 아래로 잠기지 않게
            world
                .height_at(a.x.floor() as i32, a.y.floor() as i32)
                .max(-0.14),
            a.y,
        );
        v.to = target;
        v.moving = tr.translation.distance(target) > 0.05;
    }

    // 신규 — 기존 id 사전 조회(O(1)), 프레임당 O(N×M) 순회 제거
    let live_ids: std::collections::HashSet<&str> =
        q_live.iter().map(|(_, t, _, _, _)| t.0.as_str()).collect();
    let mut spawned = 0usize;
    for a in &frame.agents {
        if live_ids.contains(a.id.as_str()) {
            continue;
        }
        if a.kind != "bot" && a.kind != "user" {
            continue;
        }
        if super::dbg_gate::off("no_villagers") {
            continue; // 진단 게이트
        }
        let look = a.look.clone();
        let path = crate::world::villager::model_path_for(&a.id, a.look.as_deref());
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let gltf = assets.load(path);
        // 착지 클램프 — 서버가 물 타일에 세워도(§17.80 취수) 몸이 불투명
        // 수면 아래로 잠기지 않게 발을 수면(-0.18) 위로 못 박는다.
        let ground = world
            .height_at(a.x.floor() as i32, a.y.floor() as i32)
            .max(-0.14);
        commands
            .spawn((
                crate::world::villager::VillagerModel(gltf, look),
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
        spawned += 1;
    }
    if spawned > 0 {
        info!("[net] 라이브 주민 스폰 {spawned}개 완료");
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
