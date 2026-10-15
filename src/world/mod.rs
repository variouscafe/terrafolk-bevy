//! 월드 — 데모 결정론 데이터 + 2.5D 아이소 렌더(지형 메시·빌보드·하루순환·수면).
//! 계약: 서버는 정사각 그리드를 주고 클라이언트가 투영(docs/data-contract.md).

pub mod billboard;
pub mod camera_rig;
pub mod data;
pub mod daynight;
pub mod terrain;
pub mod villager;
pub mod water;

use bevy::image::ImageSampler;
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<water::WaterMaterial>::default());
        app.init_asset::<water::WaterMaterial>();
        app.insert_resource(ClearColor(Color::srgb(0.30, 0.48, 0.68)))
            .insert_resource(data::WorldData::generate(20261015))
            .insert_resource(daynight::DayTime::default())
            .insert_resource(camera_rig::CameraRig::default())
            .add_systems(
                Startup,
                (
                    terrain::spawn_terrain,
                    water::spawn_water,
                    camera_rig::spawn_camera,
                    daynight::spawn_sun,
                    billboard::spawn_props,
                    villager::spawn_villagers,
                ),
            )
            .add_systems(
                Update,
                (
                    camera_rig::camera_control,
                    daynight::advance_time,
                    daynight::apply_sky,
                    villager::wander,
                    villager::animate,
                    billboard::animate_campfires,
                    enforce_nearest_sampling,
                ),
            );
    }
}

/// 픽셀 아트 규율 — 모든 텍스처 nearest 샘플링(3D 전용 빌드라 ImagePlugin 없이).
/// Default(선형) 상태만 1회 교체 — 재업로드 없음.
fn enforce_nearest_sampling(mut images: ResMut<Assets<Image>>) {
    for (_, img) in images.iter_mut() {
        if matches!(img.sampler, ImageSampler::Default) {
            img.sampler = ImageSampler::nearest();
        }
    }
}
