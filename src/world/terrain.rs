//! 지형 메시 — 버텍스 스플랫 + 세계좌표 UV(타일 경계 없는 페인티드 블렌딩).
//! 상판: 코너 정합 가중치(인접 4타일 평균+디더). 절벽: 단위단계 테라스.
//! 가중치 패킹 — COLOR=(grass,forest,sand), UV_1=(rock,dirt).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use super::data::{STEP, WATER_BED_Y, WorldData};
use super::splat::TerrainMaterial;

const TILE: f32 = 1.0;
/// 텍스처 반복률 — 1장 ≈ 2.9타일(512px ⇒ 타일당 ~176px).
const UV_SCALE: f32 = 0.34;

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

fn push_quad(
    m: &mut MeshData,
    quad: [[f32; 3]; 4],
    n: [f32; 3],
    uvs: [[f32; 2]; 4],
    ws: [[f32; 5]; 4],
) {
    let base = m.positions.len() as u32;
    for i in 0..4 {
        push_vertex(m, quad[i], n, uvs[i], ws[i]);
    }
    // 와인딩은 cull_mode None으로 방어(정적 지형)
    m.indices
        .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
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

fn build_terrain(
    world: &WorldData,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TerrainMaterial>,
) -> (Handle<Mesh>, Handle<TerrainMaterial>) {
    let mut m = MeshData::default();

    let size = world.size;
    for y in 0..size {
        for x in 0..size {
            let (xi, yi) = (x as i32, y as i32);
            let t = world.terrain_at(xi, yi);
            let is_water = t == "water";
            let h = world.height_at(xi, yi);
            let (x0, z0) = (x as f32 * TILE, y as f32 * TILE);
            let (x1, z1) = (x0 + TILE, z0 + TILE);

            // 상판 — 코너 정합 가중치·세계좌표 UV
            let corners = [
                (x0, h, z0, xi, yi),
                (x1, h, z0, xi + 1, yi),
                (x1, h, z1, xi + 1, yi + 1),
                (x0, h, z1, xi, yi + 1),
            ];
            let quad: [[f32; 3]; 4] = [
                [corners[0].0, corners[0].1, corners[0].2],
                [corners[1].0, corners[1].1, corners[1].2],
                [corners[2].0, corners[2].1, corners[2].2],
                [corners[3].0, corners[3].1, corners[3].2],
            ];
            let uvs: [[f32; 2]; 4] = [
                [corners[0].0 * UV_SCALE, corners[0].2 * UV_SCALE],
                [corners[1].0 * UV_SCALE, corners[1].2 * UV_SCALE],
                [corners[2].0 * UV_SCALE, corners[2].2 * UV_SCALE],
                [corners[3].0 * UV_SCALE, corners[3].2 * UV_SCALE],
            ];
            let ws: [[f32; 5]; 4] = [
                corner_weights(world, corners[0].3, corners[0].4),
                corner_weights(world, corners[1].3, corners[1].4),
                corner_weights(world, corners[2].3, corners[2].4),
                corner_weights(world, corners[3].3, corners[3].4),
            ];
            push_quad(&mut m, quad, [0.0, 1.0, 0.0], uvs, ws);

            if is_water {
                continue;
            }
            // 절벽 — 이웃보다 높은 만큼 단위단계 적층. 레이어: rock타일=rock, 나머지=dirt
            for dir in [[1i32, 0i32], [-1, 0], [0, 1], [0, -1]] {
                let (nx, ny) = (xi + dir[0], yi + dir[1]);
                let nh = if world.terrain_at(nx, ny) == "water" {
                    WATER_BED_Y
                } else {
                    world.height_at(nx, ny)
                };
                let diff_steps = ((h - nh) / STEP).round() as i32;
                if diff_steps <= 0 {
                    continue;
                }
                let wl: [f32; 5] = if t == "rock" {
                    [0.0, 0.0, 0.0, 1.0, 0.0]
                } else {
                    [0.0, 0.0, 0.0, 0.25, 0.75] // 흙 절벽에 약간 암석 혼합
                };
                let outward = [dir[0] as f32, 0.0, dir[1] as f32];
                for s in 0..diff_steps {
                    let top = h - s as f32 * STEP;
                    let bot = top - STEP;
                    let (q, u_axis): ([[f32; 3]; 4], f32) = match dir {
                        [1, 0] => (
                            [[x1, top, z0], [x1, top, z1], [x1, bot, z1], [x1, bot, z0]],
                            0.0,
                        ),
                        [-1, 0] => (
                            [[x0, top, z1], [x0, top, z0], [x0, bot, z0], [x0, bot, z1]],
                            0.0,
                        ),
                        [0, 1] => (
                            [[x0, top, z1], [x1, top, z1], [x1, bot, z1], [x0, bot, z1]],
                            1.0,
                        ),
                        _ => (
                            [[x1, top, z0], [x0, top, z0], [x0, bot, z0], [x1, bot, z0]],
                            1.0,
                        ),
                    };
                    // 수직면 UV — 수평축(자기축) + 높이
                    let uvs: [[f32; 2]; 4] = [
                        [
                            if u_axis == 0.0 { q[0][2] } else { q[0][0] } * UV_SCALE,
                            q[0][1] * UV_SCALE * 0.8,
                        ],
                        [
                            if u_axis == 0.0 { q[1][2] } else { q[1][0] } * UV_SCALE,
                            q[1][1] * UV_SCALE * 0.8,
                        ],
                        [
                            if u_axis == 0.0 { q[2][2] } else { q[2][0] } * UV_SCALE,
                            q[2][1] * UV_SCALE * 0.8,
                        ],
                        [
                            if u_axis == 0.0 { q[3][2] } else { q[3][0] } * UV_SCALE,
                            q[3][1] * UV_SCALE * 0.8,
                        ],
                    ];
                    let ws = [wl; 4];
                    push_quad(&mut m, q, outward, uvs, ws);
                }
            }
        }
    }

    let verts = m.positions.len();
    let _ = verts;
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
