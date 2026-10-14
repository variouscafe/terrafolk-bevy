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
    #[uniform(2)]
    pub scroll_speed: f32,
    #[uniform(3)]
    pub wave_scale: f32,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

pub fn spawn_water(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<WaterMaterial>>) {
    let size = SIZE as f32;
    let mesh = Mesh::from(bevy::shape::Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0)));
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(WaterMaterial {
            deep: Vec4::new(0.16, 0.24, 0.38, 0.82),
            shallow: Vec4::new(0.38, 0.52, 0.62, 0.82),
            scroll_speed: 0.06,
            wave_scale: 3.2,
        })),
        Transform::from_translation(Vec3::new(size / 2.0, -0.18, size / 2.0)),
    ));
}
