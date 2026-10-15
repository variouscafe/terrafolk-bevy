// 수면 — 이중 스크롤 해시노이즈·반짝임·프레넬 투명도(에셋 무함 절차).
// Bevy 0.19 wgsl — 유니폼 명시 선언(공식 custom_material.wgsl 패턴).
#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    forward_io::VertexOutput,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> deep: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> shallow: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> scroll_speed: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> wave_scale: vec4<f32>;

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
    let uv1 = in.uv * wave_scale.x + vec2<f32>(t * scroll_speed.x, t * scroll_speed.x * 0.7);
    let uv2 = in.uv * wave_scale.x * 1.9 - vec2<f32>(t * scroll_speed.x * 1.3, t * scroll_speed.x * 0.5);
    let n = vnoise(uv1) * 0.6 + vnoise(uv2) * 0.4;

    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let fres = pow(1.0 - clamp(view_dir.y, 0.0, 1.0), 2.0);
    let col = mix(shallow.rgb, deep.rgb, clamp(n * 0.7 + fres * 0.4, 0.0, 1.0));

    let sparkle = smoothstep(0.82, 0.93, vnoise(uv2 * 2.7 + vec2<f32>(t * 0.02)));

    return vec4<f32>(col + sparkle * 0.35, 0.78 + fres * 0.15);
}
