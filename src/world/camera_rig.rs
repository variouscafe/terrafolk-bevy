//! 카메라 리그 — 45° 고정 절대각(아이소 순수성) + 저 FOV.
//! 줌: 휠·핀치 제스처(트랙패드)·모바일 두 손가락·키보드(+/-) — 감도 강화.
//! 팬: 드래그 그랩(마우스·한 손가락 터치) + WASD/화살표.
//! 줌·팬 모두 목표값 누적 → 지수 스무딩(전후 0.3~4.2배).

use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touch;
use bevy::prelude::*;

use super::data::SIZE;

/// 줌 범위 — 거리 = 15×zoom(최대 줌인 4.5m ↔ 최대 줌아웃 63m 고도)
const ZOOM_MIN: f32 = 0.3;
const ZOOM_MAX: f32 = 4.2;
/// 스무딩 계수(초당 수렴 속도)
const ZOOM_SMOOTH: f32 = 9.0;
/// 카메라 고도 계수(pan_offset과 정합)
const CAM_DIST: f32 = 15.0;
/// 수직 시야 → 화면당 세계 스케일 환산(fov 32°)
const FOV_TAN: f32 = 0.287; // tan(16°)

#[derive(Resource)]
pub struct CameraRig {
    pub target: Vec2,
    pub zoom: f32,      // 현재(렌더에 적용)
    pub zoom_goal: f32, // 목표(입력 누적)
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig {
            target: Vec2::splat(SIZE as f32 / 2.0),
            zoom: 1.0,
            zoom_goal: 1.0,
        }
    }
}

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 32_f32.to_radians(),
            near: 0.5,
            far: 320.0,
            ..default()
        }),
        Transform::from_translation(pan_offset(CameraRig::default().zoom, &CameraRig::default())),
    ));
}

fn pan_offset(zoom: f32, rig: &CameraRig) -> Vec3 {
    let d = CAM_DIST * zoom;
    Vec3::new(rig.target.x, d, rig.target.y + d)
}

#[derive(Default)]
pub struct DragState {
    /// 마우스 드래그 직전 커서(논리 px)
    mouse_prev: Option<Vec2>,
    /// 한 손가락 터치 (id, 직전 위치)
    touch_prev: Option<(u64, Vec2)>,
}

pub fn camera_control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut pinch: MessageReader<PinchGesture>,
    touches: Res<Touches>,
    window: Single<&Window>,
    mut rig: ResMut<CameraRig>,
    mut q: Query<&mut Transform, With<Camera3d>>,
    mut pinch_prev: Local<Option<(u64, u64, f32)>>,
    mut drag: Local<DragState>,
) {
    // ── 화면→세계 스케일(드래그 팬 정밀도) — 줌 연동
    let (w, h) = (window.width(), window.height());
    let dist = CAM_DIST * rig.zoom;
    let per_px_h = 2.0 * dist * FOV_TAN / w.max(1.0);
    let per_px_v = per_px_h * 1.414; // 45° 부저각 수직 왜곡

    // ── 드래그 그랩 팬(마우스) — 지도를 잡고 끌기
    if mouse.pressed(MouseButton::Left) {
        if let Some(pos) = window.cursor_position() {
            if let Some(prev) = drag.mouse_prev {
                let d = pos - prev;
                rig.target.x -= d.x * per_px_h;
                rig.target.y -= d.y * per_px_v;
            }
            drag.mouse_prev = Some(pos);
        }
    } else {
        drag.mouse_prev = None;
    }

    // ── 한 손가락 터치 드래그 팬(모바일)
    let touch_list: Vec<&Touch> = touches.iter().collect();
    if touch_list.len() == 1 {
        let t = touch_list[0];
        match drag.touch_prev {
            Some((id, prev)) if id == t.id() => {
                let d = t.position() - prev;
                rig.target.x -= d.x * per_px_h;
                rig.target.y -= d.y * per_px_v;
            }
            _ => {}
        }
        drag.touch_prev = Some((t.id(), t.position()));
    } else {
        drag.touch_prev = None;
    }

    // ── 팬(WASD·화살표) — 줌 배율 반영 속도
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { dir.y -= 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { dir.y += 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { dir.x -= 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if dir != Vec2::ZERO {
        let sp = 16.0 * rig.zoom * time.delta_secs();
        rig.target += dir.normalize() * sp;
    }

    // ── 줌 입력 누적
    // 휠 — 감도 강화(한 스텝 체감 ~40%)
    if wheel.delta.y.abs() > 1e-4 {
        let d = (wheel.delta.y * 0.8).clamp(-4.0, 4.0);
        rig.zoom_goal *= 1.5_f32.powf(-d);
    }
    // 트랙패드 핀치 제스처(macOS/iOS) — 감도 ×3.5
    for g in pinch.read() {
        rig.zoom_goal *= 1.0 / (1.0 + (g.0 * 3.5).clamp(-0.7, 0.7));
    }
    // 모바일 두 손가락 핀치 — 터치 쌍 거리 비로 스케일
    if touch_list.len() == 2 {
        let (a, b) = (touch_list[0], touch_list[1]);
        let tdist = a.position().distance(b.position());
        if let Some((id0, id1, prev)) = *pinch_prev {
            if a.id() == id0 && b.id() == id1 && prev > 1.0 {
                rig.zoom_goal *= (prev / tdist).clamp(0.7, 1.4);
            }
        }
        *pinch_prev = Some((a.id(), b.id(), tdist));
    } else {
        *pinch_prev = None;
    }
    // 키보드 +/-
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        rig.zoom_goal *= 1.0 - 1.4 * time.delta_secs();
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        rig.zoom_goal *= 1.0 + 1.4 * time.delta_secs();
    }

    // ── 목표 클램프 → 지수 스무딩
    rig.zoom_goal = rig.zoom_goal.clamp(ZOOM_MIN, ZOOM_MAX);
    let k = 1.0 - (-(ZOOM_SMOOTH * time.delta_secs())).exp();
    rig.zoom += (rig.zoom_goal - rig.zoom) * k;

    // ── 적용
    rig.target = rig.target.clamp(Vec2::splat(2.0), Vec2::splat(SIZE as f32 - 2.0));
    if let Ok(mut tr) = q.single_mut() {
        tr.translation = pan_offset(rig.zoom, &rig);
        tr.look_at(Vec3::new(rig.target.x, 0.0, rig.target.y), Vec3::Y);
    }
}
