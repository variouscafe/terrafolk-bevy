//! 선택 주민 시야 확보 — 주변 오브젝트(나무·덤불·풀·건물) 페이드.
//! 관전 몰입 지원: 주민을 클릭·추적하면 반경 안 가림물이 부드럽게 반투명해져
//! 캐릭터가 숲·건물에 묻히지 않는다(상용 시뮬의 occlusion fade 패턴).
//! 재질은 인스턴스별 복제(원본 핸들 보존) — 바람 재질(WindMaterial)도 동일
//! 경로로 페이드하고, 반경 이탈 시 원본으로 되돌려 배칭·바람을 복원한다.

use bevy::prelude::*;

use super::data::PropKind;
use super::models::{Fitted, PropRoot};
use super::pick::Selection;
use super::villager::Villager;
use super::wind::{WindApplied, WindExtension, WindMaterial, is_wind_target};

/// 페이드 대상 종 — 시야를 가리는 큰 식물과 건물. 모닥불·유물 등 소형은 유지.
fn occludes(kind: PropKind) -> bool {
    matches!(
        kind,
        PropKind::TreeBroadleaf
            | PropKind::TreePine
            | PropKind::TreePalm
            | PropKind::TreeSapling
            | PropKind::TreeDead
            | PropKind::BerryBush
            | PropKind::GrassClump
            | PropKind::FlowerClump
            | PropKind::Hut
            | PropKind::StorageHut
            | PropKind::Tent
    )
}

/// 페이드 반경(월드 m) — 선택 주민 중심, 같은 타일+인접 2타일까지 덮는 폭.
const FADE_R: f32 = 3.2;
/// 페이드 투명도(진입 목표) — 실루엣은 남겨 장소감을 유지.
const FADE_ALPHA: f32 = 0.28;
/// 수렴 속도(초당 지수 계수)
const FADE_SMOOTH: f32 = 10.0;
/// 연속 페이드 수명 상한(초) — 주민이 한 자리(건물 내 취침 등)에 오래 있어도
/// 세계가 영구 유령으로 남지 않게 한다. 반경 이탈로 리셋.
const FADE_HOLD_MAX: f32 = 12.0;

/// 메시 재질 핸들 — StandardMaterial과 WindMaterial 양쪽을 하나의 경로로.
enum MatHandle {
    Std(Handle<StandardMaterial>),
    Wind(Handle<WindMaterial>),
}

impl MatHandle {
    fn set_alpha(
        &self,
        std: &mut Assets<StandardMaterial>,
        wind: &mut Assets<WindMaterial>,
        a: f32,
    ) {
        match self {
            MatHandle::Std(h) => {
                if let Some(mut m) = std.get_mut(h) {
                    m.base_color.set_alpha(a);
                }
            }
            MatHandle::Wind(h) => {
                if let Some(mut m) = wind.get_mut(h) {
                    m.base.base_color.set_alpha(a);
                }
            }
        }
    }
}

/// 페이드 진행 중인 오브젝트 루트 — 원본/페이드 재질 쌍을 메시별로 보존.
#[derive(Component)]
pub struct FadeOccluder {
    entries: Vec<(Entity, MatHandle, MatHandle)>, // (메시, 원본, 페이드본)
    alpha: f32,
    target: f32,
    /// 마지막으로 재질에 기록한 알파 — 변화 없으면 에셋 수정을 유발하지
    /// 않는다(수렴 후 매 프레임 Modified → 렌더 재추출 폭탄 방지, wasm 필수).
    applied: f32,
    /// 원본 재질 재삽입 완료 — 수명 상한(capped)으로 반경 안에 계속 있을 때
    /// 복원 커맨드가 매 프레임 재실행되지 않게 1회로 제한(건물 내 취침 경로).
    restored: bool,
    /// 연속 페이드 누적 시간 — 상한 초과 시 복원(영구 유령 방지).
    held: f32,
}

/// 서브트리의 메시 재질을 복제해 페이드본으로 교체, 원본을 반환.
/// 바람 대상종은 WindApplied 이후만 처리(이후 재질 교체 경합 차단).
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 재질 2종·계층·선택·시간
fn collect_fades(
    root_e: Entity,
    children: &Query<&Children>,
    q_std: &Query<&MeshMaterial3d<StandardMaterial>>,
    q_wind: &Query<&MeshMaterial3d<WindMaterial>>,
    std_mats: &mut Assets<StandardMaterial>,
    wind_mats: &mut Assets<WindMaterial>,
    commands: &mut Commands,
) -> Vec<(Entity, MatHandle, MatHandle)> {
    let mut entries = Vec::new();
    let mut stack = vec![root_e];
    while let Some(e) = stack.pop() {
        if let Ok(m) = q_std.get(e)
            && let Some(base) = std_mats.get(&m.0).cloned()
        {
            let faded = std_mats.add(StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                ..base
            });
            commands.entity(e).insert(MeshMaterial3d(faded.clone()));
            entries.push((e, MatHandle::Std(m.0.clone()), MatHandle::Std(faded)));
        }
        if let Ok(m) = q_wind.get(e)
            && let Some(base) = wind_mats.get(&m.0).cloned()
        {
            let faded = wind_mats.add(WindMaterial {
                base: StandardMaterial {
                    alpha_mode: AlphaMode::Blend,
                    ..base.base
                },
                extension: WindExtension::default(),
            });
            commands.entity(e).insert(MeshMaterial3d(faded.clone()));
            entries.push((e, MatHandle::Wind(m.0.clone()), MatHandle::Wind(faded)));
        }
        if let Ok(ch) = children.get(e) {
            stack.extend(ch.iter());
        }
    }
    entries
}

/// 선택 주민 주변 가림물 페이드 — 진입(재질 복제)·스무딩·이탈 복원.
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 위 collect_fades와 동일 집합
pub fn fade_occluders(
    time: Res<Time>,
    sel: Res<Selection>,
    q_villager: Query<&Transform, With<Villager>>,
    q_root: Query<(Entity, &Transform, &PropRoot, Option<&WindApplied>), With<Fitted>>,
    mut q_fade: Query<(Entity, &mut FadeOccluder, &Transform)>,
    children: Query<&Children>,
    q_std: Query<&MeshMaterial3d<StandardMaterial>>,
    q_wind: Query<&MeshMaterial3d<WindMaterial>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut wind_mats: ResMut<Assets<WindMaterial>>,
    mut commands: Commands,
) {
    let focus = sel
        .entity
        .and_then(|e| q_villager.get(e).ok().map(|t| t.translation));
    let inside = |tr: &Transform| focus.is_some_and(|f| tr.translation.distance(f) < FADE_R);
    let dt = time.delta_secs().max(1e-4);

    // ── 진입 — 반경 안 새 오브젝트: 재질 복제·교체
    if focus.is_some() {
        for (root_e, tr, prop, wind_ok) in &q_root {
            if !occludes(prop.kind) || !inside(tr) {
                continue;
            }
            if is_wind_target(prop.kind) && wind_ok.is_none() {
                continue; // 바람 재질 교체 전 — 다음 프레임 재시도
            }
            if q_fade.get(root_e).is_ok() {
                continue;
            }
            let entries = collect_fades(
                root_e,
                &children,
                &q_std,
                &q_wind,
                &mut std_mats,
                &mut wind_mats,
                &mut commands,
            );
            if !entries.is_empty() {
                commands.entity(root_e).insert(FadeOccluder {
                    entries,
                    alpha: 1.0,
                    target: FADE_ALPHA,
                    applied: 1.0,
                    restored: false,
                    held: 0.0,
                });
            }
        }
    }

    // ── 갱신 — 목표 스무딩, 복귀 완료 시 원본 재질로 되돌려 정리
    let mut finished: Vec<Entity> = Vec::new();
    let mut scrap_std: Vec<Handle<StandardMaterial>> = Vec::new();
    let mut scrap_wind: Vec<Handle<WindMaterial>> = Vec::new();
    for (root_e, mut fade, tr) in &mut q_fade {
        // 연속 페이드 수명 상한 — 주민이 건물 안에 오래 머무는(취침 등) 경우도
        // 영구 유령으로 남지 않게 일정 시간 후 복원한다. 컴포넌트는 반경 이탈
        // 시까지만 유지(재질은 복원) — 상한 후 즉시 재페이드 되는 루프 방지.
        let in_r = inside(tr);
        if in_r {
            fade.held += dt;
        } else {
            fade.held = 0.0;
        }
        let capped = fade.held >= FADE_HOLD_MAX;
        fade.target = if in_r && !capped { FADE_ALPHA } else { 1.0 };
        if focus.is_none() {
            fade.alpha = 1.0; // 선택 없음 — 애니메이션 없이 즉시 복원(잔상 방지)
        }
        fade.alpha += (fade.target - fade.alpha) * (1.0 - (-(FADE_SMOOTH * dt)).exp());
        // 지수 수렴 꼬리 즉시 완결 — 복귀가 0.98 근처에서 늘어져 유령처럼 남지 않게
        if (fade.target - fade.alpha).abs() < 0.02 {
            fade.alpha = fade.target;
        }
        if fade.applied != fade.alpha {
            for (_, _, faded) in &fade.entries {
                faded.set_alpha(&mut std_mats, &mut wind_mats, fade.alpha);
            }
            fade.applied = fade.alpha;
        }
        // 복원(1회) — 목표 복귀 완료 시 원본 재질로 되돌린다. restored 가드로
        // 수명 상한(건물 내 취침 등)으로 반경 안에 계속 있어도 재삽입 커맨드가
        // 매 프레임 재발행되지 않는다.
        if fade.target >= 1.0 && !fade.restored && (fade.alpha >= 0.999 || focus.is_none()) {
            for (mesh_e, orig, _) in &fade.entries {
                match orig {
                    MatHandle::Std(h) => {
                        commands.entity(*mesh_e).insert(MeshMaterial3d(h.clone()));
                    }
                    MatHandle::Wind(h) => {
                        commands.entity(*mesh_e).insert(MeshMaterial3d(h.clone()));
                    }
                }
            }
            fade.restored = true;
        }
        // 정리 판정은 복원과 별도 — 상한 복원(restored) 후 이탈하는 경로에서도
        // 확실히 제거한다(반경 밖·선택 없음 → 컴포넌트 폐기·재진입 허용).
        if fade.restored && (!in_r || focus.is_none()) {
            for (_, _, faded) in &fade.entries {
                match faded {
                    MatHandle::Std(h) => scrap_std.push(h.clone()),
                    MatHandle::Wind(h) => scrap_wind.push(h.clone()),
                }
            }
            finished.push(root_e);
        }
    }
    for e in finished {
        commands.entity(e).remove::<FadeOccluder>();
    }
    // 세션 폐기 — 복제 페이드 재질 에셋 회수. 커맨드 큐 순서로 원본 재삽입이
    // 자산 제거보다 먼저 적용되므로(위 큐잉 또는 이전 프레임 완료) 메시가
    // 회수된 핸들을 참조하는 프레임이 없다.
    if !scrap_std.is_empty() || !scrap_wind.is_empty() {
        commands.queue(move |world: &mut World| {
            let mut std = world.resource_mut::<Assets<StandardMaterial>>();
            for h in scrap_std {
                std.remove(&h);
            }
            let mut wind = world.resource_mut::<Assets<WindMaterial>>();
            for h in scrap_wind {
                wind.remove(&h);
            }
        });
    }
}
