//! 월드 앵커 오버레이 — 주민 머리 위치를 화면 좌표로 투영해 JS에 흘린다.
//! 말풍선·이름 라벨·활동 배지(진행 링+아이콘)·수면 zZz·욕구 경고·미니맵 뷰포트
//! 박스가 이 좌표를 공유한다(FE scene/agents.ts의 씬 오버레이군 포트).
//! WebGL2는 텍스처 아틀라스·프래그먼트 유니폼 한도가 빡빡해 씬 내 텍스트
//! 렌더가 비실용적 — DOM 오버레이가 담당하고 좌표만 Rust가 정본으로 준다.
//!
//! 역방향 브리지 `tf.panTo(x, z)` — 미니맵 클릭 점프. 수동 팬과 동일 취급으로
//! 카메라 추적을 해제하고 관성을 끈다(관전 통제권 우선, cam_follow 규약).

use bevy::prelude::*;

use super::camera_input::PointerInput;
use super::camera_rig::CameraRig;
#[cfg(target_arch = "wasm32")]
use super::camera_rig::{CAM_PITCH_DEG, VIEW_H};
use super::villager::Villager;
use crate::net::apply::LiveAgentTag;
use crate::net::resources::LiveResourceTag;

/// 오버레이 앵커 높이(지면 기준 m) — 주민 상단(VILLAGER_H 1.62) 위 여백.
/// 이름·배지·풍선은 JS가 이 앵커에서 위로 쌓는다.
#[cfg(target_arch = "wasm32")]
const ANCHOR_H: f32 = 1.85;
/// JS→Rust 카메라 점프 큐 — tf.panTo(x, z) 호출을 다음 프레임에 반영.
#[derive(Resource, Default)]
pub struct PanBridge {
    queue: std::sync::Arc<std::sync::Mutex<Vec<(f32, f32)>>>,
}

/// 브리지 등록 — JS가 tf.panTo(x, z)로 뷰 중심 이동을 요청한다(wasm).
pub fn register_pan_bridge(bridge: Res<PanBridge>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        let q = std::sync::Arc::clone(&bridge.queue);
        let pan = Closure::<dyn Fn(f32, f32)>::new(move |x: f32, z: f32| {
            if let Ok(mut q) = q.lock() {
                q.push((x, z));
            }
        });
        let window = web_sys::window().expect("window");
        let tf = js_sys::Reflect::get(&window, &"terrafolk".into()).expect("terrafolk bridge");
        let _ = js_sys::Reflect::set(&tf, &"panTo".into(), pan.as_js_value());
        pan.forget(); // 앱 수명 유지
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = bridge;
}

/// 미니맵 점프 반영 — 뷰 중심 이동 + 추적 해제 + 관성 정리. 월드 경계 클램프는
/// camera_control이 같은 프레임에 다시 적용하므로 여기선 값만 쓴다.
pub fn apply_pan_to(
    bridge: Res<PanBridge>,
    mut rig: ResMut<CameraRig>,
    mut follow: ResMut<super::cam_follow::CamFollow>,
    mut input: ResMut<PointerInput>,
) {
    let drained: Vec<(f32, f32)> = {
        let Ok(mut q) = bridge.queue.lock() else {
            return;
        };
        q.drain(..).collect()
    };
    let Some((x, z)) = drained.into_iter().last() else {
        return;
    };
    rig.target = Vec2::new(x, z);
    follow.active = false; // 수동 이동 — 선택 유지, 추적은 해제(cam_follow 규약)
    input.stop_inertia();
}

/// 투영 피드 — 매 프레임 `tf.onOverlay(json)`:
/// `{"z":줌,"v":[cx,cy,박스w,박스h],"a":[["id",화면x,화면y],…]}`
/// 좌표는 뷰포트 논리 px(좌상 원점 — DOM 좌표계와 동일). v는 미니맵 뷰포트
/// 박스용 월드 타일 단위 축정렬 근사(부저각 회전 마름모의 AABB). 선택 정보는
/// JS가 TF_UI.state.selected로 이미 알고 있어 보내지 않는다.
pub fn overlay_feed(
    q_cam: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    window: Single<&Window>,
    q_agents: Query<(&LiveAgentTag, &Transform), With<Villager>>,
    rig: Res<CameraRig>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (q_cam, window, q_agents, rig);
    }
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        let (cam, gt) = q_cam.into_inner();
        // 페이로드 조립 — id는 서버 생성 문자열이지만 안전하게 이스케이프
        let mut buf = String::with_capacity(64 + 48 * q_agents.iter().count());
        buf.push_str("{\"z\":");
        push_f(&mut buf, rig.zoom);
        buf.push_str(",\"v\":[");
        push_f(&mut buf, rig.target.x);
        buf.push(',');
        push_f(&mut buf, rig.target.y);
        buf.push(',');
        let aspect = window.width() / window.height().max(1.0);
        let (bw, bh) = view_box(rig.zoom, aspect);
        push_f(&mut buf, bw);
        buf.push(',');
        push_f(&mut buf, bh);
        buf.push_str("],\"a\":[");
        let mut first = true;
        for (tag, tr) in &q_agents {
            let anchor = tr.translation + Vec3::Y * ANCHOR_H;
            let Ok(p) = cam.world_to_viewport(gt, anchor) else {
                continue;
            };
            if !first {
                buf.push(',');
            }
            first = false;
            buf.push_str("[\"");
            esc_id(&mut buf, &tag.0);
            buf.push_str("\",");
            push_f(&mut buf, p.x);
            buf.push(',');
            push_f(&mut buf, p.y);
            buf.push(']');
        }
        buf.push_str("]}");
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(tf) = js_sys::Reflect::get(&window.into(), &"terrafolk".into()) else {
            return;
        };
        let Ok(f) = js_sys::Reflect::get(&tf, &"onOverlay".into()) else {
            return;
        };
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            let _ = f.call1(&tf, &JsValue::from_str(&buf));
        }
    }
}

/// 미니맵 뷰포트 박스 — 부저각(요 45°·피치 35°) 정사영의 지면 풋프린트는
/// 대각선 회전 마름모. 축정렬 근사로 AABB 반폭을 쓴다(FE도 카메라 rect를
/// 박스로 그린다). 세로 성분 = 시야높이/2/sin(피치), 가로 성분 = 그×종횡비.
#[cfg(target_arch = "wasm32")]
fn view_box(zoom: f32, aspect: f32) -> (f32, f32) {
    let half_h = VIEW_H * 0.5 * zoom;
    let ground_v = half_h / CAM_PITCH_DEG.to_radians().sin();
    let ground_h = half_h * aspect;
    let half = (ground_v + ground_h) * std::f32::consts::FRAC_1_SQRT_2;
    (half * 2.0, half * 2.0)
}

/// 자원 호버 — 마우스 커서 아래 상호작용 자원(건물 발판·완공 건물·묘지·유물)을
/// 판정해 `tf.onHoverRes(id|null)`로 통보한다(§17.78 "누구 집"·비문·발명 설명
/// 툴팁). 문구 구성은 JS가 TF_UI.state.resMap의 state로 한다 — Rust는 판정만.
/// 발판은 내부 포함, 나머지는 중심 거리 1.1타일 근접 판정.
#[allow(clippy::type_complexity)] // Bevy 쿼리 타입 — 호버 대상이 혼합 컴포넌트
pub fn hover_res(
    window: Single<&Window>,
    q_cam: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    q_res: Query<(
        &LiveResourceTag,
        &Transform,
        Option<&super::bld::SiteRect>,
        Option<&super::models::PropRoot>,
    )>,
    mut last_id: Local<Option<u32>>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (window, q_cam, q_res);
        *last_id = None; // 네이티브 무동작 — 상태 초기화
    }
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        let Some(cursor) = window.cursor_position() else {
            if last_id.is_some() {
                *last_id = None;
                notify_hover(None);
            }
            return;
        };
        let Ok((cam, gt)) = q_cam.single() else {
            return;
        };
        let Some(g) = super::camera_input::ground_at(cam, gt, cursor, 0.0) else {
            if last_id.is_some() {
                *last_id = None;
                notify_hover(None);
            }
            return;
        };
        let (tx, tz) = (g.x.floor() as i32, g.z.floor() as i32);
        let mut hit: Option<u32> = None;
        let mut best_d = f32::MAX;
        for (tag, tr, rect, prop) in &q_res {
            if let Some(rect) = rect {
                if rect.contains(tx, tz) {
                    hit = Some(tag.0);
                    break;
                }
            }
            let interactive = prop.is_some_and(|p| {
                matches!(
                    p.kind,
                    super::data::PropKind::Grave | super::data::PropKind::Relic
                )
            });
            if interactive {
                let d = (tr.translation.x - g.x).powi(2) + (tr.translation.z - g.z).powi(2);
                if d < 1.1 * 1.1 && d < best_d {
                    best_d = d;
                    hit = Some(tag.0);
                }
            }
        }
        if hit != *last_id {
            *last_id = hit;
            notify_hover(hit);
        }
    }
}

/// 호버 통보 — tf.onHoverRes(id|null), 없으면 조용히 스킵.
#[cfg(target_arch = "wasm32")]
fn notify_hover(id: Option<u32>) {
    use wasm_bindgen::prelude::*;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tf) = js_sys::Reflect::get(&window.into(), &"terrafolk".into()) else {
        return;
    };
    let Ok(f) = js_sys::Reflect::get(&tf, &"onHoverRes".into()) else {
        return;
    };
    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
        let arg = match id {
            Some(v) => JsValue::from_f64(v as f64),
            None => JsValue::NULL,
        };
        let _ = f.call1(&tf, &arg);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)] // wasm 빌드에서만 호출되는 스텁
fn notify_hover(_id: Option<u32>) {}

/// 소수 페이로드 — 화면 px·타일 좌표에 충분한 정밀도(0.1)로 문자열 길이 유지.
#[cfg(target_arch = "wasm32")]
fn push_f(buf: &mut String, v: f32) {
    if !v.is_finite() {
        buf.push('0');
        return;
    }
    buf.push_str(&format!("{v:.1}"));
}

/// JSON 문자열 이스케이프 — id는 서버 uuid지만 제어 문자·따옴표 방어.
#[cfg(target_arch = "wasm32")]
fn esc_id(buf: &mut String, id: &str) {
    for c in id.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                buf.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => buf.push(c),
        }
    }
}
