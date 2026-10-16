//! 강우·강설 상태 — 서버 날씨를 렌더(하늘 톤)에 전달.
//! 침전 파티클(비·눈 오버레이)은 WebGL2 프래그먼트 유니폼 한도(12) 때문에
//! 메시 파이프라인으로 만들 수 없어 DOM/CSS 오버레이로 이동했다
//! (index.html .tf-precip + tf-ui.js가 날씨로 구동 — materials_guard 참조).

use bevy::prelude::*;

/// 서버 날씨 상태(net::apply가 갱신 — 데모는 clear 고정)
#[derive(Resource)]
pub struct WeatherState {
    pub kind: String,
}

impl Default for WeatherState {
    fn default() -> Self {
        WeatherState {
            kind: "clear".to_string(),
        }
    }
}

impl WeatherState {
    /// (조도 감쇠, 하늘 탈포화, 한랭 기운) — apply_sky가 소비
    pub fn sky_mix(&self) -> (f32, f32, f32) {
        match self.kind.as_str() {
            "overcast" => (0.78, 0.5, 0.0),
            "rain" => (0.64, 0.6, 0.0),
            "storm" => (0.46, 0.72, 0.0),
            "snow" => (0.9, 0.68, 0.35),
            "fog" => (0.86, 0.62, 0.0),
            _ => (1.0, 0.0, 0.0),
        }
    }
}
