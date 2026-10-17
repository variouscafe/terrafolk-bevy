//! 렌더 회귀 이분 탐색용 URL 게이트(진단 도구 — 운영 무해).
//! `?tfdbg=no_animals,no_huts,no_villagers,no_sky,no_wind,no_scaffold,no_terrain,no_water`
//! 플래그가 붙은 종류의 라이브 비주얼 스폰을 건너뛴다. 어떤 콘텐츠가 파이프라인
//! 회귀(예: WebGL2 UBO 13개 초과)를 유발하는지 URL만 바꿔 가려낸다.

use std::sync::OnceLock;

static FLAGS: OnceLock<Vec<String>> = OnceLock::new();

fn flags() -> &'static [String] {
    FLAGS.get_or_init(|| {
        #[cfg(target_arch = "wasm32")]
        {
            let Some(window) = web_sys::window() else {
                return vec![];
            };
            let Ok(search) = window.location().search() else {
                return vec![];
            };
            search
                .trim_start_matches('?')
                .split('&')
                .find_map(|kv| kv.strip_prefix("tfdbg="))
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_default()
        }
        #[cfg(not(target_arch = "wasm32"))]
        vec![]
    })
}

/// `name` 플래그가 URL에 있는가 — 예: off("no_animals").
pub fn off(name: &str) -> bool {
    flags().iter().any(|f| f == name)
}
