// 바람 — 식물 정점 흔들림(WindMaterial 확장 버텍스).
// 표준 mesh.wgsl 버텍트 단계 복제(모프/스킨 제외 — 정적 소품 전용) 후
// 월드 공간 xz 변위 추가. 진폭 = params.x × (로컬높이/6)^params.z,
// 위상 = 월드좌표 + 시간. 프래그먼트는 StandardMaterial 기본 그대로.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}

// 통일 바람 곡선 — 진폭 = 기저 + 높이 선형(잔디~교목 자연 계층)
const WIND_BASE: f32 = 0.045;
const WIND_PER_H: f32 = 0.026;
const WIND_SPD: f32 = 1.35;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = vertex_no_morph;

    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index,
    );
#endif

#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );

    // ── 바람 변위 — 높이 증폭·월드 위상(개체별 자연 위상차)
    let wp = out.world_position.xyz;
    let h = clamp(vertex.position.y, 0.0, 12.0);
    let amp = WIND_BASE + h * WIND_PER_H;
    // 느린 돌풍 포락 — 숲이 한 덩어리로 흔들리지 않게
    let gust = 0.65 + 0.35 * sin(globals.time * 0.31 + wp.x * 0.02 + wp.z * 0.017);
    let phase = wp.x * 0.35 + wp.z * 0.28 + globals.time * WIND_SPD;
    let sway = sin(phase) * 0.62 + sin(phase * 2.17 + 1.7) * 0.38;
    out.world_position = vec4<f32>(
        wp + vec3<f32>(sway * amp * gust, 0.0, sway * amp * gust * 0.55),
        out.world_position.w,
    );

    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_no_morph.instance_index,
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_no_morph.instance_index;
#endif

    return out;
}
