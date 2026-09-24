//! Seeded PRNG for the game's `UnityEngine.Random` call sites (eye-blink intervals,
//! breath phase). Unity's generator state is not reproduced; the seed is part of the
//! export configuration so the output stays deterministic.

/// xorshift128, the same family Unity uses. Seeding is our own.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u32; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // splitmix64 to spread the seed over the state.
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        let a = next();
        let b = next();
        let mut s = [a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        if s == [0; 4] {
            s[0] = 1;
        }
        Self { s }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut t = self.s[0];
        let s = self.s[3];
        t ^= t << 11;
        t ^= t >> 8;
        self.s = [self.s[1], self.s[2], s, s ^ (s >> 19) ^ t ^ (t >> 8)];
        self.s[3]
    }

    /// `Random.Range(min, max)` for floats: uniform in `[min, max]`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        let unit = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        min + (max - min) * unit
    }

    /// `Random.Range(min, max)` for ints: uniform in `[min, max)`.
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (max - min) as u32;
        min + (self.next_u32() % span) as i32
    }
}
