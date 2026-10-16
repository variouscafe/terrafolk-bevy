//! 주민 선택 — 클릭/탭 픽킹 + 발밑 선택 링 + JS 브리지.
//! 팬 드래그와 클릭을 이동거리로 구분(6px/9px 미만 = 클릭)하고, 카메라 광선과
//! 몸통 중심(지면+0.8m)의 최단 거리로 주민을 픽한다(줌 배율 무관).
//! 선택 변화는 tf.onPickAgent(id|null)로 JS에 통보하고, JS의 tf.selectAgent는
//! SelectBridge 큐를 거쳐 프레임 시작에 반영한다(데모 주민은 demo:N 가명).

use bevy::input::mouse::MouseButton;
use bevy::math::primitives::Annulus;
use bevy::prelude::*;

use crate::net::apply::LiveAgentTag;
use crate::world::villager::Villager;

/// 마우스 클릭 판정 임계(논리 px) — 이 이상 이동하면 팬 드래그
const CLICK_PX: f32 = 6.0;
/// 터치 탭 판정 임계(논리 px)
const TAP_PX: f32 = 9.0;
/// 픽 히트 반경(월드 m) — 몸통 중심 기준
const PICK_RADIUS: f32 = 0.8;
/// 몸통 중심 높이(지면 기준 m)
const TORSO_H: f32 = 0.8;

#[derive(Resource, Default)]
pub struct Selection {
    pub id: Option<String>,
    /// 선택된 주민 엔티티 — cam_follow(추적)·링 위치가 함께 읽는다
    pub entity: Option<Entity>,
    /// 선택 사건 카운터 — 같은 id 재선택(씬 재클릭)도 cam_follow 재개 트리거로
    /// 구분되게 한다(id만으론 변화 판정 불가)
    pub epoch: u32,
}

/// 발밑 선택 링 표식
#[derive(Component)]
pub struct SelectionRing;

/// JS→Rust 선택 큐 — tf.selectAgent(id|'') 호출을 다음 프레임에 반영.
#[derive(Resource, Default)]
pub struct SelectBridge {
    queue: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

/// 선택 링 스폰(부팅 1회, 숨김 상태) — 언릿 앰버 고리.
pub fn spawn_ring(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Annulus::new(0.40, 0.52)))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.78, 0.28),
            unlit: true,
            ..default()
        })),
        SelectionRing,
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::Hidden,
    ));
}

/// 브리지 등록 — JS가 tf.selectAgent(id|'')로 선택·해제를 요청한다(wasm).
pub fn register_bridge(bridge: Res<SelectBridge>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        let q = std::sync::Arc::clone(&bridge.queue);
        let select = Closure::<dyn Fn(String)>::new(move |id: String| {
            if let Ok(mut q) = q.lock() {
                q.push(id);
            }
        });
        let window = web_sys::window().expect("window");
        let tf = js_sys::Reflect::get(&window, &"terrafolk".into()).expect("terrafolk bridge");
        let _ = js_sys::Reflect::set(&tf, &"selectAgent".into(), select.as_js_value());
        select.forget(); // 앱 수명 유지
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = bridge;
}

/// JS 선택 요청 반영 — id로 엔티티 해석. 못 찾은 요청(월드 프라임 전 스폰 지연)은
/// id를 유지하고 follow_selection이 매 프레임 재해석한다 — 주민 목록 클릭이
/// 라이브 스폰보다 빠른 경합에서 선택이 묵살되지 않게. 빈 요청은 즉시 해제.
pub fn drain_select(
    mut sel: ResMut<Selection>,
    bridge: Res<SelectBridge>,
    q: Query<(Entity, &LiveAgentTag), With<Villager>>,
) {
    let drained: Vec<String> = {
        let Ok(mut queue) = bridge.queue.lock() else {
            return;
        };
        queue.drain(..).collect()
    };
    let Some(req) = drained.into_iter().last() else {
        return;
    };
    if req.is_empty() {
        sel.id = None;
        sel.entity = None;
        notify_pick(None);
        return;
    }
    sel.id = Some(req);
    sel.entity = sel
        .id
        .as_deref()
        .and_then(|want| q.iter().find(|(_, t)| t.0 == want).map(|(e, _)| e));
    if sel.entity.is_some() {
        sel.epoch = sel.epoch.wrapping_add(1);
        notify_pick(sel.id.as_deref());
    }
}

/// 클릭/탭 픽킹 — 광선-몸통중심 최단 거리. 빈 땅 클릭·Escape는 해제.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 입력·카메라·월드를 한 프레임 안에서 순서 보정
pub fn pick_agent(
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    q_cam: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    q_agents: Query<(Entity, &Transform, Option<&LiveAgentTag>), With<Villager>>,
    mut sel: ResMut<Selection>,
    mut press: Local<Option<Vec2>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        sel.id = None;
        sel.entity = None;
        notify_pick(None);
        return;
    }

    // 후보 픽셀 — 마우스 클릭(눌림↔뗌 이동 6px 미만)·터치 탭(9px 미만)
    let mut tap: Option<Vec2> = None;
    if buttons.just_pressed(MouseButton::Left) {
        *press = window.cursor_position();
    }
    if buttons.just_released(MouseButton::Left) {
        if let (Some(start), Some(cur)) = (*press, window.cursor_position())
            && start.distance(cur) < CLICK_PX
        {
            tap = Some(cur);
        }
        *press = None;
    }
    for t in touches.iter_just_released() {
        if t.start_position().distance(t.position()) < TAP_PX {
            tap = Some(t.position());
        }
    }
    let Some(px) = tap else { return };
    let Ok((cam, gt)) = q_cam.single() else {
        return;
    };
    let Some(ray) = cam.viewport_to_world(gt, px).ok() else {
        return;
    };

    let mut best: Option<(f32, Entity, String)> = None;
    for (e, tr, tag) in &q_agents {
        let center = tr.translation + Vec3::Y * TORSO_H;
        let v = center - ray.origin;
        let along = v.dot(*ray.direction);
        if along <= 0.0 {
            continue;
        }
        let d = (v - *ray.direction * along).length();
        if d > PICK_RADIUS {
            continue;
        }
        let id = tag
            .map(|t| t.0.clone())
            .unwrap_or_else(|| format!("demo:{}", e.index()));
        if best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
            best = Some((d, e, id));
        }
    }
    match best {
        Some((_, e, id)) => {
            sel.id = Some(id);
            sel.entity = Some(e);
            sel.epoch = sel.epoch.wrapping_add(1);
            notify_pick(sel.id.as_deref());
        }
        None => {
            sel.id = None;
            sel.entity = None;
            notify_pick(None);
            // 유물 카드(§17.38) — 빈 땅 클릭 지점을 JS에 넘겨 근처 유물 판정.
            // 유물이 있으면 tf-ui가 자원 팝업+카드를 연다(주민 선택은 해제 유지).
            #[cfg(target_arch = "wasm32")]
            if let Some(g) = super::camera_input::ground_at(cam, gt, px, 0.0) {
                notify_ground(g.x, g.z);
            }
        }
    }
}

/// 선택 링 추적 — 대상 발밑에 위치·맥동, 퇴장(엔티티 소멸) 시 해제 통보.
/// 스폰 전 선택(id만 있고 엔티티 없음)은 주민이 나타나면 붙이고 그때 통보한다.
#[allow(clippy::type_complexity)] // Bevy 쿼리 타입 — 링/주민 상호 배제 접근
pub fn follow_selection(
    time: Res<Time>,
    mut sel: ResMut<Selection>,
    q: Query<&Transform, With<Villager>>,
    q_tags: Query<(Entity, &LiveAgentTag), With<Villager>>,
    mut ring: Query<(&mut Transform, &mut Visibility), (With<SelectionRing>, Without<Villager>)>,
) {
    let Ok((mut tr, mut vis)) = ring.single_mut() else {
        return;
    };
    if sel.entity.is_some_and(|e| q.get(e).is_err()) {
        sel.id = None;
        sel.entity = None;
        notify_pick(None);
    }
    // 지연 해석 — 프라임 전 요청: 라이브 주민 스폰을 기다렸다 붙이고 통보
    if sel.id.is_some() && sel.entity.is_none() {
        sel.entity = sel
            .id
            .as_deref()
            .and_then(|want| q_tags.iter().find(|(_, t)| t.0 == want).map(|(e, _)| e));
        if sel.entity.is_some() {
            notify_pick(sel.id.as_deref());
        }
    }
    match sel.entity.filter(|e| q.get(*e).is_ok()) {
        Some(e) => {
            let at = q.get(e).expect("just checked").translation;
            tr.translation = Vec3::new(at.x, at.y + 0.03, at.z);
            let pulse = 1.0 + 0.05 * (time.elapsed_secs() * 3.2).sin();
            tr.scale = Vec3::splat(pulse);
            *vis = Visibility::Visible;
        }
        None => *vis = Visibility::Hidden,
    }
}

/// 선택 변화 JS 통보 — tf.onPickAgent(id|null), 없으면 조용히 스킵.
#[cfg(target_arch = "wasm32")]
fn notify_pick(id: Option<&str>) {
    use wasm_bindgen::prelude::*;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tf) = js_sys::Reflect::get(&window, &"terrafolk".into()) else {
        return;
    };
    let Ok(f) = js_sys::Reflect::get(&tf, &"onPickAgent".into()) else {
        return;
    };
    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
        let arg = match id {
            Some(s) => JsValue::from_str(s),
            None => JsValue::NULL,
        };
        let _ = f.call1(&tf, &arg);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn notify_pick(_id: Option<&str>) {}

/// 빈 땅 클릭 지점 JS 통보 — tf.uiPick(x, y), 없으면 조용히 스킵.
#[cfg(target_arch = "wasm32")]
fn notify_ground(x: f32, z: f32) {
    use wasm_bindgen::prelude::*;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tf) = js_sys::Reflect::get(&window, &"terrafolk".into()) else {
        return;
    };
    let Ok(f) = js_sys::Reflect::get(&tf, &"uiPick".into()) else {
        return;
    };
    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
        let _ = f.call2(
            &tf,
            &JsValue::from_f64(x as f64),
            &JsValue::from_f64(z as f64),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)] // wasm 빌드에서만 호출되는 스텁
fn notify_ground(_x: f32, _z: f32) {}

/// DOM 입력 포커스 여부 — winit keydown은 window에 붙어 채팅 입력 키도
/// Bevy에 오므로(ui.js의 tf.uiTyping), 타이핑 중 팬/줌/추적 해제를 막는다.
pub fn ui_typing() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| js_sys::Reflect::get(&w.into(), &"terrafolk".into()).ok())
            .and_then(|tf| js_sys::Reflect::get(&tf, &"uiTyping".into()).ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
