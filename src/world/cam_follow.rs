//! 카메라 추적 — 주민 선택(FE camera-follow, docs §17.14) 시 리그가 대상을
//! 부드럽게 따라간다. 수동 팬 입력(WASD·드래그·터치)이 오면 즉시 해제 —
//! 관전 통제권이 우선한다. 선택 변화(목록 클릭·씬 픽)마다 추적을 재개한다.

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use super::camera_rig::CameraRig;
use super::pick::Selection;
use super::villager::Villager;

/// 초당 수렴 속도(지수 스무딩) — 카메라 리그 줌 스무딩과 비슷한 체감
const FOLLOW_SMOOTH: f32 = 3.0;

#[derive(Resource, Default)]
pub struct CamFollow {
    pub active: bool,
}

#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 입력·선택·카메라를 한 프레임 안에서 순서 보정
pub fn follow_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut rig: ResMut<CameraRig>,
    mut follow: ResMut<CamFollow>,
    sel: Res<Selection>,
    q: Query<&Transform, With<Villager>>,
    mut last_sel: Local<Option<String>>,
    mut last_epoch: Local<u32>,
) {
    // 선택 변화 → 추적 재개(같은 대상 재선택도 카메라를 데려온다). epoch은
    // 같은 id의 씬 재클릭도 재선택 사건으로 구분하게 해 준다(id만으론 불가).
    // 재개 프레임은 해제 검사를 건너뛴다 — 씬 픽 클릭의 just_pressed가
    // 같은 프레임에 추적을 꺼버리는 경합 방지(팬 해제는 다음 입력부터).
    let changed = *last_sel != sel.id || *last_epoch != sel.epoch;
    if changed {
        *last_sel = sel.id.clone();
        *last_epoch = sel.epoch;
        follow.active = sel.id.is_some();
    }
    if !follow.active {
        return;
    }
    // 수동 팬 해제 — 키보드·드래그 시작·터치 시작.
    // 채팅 입력 타이핑(winit keydown=window 스코프)은 해제 아님
    let pan_key = !super::pick::ui_typing()
        && keys.any_pressed([
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
        ]);
    if !changed
        && (pan_key
            || mouse.just_pressed(MouseButton::Left)
            || touches.iter_just_pressed().next().is_some())
    {
        follow.active = false;
        return;
    }
    let Some(entity) = sel.entity else {
        // 스폰 전 선택(id만 있음) — 기다린다(지연 해석 후 추적 개시).
        // 해제는 선택 자체가 사라질 때만.
        if sel.id.is_none() {
            follow.active = false;
        }
        return;
    };
    let Ok(tr) = q.get(entity) else {
        follow.active = false;
        return;
    };
    let target = Vec2::new(tr.translation.x, tr.translation.z);
    let k = 1.0 - (-(FOLLOW_SMOOTH * time.delta_secs())).exp();
    // ResMut Deref 복합 대입은 E0502 — 값 계산 후 단일 대입
    let cur = rig.target;
    rig.target = cur + (target - cur) * k;
}
