//! 카메라 리그 — 정사영(Orthographic) 고정 절대각 아이소 뷰.
//! 참조 스타일(코지 페인티드 3/4): 요 45°·피치 35°, 세계 수직 고정.
//! 줌: 투영 scale(작을수록 확대) — 휠·트랙패드 핀치·모바일 두 손가락·키보드(+/-),
//! 줌투커서는 정사영 축척비 공식(앵커 세계점의 뷰센터 오프셋 × 줌비)으로 보정.
//! 팬: 광선 그랩(camera_input — 1:1 추적, 정사영 평행광선에도 동일) + WASD + 관성.

use bevy::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::DebandDither;
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::ColorGrading;

use super::camera_input::{PointerInput, ground_at};
use super::data::WorldData;

/// 줌 범위 — scale = VIEW_H × zoom(최대 줌인 ≈9m 시야 ↔ 최대 줌아웃 ≈126m 시야)
pub const ZOOM_MIN: f32 = 0.3;
pub const ZOOM_MAX: f32 = 4.2;
/// 스무딩 계수(초당 수렴 속도)
const ZOOM_SMOOTH: f32 = 9.0;
/// 정사영 기본 시야 높이(월드 단위) — 줌 1에서 세로로 보이는 폭.
/// 참조 구도(마을 스케일 — 코티지 4.2m가 화면 세로의 ~20%).
pub const VIEW_H: f32 = 22.0;
/// 카메라 거리(정사영은 크기 무관 — near/far 클리핑·그림자 프러스텀용).
/// ⚠ terrain_splat.wgsl의 안개 축 상수(140.0·뷰방향)와 정합 — 변경 시 양쪽 수정.
const CAM_DIST: f32 = 140.0;
/// 부저각 — 요(대각)·피치(내려다봄). 참조 이미지 3/4 뷰.
const CAM_YAW_DEG: f32 = 45.0;
pub const CAM_PITCH_DEG: f32 = 35.0;

/// 휠 줌 — 한 스텝(노치)당 배율 기수(1.65^Δ)·Δ 게인
const WHEEL_ZOOM_BASE: f32 = 1.65;
const WHEEL_ZOOM_GAIN: f32 = 1.0;
/// 트랙패드 핀치 제스처 감도(macOS·iOS — 프레임당 스케일에 곱함)
const PINCH_GESTURE_GAIN: f32 = 5.0;
/// 모바일 두 손가락 핀치 — 프레임당 줌 비 클램프(상용 체감 폭)
pub const PINCH_FRAME_MIN: f32 = 0.55;
pub const PINCH_FRAME_MAX: f32 = 1.8;
/// 키보드 +/- 줌 속도(초당 배율 변화율)
const KEY_ZOOM_RATE: f32 = 1.4;
/// 키보드 팬 속도(초·줌 1 기준 월드 단위)
const KEY_PAN_SPEED: f32 = 16.0;

#[derive(Resource)]
pub struct CameraRig {
    pub target: Vec2,
    pub zoom: f32,      // 현재(렌더에 적용)
    pub zoom_goal: f32, // 목표(입력 누적)
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig {
            target: Vec2::splat(super::data::SIZE as f32 / 2.0),
            zoom: 1.0,
            zoom_goal: 1.0,
        }
    }
}

/// 정사영 투영 — 세로 고정(FixedVertical), scale=zoom으로 줌.
fn ortho_projection() -> Projection {
    Projection::Orthographic(OrthographicProjection {
        near: 1.0,
        far: 400.0,
        scaling_mode: ScalingMode::FixedVertical {
            viewport_height: VIEW_H,
        },
        ..OrthographicProjection::default_3d()
    })
}

/// 뷰 방향 반대편 오프셋 — 요·피치 구면(정사영이라 거리는 클리핑용 상수).
fn view_offset() -> Vec3 {
    let (p, y) = (CAM_PITCH_DEG.to_radians(), CAM_YAW_DEG.to_radians());
    let horiz = CAM_DIST * p.cos();
    Vec3::new(horiz * y.cos(), CAM_DIST * p.sin(), horiz * y.sin())
}

fn pan_offset(rig: &CameraRig) -> Vec3 {
    Vec3::new(rig.target.x, 0.0, rig.target.y) + view_offset()
}

pub fn spawn_camera(mut commands: Commands) {
    let rig = CameraRig::default();
    let mut tr = Transform::from_translation(pan_offset(&rig));
    tr.look_at(Vec3::new(rig.target.x, 0.0, rig.target.y), Vec3::Y);
    commands.spawn((
        Camera3d::default(),
        ortho_projection(),
        tr,
        // 후처리 — DebandDither(하늘 밴딩 제거), ColorGrading(무드),
        // Bloom(모닥불 불꽃 번짐).
        DebandDither::Enabled,
        ColorGrading::default(),
        Bloom::NATURAL,
    ));
}

/// 회전은 항상 동일(부저각 고정) — 위치만 옮긴 가상 카메라 변환.
/// 줌 스무딩 직후·팬 적용 전 좌표계로 그랩/앵커 광선을 계산한다.
fn camera_at(gt: &GlobalTransform, pos: Vec3) -> GlobalTransform {
    GlobalTransform::from_translation(pos - gt.translation()) * *gt
}

#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 입력·카메라·월드를 한 프레임 안에서 순서 보정
pub fn camera_control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<bevy::input::mouse::MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    mut pinch: MessageReader<PinchGesture>,
    touches: Res<Touches>,
    window: Single<&Window>,
    mut rig: ResMut<CameraRig>,
    mut input: ResMut<PointerInput>,
    world: Res<WorldData>,
    mut q: Query<(&Camera, &GlobalTransform, &mut Transform, &mut Projection), With<Camera3d>>,
) {
    let Ok((camera, gt, mut tr, mut proj)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs().max(1e-4);
    let cursor = window.cursor_position();

    // ── 1) 터치 제스처 전이 + 두 손가락 핀치 비(줌 목표 반영 전에 확정)
    let pinch_zoom = input.begin_frame(&mouse, &touches);

    // ── 2) 줌 입력 누적
    // 휠 — 상용 체감 배율(노치당 ~1.65배). 커서가 있으면 줌투커서 앵커 지정
    if wheel.delta.y.abs() > 1e-4 {
        let d = (wheel.delta.y * WHEEL_ZOOM_GAIN).clamp(-4.0, 4.0);
        rig.zoom_goal *= WHEEL_ZOOM_BASE.powf(-d);
        input.zoom_anchor = cursor;
    }
    // 트랙패드 핀치 제스처(macOS/iOS) — 감도 ×5.0, 앵커는 커서
    let mut gestured = false;
    for g in pinch.read() {
        rig.zoom_goal *= 1.0 / (1.0 + (g.0 * PINCH_GESTURE_GAIN).clamp(-0.9, 0.9));
        gestured = true;
    }
    if gestured {
        input.zoom_anchor = cursor;
    }
    // 모바일 두 손가락 핀치 비(begin_frame에서 전이 판정 완료)
    rig.zoom_goal *= pinch_zoom;
    // 키보드 +/- — 뷰 중심 줌(앵커 없음). 채팅 입력 타이핑 중에는 무시
    // (winit keydown은 window 스코프 — DOM 입력 키도 여기 온다)
    let typing = super::pick::ui_typing();
    if !typing && (keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd)) {
        rig.zoom_goal *= 1.0 - KEY_ZOOM_RATE * dt;
    }
    if !typing && (keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract)) {
        rig.zoom_goal *= 1.0 + KEY_ZOOM_RATE * dt;
    }

    // ── 3) 목표 클램프 → 지수 스무딩
    rig.zoom_goal = rig.zoom_goal.clamp(ZOOM_MIN, ZOOM_MAX);
    let k = 1.0 - (-(ZOOM_SMOOTH * dt)).exp();
    let zoom_prev = rig.zoom;
    rig.zoom += (rig.zoom_goal - rig.zoom) * k;
    let zoom_moved = (rig.zoom - zoom_prev).abs() > 1e-5;

    // ── 4) 이번 프레임 적용될 카메라(줌 반영·팬 적용 전) — 그랩 광선 기준.
    //    그랩 팬 델타는 카메라 평행이동 불변이므로 팬 전 좌표로 계산해도 정확하다.
    let gt_next = camera_at(gt, pan_offset(&rig));

    // ── 5) 그랩 팬(마우스·한 손가락·핀치 중심) + 관성
    let frame = input.apply_pans(camera, &gt_next, &world, &mouse, &touches, cursor, dt);
    rig.target += frame.pan;

    // ── 6) 키보드 팬(WASD·화살표) — 줌 배율 반영 속도. 타이핑 중 무시(위 참조)
    let mut dir = Vec2::ZERO;
    if !typing && (keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp)) {
        dir.y -= 1.0;
    }
    if !typing && (keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown)) {
        dir.y += 1.0;
    }
    if !typing && (keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft)) {
        dir.x -= 1.0;
    }
    if !typing && (keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight)) {
        dir.x += 1.0;
    }
    if dir != Vec2::ZERO {
        let sp = KEY_PAN_SPEED * rig.zoom * dt;
        rig.target += dir.normalize() * sp;
    }

    // ── 7) 줌투커서(정사영) — 능동 제스처가 없을 때만.
    //    정사영은 화면 오프셋 ∝ (세계점−뷰중심)/scale 이므로, 앵커 a₀를 커서 아래
    //    유지하려면 target += (a₀−target)·(1−zoom/zoom_prev). 앵커 아래 지형
    //    고도(2단 계산)로 잡아 언덕 위에서도 정확하다.
    if zoom_moved && !frame.gesture_active {
        if let Some(px) = input.zoom_anchor {
            let anchor = ground_at(camera, gt, px, 0.0).and_then(|g| {
                let h = world.height_at(g.x.round() as i32, g.z.round() as i32);
                ground_at(camera, gt, px, h).or(Some(g))
            });
            if let Some(a0) = anchor {
                let ratio = 1.0 - rig.zoom / zoom_prev.max(1e-4);
                let target = rig.target;
                rig.target += (Vec2::new(a0.x, a0.z) - target) * ratio;
            }
        }
    } else if !zoom_moved {
        input.zoom_anchor = None; // 수렴 완료 — 앵커 해제
    }

    // ── 8) 월드 경계 클램프 — 벽에 박으면 관성 즉시 정지(미끄러짐 없음)
    let bound = (world.size as f32 - 2.0).max(2.0);
    let unclamped = rig.target;
    rig.target = rig.target.clamp(Vec2::splat(2.0), Vec2::splat(bound));
    if rig.target != unclamped {
        input.stop_inertia();
    }

    // ── 9) 적용 — 위치·방향·정사영 scale
    tr.translation = pan_offset(&rig);
    tr.look_at(Vec3::new(rig.target.x, 0.0, rig.target.y), Vec3::Y);
    if let Projection::Orthographic(o) = &mut *proj {
        o.scale = rig.zoom;
    }
}
