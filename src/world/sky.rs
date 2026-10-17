//! 하늘 생명 — 새 떼 원 유영(FE ambient.ts 포트, 시각 연출만).
//! 무리 3개×5마리가 마을 상공(고도 9~13m)을 원 유영하며 서풍으로 표류한다.
//! 새벽부터 낮에만 활동(밤 소멸), 겨울엔 절반이 휴면(남하) — FE 계절 규칙.
//! 새는 절대각 카메라를 향한 작은 실루엣 쿼드, 날개짓은 세로 스케일 펄스.

use bevy::math::primitives::Circle;
use bevy::prelude::*;

use super::data::WorldData;
use super::daynight::DayTime;

/// 새 무리 — 중심 표류·원 유영 파라미터.
#[derive(Component)]
pub struct BirdFlock {
    pub center: Vec2,
    /// 표류 속도(서풍 → +x 방향, FE 구름그림자와 같은 바람)
    pub drift: Vec2,
    pub radius: f32,
    /// 원 유영 각속도(rad/s)
    pub speed: f32,
    pub phase: f32,
    pub count: usize,
}

/// 개별 새 — 무리 루트의 자식. flap 위상.
#[derive(Component)]
pub struct Bird {
    pub orbit_phase: f32,
    pub height: f32,
    pub flap: f32,
    pub size: f32,
}

const BIRD_COLOR: Color = Color::srgb(0.16, 0.18, 0.22);
/// 부저각 카메라 방향(요 45°·피치 35°) — 쿼드가 카메라를 향한다.
const CAM_FACE: Vec3 = Vec3::new(0.579, 0.574, 0.579);

pub fn spawn_birds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    world: Res<WorldData>,
) {
    if crate::net::dbg_gate::off("no_sky") {
        return; // 진단 게이트 — 새 떼 스폰 스킵
    }
    let mesh = meshes.add(Mesh::from(Circle::new(0.16)));
    let mat = mats.add(StandardMaterial {
        base_color: BIRD_COLOR,
        unlit: true,
        ..default()
    });
    // 결정론 분포 — 무리 3개(봄~가을 전부, 겨울엔 sky_update가 절반 숨김)
    let seeds = [
        (0.22f32, 0.30f32, 7.5f32, 0.55f32),
        (0.68, 0.62, 9.0, -0.42),
        (0.45, 0.82, 8.0, 0.38),
    ];
    for (fx, fy, radius, speed) in seeds {
        let center = Vec2::new(fx, fy) * world.size as f32;
        let flock = commands
            .spawn((
                BirdFlock {
                    center,
                    drift: Vec2::new(0.35, 0.08),
                    radius,
                    speed,
                    phase: fx * std::f32::consts::TAU,
                    count: 5,
                },
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|c| {
                for i in 0..5usize {
                    c.spawn((
                        Bird {
                            orbit_phase: i as f32 / 5.0 * std::f32::consts::TAU,
                            height: 9.0 + (i % 3) as f32 * 1.4,
                            flap: 0.6 + i as f32 * 0.37,
                            size: 0.8 + (i % 2) as f32 * 0.3,
                        },
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::default(),
                    ));
                }
            })
            .id();
        let _ = flock;
    }
}

/// 새 유영 갱신 — 무리 중심 표류(월드 순환) + 원 궤도 + 날개짓 스케일 펄스.
/// 밤(night_factor > 0.6)엔 숨김, 겨울엔 무리 절반 휴면(FE 남하 규칙).
pub fn sky_update(
    time: Res<Time>,
    day: Res<DayTime>,
    world: Res<WorldData>,
    mut flocks: Query<(&mut BirdFlock, &mut Visibility, &Children)>,
    mut birds: Query<(&Bird, &mut Transform), Without<BirdFlock>>,
) {
    let t = time.elapsed_secs();
    let size = world.size as f32;
    let night = day.night_factor;
    let winter = day.season == "winter";
    for (i, (mut flock, mut vis, children)) in flocks.iter_mut().enumerate() {
        // 표류 — 서풍(+x). 월드 밖으로 나가면 반대편에서 재등장(순환 바람)
        let drift = flock.drift * time.delta_secs();
        flock.center += drift;
        if flock.center.x > size + 12.0 {
            flock.center.x = -12.0;
        }
        if flock.center.y > size + 12.0 {
            flock.center.y = -12.0;
        }
        *vis = if night > 0.6 || (winter && i % 2 == 1) {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        for child in children.iter() {
            let Ok((bird, mut tr)) = birds.get_mut(child) else {
                continue;
            };
            let ang = flock.phase + t * flock.speed + bird.orbit_phase;
            let x = flock.center.x + ang.cos() * flock.radius;
            let z = flock.center.y + ang.sin() * flock.radius * 0.7;
            // 날개짓 — 세로 스케일 0.45↔1.15(8~10Hz 체감), 활공 구간(사인 양수)엔 접힘
            let flap = (t * 9.0 + bird.flap * std::f32::consts::TAU).sin();
            let sy = 0.8 + flap * 0.35;
            let pos = Vec3::new(x, bird.height + ang.sin() * 0.4, z);
            tr.translation = pos;
            tr.scale = Vec3::new(bird.size, bird.size * sy.max(0.35), bird.size);
            tr.look_at(pos + CAM_FACE * 10.0, Vec3::Y);
        }
    }
}
