//! Terrafolk Bevy 웹 클라이언트 — Phase 1(2.5D 아이소 렌더 프로토타입).
//! 데이터 계약: ../terrafolk/docs/data-contract.md(§17.82). 월드 데이터는 데모
//! 결정론 생성(시드 순수 함수) — Phase 2에서 Colyseus 어댑터로 교체.

pub mod world;

use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(world::WorldPlugin);
    }
}
