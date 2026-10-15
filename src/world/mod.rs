//! 월드 — 데모 결정론 데이터 + 2.5D 아이소 렌더(스플랫 지형·3D 소품·하루순환·수면).
//! 계약: 서버는 정사각 그리드를 주고 클라이언트가 투영(docs/data-contract.md).

pub mod billboard;
pub mod camera_rig;
pub mod data;
pub mod daynight;
pub mod models;
pub mod splat;
pub mod terrain;
pub mod villager;
pub mod water;
pub mod wind;

use bevy::image::ImageSampler;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<water::WaterMaterial>::default());
        app.init_asset::<water::WaterMaterial>();
        app.add_plugins(MaterialPlugin::<splat::TerrainMaterial>::default());
        app.add_plugins(MaterialPlugin::<wind::WindMaterial>::default());
        app.insert_resource(ClearColor(Color::srgb(0.30, 0.48, 0.68)))
            .insert_resource(data::WorldData::generate(20261015))
            .insert_resource(daynight::DayTime::default())
            .insert_resource(camera_rig::CameraRig::default())
            .insert_resource(PixelTextures::default())
            .add_systems(
                Startup,
                (
                    terrain::spawn_terrain,
                    water::spawn_water,
                    camera_rig::spawn_camera,
                    daynight::spawn_sun,
                    models::spawn_props,
                    villager::spawn_villagers,
                ),
            )
            .add_systems(
                Update,
                (
                    camera_rig::camera_control,
                    daynight::advance_time,
                    daynight::apply_sky,
                    models::fit_props,
                    models::apply_fit,
                    wind::apply_wind,
                    models::flicker_campfires,
                    villager::wander,
                    villager::animate,
                    enforce_pixel_sampling,
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
            .observe(bevy::render::view::screenshot::save_to_disk("/tmp/tf_native_shot.png"));
    }
}

#[cfg(target_arch = "wasm32")]
fn debug_screenshot() {}

/// 픽셀아트 텍스처 목록(주민 스프라이트) — nearest 샘플링 대상.
/// 3D GLB·지형 텍스처는 선형(페인티드)이라 전역 강제를 폐지했다.
#[derive(Resource, Default)]
pub struct PixelTextures(pub Vec<Handle<Image>>);

/// 등록된 픽셀 텍스처만 nearest로 1회 교체(재업로드 없음).
fn enforce_pixel_sampling(
    pixel: Res<PixelTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    if pixel.is_added() || pixel.is_changed() {
        for h in &pixel.0 {
            if let Some(mut img) = images.get_mut(h.id()) {
                if matches!(img.sampler, ImageSampler::Default) {
                    img.sampler = ImageSampler::nearest();
                }
            }
        }
    }
}
