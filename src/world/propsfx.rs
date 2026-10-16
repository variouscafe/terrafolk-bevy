//! 소품 미세 연출 — 유물 부유(FE relic 아이콘 sin 트윈)·광맥 점멸
//! (FE 귀한 광맥 spark). 자원 디테일 패리티의 3D 담당.
//! 유물: 지면 위 sin 부유 + 느린 자전(주민 시선 유도). 광맥: emissive
//! 스파크 쿼드가 밝기를 맥동 — Bloom이 점멸로 읽힌다(Blend 불가 대응).

use bevy::math::primitives::Rectangle;
use bevy::prelude::*;

/// 유물 부유 — 기준 높이에서 sin 오실레이션.
#[derive(Component)]
pub struct FloatBob {
    pub base_y: f32,
    pub phase: f32,
}

/// 광맥 스파크 — emissive 맥동 쿼드.
#[derive(Component)]
pub struct VeinSpark;

/// 스파크 위상 — 종마다 어긋나게 점멸(동시 깜빡임 방지).
#[derive(Component)]
pub struct SparkPhase(pub f32);

const SPARK_COLOR: LinearRgba = LinearRgba::new(1.0, 0.83, 0.35, 1.0);
/// 부저각 카메라 방향(요 45°·피치 35°) — 스파크 쿼드가 카메라를 향한다.
const CAM_FACE: Vec3 = Vec3::new(0.579, 0.574, 0.579);

/// 스파크 쿼드 스폰 — 광맥 상단, 절대각 카메라를 바라보는 사각형.
pub fn spawn_vein_spark(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    x: f32,
    ground: f32,
    z: f32,
    phase: f32,
) -> Entity {
    let pos = Vec3::new(x, ground + 0.35, z);
    let mut tr = Transform::from_translation(pos);
    tr.look_at(pos - CAM_FACE, Vec3::Y); // 쿼드 +Z면이 카메라 방향
    commands
        .spawn((
            VeinSpark,
            SparkPhase(phase),
            Mesh3d(meshes.add(Mesh::from(Rectangle::new(0.14, 0.14)))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: SPARK_COLOR,
                ..default()
            })),
            tr,
        ))
        .id()
}

/// 유물 부유 갱신 — sin(진폭 5cm·주기 2.4s) + 회전(0.5 rad/s).
pub fn bob_relics(time: Res<Time>, mut q: Query<(&FloatBob, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (bob, mut tr) in &mut q {
        tr.translation.y = bob.base_y + 0.05 * (t * std::f32::consts::TAU / 2.4 + bob.phase).sin();
        tr.rotation = Quat::from_rotation_y(t * 0.5 + bob.phase);
    }
}

/// 광맥 점멸 — emissive 밝기 맥동(주기 1.3s, 종별 위상차).
pub fn spark_veins(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&SparkPhase, &MeshMaterial3d<StandardMaterial>), With<VeinSpark>>,
) {
    let t = time.elapsed_secs();
    for (phase, mat) in &mut q {
        let k = 0.875 + 0.725 * (t * std::f32::consts::TAU / 1.3 + phase.0).sin();
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.emissive = SPARK_COLOR * k;
        }
    }
}
