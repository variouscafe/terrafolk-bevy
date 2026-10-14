//! 빌보드 — 고정 절대각 카메라를 향한 스태틱 쿼드(회전 불변 → 매 프레임 정렬 불필요).
//! pixel-art 규율: nearest 샘플링·알파 마스크(0/255)·이중면(플립 안전).

use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use super::data::{PropKind, WorldData, SIZE};

pub const PIXEL: f32 = 0.025;

/// 카메라 절대각(45° 부저각) — 전 빌보드 공통 방향
pub fn billboard_quat() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2 * 0.5)
}

pub fn spawn_billboard(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world: &WorldData,
    texture: Handle<Image>,
    w_px: f32,
    h_px: f32,
    x: usize,
    y: usize,
    pixel: f32,
) -> Entity {
    let w = w_px * pixel;
    let h = h_px * pixel;
    let mesh = Mesh::from(bevy::shape::Rectangle::new(w / 2.0, h / 2.0));
    let ground = world.height_at(x as i32, y as i32);
    commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                alpha_mode: AlphaMode::Mask(0.5),
                double_sided: true,
                perceptual_roughness: 1.0,
                metallic: 0.0,
                ..default()
            })),
            Transform::from_translation(Vec3::new(
                x as f32 + 0.5,
                ground + h / 2.0 - 0.06,
                y as f32 + 0.5,
            ))
            .with_rotation(billboard_quat()),
        ))
        .id()
}

/// 이미지 크기를 아는 뒤 스폰(간편 래퍼) — 캐시된 Handle 재사용
pub fn spawn_prop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world: &WorldData,
    images: &mut Assets<Image>,
    texture: &Handle<Image>,
    x: usize,
    y: usize,
    pixel: f32,
) -> Entity {
    let (w, h) = images
        .get(texture)
        .map(|i| (i.width() as f32, i.height() as f32))
        .unwrap_or((32.0, 32.0));
    spawn_billboard(commands, meshes, materials, world, texture.clone(), w, h, x, y, pixel)
}

#[derive(Component)]
pub struct Campfire {
    pub textures: [Handle<Image>; 3],
    pub frame: usize,
    pub timer: f32,
}

pub fn spawn_props(
    mut commands: Commands,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut campfire_handles: Option<[Handle<Image>; 3]> = None;
    for p in &world.props {
        let tex: Handle<Image> = assets.load(p.kind.texture());
        // 이미지가 아직 로드 전일 수 있다 — 크기는 정적으로 알고 있다(매니페스트 값)
        let (w, h) = match p.kind {
            PropKind::Hut => (72.0, 72.0),
            PropKind::TreeBroadleaf => (48.0, 64.0),
            PropKind::TreePine => (40.0, 64.0),
            PropKind::TreeSapling => (16.0, 24.0),
            PropKind::BerryBush => (24.0, 24.0),
            PropKind::Boulder | PropKind::GoldVein => (32.0, 32.0),
            PropKind::Windbreak => (40.0, 32.0),
            PropKind::Grave => (24.0, 32.0),
            PropKind::Stash => (24.0, 24.0),
            PropKind::Relic => (20.0, 24.0),
            PropKind::Campfire => (24.0, 32.0),
        };
        if p.kind == PropKind::Campfire {
            let arr = campfire_handles.get_or_insert_with(|| [
                assets.load("props/campfire_0.png"),
                assets.load("props/campfire_1.png"),
                assets.load("props/campfire_2.png"),
            ]);
            let e = spawn_billboard(
                &mut commands, &mut meshes, &mut materials, &world,
                arr[0].clone(), w, h, p.x, p.y, 0.03,
            );
            commands.entity(e).insert(Campfire {
                textures: arr.clone(),
                frame: 0,
                timer: 0.0,
            });
            // 모닥불 빛 — 따뜻한 포인트 라이트
            let ground = world.height_at(p.x as i32, p.y as i32);
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.72, 0.42),
                    intensity: 80000.0,
                    range: 7.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(Vec3::new(p.x as f32 + 0.5, ground + 0.6, p.y as f32 + 0.5)),
            ));
            continue;
        }
        spawn_prop(&mut commands, &mut meshes, &mut materials, &world, &mut images, &tex, p.x, p.y, PIXEL);
    }
    let _ = SIZE;
}

pub fn animate_campfires(
    time: Res<Time>,
    mut q: Query<(&mut Campfire, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (mut fire, mat_handle) in &mut q {
        fire.timer += time.delta_secs();
        if fire.timer < 0.16 {
            continue;
        }
        fire.timer = 0.0;
        fire.frame = (fire.frame + 1) % 3;
        if let Some(mut mat) = materials.get_mut(mat_handle.id()) {
            mat.base_color_texture = Some(fire.textures[fire.frame].clone());
        }
    }
}
