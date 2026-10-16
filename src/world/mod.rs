//! 월드 — 데모 결정론 데이터 + 2.5D 아이소 렌더(스플랫 지형·3D 소품·하루순환·수면).
//! 계약: 서버는 정사각 그리드를 주고 클라이언트가 투영(docs/data-contract.md).

pub mod cam_follow;
pub mod camera_input;
pub mod camera_rig;
pub mod data;
pub mod daynight;
pub mod models;
pub mod occlusion;
pub mod pick;
pub mod precip;
pub mod splat;
pub mod terrain;
pub mod villager;
pub mod water;
pub mod wind;

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<water::WaterMaterial>::default());
        app.init_asset::<water::WaterMaterial>();
        app.add_plugins(MaterialPlugin::<splat::TerrainMaterial>::default());
        app.add_plugins(MaterialPlugin::<wind::WindMaterial>::default());
        app.add_plugins(MaterialPlugin::<precip::PrecipMaterial>::default());
        app.insert_resource(ClearColor(Color::srgb(0.30, 0.48, 0.68)))
            .insert_resource(data::WorldData::generate(20261015))
            .insert_resource(daynight::DayTime::default())
            .insert_resource(precip::WeatherState::default())
            .insert_resource(precip::PrecipRig::default())
            .insert_resource(camera_rig::CameraRig::default())
            .insert_resource(camera_input::PointerInput::default())
            .insert_resource(pick::Selection::default())
            .insert_resource(pick::SelectBridge::default())
            .insert_resource(cam_follow::CamFollow::default())
            .add_systems(
                Startup,
                (
                    camera_rig::spawn_camera,
                    daynight::spawn_sun,
                    pick::spawn_ring,
                    pick::register_bridge,
                ),
            );
        // 데모 결정론 월드는 네이티브 개발 빌드 전용 — 웹(wasm)은 라이브 접속→
        // 프라임으로 곧장 간다(스폰-철거 낭비·플래시 제거. 로딩 오버레이는
        // tf.uiPrime 통보로 해제).
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(
            Startup,
            (
                terrain::spawn_terrain,
                water::spawn_water,
                models::spawn_props,
                villager::spawn_villagers,
            ),
        );
        app.add_systems(
            Update,
            (
                camera_rig::camera_control,
                daynight::advance_time,
                daynight::apply_sky,
                precip::attach_precip,
                precip::sync_precip,
                models::fit_props,
                models::apply_fit,
                wind::apply_wind,
                models::flicker_campfires,
                villager::wander,
                villager::animate_walk_idle,
                (pick::drain_select, pick::pick_agent, pick::follow_selection).chain(),
                occlusion::fade_occluders,
                cam_follow::follow_camera,
                debug_screenshot,
            ),
        );
    }
}

/// 검증용 스크린샷 — 부팅 6초 후 1회 또는 P키(네이티브만).
#[cfg(not(target_arch = "wasm32"))]
fn debug_screenshot(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    if time.elapsed_secs() > 6.0 || keys.just_pressed(KeyCode::KeyP) {
        *done = true;
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                "/tmp/tf_native_shot.png",
            ));
    }
}

#[cfg(target_arch = "wasm32")]
fn debug_screenshot() {}
