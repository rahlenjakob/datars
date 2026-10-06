//! Scene hashes. A scene hashes via its canonical JSON (serde_json's float formatting is the
//! shortest round-trip representation and identical on every target), so the hash, the snapshot and
//! the bundle encoding agree by construction.

use crate::Scene;
use datars_math::Hash64;

impl Scene {
    /// A stable 64-bit hash of the whole scene (P1: equal on every target for equal inputs).
    pub fn hash(&self) -> u64 {
        let mut h = Hash64::new();
        h.bytes(serde_json::to_string(self).unwrap_or_default().as_bytes());
        h.finish()
    }
    pub fn hash_hex(&self) -> String {
        format!("{:016x}", self.hash())
    }
}
