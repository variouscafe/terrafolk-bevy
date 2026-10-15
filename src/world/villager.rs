//! 주민 — 분할 프레임 PNG(4방향×4프레임, W계 미러) 기반 보행 + 배회.
//! Phase 2에서 서버 agents 스키마 좌표로 구동된다.

use bevy::prelude::*;

use super::billboard::{billboard_quat, PIXEL};
use super::data::{WorldData, SIZE};

const WALK_TIME: f32 = 0.85;
const FRAME_TIME: f32 = 0.14;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    S,
    SE,
    E,
    N,
}

impl Facing {
    fn row(self) -> usize {
        match self {
            Facing::S => 0,
            Facing::SE => 1,
            Facing::E => 2,
            Facing::N => 3,
        }
    }
    fn from_dir(dx: i32, dy: i32) -> (Facing, bool) {
        // (방향, 좌우 미러) — 세계 +Z가 카메라쪽(S), -Z가 N
        if dy >= 1 && dx == 0 {
            (Facing::S, false)
        } else if dy <= -1 && dx == 0 {
            (Facing::N, false)
        } else if dx >= 1 && dy == 0 {
            (Facing::E, false)
        } else if dx <= -1 && dy == 0 {
            (Facing::E, true)
        } else if dx >= 1 && dy >= 1 {
            (Facing::SE, false)
        } else if dx <= -1 && dy >= 1 {
            (Facing::SE, true)
        } else if dx >= 1 {
            (Facing::N, false)
        } else {
            (Facing::N, true)
        }
    }
}

#[derive(Component)]
pub struct Villager {
    pub textures: Vec<Vec<Handle<Image>>>, // [row][frame]
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub moving: bool,
    pub idle: f32,
    pub facing: Facing,
    pub flip: bool,
    pub frame: usize,
    pub frame_timer: f32,
}

pub fn spawn_villagers(
    mut commands: Commands,
    world: Res<WorldData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let rows = ["S", "SE", "E", "N"];
    let mut textures: Vec<Vec<Handle<Image>>> = Vec::new();
    for row in rows {
        let mut frames = Vec::new();
        for f in 0..4 {
            frames.push(assets.load(format!("chars/villager_{}_{}.png", row, f)));
        }
        textures.push(frames);
    }

    let mut rng = super::data::Rng::new(99);
    let c = SIZE as i32 / 2;
    let mut placed = 0;
    let mut attempts = 0;
    while placed < 6 && attempts < 200 {
        attempts += 1;
        let x = rng.range_i(c - 7, c + 7);
        let y = rng.range_i(c - 7, c + 7);
        if !world.walkable(x, y) {
            continue;
        }
        let pos = Vec3::new(x as f32 + 0.5, world.height_at(x, y) + 0.7, y as f32 + 0.5);
        let mesh = Mesh::from(bevy::math::primitives::Rectangle::new(32.0 * 0.028, 48.0 * 0.028));
        commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(textures[0][0].clone()),
                    alpha_mode: AlphaMode::Mask(0.5),
                    double_sided: true,
                    perceptual_roughness: 1.0,
                    metallic: 0.0,
                    ..default()
                })),
                Transform::from_translation(pos).with_rotation(billboard_quat()),
                Villager {
                    textures: textures.clone(),
                    from: pos,
                    to: pos,
                    t: 0.0,
                    moving: false,
                    idle: rng.range(0.0, 2.0),
                    facing: Facing::S,
                    flip: false,
                    frame: 0,
                    frame_timer: 0.0,
                },
            ))
            .id();
        placed += 1;
    }
    let _ = PIXEL;
}

pub fn wander(time: Res<Time>, world: Res<WorldData>, mut q: Query<(&mut Villager, &mut Transform)>) {
    for (mut v, mut tr) in &mut q {
        if v.moving {
            v.t += time.delta_secs() / WALK_TIME;
            if v.t >= 1.0 {
                v.t = 0.0;
                v.moving = false;
                v.idle = 0.4 + rand_free((time.elapsed_secs() * 1000.0) as u64 ^ (tr.translation.x as u64).wrapping_mul(31), 2.0);
                tr.translation = v.to;
            } else {
                let s = v.t * v.t * (3.0 - 2.0 * v.t); // smoothstep
                tr.translation = v.from.lerp(v.to, s);
                // 고도 보간 — 계단 절벽에서 직선 관통 방지(낮은 쪽 따라감)
                tr.translation.y = tr.translation.y.max(v.from.y.min(v.to.y));
            }
            continue;
        }
        v.idle -= time.delta_secs();
        if v.idle > 0.0 {
            continue;
        }
        let here = (
            tr.translation.x.floor() as i32,
            tr.translation.z.floor() as i32,
        );
        let mut rng = super::data::Rng::new(
            (here.0 as u64 as u64 * 31 + here.1 as u64).wrapping_mul(2654435761) ^ (time.elapsed_secs() as u64),
        );
        let dirs = [[0, 1], [0, -1], [1, 0], [-1, 0], [1, 1], [-1, 1], [1, -1], [-1, -1]];
        let start = (rng.next_u64() % 8) as usize;
        for i in 0..8 {
            let d = dirs[(start + i) % 8];
            let (nx, ny) = (here.0 + d[0], here.1 + d[1]);
            if world.walkable(nx, ny) {
                let (f, flip) = Facing::from_dir(d[0], d[1]);
                v.facing = f;
                if v.flip != flip {
                    v.flip = flip;
                    tr.scale.x = if flip { -1.0 } else { 1.0 };
                }
                v.from = tr.translation;
                v.to = Vec3::new(nx as f32 + 0.5, world.height_at(nx, ny) + 0.7, ny as f32 + 0.5);
                v.moving = true;
                v.t = 0.0;
                break;
            }
        }
        if !v.moving {
            v.idle = rand_free((time.elapsed_secs() * 1000.0) as u64 ^ (here.0 as u64 * 31 + here.1 as u64), 1.5);
        }
    }
}

/// 시간 기반 자유 난수(배회 휴식 길이) — 결정론 대상 아님(연출).
/// wasm엔 SystemTime이 없어 bevy Time 엔트로피를 시드로 쓴다.
fn rand_free(seed: u64, scale: f32) -> f32 {
    super::data::Rng::new(seed.wrapping_mul(2654435761)).f32() * scale
}

pub fn animate(
    time: Res<Time>,
    mut q: Query<(&mut Villager, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (mut v, mat_handle) in &mut q {
        v.frame_timer += time.delta_secs();
        if v.frame_timer < FRAME_TIME {
            continue;
        }
        v.frame_timer = 0.0;
        v.frame = (v.frame + 1) % 4;
        if let Some(mut mat) = materials.get_mut(mat_handle.id()) {
            mat.base_color_texture = Some(v.textures[v.facing.row()][v.frame].clone());
        }
    }
}
