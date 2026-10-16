//! 지형 메시 — 스무스 하프타일 고도장 + 버텍스 스플랫(세계좌표 UV, 시임 없는 블렌딩).
//! 코너 고도 = 인접 4타일 평그(WorldData::corner_height — 착지·보행과 동일 장),
//! 정점은 코너장 smoothstep 이중보간으로 라운딩(구릉). 절벽은 벽면 메시가 아니라
//! rock 고지의 경사+셰이더 경사 블렌드로 만든다(참조: 조각된 자연 절벽).
//! 가중치 패킹 — COLOR=(grass,forest,sand), UV_1=(rock,dirt).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use super::data::{WATER_BED_Y, WorldData};
use super::splat::TerrainMaterial;

const TILE: f32 = 1.0;
/// 텍스처 반복률 — 1장 ≈ 2.9타일(512px ⇒ 타일당 ~176px).
const UV_SCALE: f32 = 0.34;
/// 하프타일 격자 간격(타일)
const SUBDIV: f32 = 0.5;

/// GID 이름 → 스플랫 레이어 인덱스
fn layer_of(t: &str) -> Option<usize> {
    Some(match t {
        "grass" => 0,
        "forest" => 1,
        "sand" => 2,
        "rock" => 3,
        "dirt" => 4,
        _ => return None,
    })
}

fn corner_hash(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ salt;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFFFF) as f32 / 0xFFFFFF as f32
}

/// 격자 정점 (cx,cy)의 스플랫 가중치 — 인접 4타일 지시 평균 + 채널별 디더.
/// 같은 정점을 공유하는 타일들이 동일 입력을 받아 경계가 시임 없이 블렌딩된다.
fn corner_weights(world: &WorldData, cx: i32, cy: i32) -> [f32; 5] {
    let mut w = [0.0f32; 5];
    for (dx, dy) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        let t = world.terrain_at(cx + dx, cy + dy);
        if t == "water" {
            // 물밑 — 모래+흙 반반(수중 하층)
            w[2] += 0.55;
            w[4] += 0.45;
        } else if let Some(l) = layer_of(t) {
            w[l] += 1.0;
        }
    }
    // 채널별 위상 디더 — 밴딩 분해(유기적 경계)
    for (i, w_i) in w.iter_mut().enumerate() {
        let d = corner_hash(cx, cy, 0x51 + i as u32 * 97);
        *w_i *= 0.74 + 0.52 * d;
    }
    let sum: f32 = w.iter().sum::<f32>() + 1e-5;
    for w_i in w.iter_mut() {
        *w_i /= sum;
    }
    w
}

#[derive(Default)]
struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    weights2: Vec<[f32; 2]>, // UV_1: (rock, dirt)
    colors: Vec<[f32; 4]>,   // COLOR: (grass, forest, sand, 1)
    indices: Vec<u32>,
}

fn push_vertex(m: &mut MeshData, p: [f32; 3], n: [f32; 3], uv: [f32; 2], w: [f32; 5]) {
    m.positions.push(p);
    m.normals.push(n);
    m.uvs.push(uv);
    m.colors.push([w[0], w[1], w[2], 1.0]);
    m.weights2.push([w[3], w[4]]);
}

/// 지형 메시 태그 — 라이브 전환 시 일괄 제거용.
#[derive(Component)]
pub struct TerrainMesh;

pub fn spawn_terrain(
    mut commands: Commands,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let (mesh_h, mat_h) = build_terrain(&world, &assets, &mut meshes, &mut materials);
    commands.spawn((TerrainMesh, Mesh3d(mesh_h), MeshMaterial3d(mat_h)));
}

/// 라이브 지형 스폰(프라임 전환용) — 데모와 동일 빌더·마커.
pub fn spawn_live_terrain(
    commands: &mut Commands,
    world: &WorldData,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TerrainMaterial>,
) {
    let (mesh_h, mat_h) = build_terrain(world, assets, meshes, materials);
    commands.spawn((TerrainMesh, Mesh3d(mesh_h), MeshMaterial3d(mat_h)));
}

/// 하프타일 격자의 표면 고도 — WorldData 표면장에서 직접(메시·착지 정합).
fn grid_height(world: &WorldData, fx: f32, fy: f32) -> f32 {
    world.surface_height(fx, fy)
}

/// 정점 노멀 — 표면장 중앙차분(격자 경계는 편측).
fn grid_normal(world: &WorldData, fx: f32, fy: f32, n: f32) -> [f32; 3] {
    let e = SUBDIV;
    let hx = grid_height(world, (fx + e).min(n), fy) - grid_height(world, (fx - e).max(0.0), fy);
    let hz = grid_height(world, fx, (fy + e).min(n)) - grid_height(world, fx, (fy - e).max(0.0));
    let span = 2.0 * e;
    let v = Vec3::new(-hx / span, 1.0, -hz / span).normalize_or_zero();
    [v.x, v.y, v.z]
}

fn build_terrain(
    world: &WorldData,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TerrainMaterial>,
) -> (Handle<Mesh>, Handle<TerrainMaterial>) {
    let mut m = MeshData::default();
    let n = world.size;

    // ── 코너 가중치장 (n+1)² — 1회 계산 후 정점에서 이중보간
    let stride_c = n + 1;
    let mut cw = vec![[0.0f32; 5]; stride_c * stride_c];
    for cy in 0..=n {
        for cx in 0..=n {
            cw[cy * stride_c + cx] = corner_weights(world, cx as i32, cy as i32);
        }
    }

    // ── 하프타일 쿼드 방출 — 정점 (2i/2, 2j/2) 격자
    let emit = |m: &mut MeshData, i: usize, j: usize| {
        let base = m.positions.len() as u32;
        for (di, dj) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            let (vi, vj) = (i + di, j + dj);
            let (fx, fy) = (vi as f32 * SUBDIV, vj as f32 * SUBDIV);
            let h = grid_height(world, fx, fy);
            let nrm = grid_normal(world, fx, fy, n as f32);
            // 코너 가중치 smoothstep 이중보간(고도와 동일 파라미터).
            // 경계 클램프 — fx=n에서 마지막 타일 보간분 1.0로.
            let (tx, ty) = (
                (fx.floor() as usize).min(n - 1),
                (fy.floor() as usize).min(n - 1),
            );
            let (rx, ry) = (fx - tx as f32, fy - ty as f32);
            let u = rx * rx * (3.0 - 2.0 * rx);
            let v = ry * ry * (3.0 - 2.0 * ry);
            let w00 = cw[ty * stride_c + tx];
            let w10 = cw[ty * stride_c + tx + 1];
            let w01 = cw[(ty + 1) * stride_c + tx];
            let w11 = cw[(ty + 1) * stride_c + tx + 1];
            let mut w = [0.0f32; 5];
            for k in 0..5 {
                let top = w00[k] + (w10[k] - w00[k]) * u;
                let bot = w01[k] + (w11[k] - w01[k]) * u;
                w[k] = top + (bot - top) * v;
            }
            push_vertex(
                m,
                [fx * TILE, h, fy * TILE],
                nrm,
                [fx * UV_SCALE, fy * UV_SCALE],
                w,
            );
        }
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    };

    let quads_per_side = ((n as f32) / SUBDIV) as usize; // 2n
    for j in 0..quads_per_side {
        for i in 0..quads_per_side {
            emit(&mut m, i, j);
        }
    }

    // ── 해양 밑판 — 맵 밖 바다의 얕은 바닥(물 평면 3× 확장 대응).
    //    순수 모래 가중치, 베드보다 살짝 아래(맵 내 지형과 z-fight 없음).
    {
        let ext = n as f32; // 맵 크기만큼 사방 확장
        let y = WATER_BED_Y - 0.08;
        let corners: [[f32; 3]; 4] = [
            [-ext, y, -ext],
            [2.0 * n as f32 + ext, y, -ext],
            [2.0 * n as f32 + ext, y, 2.0 * n as f32 + ext],
            [-ext, y, 2.0 * n as f32 + ext],
        ];
        let base = m.positions.len() as u32;
        let w_sand = [0.0f32, 0.0, 1.0, 0.0, 0.0];
        for (idx, p) in corners.iter().enumerate() {
            push_vertex(
                &mut m,
                *p,
                [0.0, 1.0, 0.0],
                [p[0] * UV_SCALE, p[2] * UV_SCALE],
                w_sand,
            );
            let _ = idx;
        }
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    #[allow(unused_variables)] // wasm 콘솔 진단 로그 전용
    let verts = m.positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, m.weights2);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors);
    mesh.insert_indices(Indices::U32(m.indices));

    let ext = super::splat::TerrainExtension {
        grass: assets.load("textures/ground/grass.png"),
        forest: assets.load("textures/ground/forest.png"),
        sand: assets.load("textures/ground/sand.png"),
        rock: assets.load("textures/ground/rock.png"),
        dirt: assets.load("textures/ground/dirt.png"),
    };
    let mat = TerrainMaterial {
        base: StandardMaterial {
            perceptual_roughness: 1.0,
            metallic: 0.0,
            cull_mode: None, // 와인딩 방어 — 정적 지형
            ..default()
        },
        extension: ext,
    };
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("TERRAIN SPAWNED verts={}", verts).into());
    (meshes.add(mesh), materials.add(mat))
}
