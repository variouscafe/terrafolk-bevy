//! 카메라 포인터 입력 — 광선 그랩 팬·핀치 줌·관성.
//! 팬은 화면→세계 근사 스케일 대신 **커서 아래 세계 지점을 잡는 그랩 방식**:
//! `Camera::viewport_to_world` 광선을 기준면(y=그랩 높이)에 교차시켜
//! 손가락·커서를 1:1로 추적한다(감도 계수가 원천 불필요).
//! 관성: 드래그 해제 시 마지막 속도 EMA로 지수 감쇠 미끄러짐(상용 게임 질감).

use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touch;
use bevy::prelude::*;

use super::data::WorldData;

/// 관성 마찰 — 프레임당 감쇠(60fps 환산, 0.94 ≔ 시정수 0.27s)
const INERTIA_FRICTION: f32 = 0.94;
/// 관성 발동 최소 플링 속도(월드 단위/초) — 이하면 즉시 정지
const INERTIA_FLING_MIN: f32 = 4.0;
/// 관성 스톱 관문(월드 단위/초)
const INERTIA_MIN_SPEED: f32 = 1.0;
/// 드래그 속도 EMA 시정수(초)
const VEL_TAU: f32 = 0.06;

/// 뷰포트(논리 px) → 기준면(y=plane_y) 교차점.
/// 카메라는 항상 45° 부저각이라 광선이 반드시 아래를 향한다(실패는 None).
pub fn ground_at(camera: &Camera, gt: &GlobalTransform, px: Vec2, plane_y: f32) -> Option<Vec3> {
    let ray = camera.viewport_to_world(gt, px).ok()?;
    if ray.direction.y > -1e-6 {
        return None;
    }
    let t = (plane_y - ray.origin.y) / ray.direction.y;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    Some(ray.origin + *ray.direction * t)
}

/// 지형 고도 2단 계산 — 1차(y=0) 교차점의 실제 지형 높이로 재교차.
/// 그랩 시작점·줌 앵커가 언덕 위에서도 정확히 잡히도록 한다.
fn ground_refined(
    camera: &Camera,
    gt: &GlobalTransform,
    px: Vec2,
    world: &WorldData,
) -> Option<Vec3> {
    let g0 = ground_at(camera, gt, px, 0.0)?;
    let h = world.height_at(g0.x.round() as i32, g0.z.round() as i32);
    if h.abs() > 1e-3 {
        ground_at(camera, gt, px, h).or(Some(g0))
    } else {
        Some(g0)
    }
}

/// 두 손가락 핀치 상태 — id 쌍·직전 거리·중심 그랩(무게중점 아래 세계점).
#[derive(Clone, Copy)]
struct PinchState {
    id0: u64,
    id1: u64,
    dist: f32,
    centroid_grab: Option<Vec3>,
}

/// 포인터 입력 상태(마우스 그랩·터치 팬/핀치·관성 속도).
#[derive(Default, Resource)]
pub struct PointerInput {
    /// 마우스 왼쪽 드래그로 잡은 세계점(.y = 그랩 기준면 높이)
    mouse_grab: Option<Vec3>,
    /// 한 손가락 터치 팬 (id, 그랩 세계점)
    touch_grab: Option<(u64, Vec3)>,
    /// 두 손가락 핀치
    pinch: Option<PinchState>,
    /// 드래그 속도 EMA(월드/초) — 해제 시 관성 시동값
    vel: Vec2,
    /// 관성 속도(월드/초) — 0이면 정지
    inertia: Vec2,
    /// 휠·트랙패드 핀치가 지정한 줌 앵커(뷰포트 논리 px). 줌 수렴·제스처 시작 시 해제
    pub zoom_anchor: Option<Vec2>,
}

/// 세계점 (x, 높이, z) → 타깃 좌표계 (x, z)
fn xz(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}

/// apply_pans 결과 — 메인 시스템이 target 반영·줌 앵커 판단에 쓴다.
pub struct PanFrame {
    /// 이번 프레임 팬 델타(월드 단위, 관성 포함) — 호출자가 target에 더한다
    pub pan: Vec2,
    /// 드래그·핀치 등 능동 제스처 진행 중(관성 아님)
    pub gesture_active: bool,
}

/// 눌린 터치를 id 정렬로 수집 — 프레임 간 페어링을 결정적으로 유지.
fn pressed_touches(touches: &Touches) -> Vec<&Touch> {
    let mut list: Vec<&Touch> = touches.iter().collect();
    list.sort_by_key(|t| t.id());
    list
}

impl PointerInput {
    /// 프레임 전반 — 제스처 전이 판정 + 핀치 줌 비(줌 목표에 곱할 값).
    /// 카메라 좌표계가 필요 없는 부분만 다뤄서 줌 스무딩 전에 호출한다.
    /// 그랩 앵커는 전이 시 None으로 리셋 — apply_pans에서 재설정(점프 없음).
    pub fn begin_frame(&mut self, mouse: &ButtonInput<MouseButton>, touches: &Touches) -> f32 {
        let list = pressed_touches(touches);
        let mut pinch_zoom = 1.0;

        // ── 마우스 그랩 해제 → 관성 시동
        if !mouse.pressed(MouseButton::Left) && self.mouse_grab.take().is_some() {
            self.release();
        }

        // ── 터치 상태 기계: 0/1/2+ 손가락 전이
        match list.len() {
            0 => {
                if self.touch_grab.take().is_some() || self.pinch.take().is_some() {
                    self.release();
                }
            }
            1 => {
                let id = list[0].id();
                // 핀치→잔여 한 손가락 팬 재개: 그랩 재설정(새 앵커, 이동 없음)
                if self.pinch.take().is_some() {
                    self.touch_grab = None;
                    self.stop_inertia();
                }
                // 기존 팬이 다른 손가락이면(빠른 재터치) 앵커 재설정
                if let Some((prev_id, _)) = self.touch_grab
                    && prev_id != id
                {
                    self.touch_grab = None;
                    self.stop_inertia();
                }
                if self.touch_grab.is_none() {
                    self.stop_inertia(); // 관성 중 새 터치 → 즉시 정지
                }
            }
            _ => {
                // 두 손가락 이상 → id 순 두 개로 핀치. 팬 그랩은 반납.
                let id0 = list[0].id();
                let id1 = list[1].id();
                let dist = list[0].position().distance(list[1].position());
                match self.pinch {
                    Some(ref mut p) if p.id0 == id0 && p.id1 == id1 && p.dist > 1.0 => {
                        // 진행 핀치 — 거리 비로 줌(클램프 0.55~1.8, 상용 체감)
                        pinch_zoom = (p.dist / dist).clamp(super::camera_rig::PINCH_FRAME_MIN, super::camera_rig::PINCH_FRAME_MAX);
                        p.dist = dist;
                    }
                    _ => {
                        // 신규·구성 변경 → 앵커 재설정(점프 없음), 관성 정지
                        self.pinch = Some(PinchState { id0, id1, dist, centroid_grab: None });
                        self.stop_inertia();
                    }
                }
                self.touch_grab = None;
            }
        }
        pinch_zoom
    }

    /// 제스처 종료 공통 — 속도 EMA가 관문을 넘으면 관성으로 전환.
    fn release(&mut self) {
        self.inertia = if self.vel.length() > INERTIA_FLING_MIN { self.vel } else { Vec2::ZERO };
        self.vel = Vec2::ZERO;
    }

    /// 관성·속도 즉시 정지(새 제스처 시작·클램프 벽 충돌).
    pub fn stop_inertia(&mut self) {
        self.inertia = Vec2::ZERO;
        self.zoom_anchor = None;
    }

    /// 프레임 후반 — 그랩 팬(광선 1:1 추적)·핀치 중심 팬·관성 적용.
    /// `gt`는 이번 프레임 적용될 카메라(줌 반영) — 줌과 동시에 잡은 점이 커서를 따라온다.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_pans(
        &mut self,
        camera: &Camera,
        gt: &GlobalTransform,
        world: &WorldData,
        mouse: &ButtonInput<MouseButton>,
        touches: &Touches,
        cursor: Option<Vec2>,
        dt: f32,
    ) -> PanFrame {
        let mut pan = Vec2::ZERO;
        let mut drag_delta: Option<Vec2> = None;

        // ── 마우스 그랩 팬 — 잡은 세계점이 커서를 따라오게 target 보정
        if mouse.pressed(MouseButton::Left)
            && let Some(px) = cursor
        {
            match self.mouse_grab {
                // 그랩 점은 제스처 내내 불변 — "잡은 세계점이 커서 아래" 제약이
                // 매 프레임 d=grab−ground(px,현재) 로 정확 복원된다(갱신 불필요).
                Some(grab) => {
                    if let Some(g) = ground_at(camera, gt, px, grab.y) {
                        let d = xz(grab) - xz(g);
                        pan += d;
                        drag_delta = Some(d);
                    }
                }
                None => self.mouse_grab = ground_refined(camera, gt, px, world),
            }
        }

        // ── 한 손가락 터치 그랩 팬
        let list = pressed_touches(touches);
        if list.len() == 1 {
            let (id, px) = (list[0].id(), list[0].position());
            match self.touch_grab {
                Some((gid, grab)) if gid == id => {
                    // 그랩 점 불변 (마우스와 동일 원리)
                    if let Some(g) = ground_at(camera, gt, px, grab.y) {
                        let d = xz(grab) - xz(g);
                        pan += d;
                        drag_delta = Some(d);
                    }
                }
                _ => self.touch_grab = ground_refined(camera, gt, px, world).map(|g| (id, g)),
            }
        }

        // ── 핀치 중심 팬 — 두 손가락 무게중심 아래 세계점을 그랩(회전은 고정 각이라 무시)
        if list.len() >= 2 {
            let c = (list[0].position() + list[1].position()) / 2.0;
            if let Some(ref mut p) = self.pinch {
                match p.centroid_grab {
                    Some(grab) => {
                        // 중심 그랩 점 불변 — 줌(비 변화)과 동시에도 중심 아래 세계점 유지
                        if let Some(g) = ground_at(camera, gt, c, grab.y) {
                            let d = xz(grab) - xz(g);
                            pan += d;
                            drag_delta = Some(d);
                        }
                    }
                    None => p.centroid_grab = ground_refined(camera, gt, c, world),
                }
            }
        }

        // ── 드래그 속도 EMA(관성 시드)
        if let Some(d) = drag_delta {
            let v_inst = d / dt;
            let k = 1.0 - (-dt / VEL_TAU).exp();
            self.vel += (v_inst - self.vel) * k;
        }

        // ── 관성 미끄러짐(능동 제스처 없을 때만 — 시작 시 이미 정지됨)
        let gesture_active = drag_delta.is_some();
        if !gesture_active && self.inertia.length_squared() > 0.0 {
            pan += self.inertia * dt;
            self.inertia *= INERTIA_FRICTION.powf(dt * 60.0);
            if self.inertia.length() < INERTIA_MIN_SPEED {
                self.inertia = Vec2::ZERO;
            }
        }

        PanFrame { pan, gesture_active }
    }
}
