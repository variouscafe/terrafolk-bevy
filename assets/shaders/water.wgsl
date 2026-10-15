// 수면 — 이중 스크롤 해시노이즈·반짝임·프레넬 투명도(에셋 무함 절차).
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(127.1, 311.7));
    let r = q * (17.0 + dot(q, q * 43.0));
    return fract(r.x * r.y * 95.4337);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    let uv1 = in.uv * wave_scale + vec2<f32>(t * scroll_speed, t * scroll_speed * 0.7);
    let uv2 = in.uv * wave_scale * 1.9 - vec2<f32>(t * scroll_speed * 1.3, t * scroll_speed * 0.5);
    let n = vnoise(uv1) * 0.6 + vnoise(uv2) * 0.4;

    let view_dir = normalize(view.camera_position - in.world_position.xyz);
    let fres = pow(1.0 - clamp(view_dir.y, 0.0, 1.0), 2.0);
    let col = mix(shallow.rgb, deep.rgb, clamp(n * 0.7 + fres * 0.4, 0.0, 1.0));

    let sparkle = smoothstep(0.82, 0.93, vnoise(uv2 * 2.7 + vec2<f32>(t * 0.02)));

    return vec4<f32>(col + sparkle * 0.35, 0.78 + fres * 0.15);
}
