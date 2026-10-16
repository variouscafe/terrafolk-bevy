//! 하루 순환 — 게임 시각의 순수 함수로 태양·앰비언트·하늘색 유도(Godot daynight 포트).
//! 1실초 = 1게임분(서버 §17.37 기본 배속). T키 배속 토글.

use bevy::prelude::*;

#[derive(Resource)]
pub struct DayTime {
    pub hour: f32,
    pub time_scale: f32,
    pub night_factor: f32,
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
            sunrise_min: 360.0,
            sunset_min: 1080.0,
        }
    }
}

#[derive(Component)]
pub struct Sun;

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        Sun,
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default(),
    ));
}

pub fn advance_time(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut day: ResMut<DayTime>) {
    if keys.just_pressed(KeyCode::KeyT) {
        day.time_scale = if day.time_scale == 1.0 { 40.0 } else { 1.0 };
    }
    day.hour = (day.hour + time.delta_secs() * day.time_scale / 60.0) % 24.0;
}

pub fn apply_sky(
    mut day: ResMut<DayTime>,
    weather: Res<super::precip::WeatherState>,
    mut sun_q: Query<&mut Transform, With<Sun>>,
    mut light_q: Query<&mut DirectionalLight>,
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

    // 태양 배치 — 고도·방위각 구면 위치에서 원점 look_at
    if let Ok(mut tr) = sun_q.single_mut() {
        let (e, a) = (elev.to_radians(), azimuth.to_radians());
        let pos = Vec3::new(a.cos() * e.cos(), e.sin().max(0.06), a.sin() * e.cos()) * 60.0;
        tr.translation = pos;
        tr.look_at(Vec3::ZERO, Vec3::Y);
    }

    const NOON: [f32; 3] = [1.0, 0.9, 0.76];
    const DUSK: [f32; 3] = [1.0, 0.55, 0.30];
    const MOON: [f32; 3] = [0.52, 0.62, 0.85];
    // ── 날씨 — 조도 감쇠·탈포화 계수(§17.39 clear|overcast|rain|storm|snow|fog)
    let (dim, gray, cool) = weather.sky_mix();
    if let Ok(mut light) = light_q.single_mut() {
        let c = lerp3(lerp3(NOON, DUSK, dusk_w), MOON, night);
        light.color = Color::srgb(c[0], c[1], c[2]);
        light.illuminance = 21000.0 * (1.0 - night * 0.9) * dim;
    }

    const SKY_DAY: [f32; 3] = [0.60, 0.78, 0.95];
    const SKY_DUSK: [f32; 3] = [0.66, 0.46, 0.36];
    const SKY_NIGHT: [f32; 3] = [0.04, 0.055, 0.12];
    let mut sky = lerp3(lerp3(SKY_DAY, SKY_DUSK, dusk_w), SKY_NIGHT, night);

    // ── 날씨 — 조도 감쇠·하늘 탈포화(§17.39 clear|overcast|rain|storm|snow|fog)
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
}
