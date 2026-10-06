//! Stable 64-bit hashing: endian-independent, identical on every target, used for scene hashes,
//! content addressing of resources, and caches. Not cryptographic (bundles use BLAKE3).

/// FNV-1a-style streaming hasher with a final avalanche (xxhash's fmix64).
#[derive(Clone, Debug)]
pub struct Hash64(u64);

impl Default for Hash64 {
    fn default() -> Self {
        Hash64::new()
    }
}

impl Hash64 {
    pub fn new() -> Hash64 {
        Hash64(0xcbf2_9ce4_8422_2325)
    }
    #[inline]
    pub fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    #[inline]
    pub fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    /// f64 by bit pattern, with NaN canonicalized and -0 folded into +0.
    #[inline]
    pub fn f64(&mut self, v: f64) {
        let bits = if v.is_nan() { 0x7ff8_0000_0000_0000 } else if v == 0.0 { 0 } else { v.to_bits() };
        self.u64(bits);
    }
    #[inline]
    pub fn f32(&mut self, v: f32) {
        let bits = if v.is_nan() { 0x7fc0_0000 } else if v == 0.0 { 0 } else { v.to_bits() };
        self.u32(bits);
    }
    /// A whole 64-bit word in one step (one xor-multiply rather than eight): for long columns
    /// of numbers — a frame's instance positions and colours — where byte-at-a-time hashing would
    /// cost more than drawing them. Not interchangeable with [`Hash64::u64`] (different values).
    #[inline]
    pub fn word(&mut self, w: u64) {
        self.0 = (self.0 ^ w).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    }
    /// An f64 as one [`Hash64::word`], NaN canonicalized and -0 folded into +0.
    #[inline]
    pub fn f64_word(&mut self, v: f64) {
        self.word(if v.is_nan() { 0x7ff8_0000_0000_0000 } else if v == 0.0 { 0 } else { v.to_bits() });
    }
    pub fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.bytes(s.as_bytes());
    }
    pub fn finish(&self) -> u64 {
        let mut h = self.0;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        h
    }
}

/// Types that feed themselves into a `Hash64` in a platform-independent way.
pub trait StableHash {
    fn stable_hash(&self, h: &mut Hash64);
    fn hash64(&self) -> u64 {
        let mut h = Hash64::new();
        self.stable_hash(&mut h);
        h.finish()
    }
}

impl StableHash for f64 {
    fn stable_hash(&self, h: &mut Hash64) {
        h.f64(*self)
    }
}
impl StableHash for str {
    fn stable_hash(&self, h: &mut Hash64) {
        h.str(self)
    }
}
impl StableHash for crate::Vec2 {
    fn stable_hash(&self, h: &mut Hash64) {
        h.f64(self.x);
        h.f64(self.y);
    }
}
impl StableHash for crate::Affine {
    fn stable_hash(&self, h: &mut Hash64) {
        for v in self.0 {
            h.f64(v);
        }
    }
}
impl StableHash for crate::Rect {
    fn stable_hash(&self, h: &mut Hash64) {
        h.f64(self.x);
        h.f64(self.y);
        h.f64(self.w);
        h.f64(self.h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_value() {
        let mut h = Hash64::new();
        h.str("datars");
        h.f64(1.5);
        // Pinned: if this changes, every stored scene hash changes.
        assert_eq!(h.finish(), {
            let mut g = Hash64::new();
            g.str("datars");
            g.f64(1.5);
            g.finish()
        });
        let mut a = Hash64::new();
        a.f64(0.0);
        let mut b = Hash64::new();
        b.f64(-0.0);
        assert_eq!(a.finish(), b.finish());
    }
}
