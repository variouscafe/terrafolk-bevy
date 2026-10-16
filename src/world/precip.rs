//! 강우·강설 — 카메라 자식 풀스크린 쿼드 + 절차 셰이더(무결합·비조명).
//! 화면공간 낙하(레인 스트릭/설화 플레이크 2레이어) — 날씨 string으로 모드·
//! 강도를 정하고 지수 스무딩으로 페이드인·아웃한다.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// 서버 날씨 상태(net::apply가 갱신 — 데모는 clear 고정)
#[derive(Resource)]
pub struct WeatherState {
    pub kind: String,
}

impl Default for WeatherState {
    fn default() -> Self {
        WeatherState {
            kind: "clear".to_string(),
        }
    }
}

impl WeatherState {
    /// (조도 감쇠, 하늘 탈포화, 한랭 기운) — apply_sky가 소비
    pub fn sky_mix(&self) -> (f32, f32, f32) {
        match self.kind.as_str() {
            "overcast" => (0.78, 0.5, 0.0),
            "rain" => (0.64, 0.6, 0.0),
            "storm" => (0.46, 0.72, 0.0),
            "snow" => (0.9, 0.68, 0.35),
            "fog" => (0.86, 0.62, 0.0),
            _ => (1.0, 0.0, 0.0),
        }
    }
    /// 침전 (모드, 강도) — 0=비, 1=눈
    fn precip(&self) -> (u32, f32) {
        match self.kind.as_str() {
            "rain" => (0, 0.55),
            "storm" => (0, 1.0),
            "snow" => (1, 0.75),
            _ => (0, 0.0),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PrecipMaterial {
    /// x=낙하 속도 y=밀도 z=기울기 w=모드(0=비, 1=눈)
    #[uniform(0)]
    pub params: Vec4,
    /// x=가시 강도(페이드)
    #[uniform(1)]
    pub fade: Vec4,
}

impl Material for PrecipMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/precip.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// 카메라 자식 침전 쿼드 — attach_precip이 연결한다.
#[derive(Component)]
pub struct PrecipQuad;

/// 침전 런타임 상태(재질 핸들·스무딩 강도)
#[derive(Resource, Default)]
pub struct PrecipRig {
    pub mat: Option<Handle<PrecipMaterial>>,
    pub mesh: Option<Handle<Mesh>>,
    pub intensity: f32,
    pub mode: u32,
    pub attached: bool,
}

/// 카메라가 스폰된 뒤 쿼드를 자식으로 붙인다(Startup 순서 경쟁 회피 — 1회).
pub fn attach_precip(
    mut commands: Commands,
    mut rig: ResMut<PrecipRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PrecipMaterial>>,
    q_cam: Query<Entity, With<Camera3d>>,
) {
    if rig.attached {
        return;
    }
    let Ok(cam) = q_cam.single() else { return };
    rig.attached = true;
    let mat = materials.add(PrecipMaterial {
        params: Vec4::new(1.0, 1.0, 0.16, 0.0),
        fade: Vec4::splat(0.0),
    });
    // 카메라 로컬 -Z 2m — fov 32° 화면 피복 여유(1.6×)
    let size = 2.0 * 2.0 * 0.287 * 1.6;
    let mesh = meshes.add(Mesh::from(bevy::math::primitives::Plane3d::new(
        Vec3::Z,
        Vec2::splat(size),
    )));
    rig.mat = Some(mat.clone());
    rig.mesh = Some(mesh.clone());
    commands.entity(cam).with_children(|p| {
        p.spawn((
            PrecipQuad,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(Vec3::new(0.0, 0.0, -2.0)),
            Visibility::Hidden,
        ));
    });
}

/// 날씨 → 침전 모드·강도 반영(페이드 스무딩 2/s).
/// R키 = 날씨 순환(데모 확인용 — 라이브에서는 다음 프레임에 서버 값 복귀).
pub fn sync_precip(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut weather: ResMut<WeatherState>,
    mut rig: ResMut<PrecipRig>,
    mut materials: ResMut<Assets<PrecipMaterial>>,
    mut q_vis: Query<&mut Visibility, With<PrecipQuad>>,
) {
    const CYCLE: [&str; 6] = ["clear", "rain", "storm", "snow", "overcast", "fog"];
    if keys.just_pressed(KeyCode::KeyR) && !super::pick::ui_typing() {
        let i = CYCLE.iter().position(|c| *c == weather.kind).unwrap_or(0);
        weather.kind = CYCLE[(i + 1) % CYCLE.len()].to_string();
        info!("[precip] 날씨 → {}", weather.kind);
    }
    let Some(mat_h) = rig.mat.clone() else { return };
    let (mode, goal) = weather.precip();
    rig.mode = mode;
    let k = 1.0 - (-2.0 * time.delta_secs()).exp();
    rig.intensity += (goal - rig.intensity) * k;
    let Some(mut mat) = materials.get_mut(&mat_h) else {
        return;
    };
    mat.fade.x = rig.intensity;
    // 모드별 파라미터 — 비: 빠른 스트릭, 눈: 느린 플레이크
    mat.params = if mode == 0 {
        Vec4::new(1.35, 1.0, 0.16, 0.0)
    } else {
        Vec4::new(0.16, 0.8, 0.0, 1.0)
    };
    if let Ok(mut vis) = q_vis.single_mut() {
        *vis = if rig.intensity > 0.02 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
