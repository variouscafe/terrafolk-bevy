//! 건축 발판·창불 — §17.75·§17.78의 3D 포트(FE scene/bldRender.ts).
//! 다중 타일 발판(서버 앵커=좌상단 타일, state.w/h — HUT 2×2·LONGHOUSE 3×2·
//! WAREHOUSE 3×3)을 중심 정렬로 스폰한다. 공사 중: 모서리 말뚝+밧줄 외곽+
//! 공사 링 펄스(#ffd28a) + 진행 바. 완공: GLB 건물 + 창불(발판 안 주민 →
//! 따뜻한 발광 쿼드, "집안에 누가 왔다"가 멀리서 보인다).
//!
//! WebGL2 제약 대응: materials_guard가 Blend를 Mask로 강제하므로 알파 페이드
//! 대신 비발광(unlit) 색 밝기 펄스로 링을 맥동시키고, 창불은 Bloom이 잡는
//! 낮은 emissive 쿼드로 연출한다.

use bevy::math::primitives::{Annulus, Circle};
use bevy::prelude::*;

use crate::net::apply::LiveAgentTag;

/// 발판 사각형(타일, 좌상단 앵커) — 사이트·완공 건물·창불·호버 판정이 공유.
#[derive(Component, Clone, Copy)]
pub struct SiteRect {
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
}

impl SiteRect {
    pub fn contains(&self, tx: i32, ty: i32) -> bool {
        tx >= self.x && tx < self.x + self.w as i32 && ty >= self.y && ty < self.y + self.h as i32
    }
}

/// 공사 루트 메타 — 진행률(0..1)은 스냅숏마다 갱신, 시각은 재스폰 없이 따라감.
#[derive(Component)]
pub struct SiteMeta {
    pub progress: f32,
}

/// 공사 링 — 색 밝기 펄스 대상(FE 알파 펄스의 Mask-safe 대응).
#[derive(Component)]
pub struct ConstructionRing;

/// 진행 바 채움 — 좌측 고정, scale.x = max(0.02, progress)×full_w.
#[derive(Component)]
pub struct SiteBarFill {
    pub full_w: f32,
    pub cx: f32,
}

/// 완공 건물 창불 쿼드 — 발판 안 주민 점등 시 Visible.
#[derive(Component)]
pub struct WindowGlow;

const AMBER_LIN: LinearRgba = LinearRgba::new(1.0, 0.824, 0.541, 1.0); // #ffd28a
const POST_COLOR: Color = Color::srgb(0.45, 0.34, 0.22);
const ROPE_COLOR: Color = Color::srgb(0.62, 0.5, 0.34);
const AMBER: Color = Color::srgb(1.0, 0.824, 0.541); // #ffd28a
const GLOW_EMISSIVE: LinearRgba = LinearRgba::new(0.55, 0.435, 0.297, 1.0); // #ffc98a × 0.55

/// 공사 발판 스폰 — 말뚝·밧줄·링·진행 바. 서버 앵커(x,y)=좌상단 타일.
/// 루트 엔티티(SiteRect+SiteMeta)를 반환 — 호출처가 LiveResourceTag를 붙인다.
#[allow(clippy::too_many_arguments)] // 스폰 파라미터 — 발판 규격·지면·진행률 주입
pub fn spawn_site(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    x: i32,
    y: i32,
    w: u16,
    h: u16,
    ground: f32,
    progress: f32,
) -> Entity {
    let fw = w as f32;
    let fh = h as f32;
    let cx = x as f32 + fw / 2.0;
    let cz = y as f32 + fh / 2.0;
    let root = commands
        .spawn((
            SiteRect { x, y, w, h },
            SiteMeta { progress },
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // 모서리 말뚝 4개 — 높이 0.9m
    let post = meshes.add(Cylinder::new(0.045, 0.9));
    let post_mat = mats.add(StandardMaterial {
        base_color: POST_COLOR,
        ..default()
    });
    let hx = fw / 2.0 - 0.08;
    let hz = fh / 2.0 - 0.08;
    for (dx, dz) in [(-hx, -hz), (hx, -hz), (-hx, hz), (hx, hz)] {
        let post_e = commands
            .spawn((
                Mesh3d(post.clone()),
                MeshMaterial3d(post_mat.clone()),
                Transform::from_translation(Vec3::new(cx + dx, ground + 0.42, cz + dz)),
            ))
            .id();
        // 루트 자식화 — 철거(LiveResourceTag despawn) 시 말뚝·밧줄도 함께
        // 사라지게(리뷰 #5 — 완공 후에도 발판이 남아 벽을 뚫던 누수)
        commands.entity(root).add_child(post_e);
    }
    // 밧줄 외곽 — 발판 가장자리 4변(지면 위 3cm, unlit)
    let rope = meshes.add(Cuboid::new(1.0, 0.025, 0.03));
    let rope_mat = mats.add(StandardMaterial {
        base_color: ROPE_COLOR,
        unlit: true,
        ..default()
    });
    for (len, px, pz, rot) in [
        (fw - 0.16, cx, cz - hz, 0.0),
        (fw - 0.16, cx, cz + hz, 0.0),
        (fh - 0.16, cx - hx, cz, std::f32::consts::FRAC_PI_2),
        (fh - 0.16, cx + hx, cz, std::f32::consts::FRAC_PI_2),
    ] {
        let mut tr = Transform::from_translation(Vec3::new(px, ground + 0.05, pz));
        tr.rotation = Quat::from_rotation_y(rot);
        tr.scale.x = len;
        let rope_e = commands
            .spawn((Mesh3d(rope.clone()), MeshMaterial3d(rope_mat.clone()), tr))
            .id();
        commands.entity(root).add_child(rope_e);
    }
    // 공사 링 — 발판 하단 가장자리 기준 반경 max(w,h)×0.6(FE 비율).
    // 링마다 전용 재질 인스턴스 — 밝기 펄스가 다른 발판과 섞이지 않게.
    let r_mid = (w.max(h) as f32 * 0.6).max(0.85);
    let ring = commands
        .spawn((
            ConstructionRing,
            Mesh3d(meshes.add(Mesh::from(Annulus::new(r_mid - 0.05, r_mid + 0.05)))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: AMBER,
                unlit: true,
                ..default()
            })),
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(cx, ground + 0.04, cz + hz)),
        ))
        .id();
    commands.entity(root).add_child(ring);

    // 진행 바 — 건물 상단 부유(어두운 바탕판 + 호박색 채움, 좌측 고정 성장)
    let bar_w = (fw - 0.25).max(0.7);
    let bar_y = ground + 2.6 + fh * 0.5;
    let bg = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(bar_w, 0.055, 0.09))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: Color::srgb(0.05, 0.05, 0.05),
                unlit: true,
                ..default()
            })),
            Transform::from_translation(Vec3::new(cx, bar_y, cz)),
        ))
        .id();
    let fill = commands
        .spawn((
            SiteBarFill {
                full_w: bar_w - 0.02,
                cx,
            },
            Mesh3d(meshes.add(Cuboid::new(1.0, 0.045, 0.075))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: AMBER,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(Vec3::new(cx, bar_y + 0.001, cz)),
        ))
        .id();
    commands.entity(root).add_child(bg);
    commands.entity(root).add_child(fill);
    root
}

/// 완공 건물 창불 쿼드 — 건물 중심 상공의 낮은 발광판(Bloom 번짐).
/// FE ADD glow(alpha .13)의 WebGL2 대응 — Mask 강제 하에서도 성립.
/// 발판 사각형을 함께 부여해 hut_lights가 점등 판정에 쓴다.
#[allow(clippy::too_many_arguments)] // 스폰 파라미터 — 발판 사각형·중심·높이 주입
pub fn spawn_window_glow(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    rect: SiteRect,
    cx: f32,
    cz: f32,
    ground: f32,
    building_h: f32,
) -> Entity {
    let r = (rect.w.max(rect.h) as f32 * 0.55).max(0.9);
    commands
        .spawn((
            WindowGlow,
            rect,
            Mesh3d(meshes.add(Mesh::from(Circle::new(r)))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: GLOW_EMISSIVE,
                ..default()
            })),
            Transform::from_translation(Vec3::new(cx, ground + building_h * 0.35, cz))
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::Hidden,
        ))
        .id()
}

/// 공사 장식 갱신 — 링 밝기 펄스(0.4↔0.9, FE 900ms yoyo 상당) + 진행 바 채움.
/// 진행률은 부모 SiteMeta에서 읽는다(sync_resources가 스냅숏마다 갱신).
pub fn site_dressing(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_meta: Query<&SiteMeta>,
    q_childof: Query<&ChildOf>,
    mut q_ring: Query<&MeshMaterial3d<StandardMaterial>, With<ConstructionRing>>,
    mut q_bar: Query<(Entity, &SiteBarFill, &mut Transform), Without<ConstructionRing>>,
) {
    let pulse = 0.65 + 0.25 * (time.elapsed_secs() * std::f32::consts::TAU / 1.8).sin();
    for mat in &mut q_ring {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color = (AMBER_LIN * pulse).into();
        }
    }
    for (e, fill, mut tr) in &mut q_bar {
        let prog = q_childof
            .get(e)
            .ok()
            .and_then(|c| q_meta.get(c.parent()).ok())
            .map(|m| m.progress.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let w = (fill.full_w * prog.max(0.02)).max(0.02);
        tr.scale.x = w;
        // 좌측 고정 — 바 중심(cx) 기준 왼쪽 끝을 바탕판에 맞춘다
        tr.translation.x = fill.cx - fill.full_w / 2.0 + w / 2.0;
    }
}

/// 창불 점등 — 발판 안(입구·실내) 주민이 있으면 건물에 따뜻한 불이 켠다
/// (§17.78). 마을 집 수는 적어 전수 스캔이 저렴하다(FE와 동일 전략).
pub fn hut_lights(
    q_agents: Query<&Transform, With<LiveAgentTag>>,
    mut q_glow: Query<(&SiteRect, &mut Visibility), With<WindowGlow>>,
) {
    for (rect, mut vis) in &mut q_glow {
        let lit = q_agents.iter().any(|tr| {
            rect.contains(
                tr.translation.x.floor() as i32,
                tr.translation.z.floor() as i32,
            )
        });
        *vis = if lit {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
