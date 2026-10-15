//! 카메라 리그 — 45° 고정 절대각(아이소 순수성) + 저 FOV.
//! 줌: 휠·핀치 제스처(트랙패드)·모바일 두 손가락·키보드(+/-) 입력을
//! 목표값에 누적하고 지수 스무딩으로 부드럽게 추적(전후 0.3~4.2배).

use bevy::input::gestures::PinchGesture;
use bevy::input::touch::Touch;
use bevy::prelude::*;

use super::data::SIZE;

/// 줌 범위 — 거리 = 15×zoom(최대 줌인 4.5m ↔ 최대 줌아웃 63m 고도)
const ZOOM_MIN: f32 = 0.3;
const ZOOM_MAX: f32 = 4.2;
/// 스무딩 계수(초당 수렴 속도)
const ZOOM_SMOOTH: f32 = 9.0;

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
    let d = 15.0 * zoom;
    Vec3::new(rig.target.x, d, rig.target.y + d)
}

pub fn camera_control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut pinch: MessageReader<PinchGesture>,
    touches: Res<Touches>,
    mut rig: ResMut<CameraRig>,
    mut q: Query<&mut Transform, With<Camera3d>>,
    mut pinch_prev: Local<Option<(u64, u64, f32)>>,
) {
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
    // 휠(트랙패드 스크롤 포함) — 위=줌인
    if wheel.delta.y.abs() > 1e-4 {
        rig.zoom_goal *= (1.15_f32).powf(-wheel.delta.y * 0.4);
    }
    // 트랙패드 핀치 제스처(macOS/iOS) — 양수=벌림(줌인)
    for g in pinch.read() {
        rig.zoom_goal *= 1.0 / (1.0 + g.0.clamp(-0.5, 0.5));
    }
    // 모바일 두 손가락 핀치 — 터치 쌍 거리 비로 스케일
    let active: Vec<&Touch> = touches.iter().collect();
    if active.len() == 2 {
        let (a, b) = (active[0], active[1]);
        let dist = a.position().distance(b.position());
        if let Some((id0, id1, prev)) = *pinch_prev {
            if a.id() == id0 && b.id() == id1 && prev > 1.0 {
                rig.zoom_goal *= (prev / dist).clamp(0.8, 1.25);
            }
        }
        *pinch_prev = Some((a.id(), b.id(), dist));
    } else {
        *pinch_prev = None;
    }
    // 키보드 +/-
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        rig.zoom_goal *= 1.0 - 1.2 * time.delta_secs();
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        rig.zoom_goal *= 1.0 + 1.2 * time.delta_secs();
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
