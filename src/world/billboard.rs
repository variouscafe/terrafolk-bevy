//! 빌보드 헬퍼 — 고정 절대각 카메라를 향한 스태틱 쿼드(G3에서 3D 캐릭터로 대체됨).
//! 현재 사용처: 주민 픽셀 스프라이트(방향 프레임 → 회전 불변이라 매 프레임 정렬 불필요).

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

pub const PIXEL: f32 = 0.028;

/// 카메라 절대각(45° 부저각) — 전 빌보드 공통 방향
pub fn billboard_quat() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2 * 0.5)
}
