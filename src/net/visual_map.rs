//! 라이브 자원 → 비주얼 매핑 — 서버 kind/species/stateJson을 PropKind로
//! 환산하는 순수 매핑 계층(resources.rs에서 분리, 500줄 규칙).

use crate::world::data::PropKind;

/// 자원 stateJson 최소 파싱 — 동물 {alive}·건축 {stage,type,w,h,progress}·
/// 나무 {maturity}(§17.75 다중 타일·성장 단계).
#[derive(Default, serde::Deserialize)]
pub(crate) struct ResState {
    #[serde(default)]
    pub(crate) alive: Option<bool>,
    #[serde(default)]
    pub(crate) stage: Option<String>,
    #[serde(default, rename = "type")]
    pub(crate) bld_type: Option<String>,
    #[serde(default)]
    pub(crate) w: Option<u16>,
    #[serde(default)]
    pub(crate) h: Option<u16>,
    #[serde(default)]
    pub(crate) progress: Option<f32>,
    #[serde(default)]
    pub(crate) maturity: Option<u8>,
}

/// 발판 크기 — state.w/h 우선, 레거시 스냅숏은 타입 기본값(FE BLD_SEED_SIZE).
pub(crate) fn bld_size(state: &ResState, bld_type: &str) -> (u16, u16) {
    if let (Some(w), Some(h)) = (state.w, state.h)
        && w >= 1
        && h >= 1
    {
        return (w, h);
    }
    match bld_type {
        "WINDBREAK" => (1, 1),
        "LONGHOUSE" => (3, 2),
        "WAREHOUSE" => (3, 3), // §17.81 창고
        _ => (2, 2),           // HUT·미지 타입
    }
}

/// 서버 kind → (소품 종, 목표 높이). None = 비표시(공사 중 hut은 발판 경로).
/// 동물은 사냥 사망(alive=false) 시 소멸, 나무는 성장 단계(maturity 0..3)별
/// 종·스케일(FE 4단계 성장), 완공 건물은 타입별 GLB.
pub(crate) fn live_visual(kind: &str, species: &str, state: &ResState) -> Option<(PropKind, f32)> {
    if super::dbg_gate::off(&format!("no_{kind}")) {
        return None; // 진단 게이트 — ?tfdbg=no_animals 등
    }
    match kind {
        "animal" => {
            if state.alive == Some(false) {
                return None;
            }
            let k = match species {
                "boar" => PropKind::AnimalBoar,
                "deer" => PropKind::AnimalDeer,
                "water_deer" => PropKind::AnimalWaterDeer,
                "squirrel" => PropKind::AnimalSquirrel,
                _ => PropKind::AnimalPheasant, // pheasant·기타 조류
            };
            Some((k, crate::world::models::prop_model(k).0))
        }
        "hut" => {
            // 완공만 GLB — 공사 중(PLANNED~ROOF_READY)은 bld::spawn_site 발판.
            let stage = state.stage.as_deref().unwrap_or("COMPLETED");
            if stage != "COMPLETED" {
                return None;
            }
            let bld_type = state.bld_type.as_deref().unwrap_or("HUT");
            let (k, h) = match bld_type {
                "WINDBREAK" => (
                    PropKind::Windbreak,
                    crate::world::models::prop_model(PropKind::Windbreak).0,
                ),
                "LONGHOUSE" => (PropKind::Hut, 4.2 * 1.5),
                "WAREHOUSE" => (
                    PropKind::StorageHut,
                    crate::world::models::prop_model(PropKind::StorageHut).0 * 1.35,
                ),
                _ => (PropKind::Hut, 4.2),
            };
            Some((k, h))
        }
        "tree" => {
            // 성장 단계 — 0:묘목(전용 GLB) 1:어린 나무 0.5× 2:반성장 0.78× 3:성목
            let m = state.maturity.unwrap_or(3).min(3);
            let pine = species.contains("pine");
            let k = if m == 0 {
                PropKind::TreeSapling
            } else if pine {
                PropKind::TreePine
            } else {
                PropKind::TreeBroadleaf
            };
            let (h, _) = crate::world::models::prop_model(k);
            let scale = match m {
                1 => 0.5,
                2 => 0.78,
                _ => 1.0,
            };
            Some((k, h * scale))
        }
        _ => {
            let k = live_prop(kind, species)?;
            let (h, _) = crate::world::models::prop_model(k);
            Some((k, h))
        }
    }
}

/// 서버 kind → 소품 종(정적 자원).
pub(crate) fn live_prop(kind: &str, species: &str) -> Option<PropKind> {
    match kind {
        "tree" => Some(if species.contains("pine") {
            PropKind::TreePine
        } else {
            PropKind::TreeBroadleaf
        }),
        "plant" => Some(PropKind::BerryBush),
        "gold_vein" => Some(PropKind::GoldVein),
        "campfire" => Some(PropKind::Campfire),
        "stash" => Some(PropKind::Stash),
        "grave" => Some(PropKind::Grave),
        "relic" => Some(PropKind::Relic),
        _ => None,
    }
}
