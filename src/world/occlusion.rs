//! 선택 주민 시야 확보 — 주변 오브젝트(나무·덤불·건물) 은닉.
//! 관전 몰입 지원: 주민을 클릭·추적하면 반경 안 가림물이 사라져 캐릭터가
//! 숲·건물에 묻히지 않는다(상용 시뮬의 occlusion 패턴).
//!
//! 구현은 Visibility 은닉 — 원래는 재질 복제 반투명 페이드였으나
//! StandardMaterial + AlphaMode::Blend가 WebGL2 프래그먼트 유니폼 한도
//! 초과로 alpha_blend 파이프라인 자체가 생성 불가라 방향을 바꿨다
//! (materials_guard 참조). 수명 상한으로 영구 은닉을 막는 계약은 동일.

use bevy::prelude::*;

use super::data::PropKind;
use super::models::{Fitted, PropRoot};
use super::pick::Selection;
use super::villager::Villager;

/// 은닉 대상 종 — 시야를 가리는 큰 식물과 건물. 모닥불·유물 등 소형은 유지.
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

/// 은닉 반경(월드 m) — 선택 주민 중심, 같은 타일+인접 2타일까지 덮는 폭.
const HIDE_R: f32 = 3.2;
/// 연속 은닉 수명 상한(초) — 주민이 한 자리(건물 내 취침 등)에 오래 있어도
/// 세계가 영구히 사라지지 않게 한다. 반경 이탈로 리셋.
const HIDE_HOLD_MAX: f32 = 12.0;

/// 은닉 진행 중인 오브젝트 루트 — 수명 추적·복원 대상.
#[derive(Component)]
pub struct Concealer {
    /// 연속 은닉 누적 시간 — 상한 초과 시 복원(영구 결손 방지).
    held: f32,
}

/// 선택 주민 주변 가림물 은닉 — 진입·수명 상한·이탈 복원.
pub fn conceal_occluders(
    time: Res<Time>,
    sel: Res<Selection>,
    q_villager: Query<&Transform, With<Villager>>,
    q_root: Query<(Entity, &Transform, &PropRoot), (With<Fitted>, Without<Concealer>)>,
    mut q_hide: Query<(Entity, &Transform, &mut Concealer, &mut Visibility)>,
    mut commands: Commands,
) {
    let focus = sel
        .entity
        .and_then(|e| q_villager.get(e).ok().map(|t| t.translation));
    let inside = |tr: &Transform| focus.is_some_and(|f| tr.translation.distance(f) < HIDE_R);
    let dt = time.delta_secs().max(1e-4);

    // ── 진입 — 반경 안 새 오브젝트 숨김(컴포넌트는 다음 프레임부터 추적)
    if focus.is_some() {
        for (root_e, tr, prop) in &q_root {
            if !occludes(prop.kind) || !inside(tr) {
                continue;
            }
            commands
                .entity(root_e)
                .insert((Concealer { held: 0.0 }, Visibility::Hidden));
        }
    }

    // ── 갱신 — 수명 상한·이탈·선택 없음 복원
    let mut finished: Vec<Entity> = Vec::new();
    for (root_e, tr, mut hide, mut vis) in &mut q_hide {
        let in_r = inside(tr);
        if in_r && focus.is_some() {
            hide.held += dt;
        } else {
            hide.held = 0.0;
        }
        let capped = hide.held >= HIDE_HOLD_MAX;
        let should_hide = in_r && focus.is_some() && !capped;
        *vis = if should_hide {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if !in_r || focus.is_none() {
            finished.push(root_e); // 반경 밖·선택 없음 — 컴포넌트 정리(재진입 허용)
        }
        // 상한 도달로 반경 안 복원 — 컴포넌트 유지(held 보존, 재은닉 차단)
    }
    for e in finished {
        commands.entity(e).remove::<Concealer>();
    }
}
