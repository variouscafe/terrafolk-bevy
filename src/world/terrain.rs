//! 지형 메시 — 그리드 → 단일 Mesh. 타일 상판(아틀라스 UV) + 단위단계 절벽 측벽.
//! 노멀은 면별 명시(상판 +Y·측벽 외향). 아틀라스 셀 좌표는 terrain_atlas.json 정본.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use super::data::{WorldData, SIZE, STEP, WATER_BED_Y};

const TILE: f32 = 1.0;
const ATLAS_W: f32 = 256.0;
const ATLAS_H: f32 = 128.0;
const CELL: f32 = 64.0;

/// 아틀라스 셀(x,y 픽셀) — gen_terrain.py 산출물과 동일
fn cell_rect(kind: &str) -> (f32, f32) {
    match kind {
        "grass" => (0.0, 0.0),
        "forest" => (64.0, 0.0),
        "sand" => (128.0, 0.0),
        "rock" => (192.0, 0.0),
        "dirt" => (0.0, 64.0),
        "cliff_rock" => (64.0, 64.0),
        "cliff_dirt" => (128.0, 64.0),
        _ => (192.0, 64.0), // water_bed
    }
}

fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
    n: [f32; 3],
    cx: f32,
    cy: f32,
) {
    let base = positions.len() as u32;
    let u0 = cx / ATLAS_W;
    let v0 = cy / ATLAS_H;
    let u1 = (cx + CELL) / ATLAS_W;
    let v1 = (cy + CELL) / ATLAS_H;
    for (p, uv) in [
        (a, [u0, v0]),
        (b, [u1, v0]),
        (c, [u1, v1]),
        (d, [u0, v1]),
    ] {
        positions.push(p);
        normals.push(n);
        uvs.push(uv);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
}

pub fn spawn_terrain(mut commands: Commands, world: Res<WorldData>, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut positions: Vec<[f32; 3]> = vec![];
    let mut normals: Vec<[f32; 3]> = vec![];
    let mut uvs: Vec<[f32; 2]> = vec![];
    let mut indices: Vec<u32> = vec![];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let (xi, yi) = (x as i32, y as i32);
            let t = world.terrain_at(xi, yi);
            let is_water = t == "water";
            let (cx, cy) = cell_rect(if is_water { "water_bed" } else { t });
            let h = world.height_at(xi, yi);
            let x0 = x as f32 * TILE;
            let x1 = x0 + TILE;
            let z0 = y as f32 * TILE;
            let z1 = z0 + TILE;
            push_quad(
                &mut positions, &mut normals, &mut uvs, &mut indices,
                [x0, h, z0], [x1, h, z0], [x1, h, z1], [x0, h, z1],
                [0.0, 1.0, 0.0], cx, cy,
            );
            if is_water {
                continue;
            }
            // 절벽 — 이웃보다 높은 만큼 단위단계 quad 적층(셀 v 0..1 반복)
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
                let (ccx, ccy) = cell_rect(if t == "rock" { "cliff_rock" } else { "cliff_dirt" });
                let outward = [dir[0] as f32, 0.0, dir[1] as f32];
                for s in 0..diff_steps {
                    let top = h - s as f32 * STEP;
                    let bot = top - STEP;
                    let q = match dir {
                        [1, 0] => (
                            [x1, top, z0], [x1, top, z1], [x1, bot, z1], [x1, bot, z0],
                        ),
                        [-1, 0] => (
                            [x0, top, z1], [x0, top, z0], [x0, bot, z0], [x0, bot, z1],
                        ),
                        [0, 1] => (
                            [x0, top, z1], [x1, top, z1], [x1, bot, z1], [x0, bot, z1],
                        ),
                        _ => (
                            [x1, top, z0], [x0, top, z0], [x0, bot, z0], [x1, bot, z0],
                        ),
                    };
                    push_quad(
                        &mut positions, &mut normals, &mut uvs, &mut indices,
                        q.0, q.1, q.2, q.3, outward, ccx, ccy,
                    );
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    let texture: Handle<Image> = assets.load("tiles/terrain_atlas.png");
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(texture),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            cull_mode: None, // 와인딩 방어 — 정적 지형
            ..default()
        })),
    ));
}
