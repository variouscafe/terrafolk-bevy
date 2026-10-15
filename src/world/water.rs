//! 수면 — 월드 전역 쿼드 + 절차 셰이더(해시 노이즈 스크롤·반짝임·프레넬).
//! 에셋 무함 — 노이즈는 셰이더 내 해시. 물바닥은 지형 메시(water_bed 셀)가 담당.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use super::data::SIZE;

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
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub fn spawn_water(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<WaterMaterial>>) {
    let size = SIZE as f32;
    let mesh = Mesh::from(bevy::math::primitives::Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0)));
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(WaterMaterial {
            deep: Vec4::new(0.21, 0.35, 0.55, 0.82),
            shallow: Vec4::new(0.42, 0.62, 0.75, 0.82),
            scroll_speed: Vec4::new(0.06, 0.0, 0.0, 0.0),
            wave_scale: Vec4::new(3.2, 0.0, 0.0, 0.0),
        })),
        Transform::from_translation(Vec3::new(size / 2.0, -0.18, size / 2.0)),
    ));
}
