//! 수면 — 월드 전역 쿼드 + 절차 셰이더(노이즈 스크롤·프레넬·해안 포말).
//! 포말용 해안거리 텍스처를 월드 데이터에서 BFS로 베이크(R8, 56×56).

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use super::data::WorldData;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub deep: Vec4,
    #[uniform(1)]
    pub shallow: Vec4,
    /// x=스크롤 속도 — WebGL2 uniform 바인딩 16바이트 정렬(4바이트 스칼라 금지)
    #[uniform(2)]
    pub scroll_speed: Vec4,
    /// x=파형 스케일
    #[uniform(3)]
    pub wave_scale: Vec4,
    /// 해안거리(R8 정규화 — 셰이더에서 ×MAX_SHORE_DIST)
    #[texture(4)]
    #[sampler(5)]
    pub shore: Handle<Image>,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// 해안거리 정규화 스케일(타일)
pub const MAX_SHORE_DIST: f32 = 14.0;

/// 다중 출발점 BFS — 각 물 셀에서 가장 가까운 육지까지의 타일 거리.
fn shore_distance_grid(world: &WorldData) -> Vec<u8> {
    use std::collections::VecDeque;
    let n = world.size;
    let mut dist = vec![i32::MAX; n * n];
    let mut q: VecDeque<(usize, usize)> = VecDeque::new();
    for y in 0..n {
        for x in 0..n {
            if world.terrain_at(x as i32, y as i32) != "water" {
                dist[y * n + x] = 0;
                q.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = dist[y * n + x];
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if dist[ny * n + nx] > d + 1 {
                dist[ny * n + nx] = d + 1;
                q.push_back((nx, ny));
            }
        }
    }
    dist.iter()
        .map(|&d| ((d.min(i32::MAX - 1) as f32 / MAX_SHORE_DIST).min(1.0) * 255.0) as u8)
        .collect()
}

/// 수면 메시 태그 — 라이브 전환 시 재스폰용.
#[derive(Component)]
pub struct WaterMesh;

pub fn spawn_water(
    mut commands: Commands,
    world: Res<WorldData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let handle = build_water(&world, &mut meshes, &mut materials, &mut images);
    let size = world.size as f32;
    commands.spawn((
        WaterMesh,
        handle,
        Transform::from_translation(Vec3::new(size / 2.0, -0.18, size / 2.0)),
    ));
}

/// 라이브 수면(전환용) — 동일 빌더·마커.
pub fn spawn_live_water(
    commands: &mut Commands,
    world: &WorldData,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    images: &mut Assets<Image>,
) {
    let handle = build_water(world, meshes, materials, images);
    let size = world.size as f32;
    commands.spawn((
        WaterMesh,
        handle,
        Transform::from_translation(Vec3::new(size / 2.0, -0.18, size / 2.0)),
    ));
}

/// 수면 스폰 헬퍼 — 메시+재질 번들.
fn build_water(
    world: &WorldData,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    images: &mut Assets<Image>,
) -> (Mesh3d, MeshMaterial3d<WaterMaterial>) {
    let size = world.size as f32;
    // 해안거리 베이크 — 셀 중심 정렬(샘플 uv = world/size)
    let data = shore_distance_grid(world);
    let shore = images.add(Image::new(
        Extent3d {
            width: world.size as u32,
            height: world.size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    ));
    let mesh = Mesh::from(bevy::math::primitives::Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0)));
    (
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(WaterMaterial {
            deep: Vec4::new(0.21, 0.35, 0.55, 0.82),
            shallow: Vec4::new(0.42, 0.62, 0.75, 0.82),
            scroll_speed: Vec4::new(0.06, 0.0, 0.0, 0.0),
            wave_scale: Vec4::new(3.2, 0.0, 0.0, 0.0),
            shore,
        })),
    )
}
