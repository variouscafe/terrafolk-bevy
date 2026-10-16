//! 하루 순환 — 게임 시각의 순수 함수로 태양·앰비언트·하늘색 유도(Godot daynight 포트).
//! 1실초 = 1게임분(서버 §17.37 기본 배속). T키 배속 토글.

use bevy::prelude::*;
use bevy::render::view::ColorGrading;

#[derive(Resource)]
pub struct DayTime {
    pub hour: f32,
    pub time_scale: f32,
    pub night_factor: f32,
    /// 현재 계절(서버 프레임 — 새 겨울 휴면 등 하늘 생명이 소비)
    pub season: String,
    /// 오늘 일출/일몰(분) — 서버 계절별 값, 데모는 6:00/18:00 고정
    pub sunrise_min: f32,
    pub sunset_min: f32,
}

impl Default for DayTime {
    fn default() -> Self {
        DayTime {
            hour: 9.0,
            time_scale: 1.0,
            night_factor: 0.0,
            season: "summer".into(),
            sunrise_min: 360.0,
            sunset_min: 1080.0,
        }
    }
}

#[derive(Component)]
pub struct Sun;

/// 달 디스크 — 밤에 태양 반대 방위(고도 45° 고정)에 뜨는 언릿 원반.
#[derive(Component)]
pub struct MoonDisc;

/// 별밤 루트 — 카메라를 따라다니는 자식 별 쿼드 무리(밤에만 표시).
#[derive(Component)]
pub struct StarsRoot;

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// 별 분포용 결정론 PRNG(data.rs Rng와 동일 알고리즘 — 의존 없이 재현).
fn star_rng(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 11) as f32 / (1u64 << 53) as f32).clamp(0.0, 1.0)
}

pub fn spawn_sun(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Sun,
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default(),
    ));
    // 달 — 언릿 창백 원반(밤 하늘 앵커, apply_sky가 위치·가시성 소유)
    commands.spawn((
        MoonDisc,
        Transform::from_scale(Vec3::splat(5.0)),
        Mesh3d(meshes.add(Mesh::from(bevy::math::primitives::Circle::new(1.0)))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.91, 0.93, 0.96),
            unlit: true,
            ..default()
        })),
        Visibility::Hidden,
    ));
    // 별 — 상반구 90개, 절대각 카메라를 향한 작은 쿼드(루트가 카메라 추적)
    let star_mesh = meshes.add(Mesh::from(bevy::math::primitives::Circle::new(0.30)));
    let star_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.80, 0.85, 0.94),
        unlit: true,
        ..default()
    });
    let root = commands
        .spawn((StarsRoot, Visibility::Hidden, Transform::default()))
        .with_children(|c| {
            let mut rng = 20261017u64;
            for _ in 0..90 {
                let az = star_rng(&mut rng) * std::f32::consts::TAU;
                let el = (0.12 + star_rng(&mut rng) * 0.8).min(1.4);
                let r = 120.0;
                let pos = Vec3::new(
                    az.cos() * el.cos() * r,
                    el.sin() * r + 6.0,
                    az.sin() * el.cos() * r,
                );
                let mut tr = Transform::from_translation(pos)
                    .with_scale(Vec3::splat(0.7 + star_rng(&mut rng) * 0.8));
                tr.look_at(pos * 2.0, Vec3::Y); // +Z면을 카메라(원점)쪽으로
                c.spawn((
                    Mesh3d(star_mesh.clone()),
                    MeshMaterial3d(star_mat.clone()),
                    tr,
                ));
            }
        })
        .id();
    let _ = root;
}

pub fn advance_time(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut day: ResMut<DayTime>) {
    if keys.just_pressed(KeyCode::KeyT) {
        day.time_scale = if day.time_scale == 1.0 { 40.0 } else { 1.0 };
    }
    day.hour = (day.hour + time.delta_secs() * day.time_scale / 60.0) % 24.0;
}

#[allow(clippy::type_complexity)] // 하늘 구성 쿼리 — 태양·달·별 상호 배제 접근
#[allow(clippy::too_many_arguments)] // Bevy 시스템 파라미터 — 시간·날씨·카메라·하늘 구성
pub fn apply_sky(
    mut day: ResMut<DayTime>,
    weather: Res<super::precip::WeatherState>,
    rig: Res<super::camera_rig::CameraRig>,
    mut sun_q: Query<&mut Transform, (With<Sun>, Without<MoonDisc>, Without<StarsRoot>)>,
    mut moon_q: Query<
        (&mut Transform, &mut Visibility),
        (With<MoonDisc>, Without<Sun>, Without<StarsRoot>),
    >,
    mut stars_q: Query<
        (&mut Transform, &mut Visibility),
        (With<StarsRoot>, Without<Sun>, Without<MoonDisc>),
    >,
    mut light_q: Query<&mut DirectionalLight>,
    mut grading_q: Query<&mut ColorGrading, With<Camera3d>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let h = day.hour;
    // 일출~일몰 정규화(서버 계절 반영 — 겨울 짧은 낮)
    let day_len = (day.sunset_min - day.sunrise_min).max(60.0);
    let day_t = ((h * 60.0 - day.sunrise_min) / day_len).clamp(-0.2, 1.2);
    let elev = (day_t.clamp(0.0, 1.0) * std::f32::consts::PI).sin() * 68.0;
    let azimuth = -110.0 + (day_t.clamp(0.0, 1.0) * 220.0);
    let night = if day_t <= 0.0 || day_t >= 1.0 {
        1.0
    } else {
        (1.0 - (day_t * 2.4).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    };
    let dusk_w = ((1.0 - (day_t - 0.06).abs() * 7.0).clamp(0.0, 1.0)
        + (1.0 - (day_t - 0.94).abs() * 7.0).clamp(0.0, 1.0))
    .clamp(0.0, 1.0);
    day.night_factor = night;
    // ── 날씨 — 조도 감쇠·탈포화 계수(§17.39 clear|overcast|rain|storm|snow|fog)
    let (dim, gray, cool) = weather.sky_mix();

    // 태양 배치 — 고도·방위각 구면 위치에서 원점 look_at
    if let Ok(mut tr) = sun_q.single_mut() {
        let (e, a) = (elev.to_radians(), azimuth.to_radians());
        let pos = Vec3::new(a.cos() * e.cos(), e.sin().max(0.06), a.sin() * e.cos()) * 60.0;
        tr.translation = pos;
        tr.look_at(Vec3::ZERO, Vec3::Y);
    }

    // ── 달·별 밤하늘(FE solar·씬 야경 포트) — 팬해도 하늘이 따라오게
    //    카메라 중심 기준 배치. 흐림·비(gray)에선 가려진다.
    let ground = Vec3::new(rig.target.x, 0.0, rig.target.y);
    let overcast = gray > 0.45;
    if let Ok((mut mt, mut mv)) = moon_q.single_mut() {
        let az = (azimuth + 180.0).to_radians();
        let el = 45f32.to_radians();
        let pos = ground + Vec3::new(az.cos() * el.cos(), el.sin(), az.sin() * el.cos()) * 110.0;
        mt.translation = pos;
        mt.look_at(ground, Vec3::Y);
        *mv = if night > 0.35 && !overcast {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok((mut st, mut sv)) = stars_q.single_mut() {
        st.translation = ground;
        *sv = if night > 0.55 && !overcast {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    const NOON: [f32; 3] = [1.0, 0.9, 0.76];
    const DUSK: [f32; 3] = [1.0, 0.55, 0.30];
    const MOON: [f32; 3] = [0.52, 0.62, 0.85];
    if let Ok(mut light) = light_q.single_mut() {
        let c = lerp3(lerp3(NOON, DUSK, dusk_w), MOON, night);
        light.color = Color::srgb(c[0], c[1], c[2]);
        light.illuminance = 21000.0 * (1.0 - night * 0.9) * dim;
    }

    const SKY_DAY: [f32; 3] = [0.60, 0.78, 0.95];
    const SKY_DUSK: [f32; 3] = [0.66, 0.46, 0.36];
    const SKY_NIGHT: [f32; 3] = [0.04, 0.055, 0.12];
    let mut sky = lerp3(lerp3(SKY_DAY, SKY_DUSK, dusk_w), SKY_NIGHT, night);

    // ── 날씨 — 하늘 탈포화(sky_mix은 상단에서 조기 계산)
    let luma = sky[0] * 0.3 + sky[1] * 0.6 + sky[2] * 0.1;
    sky = lerp3(sky, [luma, luma, luma], gray);
    sky = lerp3(sky, [0.74, 0.79, 0.9], cool);

    clear.0 = Color::srgb(sky[0], sky[1], sky[2]);
    ambient.brightness = (70.0 + (1.0 - night) * 160.0) * (0.55 + 0.45 * dim);
    ambient.color = Color::srgb(
        lerp3([1.0, 0.96, 0.88], [0.45, 0.55, 0.85], night)[0],
        lerp3([1.0, 0.96, 0.88], [0.45, 0.55, 0.85], night)[1],
        lerp3([1.0, 0.96, 0.88], [0.45, 0.55, 0.85], night)[2],
    );

    // ── 컬러그레이딩 — 이미지 전체 화이트밸런스·채도(조명색과 독립적인 무드 층)
    if let Ok(mut g) = grading_q.single_mut() {
        let gg = &mut g.global;
        // 노을 웜(빨강+) ↔ 밤·흐림 쿨(파랑-) — CIE xy 크로마시티 시프트
        gg.temperature = dusk_w * 0.22 - night * 0.16 - cool * 0.10;
        // 밤·흐림 탈포화(코지 팔레트 지키는 미세 범위), 노을 살짝 증포화
        gg.post_saturation = (1.0 + dusk_w * 0.08 - night * 0.16 - gray * 0.14).clamp(0.6, 1.2);
        // 밤 노출 살짝 들어올림(+0.2 stop) — 어둠 속 실루엣만 남지 않게
        gg.exposure = night * 0.2;
    }
}
