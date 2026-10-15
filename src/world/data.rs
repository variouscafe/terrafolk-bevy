//! 데모 월드 데이터 — 시드 순수 함수(관전자·프레임률·재실행 무관 결정론).
//! 지형 GID·자원 kinds는 shared/client-meta.json 어휘(water/sand/grass/forest/rock/dirt).
//! Phase 2에서 Colyseus 스냅숏 어댑터가 이 리소스를 대체한다(구조 동일).

use bevy::prelude::*;

pub const SIZE: usize = 56;
pub const STEP: f32 = 0.35; // 고도 단계
pub const WATER_BED_Y: f32 = -0.7;
pub const TERRAINS: [&str; 6] = ["water", "sand", "grass", "forest", "rock", "dirt"]; // GID 1..6 순

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PropSpec {
    pub kind: PropKind,
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropKind {
    TreeBroadleaf,
    TreePine,
    TreePalm,
    TreeSapling,
    TreeDead,
    BerryBush,
    Boulder,
    GoldVein,
    FlowerClump,
    GrassClump,
    Hut,
    StorageHut,
    Tent,
    Campfire,
    Windbreak,
    Grave,
    Stash,
    Relic,
    WoodLog,
    // 동물(라이브 전용 — resources kind='animal', Poly by Google CC-BY)
    AnimalBoar,
    AnimalDeer,
    AnimalWaterDeer,
    AnimalSquirrel,
    AnimalPheasant,
}

/// xorshift64* — 외부 rand 크래이트 없는 결정론 PRNG(wasm 무의존).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }
    pub fn range_i(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.f32() * ((hi - lo + 1) as f32)) as i32
    }
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    // 정수 해시 → [0,1)
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFFFF) as f32 / 0xFFFFFF as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 랩 값노이즈 — 노드 그리드가 주기(=period)에서 순환(타일 시임 없음).
fn value_noise(x: f32, y: f32, period: f32, seed: u32) -> f32 {
    let gx = x / period;
    let gy = y / period;
    let x0 = gx.floor() as i32;
    let y0 = gy.floor() as i32;
    let tx = smooth(gx - gx.floor());
    let ty = smooth(gy - gy.floor());
    let n = period as i32; // 주기 = 노드 수(격자 랩)
    let wrap = |v: i32| ((v % n) + n) % n;
    let x0w = wrap(x0);
    let x1w = wrap(x0 + 1);
    let y0w = wrap(y0);
    let y1w = wrap(y0 + 1);
    let a = hash2(x0w, y0w, seed);
    let b = hash2(x1w, y0w, seed);
    let c = hash2(x0w, y1w, seed);
    let d = hash2(x1w, y1w, seed);
    let top = a + (b - a) * tx;
    let bot = c + (d - c) * tx;
    (top + (bot - top) * ty) * 2.0 - 1.0 // [-1, 1]
}

fn fbm(x: f32, y: f32, base_period: f32, seed: u32) -> f32 {
    let main = value_noise(x, y, base_period, seed);
    let detail = value_noise(x, y, base_period * 0.25, seed + 101);
    main * 0.82 + detail * 0.18
}

#[derive(Resource)]
pub struct WorldData {
    pub terrain: Vec<u8>, // size*size, GID 1..6
    pub height: Vec<i8>,  // size*size, 단계(물 -2, 육지 0..4)
    pub props: Vec<PropSpec>,
    pub size: usize,
    /// 데모(절차) 지형 여부 — false면 라이브(mapJson) 지형
    pub demo: bool,
}

impl WorldData {
    pub fn idx(x: usize, y: usize) -> usize {
        y * SIZE + x
    }
    pub fn terrain_at(&self, x: i32, y: i32) -> &'static str {
        if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
            return "water";
        }
        TERRAINS[(self.terrain[Self::idx(x as usize, y as usize)] - 1) as usize]
    }
    pub fn height_at(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
            return WATER_BED_Y;
        }
        if self.terrain_at(x, y) == "water" {
            return WATER_BED_Y;
        }
        self.height[Self::idx(x as usize, y as usize)] as f32 * STEP
    }
    /// 라이브 Tiled 맵 → 평면 지형(실 고도 없음 — 물은 -2단).
    pub fn from_map(gids: &[u8], w: usize, h: usize) -> Self {
        let n = w.min(h).min(256);
        let mut terrain = vec![1u8; n * n];
        for y in 0..n {
            for x in 0..n {
                let i = y * w + x;
                terrain[y * n + x] = gids.get(i).copied().unwrap_or(1).clamp(1, 6);
            }
        }
        let mut height = vec![0i8; n * n];
        for i in 0..n * n {
            if TERRAINS[(terrain[i] - 1) as usize] == "water" {
                height[i] = -2;
            }
        }
        WorldData {
            terrain,
            height,
            props: vec![],
            size: n,
            demo: false,
        }
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        let t = self.terrain_at(x, y);
        t != "water" && t != "rock"
    }

    pub fn generate(seed: u64) -> Self {
        let s = seed as u32;
        let mut terrain = vec![0u8; SIZE * SIZE];
        let mut height = vec![0i8; SIZE * SIZE];
        let c = SIZE as f32 / 2.0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let (x, y) = (x as f32, y as f32);
                let dx = (x - c) / c;
                let dy = (y - c) / c;
                let radial = (dx * dx + dy * dy).sqrt();
                // 섬 마스크: 노이즈 + 방사 감쇠(Godot WorldData와 동일 구조)
                let land = fbm(x, y, 22.0, s) + (1.0 - radial) * 1.15;
                let h = fbm(x, y, 9.0, s + 7);
                let flora = value_noise(x, y, 7.0, s + 13);
                let (gid, hstep): (u8, i8) = if land > 0.92 {
                    (5, 2 + (h.max(0.0) * 1.9) as i8) // rock 고지
                } else if land > 0.55 {
                    if flora > 0.12 {
                        (4, (h.clamp(-0.4, 1.0) * 2.2) as i8) // forest
                    } else if flora < -0.55 {
                        (6, (h.clamp(-0.4, 1.0) * 2.2) as i8) // dirt
                    } else {
                        (3, (h.clamp(-0.4, 1.0) * 2.2) as i8) // grass
                    }
                } else if land > 0.38 {
                    (2, 0) // sand 해안
                } else {
                    (1, -2) // water
                };
                let i = Self::idx(x as usize, y as usize);
                terrain[i] = gid;
                height[i] = hstep;
            }
        }

        // 소품 산포 — 결정론. 지형별 밀도(페인티드 군락 감각).
        let probe = Self {
            terrain: terrain.clone(),
            height: height.clone(),
            props: vec![],
            size: SIZE,
            demo: true,
        };
        let mut rng = Rng::new(seed ^ 0x5EED);
        let mut props = Vec::new();
        for y in 2..SIZE - 2 {
            for x in 2..SIZE - 2 {
                let t = probe.terrain_at(x as i32, y as i32);
                let r = rng.f32();
                let p = |kind, x, y| PropSpec { kind, x, y };
                if t == "forest" && r < 0.34 {
                    let kind = if rng.f32() < 0.55 {
                        PropKind::TreePine
                    } else {
                        PropKind::TreeBroadleaf
                    };
                    props.push(p(kind, x, y));
                } else if t == "forest" && r < 0.37 {
                    props.push(p(PropKind::TreeSapling, x, y));
                } else if t == "grass" && r < 0.05 {
                    props.push(p(PropKind::TreeBroadleaf, x, y));
                } else if t == "grass" && r < 0.062 {
                    props.push(p(PropKind::BerryBush, x, y));
                } else if t == "grass" && r < 0.115 {
                    props.push(p(PropKind::GrassClump, x, y));
                } else if t == "grass" && r < 0.132 {
                    props.push(p(PropKind::FlowerClump, x, y));
                } else if t == "rock" && r < 0.10 {
                    props.push(p(PropKind::Boulder, x, y));
                } else if t == "rock" && r < 0.13 {
                    props.push(p(PropKind::GoldVein, x, y));
                } else if (t == "rock" && r < 0.15) || (t == "dirt" && r < 0.02) {
                    props.push(p(PropKind::TreeDead, x, y));
                } else if t == "sand" && r < 0.018 {
                    props.push(p(PropKind::TreePalm, x, y));
                } else if t == "sand" && r < 0.028 {
                    props.push(p(PropKind::GrassClump, x, y));
                }
            }
        }
        // 중앙 촌락 씬 — 오두막·모닥불·짐·묘비·유물·울타리·텐트(히어로 구도)
        let cc = c as usize;
        for (kind, ox, oy) in [
            (PropKind::Hut, cc - 3, cc - 2),
            (PropKind::StorageHut, cc + 2, cc + 1),
            (PropKind::Tent, cc - 6, cc - 1),
            (PropKind::Windbreak, cc - 5, cc + 2),
            (PropKind::Windbreak, cc - 4, cc + 2),
            (PropKind::Windbreak, cc - 3, cc + 2),
            (PropKind::Campfire, cc, cc),
            (PropKind::Stash, cc + 1, cc - 1),
            (PropKind::WoodLog, cc - 1, cc + 1),
            (PropKind::Grave, cc + 4, cc - 3),
            (PropKind::Relic, cc - 1, cc + 4),
            (PropKind::FlowerClump, cc - 2, cc + 1),
            (PropKind::FlowerClump, cc + 2, cc - 2),
        ] {
            props.push(PropSpec { kind, x: ox, y: oy });
        }

        WorldData {
            terrain,
            height,
            props,
            size: SIZE,
            demo: true,
        }
    }
}
