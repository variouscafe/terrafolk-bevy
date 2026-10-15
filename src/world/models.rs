//! 3D 소품 — poly.pizza/Quaternius CC0 GLB 씬 스폰 + 자동 핏.
//! 원천 스케일이 제각각(0.57m 오두막~25m 텐트)이므로 계층 AABB를 측정해
//! 목표 높이로 정규화한다(결정론 — 좌표 해시 회전·지터 포함).
//! 번들 GLB(나무 5종 나란히)는 메시명으로 변형 1개만 유지한다.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::world_serialization::WorldAssetRoot;
use std::collections::HashMap;

use super::data::{PropKind, WorldData};

/// 소품 → GLB 명세. variants: 번들 내 메시명(빈 배열=통째 사용).
pub struct ModelSpec {
    pub path: &'static str,
    pub target_h: f32,
    pub variants: &'static [&'static str],
}

impl PropKind {
    pub fn model(self) -> ModelSpec {
        match self {
            PropKind::TreeBroadleaf => ModelSpec {
                path: "models/MapleTrees.glb",
                target_h: 5.6,
                variants: &["MapleTree_1", "MapleTree_2", "MapleTree_3", "MapleTree_4", "MapleTree_5"],
            },
            PropKind::TreePine => ModelSpec {
                path: "models/PineTrees.glb",
                target_h: 6.4,
                variants: &["PineTree_1", "PineTree_2", "PineTree_3", "PineTree_4", "PineTree_5"],
            },
            PropKind::TreePalm => ModelSpec {
                path: "models/PalmTrees.glb",
                target_h: 4.8,
                variants: &["PalmTree_1", "PalmTree_2", "PalmTree_3", "PalmTree_4", "PalmTree_5"],
            },
            PropKind::TreeSapling => ModelSpec {
                path: "models/Trees_Normal.glb",
                target_h: 2.3,
                variants: &["NormalTree_1", "NormalTree_2", "NormalTree_3"],
            },
            PropKind::TreeDead => ModelSpec {
                path: "models/DeadTrees.glb",
                target_h: 5.2,
                variants: &["DeadTree_8", "DeadTree_9", "DeadTree_10"],
            },
            PropKind::BerryBush => ModelSpec {
                path: "models/Bushes.glb",
                target_h: 1.15,
                variants: &["Bush", "Plant_1", "Bush_Flowers"],
            },
            PropKind::Boulder => ModelSpec {
                path: "models/Rocks.glb",
                target_h: 1.5,
                variants: &["Rock_1", "Rock_2", "Rock_3", "Rock_4"],
            },
            PropKind::GoldVein => ModelSpec {
                path: "models/Rocks.glb",
                target_h: 1.05,
                variants: &["Rock_2"],
            },
            PropKind::FlowerClump => ModelSpec {
                path: "models/Flowers.glb",
                target_h: 0.6,
                variants: &[
                    "Flower_1_Clump", "Flower_2_Clump", "Flower_3_Clump",
                    "Flower_4_Clump", "Flower_5_Clump",
                ],
            },
            PropKind::GrassClump => ModelSpec {
                path: "models/Grass.glb",
                target_h: 0.55,
                variants: &["Grass_Large_Extruded", "Grass_Small"],
            },
            PropKind::Hut => ModelSpec {
                path: "models/Hut.glb",
                target_h: 3.4,
                variants: &[],
            },
            PropKind::StorageHut => ModelSpec {
                path: "models/StorageHut.glb",
                target_h: 2.7,
                variants: &[],
            },
            PropKind::Tent => ModelSpec {
                path: "models/Tent.glb",
                target_h: 2.5,
                variants: &[],
            },
            PropKind::Campfire => ModelSpec {
                path: "models/Bonfire.glb",
                target_h: 1.15,
                variants: &[],
            },
            PropKind::Windbreak => ModelSpec {
                path: "models/Fence.glb",
                target_h: 1.05,
                variants: &[],
            },
            PropKind::Grave => ModelSpec {
                path: "models/Grave.glb",
                target_h: 1.3,
                variants: &[],
            },
            PropKind::Stash => ModelSpec {
                path: "models/CubeCrate.glb",
                target_h: 1.0,
                variants: &[],
            },
            PropKind::Relic => ModelSpec {
                path: "models/RockLarge.glb",
                target_h: 2.9,
                variants: &[],
            },
            PropKind::WoodLog => ModelSpec {
                path: "models/WoodLog.glb",
                target_h: 0.9,
                variants: &[],
            },
        }
    }
}

#[derive(Component)]
pub struct PropRoot {
    pub kind: PropKind,
    pub target_h: f32,
    pub ground_y: f32,
    pub rot_y: f32,
    pub scale_jitter: f32,
    pub variants: &'static [&'static str],
}

#[derive(Component)]
pub struct Fitted;

#[derive(Component)]
pub struct CampfireGlow;

fn pos_hash(x: usize, y: usize, salt: u64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
        ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ salt.wrapping_mul(0x165667B19E3779F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 32;
    h
}

pub fn spawn_props(mut commands: Commands, world: Res<WorldData>, assets: Res<AssetServer>) {
    // 종별 씬 핸들 캐시 — 동일 모델 인스턴싱(배칭) 유도
    let mut scene_cache: HashMap<&'static str, _> = HashMap::new();
    for p in &world.props {
        let spec = p.kind.model();
        let scene = scene_cache
            .entry(spec.path)
            .or_insert_with(|| assets.load(GltfAssetLabel::Scene(0).from_asset(spec.path)))
            .clone();
        let ground = world.height_at(p.x as i32, p.y as i32);
        let h1 = pos_hash(p.x, p.y, 1);
        let h2 = pos_hash(p.x, p.y, 2);
        let rot_y = (h1 % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
        let scale_jitter = 0.92 + (h2 % 1000) as f32 / 1000.0 * 0.16;
        commands.spawn((
            WorldAssetRoot(scene),
            PropRoot {
                kind: p.kind,
                target_h: spec.target_h,
                ground_y: ground,
                rot_y,
                scale_jitter,
                variants: spec.variants,
            },
            Transform::from_translation(Vec3::new(
                p.x as f32 + 0.5,
                ground,
                p.y as f32 + 0.5,
            )),
        ));
        if p.kind == PropKind::Campfire {
            commands.spawn((
                CampfireGlow,
                PointLight {
                    color: Color::srgb(1.0, 0.66, 0.32),
                    intensity: 120000.0,
                    range: 9.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(Vec3::new(
                    p.x as f32 + 0.5,
                    ground + 1.0,
                    p.y as f32 + 0.5,
                )),
            ));
        }
    }
}

/// 메시 POSITION 속성에서 AABB(GPU 미추출 독립 — final_aabb는 추출 후에만 존재).
fn mesh_aabb(mesh: &Mesh) -> Option<(Vec3, Vec3)> {
    let vals = mesh.attribute(bevy::render::mesh::Mesh::ATTRIBUTE_POSITION)?;
    let VertexAttributeValues::Float32x3(pts) = vals else {
        return None;
    };
    let mut mn = Vec3::splat(f32::MAX);
    let mut mx = Vec3::splat(f32::MIN);
    for p in pts {
        let v = Vec3::new(p[0], p[1], p[2]);
        mn = mn.min(v);
        mx = mx.max(v);
    }
    Some((mn, mx))
}

/// 핏 측정용 쿼리 타입
type FitQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Name>,
        Option<&'static Children>,
        Option<&'static Mesh3d>,
        &'static Transform,
        Option<&'static ChildOf>,
    ),
>;

/// 메시 엔티티의 루트 미포함 조상 체인 로컬 행렬(자신 포함).
/// GLB 번들은 RootNode에 전역 스케일이 있는 경우가 있어 체인 전체 곱이 필수.
fn chain_matrix(entity: Entity, root_e: Entity, q: &FitQuery<'_, '_>) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    let mut cur = entity;
    for _ in 0..16 {
        if cur == root_e {
            break;
        }
        if let Ok((_, _, _, tf, parent)) = q.get(cur) {
            m = tf.to_matrix() * m;
            match parent {
                Some(p) => cur = p.parent(),
                None => break,
            }
        } else {
            break;
        }
    }
    m
}

/// 유지 메시들의 결합 AABB — 각 메시를 체인 행렬로 루트 로컬 공간으로.
fn meshes_aabb(
    ents: &[Entity],
    root_e: Entity,
    q: &FitQuery<'_, '_>,
    meshes: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let mut mn = Vec3::splat(f32::MAX);
    let mut mx = Vec3::splat(f32::MIN);
    for &e in ents {
        let Ok((_, _, mesh, _, _)) = q.get(e) else { continue };
        let Some((amn, amx)) = mesh
            .and_then(|mh| meshes.get(&mh.0))
            .and_then(mesh_aabb)
        else {
            return None; // 미로딩 — 재시도
        };
        let mat = chain_matrix(e, root_e, q);
        for cx in [amn.x, amx.x] {
            for cy in [amn.y, amx.y] {
                for cz in [amn.z, amx.z] {
                    let v = mat.transform_point3(Vec3::new(cx, cy, cz));
                    mn = mn.min(v);
                    mx = mx.max(v);
                }
            }
        }
    }
    (mn.x <= mx.x).then_some((mn, mx))
}

/// 서브트리의 메시 엔티티 전수 수집(이름 병기).
fn collect_meshes(
    entity: Entity,
    q: &FitQuery<'_, '_>,
    out: &mut Vec<(Entity, String)>,
) {
    let Ok((name, children, mesh, _, _)) = q.get(entity) else { return };
    if mesh.is_some() {
        out.push((entity, name.map(|n| n.as_str().to_string()).unwrap_or_default()));
    }
    if let Some(children) = children {
        for c in children.iter() {
            collect_meshes(c, q, out);
        }
    }
}

/// 자동 핏(1단계) — 씬 자식이 생기면: 변형 선택 → AABB 측정 → 목표 계산.
/// GLB 번들은 RootNode 아래 변형 메시들이 나란히 붙는 구조 — 서브트리 전체에서
/// 이름 매칭 메시 1개만 남기고 타 메시는 재귀 파기(묶음 통째 렌더 방지).
pub fn fit_props(
    mut commands: Commands,
    q_root: Query<(Entity, &PropRoot, &Children), (With<PropRoot>, Without<Fitted>, Without<FitTransform>)>,
    q_all: FitQuery,
    meshes: Res<Assets<Mesh>>,
) {
    for (root_e, prop, children) in &q_root {
        let mut keep_mesh: Option<Entity> = None;
        if !prop.variants.is_empty() {
            let pick_idx = (pos_hash(
                (prop.ground_y.to_bits() as usize) % 997,
                (prop.rot_y.to_bits() as usize) % 991,
                7,
            ) as usize)
                % prop.variants.len();
            let pick = prop.variants[pick_idx];
            let mut all_meshes = Vec::new();
            for c in children.iter() {
                collect_meshes(c, &q_all, &mut all_meshes);
            }
            for (e, name) in &all_meshes {
                if name.starts_with(pick) {
                    if keep_mesh.is_none() {
                        keep_mesh = Some(*e); // 첫 매칭 유지
                    } else {
                        commands.entity(*e).despawn();
                    }
                } else {
                    commands.entity(*e).despawn();
                }
            }
        }
        // 측정 — 유지 메시들만(변형 미지정이면 전체 메시)
        let mut measure: Vec<Entity> = Vec::new();
        if !prop.variants.is_empty() {
            if let Some(k) = keep_mesh {
                measure.push(k);
            }
        }
        if measure.is_empty() {
            let mut all = Vec::new();
            for c in children.iter() {
                collect_meshes(c, &q_all, &mut all);
            }
            measure = all.into_iter().map(|(e, _)| e).collect();
        }
        // 메시 미로딩이면 다음 프레임 재시도(핏될 때까지 매 프레임 대상 유지)
        let Some((mn, mx)) = meshes_aabb(&measure, root_e, &q_all, &meshes) else {
            continue;
        };
        let size = (mx - mn).max(Vec3::splat(1e-4));
        let mut s = (prop.target_h / size.y) * prop.scale_jitter;
        if !s.is_finite() {
            continue;
        }
        // 방어 클램프 — 병리 데이터로 거대화 차단(원천 0.05~5배)
        let clamped = s.clamp(0.02, 6.0);
        #[cfg(target_arch = "wasm32")]
        if (s - clamped).abs() > 1e-6 {
            web_sys::console::log_1(&format!("FIT 클램프 {:.2}→{:.2} size=({:.2},{:.2},{:.2}) variants={:?}",
                s, clamped, size.x, size.y, size.z, prop.variants).into());
        }
        s = clamped;
        let root_y = prop.ground_y - mn.y * s;
        commands.entity(root_e).insert(FitTransform { root_y, scale: s, rot_y: prop.rot_y });
    }
}

#[derive(Component)]
pub struct FitTransform {
    root_y: f32,
    scale: f32,
    rot_y: f32,
}

/// 자동 핏(2단계) — 기존 x,z 유지, y·스케일·회전 적용.
pub fn apply_fit(
    mut commands: Commands,
    q: Query<(Entity, &FitTransform, &Transform), Without<Fitted>>,
) {
    for (e, fit, tf) in &q {
        let mut new_tf = *tf;
        new_tf.translation.y = fit.root_y;
        new_tf.scale = Vec3::splat(fit.scale);
        new_tf.rotation = Quat::from_rotation_y(fit.rot_y);
        commands.entity(e).insert(new_tf).insert(Fitted).remove::<FitTransform>();
    }
}

/// 모닥불 빛 플리커 — 따뜻한 요동(결정론 아님, 연출).
pub fn flicker_campfires(time: Res<Time>, mut q: Query<&mut PointLight, With<CampfireGlow>>) {
    let t = time.elapsed_secs();
    for mut light in &mut q {
        let f = 0.82
            + 0.13 * (t * 9.0).sin()
            + 0.05 * (t * 23.0 + 1.7).sin()
            + 0.04 * (t * 5.0 + 4.1).cos();
        light.intensity = 120000.0 * f;
    }
}
