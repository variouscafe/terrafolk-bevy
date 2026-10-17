//! 월드 — 데모 결정론 데이터 + 2.5D 아이소 렌더(스플랫 지형·3D 소품·하루순환·수면).
//! 계약: 서버는 정사각 그리드를 주고 클라이언트가 투영(docs/data-contract.md).

pub mod bld;
pub mod cam_follow;
pub mod camera_input;
pub mod camera_rig;
pub mod data;
pub mod daynight;
pub mod materials_guard;
pub mod models;
pub mod occlusion;
pub mod legacy_world;
pub mod overlay;
pub mod pick;
pub mod precip;
pub mod propsfx;
pub mod sky;
pub mod splat;
pub mod terrain;
pub mod villager;
pub mod water;
pub mod wind;

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub struct WorldPlugin;

/// 안드로이드 GPU 전처리 강제 OFF — 시스템 본체(위 cfg 블록에서 등록).
#[cfg(target_os = "android")]
fn clamp_gpu_preprocessing(
    mut s: bevy::prelude::ResMut<bevy::render::batching::gpu_preprocessing::GpuPreprocessingSupport>,
) {
    s.max_supported_mode = bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode::None;
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        materials_guard::nonfatal_render_errors(app);
        app.add_plugins(bevy::pbr::MaterialPlugin::<water::WaterMaterial>::default());
        app.init_asset::<water::WaterMaterial>();
        app.add_plugins(MaterialPlugin::<splat::TerrainMaterial>::default());
        app.add_plugins(MaterialPlugin::<wind::WindMaterial>::default());
        app.insert_resource(ClearColor(Color::srgb(0.30, 0.48, 0.68)))
            .init_resource::<models::MeshAabbCache>()
            .insert_resource(data::WorldData::generate(20261015))
            .insert_resource(daynight::DayTime::default())
            .insert_resource(precip::WeatherState::default())
            .insert_resource(camera_rig::CameraRig::default())
            .insert_resource(camera_input::PointerInput::default())
            .insert_resource(pick::Selection::default())
            .insert_resource(pick::SelectBridge::default())
            .insert_resource(cam_follow::CamFollow::default())
            .insert_resource(overlay::PanBridge::default())
            .add_systems(
                Startup,
                (
                    camera_rig::spawn_camera,
                    daynight::spawn_sun,
                    sky::spawn_birds,
                    pick::spawn_ring,
                    pick::register_bridge,
                    overlay::register_pan_bridge,
                ),
            );
        // 데모 결정론 월드는 네이티브 개발 빌드 전용 — 웹(wasm)은 라이브 접속→
        // 프라임으로 곧장 간다(스폰-철거 낭비·플래시 제거. 로딩 오버레이는
        // tf.uiPrime 통보로 해제).
        // 네이티브 데모 — 레거시 FE와 동일한 공용 맵(시드 20261011)으로 구동.
        // 웹은 prime_live_world가 라이브 맵으로 교체한다.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, legacy_world::prime_legacy_world);
        // 안드로이드 — GPU 전처리(버퍼 매핑 리드백) 강제 OFF. 소프트웨어 GPU
        // (SwiftShader)와 일부 모바일 드라이버에서 DeviceLost 후 매핑 패닉으로
        // 앱이 죽는다(2026-10-17 에뮬레이터 실측). CPU 배칭으로 돌려 안정화 —
        // WebGL2 웹 경로도 원래 CPU 전처리라 품질 차이 없다.
        #[cfg(target_os = "android")]
        {
            if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
                render_app.add_systems(
                    bevy::render::Render,
                    clamp_gpu_preprocessing
                        .in_set(bevy::render::RenderSystems::Prepare),
                );
            }
        }
        app.add_systems(Update, materials_guard::guard_blend_materials);
        app.add_systems(
            Update,
            (
                overlay::apply_pan_to,
                camera_rig::camera_control,
                overlay::overlay_feed,
                daynight::advance_time,
                daynight::apply_sky,
                models::fit_props,
                models::apply_fit,
                wind::apply_wind,
                models::flicker_campfires,
                (
                    bld::site_dressing,
                    bld::hut_lights,
                    overlay::hover_res, // §17.78 자원 호버 툴팁 — 미등록으로 죽어있었다(리뷰 #3)
                    propsfx::bob_relics,
                    propsfx::spark_veins,
                    sky::sky_update,
                ),
                villager::wander,
                villager::animate_walk_idle,
                villager::retry_villager_setup,
                // 렌더 진단 프로브 — dev 빌드 전용(운영 콘솔 지저분함 방지, 리뷰 #9)
                #[cfg(debug_assertions)]
                villager::villager_render_probe,
                (pick::drain_select, pick::pick_agent, pick::follow_selection).chain(),
                occlusion::conceal_occluders,
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
