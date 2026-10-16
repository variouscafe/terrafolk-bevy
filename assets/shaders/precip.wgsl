// 침전 오버레이 — 화면공간 비 스트릭/눈 플레이크(비조명 알파).
// 카메라 자식 쿼드: uv가 화면 좌표. params=(speed, density, slant, fade).
// 모드는 speed로 판별(비 1.35·눈 0.16) — 유니폼 1개 한도(WebGL2 12개).
#import bevy_pbr::{
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q *= (17.0 + dot(q, q * 43.0));
    return fract(q.x * q.y * 95.4337);
}

// 비 — 열(cell)마다 낙하 스트릭: 해시 출발위치·속도차로 층이 갈라진다.
fn streaks(uv: vec2<f32>, t: f32, cols: f32, speed: f32, cover: f32, len: f32, seed: f32) -> f32 {
    let p = vec2<f32>(uv.x + uv.y * params.z, uv.y);
    let cx = floor(p.x * cols);
    let h1 = hash(vec2<f32>(cx, seed));
    let h2 = hash(vec2<f32>(cx, seed + 13.7));
    if h1 > cover {
        return 0.0;
    }
    let xo = (h2 - 0.5) * 0.6;
    let fx = abs(fract(p.x * cols) - 0.5 - xo);
    let w = 0.10 + h1 * 0.10;
    let ph = fract(t * speed * (0.7 + h2 * 0.6) + h1);
    let fr = fract(p.y - ph);
    let d = min(fr, 1.0 - fr); // 스트릭 중심(랩 포인트)까지 거리
    let m = (1.0 - smoothstep(0.0, w, fx)) * (1.0 - smoothstep(0.0, len, d));
    return m * (0.5 + h1 * 0.6);
}

// 눈 — 스크롤 그리드 셀마다 플레이크, 사인 횡동·크기 변주.
fn flakes(uv: vec2<f32>, t: f32, cols: f32, speed: f32, cover: f32, seed: f32) -> f32 {
    var p = uv * cols;
    p.y -= t * speed;
    let id = floor(p);
    let f = fract(p);
    let h1 = hash(id + vec2<f32>(seed, seed * 1.7));
    let h2 = hash(id + vec2<f32>(seed * 2.3, seed * 0.6));
    let h3 = hash(id + vec2<f32>(seed * 0.9, seed * 3.1));
    if h1 > cover {
        return 0.0;
    }
    let pos = vec2<f32>(h2 + sin(t * 0.9 + h1 * 6.283) * 0.22, h3);
    let d = distance(f, pos);
    let r = 0.10 + h2 * 0.09;
    return (1.0 - smoothstep(r * 0.35, r, d)) * (0.6 + h1 * 0.5);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    let uv = in.uv;
    // 화면 가장자리 소프트 페이드(쿼드 경계 은닉)
    let edge = (1.0 - smoothstep(0.32, 0.5, max(abs(uv.x - 0.5), abs(uv.y - 0.5)))) * 0.4 + 0.6;

    var col = vec3<f32>(0.78, 0.84, 0.95);
    var m = 0.0;
    if params.x < 0.7 {
        // 비 — 원근 2레이어(뒤 느리게·옅게)
        m = streaks(uv, t, 34.0 * params.y, params.x, 0.55, 0.30, 3.0) * 0.4
          + streaks(uv, t, 62.0 * params.y, params.x * 1.7, 0.5, 0.18, 17.0) * 0.75;
        col = vec3<f32>(0.72, 0.80, 0.94);
    } else {
        // 눈 — 느린 2층
        m = flakes(uv, t, 22.0 * params.y, params.x, 0.5, 5.0) * 0.5
          + flakes(uv, t, 38.0 * params.y, params.x * 1.5, 0.45, 11.0) * 0.85;
        col = vec3<f32>(0.97, 0.98, 1.0);
    }

    let alpha = clamp(m * edge * params.w, 0.0, 0.85);
    return vec4<f32>(col, alpha);
}
