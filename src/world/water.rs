//! 수면 — 월드 전역 쿼드 + 절차 셰이더(노이즈 스크롤·프레넬·해안 포말).
//! 포말용 해안거리 텍스처를 월드 데이터에서 BFS로 베이크(R8, 56×56).

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;

use super::data::WorldData;

/// 수면 재질 — 유니폼 3개로 패킹(2026-10-17 사고 대응).
/// WebGL2 프래그먼트 유니폼 버퍼 한도(스펙 최소 12)는 뷰 그룹(조명·클러스터
/// 등 ~8개)과 재질 그룹을 합산해 계산된다. 라이브 월드에서 수면 파이프라인이
/// 13개로 초과해 생성 실패 → 수면이 통째로 안 그려지고(배경이 비치는 유백색
/// 물) 오류가 매 프레임 재시도되는 회귀가 있었다. 스칼라 파라미터는 Vec4에
/// 겹쳐 패킹해 예산을 2개 환수(13→11) — 스펙 최소 어댑터에서도 여유 1개.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub deep: Vec4,
    #[uniform(1)]
    pub shallow: Vec4,
    /// x=스크롤 속도 · y=파형 스케일 · z=월드 크기(포말 UV 정규화) — WebGL2
    /// uniform 바인딩 16바이트 정렬(4바이트 스칼라 금지)
    #[uniform(2)]
    pub params: Vec4,
    /// 해안거리(R8 정규화 — 셰이더에서 ×MAX_SHORE_DIST)
    #[texture(3)]
    #[sampler(4)]
    pub shore: Handle<Image>,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // WebGL2 방어 — Blend(alpha_blend 파이프라인)는 프래그먼트 유니폼이
        // opaque보다 1개 더 많아 한도(12) 초과. 불투명 수면 + 셰이더 프레넬·
        // 포말로 표현(바닥 투과는 포기 — materials_guard 참조).
        AlphaMode::Opaque
    }
}

/// 해안거리 정규화 스케일(타일)
pub const MAX_SHORE_DIST: f32 = 14.0;

/// 해양 평면 확장 배수 — 섬이 바다 한복판에 떠 있는 구도(지평선까지).
/// 확장 영역은 shore UV가 [0,1] 밖으로 나가므로 샘플러 ClampToEdge 필수.
const OCEAN_EXTEND: f32 = 3.0;

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
    // 해안거리 베이크 — 셀 중심 정렬(샘플 uv = world/size).
    // 샘플러 ClampToEdge — 해양 확장 영역(uv>1)이 가장자리 값(원해)으로 이어진다.
    let data = shore_distance_grid(world);
    let mut shore_img = Image::new(
        Extent3d {
            width: world.size as u32,
            height: world.size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    shore_img.sampler =
        bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
            address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
            address_mode_v: bevy::image::ImageAddressMode::ClampToEdge,
            ..default()
        });
    let shore = images.add(shore_img);
    let mesh = Mesh::from(bevy::math::primitives::Plane3d::new(
        Vec3::Y,
        Vec2::splat(size * OCEAN_EXTEND / 2.0),
    ));
    (
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(WaterMaterial {
            // 청록-하늘색 파레트(참조) — 커스텀 재질이라 값은 sRGB 감각(셰이더 그대로 출력)
            deep: Vec4::new(0.16, 0.50, 0.66, 0.80),
            shallow: Vec4::new(0.38, 0.76, 0.80, 0.80),
            // x=스크롤 속도 · y=파형 스케일 · z=월드 크기
            params: Vec4::new(0.06, 3.2, size, 0.0),
            shore,
        })),
    )
}
