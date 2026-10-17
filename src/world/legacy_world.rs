//! 네이티브 데모 월드 — 레거시 FE와 동일한 공용 월드 맵.
//! 백엔드 worldsim_worlds.map_data(시드 20261011, Tiled JSON)를 컴파일 시간
//! 임베드해 구동한다. 웹은 라이브 맵을 받아 쓰므로 이 경로를 타지 않는다.
//! 절차 generate() 대신 from_map → 라이브 프라임과 동일한 지형·물·장식 체인.

use bevy::prelude::*;

use super::data::WorldData;

const LEGACY_MAP_JSON: &str = include_str!("../../assets/maps/shared-map.json");

#[derive(serde::Deserialize)]
struct TiledMap {
    width: usize,
    layers: [Layer; 1],
}

#[derive(serde::Deserialize)]
struct Layer {
    data: Vec<u8>,
}

/// 레거시 맵 프라임 — 최초 1회: WorldData 교체 + 지형·물·장식·주민 배치.
/// 실패 시 절차 데모(generate)를 유지해 앱이 빈 화면이 되지 않게 한다.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 라이브 프라임과 동일 자원들
pub fn prime_legacy_world(
    mut commands: Commands,
    mut primed: Local<bool>,
    mut world: ResMut<WorldData>,
    mut rig: ResMut<super::camera_rig::CameraRig>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_mats: ResMut<Assets<super::splat::TerrainMaterial>>,
    mut water_mats: ResMut<Assets<super::water::WaterMaterial>>,
    mut images: ResMut<Assets<bevy::image::Image>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
) {
    if *primed {
        return;
    }
    *primed = true;
    let Ok(map) = serde_json::from_str::<TiledMap>(LEGACY_MAP_JSON) else {
        warn!("[legacy] 맵 파싱 실패 — 절차 데모 유지");
        return;
    };
    info!("[legacy] 레거시 맵 프라임 {}×{}", map.width, map.width);
    *world = WorldData::from_map(&map.layers[0].data, map.width, map.width);
    // 카메라 중심 — 맵 중심(기본 rig는 절차 데모 SIZE=56 기준 28,28)
    let center = Vec2::splat(world.size as f32 / 2.0);
    rig.target = center;
    super::terrain::spawn_live_terrain(
        &mut commands,
        &world,
        &assets,
        &mut meshes,
        &mut terrain_mats,
    );
    super::water::spawn_live_water(
        &mut commands,
        &world,
        &mut meshes,
        &mut water_mats,
        &mut images,
    );
    // 장식 산포 — 라이브 프라임과 동일(꽃·풀·자갈, 결정론)
    let mut scene_cache = std::collections::HashMap::new();
    for p in world.decorative_props() {
        super::models::spawn_one_prop(
            &mut commands,
            &world,
            &assets,
            &mut meshes,
            &mut std_mats,
            &mut scene_cache,
            &p,
        );
    }
    super::villager::spawn_legacy_villagers(&mut commands, &world, &assets);
}
