// 수면 — 이중 스크롤 해시노이즈·반짝임·프레넬 투명도 + 해안 포말(거리장 베이크).
// Bevy 0.19 wgsl — 유니폼 명시 선언(공식 custom_material.wgsl 패턴).
#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    forward_io::VertexOutput,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> deep: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> shallow: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> scroll_speed: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> wave_scale: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var t_shore: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var s_shore: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> world_size: vec4<f32>;

const MAX_SHORE: f32 = 14.0;

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q *= (17.0 + dot(q, q * 43.0));
    return fract(q.x * q.y * 95.4337);
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
    let wp = in.world_position.xz;
    let uv1 = in.uv * wave_scale.x + vec2<f32>(t * scroll_speed.x, t * scroll_speed.x * 0.7);
    let uv2 = in.uv * wave_scale.x * 1.9 - vec2<f32>(t * scroll_speed.x * 1.3, t * scroll_speed.x * 0.5);
    let n = vnoise(uv1) * 0.6 + vnoise(uv2) * 0.4;

    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let fres = pow(1.0 - clamp(view_dir.y, 0.0, 1.0), 2.0);
    var col = mix(shallow.rgb, deep.rgb, clamp(n * 0.7 + fres * 0.4, 0.0, 1.0));

    // ── 해안 포말 — 베이크 거리장 + 노이즈 요동으로 유기적 물결선
    let sd = textureSample(t_shore, s_shore, wp / world_size.x).r * MAX_SHORE;
    let wob = (vnoise(wp * 1.1 + vec2<f32>(t * 0.22, -t * 0.17)) - 0.5) * 1.1;
    let band = 0.75 + wob;
    let foam_edge = 1.0 - smoothstep(band - 0.35, band + 0.55, sd);
    // 포말 텍스처 — 미세 거품 얼룩
    let foam_tex = vnoise(wp * 3.0 + vec2<f32>(t * 0.35, t * 0.2)) * 0.7
                 + vnoise(wp * 6.5 - vec2<f32>(t * 0.5)) * 0.3;
    let foam = foam_edge * smoothstep(0.35, 0.75, foam_tex + foam_edge * 0.35);
    col = mix(col, vec3<f32>(0.94, 0.97, 0.98), clamp(foam, 0.0, 1.0) * 0.92);

    // 얕은수 — 해안 근처 투명·밝게(수심 환상)
    let shallow_w = 1.0 - smoothstep(0.0, 3.2, sd);
    col = mix(col, shallow.rgb * 1.12, shallow_w * 0.35);

    let sparkle = smoothstep(0.82, 0.93, vnoise(uv2 * 2.7 + vec2<f32>(t * 0.02)));
    col += sparkle * 0.3 * (1.0 - foam_edge * 0.6);

    var alpha = 0.80 + fres * 0.12;
    alpha = mix(alpha, 0.62, shallow_w);          // 얕은수 투명
    alpha = max(alpha, foam_edge * 0.95);          // 포말선 불투명
    return vec4<f32>(col, clamp(alpha, 0.0, 1.0));
}
