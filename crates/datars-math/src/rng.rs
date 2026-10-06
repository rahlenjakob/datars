/// A small, fast, seeded PRNG (SplitMix64 seeding a xoshiro256** state). Same seed → same sequence
/// on every platform. The only randomness the engine allows.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A stateless 64-bit mix (SplitMix64's step): a well-spread hash of `x`, for counter-based
/// randomness where each row needs its own number without a sequence to draw from (the
/// expression language's `rand(i)`, point-pyramid priorities). Same input → same output everywhere.
pub fn mix64(x: u64) -> u64 {
    let mut s = x;
    splitmix(&mut s)
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut x = seed;
        Rng { s: [splitmix(&mut x), splitmix(&mut x), splitmix(&mut x), splitmix(&mut x)] }
    }
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }
    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
    /// Uniform integer in [0, n).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_seed_same_sequence() {
        let mut a = super::Rng::new(42);
        let mut b = super::Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let v = super::Rng::new(1).next_f64();
        assert!((0.0..1.0).contains(&v));
    }

    #[test]
    fn mix64_is_a_fixed_spread_hash() {
        // Pinned values: every target must agree (they seed per-row randomness in documents).
        assert_eq!(super::mix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(super::mix64(1), 0x910A_2DEC_8902_5CC1);
        // Neighbouring inputs land far apart.
        assert!((super::mix64(41) ^ super::mix64(42)).count_ones() > 16);
    }
}
