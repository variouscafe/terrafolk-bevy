use bevy::prelude::*;
use bevy::window::WindowMode;
use bevy::winit::WinitSettings;
use terrafolk_bevy::GamePlugin;

#[unsafe(no_mangle)]
unsafe extern "C" fn main_rs() {
    main();
}

// this macro is a no-op on ios and only needed for android since bevy 0.16
// see https://github.com/bevyengine/bevy/pull/14780
#[bevy_main]
fn main() {
    #[cfg(target_os = "ios")]
    unsafe {
        // Sets our audio session to Ambient mode to prevent background music from stopping.
        // The default for iOS apps is SoloAmbient, which stops background music.
        // See apple docs: https://developer.apple.com/documentation/avfaudio/avaudiosession/category-swift.struct/ambient
        if let Err(e) = objc2_avf_audio::AVAudioSession::sharedInstance()
            .setCategory_error(objc2_avf_audio::AVAudioSessionCategoryAmbient.unwrap())
        {
            println!("Error setting audio session category: {:?}", e);
        }
    }

    App::new()
        // 연속 렌더링 — mobile() 기본(React: 입력 시에만 프레임)은 살아있는
        // 세계 관전에 부적합(2026-10-17 실측: 무입력 시 정지 프레임, CPU 0%).
        // game 프리셋 = 포커스 중 Continuous + 백그라운드 60Hz 폴.
        .insert_resource(WinitSettings::game())
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resizable: false,
                    mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                    ..default()
                }),
                ..default()
            }),
            GamePlugin,
        ))
        .run();
}
