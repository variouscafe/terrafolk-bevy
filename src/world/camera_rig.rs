//! 카메라 리그 — 45° 고정 절대각(아이소 순수성) + 저 FOV. WASD/화살표 팬·휠 줌.

use bevy::prelude::*;

use super::data::SIZE;

#[derive(Resource)]
pub struct CameraRig {
    pub target: Vec2,
    pub zoom: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig { target: Vec2::splat(SIZE as f32 / 2.0), zoom: 1.0 }
    }
}

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 32_f32.to_radians(),
            near: 0.5,
            far: 220.0,
            ..default()
        }),
        Transform::from_translation(pan_offset(&CameraRig::default())),
    ));
}

fn pan_offset(rig: &CameraRig) -> Vec3 {
    let d = 15.0 * rig.zoom;
    Vec3::new(rig.target.x, d, rig.target.y + d)
}

pub fn camera_control(
    keys: Res<ButtonInput<KeyCode>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut rig: ResMut<CameraRig>,
    mut q: Query<&mut Transform, With<Camera3d>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { dir.y -= 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { dir.y += 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { dir.x -= 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if dir != Vec2::ZERO {
        let sp = 14.0 * rig.zoom * 0.016;
        rig.target += dir.normalize() * sp;
    }
    if wheel.delta.y.abs() > 1e-4 {
        rig.zoom = (rig.zoom * (1.1_f32).powf(-wheel.delta.y)).clamp(0.45, 2.2);
    }
    rig.target = rig.target.clamp(Vec2::splat(2.0), Vec2::splat(SIZE as f32 - 2.0));
    if let Ok(mut tr) = q.single_mut() {
        tr.translation = pan_offset(&rig);
        tr.look_at(Vec3::new(rig.target.x, 0.0, rig.target.y), Vec3::Y);
    }
}
