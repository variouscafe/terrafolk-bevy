//! WebGL2 재질 가드 — StandardMaterial의 AlphaMode::Blend 금지.
//!
//! 이 스택(bevy 0.19 PBR)에서 alpha_blend 메시 파이프라인은 프래그먼트
//! 유니폼 버퍼 13개를 요구하는데 WebGL2 명세 한도가 12개라 생성 즉시
//! 검증 실패 → bevy 오류 핸들러가 앱을 종료해 버린다(관전 화면이 로드
//! 몇 초 뒤 얼어붙고 주민이 사라지는 현상의 실체). 잎·꽃 텍스처는
//! 0/1 알파라 컷아웃(Mask)과 위상이 같으므로 전 표준 재질을 강제 전환해
//! alpha_blend 파이프라인이 아예 생성되지 않게 한다. 커스텀 재질(물·침전)은
//! 가벼운 자체 레이아웃이라 한도 안 — Blend 유지.

use bevy::prelude::*;

/// 매 프레임 전 재질 스캔(멱등·저비용) — GLB 재질이 비동기 삽입되는 어떤
/// 시점에도 추출(파이프라인 특수화) 전에 한 프레임 안에 가드된다.
pub fn guard_blend_materials(
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut wind_mats: ResMut<Assets<crate::world::wind::WindMaterial>>,
) {
    for (_, m) in mats.iter_mut() {
        if m.alpha_mode == AlphaMode::Blend {
            m.alpha_mode = AlphaMode::Mask(0.5);
        }
    }
    // 바람 전환 재질도 동일 가드 — 원본이 Blend면 복제본도 Blend로 흘러들어
    // 확장 파이프라인(alpha_blend)을 요구할 수 있다.
    for (_, m) in wind_mats.iter_mut() {
        if m.base.alpha_mode == AlphaMode::Blend {
            m.base.alpha_mode = AlphaMode::Mask(0.5);
        }
    }
}

/// 렌더 검증 오류를 무시하고 계속 — BLEND 재질이 가드 스캔과 추출 사이에
/// 삽입되는 경합(에셋 비동기 로드)에서 앱이 종료되지 않게 한다. 실패한
/// alpha_blend 파이프라인 키는 다시 요청되지 않고(가드가 다음 스캔에서
/// Mask로 전환) alpha_mask 파이프라인으로 자가 치유된다.
pub fn nonfatal_render_errors(app: &mut App) {
    app.insert_resource(
        bevy::render::error_handler::RenderErrorHandler(
            |_err, _main, _render| bevy::render::error_handler::RenderErrorPolicy::Ignore,
        ),
    );
}
