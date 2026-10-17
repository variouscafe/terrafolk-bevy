//! 바람 — 식물 소품 정점 흔들림(WindMaterial = StandardMaterial + 버텍스 확장).
//! WebGL2 제약 대응: 유니폼 바인딩 추가 불가(fragment 12개 한도 초과) →
//! 파라미터는 specialize 시 셰이더 상수(ShaderDefVal::UInt, ÷1000)로 주입.
//! GLB가 로드한 StandardMaterial을 종별로 복제 교체(핸들 캐시로 배칭 유지).

use bevy::asset::Asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use std::collections::HashMap;

use super::data::PropKind;

/// 유니폼·파라미터 없음 — 바람 성격은 셰이더 통일 곡선(높이 선형 증폭).
/// WebGL2 fragment 유니폼 12개 한도로 어떤 확장 바인딩도 불가 — 확장이
/// 바인딩을 0개 갖는다(장식용 _pad 유니폼조차 AlphaBlend 파이프라인을
/// 13개로 밀어 죽였다).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct WindExtension {}

impl MaterialExtension for WindExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/wind.vert.wgsl".into()
    }
}

pub type WindMaterial = ExtendedMaterial<StandardMaterial, WindExtension>;

pub fn is_wind_target(kind: PropKind) -> bool {
    matches!(
        kind,
        PropKind::TreeBroadleaf
            | PropKind::TreePine
            | PropKind::TreePalm
            | PropKind::TreeSapling
            | PropKind::TreeDead
            | PropKind::BerryBush
            | PropKind::GrassClump
            | PropKind::FlowerClump
    )
}

#[derive(Component)]
pub struct WindApplied;

/// 핏 완료 소품의 GLB 재질을 WindMaterial로 교체 — 출처 핸들+코드 캐시로
/// 동일 (재질, 종) 조합은 하나의 변환 재질을 공유(오토 인스턴싱 배칭 유지).
#[allow(clippy::type_complexity)] // Bevy 쿼리 타입 — 배타 컴포넌트 필터 조합
pub fn apply_wind(
    mut commands: Commands,
    q_root: Query<
        (Entity, &super::models::PropRoot),
        (With<super::models::Fitted>, Without<WindApplied>),
    >,
    q_walk: Query<(Option<&Children>, Option<&MeshMaterial3d<StandardMaterial>>)>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut wind_mats: ResMut<Assets<WindMaterial>>,
    mut cache: Local<HashMap<AssetId<StandardMaterial>, Handle<WindMaterial>>>,
) {
    if crate::net::dbg_gate::off("no_wind") {
        return; // 진단 게이트 — WindMaterial 교체 스킵
    }
    let mut visited: Vec<Entity> = Vec::new();
    for (root_e, prop) in &q_root {
        if !is_wind_target(prop.kind) {
            commands.entity(root_e).insert(WindApplied);
            continue;
        }
        visited.clear();
        visited.push(root_e);
        while let Some(e) = visited.pop() {
            let Ok((children, mat)) = q_walk.get(e) else {
                continue;
            };
            if let Some(mat) = mat {
                let handle = cache
                    .entry(mat.id())
                    .or_insert_with(|| {
                        let mut base = std_mats.get(mat).cloned().unwrap_or_default();
                        // 잎 재질 원천이 alphaMode BLEND(Quaternius 잎 텍스처)라
                        // 반투명 겹침으로 화면이 유령 숲처럼 뿌옇게 덮인다.
                        // 알파 컷아웃(Mask)으로 전환 — 불투명 잎·깊이 쓰기·정렬 문제 제거.
                        base.alpha_mode = AlphaMode::Mask(0.5);
                        wind_mats.add(WindMaterial {
                            base,
                            extension: WindExtension {},
                        })
                    })
                    .clone();
                commands
                    .entity(e)
                    .remove::<MeshMaterial3d<StandardMaterial>>()
                    .insert(MeshMaterial3d(handle));
            }
            if let Some(children) = children {
                visited.extend(children.iter());
            }
        }
        commands.entity(root_e).insert(WindApplied);
    }
}
