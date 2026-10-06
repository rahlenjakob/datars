//! Retained GPU resources, keyed by content and evicted least-recently-used by frames unseen.
//!
//! Eviction order is a function of (last frame used, key), never of map order (P1). The map is
//! hashed (a seedless hasher, insertion-ordered): a county map looks up thousands of meshes a
//! frame, and an ordered map's comparisons down the tree were half its raster time.

use indexmap::IndexMap;
use std::hash::{BuildHasherDefault, Hash};
use std::sync::Arc;

/// FxHash: fast, and the same on every run and platform (no random seed).
pub type Fx = BuildHasherDefault<rustc_hash::FxHasher>;

/// What a mesh was built from. Everything that changes the triangles is in the key; everything
/// that only changes where or how they're drawn (transform, paint, opacity) is not.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshKey {
    /// `tol` is the tessellation-tolerance bucket (log2 of the device scale, rounded up).
    Fill { path: u64, rule: u8, tol: i32 },
    /// `space` is `None` for meshes in local units (similarity transforms), or the bits of the
    /// linear transform a device-space mesh was built under (non-uniform scale, shear). `width` is
    /// a quarter-octave bucket of the width in mesh units (the shader applies the exact width);
    /// `dash` is the pattern in mesh units, bit for bit.
    Stroke { path: u64, space: Option<[u64; 4]>, width: i32, cap: u8, join: u8, miter: u64, dash: Option<Vec<u64>>, tol: i32 },
    /// A glyph outline at an eighth-octave size bucket.
    Glyph { font: Arc<str>, glyph: u16, size: i32, tol: i32 },
    /// A glyph's halo stroke.
    Halo { font: Arc<str>, glyph: u16, size: i32, width: i32, tol: i32 },
    /// A text run's glyphs as one outline (`glyphs`: a hash of their ids and places), and its
    /// halo (`width` as for `Halo`; `None` for the fill).
    Run { font: Arc<str>, glyphs: u64, size: i32, width: Option<i32>, tol: i32 },
}

/// When to drop resources nobody drew for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachePolicy {
    /// Evict anything not drawn in this many frames.
    pub max_unseen_frames: u64,
    /// Above this many bytes, evict the least recently used first (never this frame's).
    pub max_bytes: u64,
}

impl Default for CachePolicy {
    fn default() -> Self {
        CachePolicy { max_unseen_frames: 240, max_bytes: 256 << 20 }
    }
}

struct Entry<V> {
    value: V,
    bytes: u64,
    last_used: u64,
}

/// Frames between scans for stale entries. Walking the whole cache every frame cost more than
/// drawing at street level (thousands of tile meshes), and an entry evicted a few frames late
/// changes nothing but memory.
const SCAN_EVERY: u64 = 32;

/// An LRU-by-frame cache.
pub struct Lru<K: Ord + Hash + Clone, V> {
    map: IndexMap<K, Entry<V>, Fx>,
    bytes: u64,
    next_scan: u64,
    /// Values an insert replaced, handed out by the next `evict` (to release like evicted ones).
    replaced: Vec<V>,
}

impl<K: Ord + Hash + Clone, V> Default for Lru<K, V> {
    fn default() -> Self {
        Lru { map: IndexMap::default(), bytes: 0, next_scan: 0, replaced: Vec::new() }
    }
}

impl<K: Ord + Hash + Clone, V> Lru<K, V> {
    /// Look up and mark used in `frame`.
    pub fn touch(&mut self, key: &K, frame: u64) -> Option<&V> {
        let e = self.map.get_mut(key)?;
        e.last_used = frame;
        Some(&e.value)
    }
    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }
    pub fn insert(&mut self, key: K, value: V, bytes: u64, frame: u64) -> &V {
        if let Some(old) = self.map.swap_remove(&key) {
            self.bytes -= old.bytes;
            self.replaced.push(old.value);
        }
        self.bytes += bytes;
        &self.map.entry(key).or_insert(Entry { value, bytes, last_used: frame }).value
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn clear(&mut self) -> Vec<V> {
        self.bytes = 0;
        let mut out = std::mem::take(&mut self.replaced);
        out.extend(std::mem::take(&mut self.map).into_values().map(|e| e.value));
        out
    }
    /// Evict per `policy` after `frame`; returns the evicted values (to release GPU memory). Stale
    /// entries are looked for every `SCAN_EVERY` frames; over budget, the least recently used go
    /// until a quarter of the budget is free again, so the next frames don't sort the cache anew.
    pub fn evict(&mut self, frame: u64, policy: &CachePolicy) -> Vec<V> {
        let mut out = std::mem::take(&mut self.replaced);
        if frame >= self.next_scan {
            self.next_scan = frame + SCAN_EVERY;
            let stale: Vec<K> =
                self.map.iter().filter(|(_, e)| frame.saturating_sub(e.last_used) > policy.max_unseen_frames).map(|(k, _)| k.clone()).collect();
            for k in stale {
                if let Some(e) = self.map.swap_remove(&k) {
                    self.bytes -= e.bytes;
                    out.push(e.value);
                }
            }
        }
        if self.bytes > policy.max_bytes {
            let target = policy.max_bytes - policy.max_bytes / 4;
            let mut order: Vec<(u64, K)> = self.map.iter().filter(|(_, e)| e.last_used < frame).map(|(k, e)| (e.last_used, k.clone())).collect();
            order.sort();
            for (_, k) in order {
                if self.bytes <= target {
                    break;
                }
                if let Some(e) = self.map.swap_remove(&k) {
                    self.bytes -= e.bytes;
                    out.push(e.value);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_by_frames_unseen_then_by_budget() {
        let mut c: Lru<u32, &str> = Lru::default();
        c.insert(1, "a", 10, 1);
        c.insert(2, "b", 10, 1);
        c.insert(3, "c", 10, 5);
        assert!(c.touch(&2, 4).is_some());
        let p = CachePolicy { max_unseen_frames: 3, max_bytes: 1000 };
        assert_eq!(c.evict(5, &p), vec!["a"], "1 was last drawn 4 frames ago");
        assert_eq!(c.len(), 2);
        assert_eq!(c.bytes(), 20);
        let tight = CachePolicy { max_unseen_frames: 100, max_bytes: 10 };
        assert_eq!(c.evict(5, &tight), vec!["b"], "over budget: oldest first, this frame's kept");
        assert_eq!(c.evict(5, &CachePolicy { max_unseen_frames: 100, max_bytes: 0 }), Vec::<&str>::new());
        assert!(!c.is_empty());
        assert_eq!(c.clear(), vec!["c"]);
        assert!(c.is_empty() && c.bytes() == 0);
    }

    #[test]
    fn stale_entries_are_looked_for_every_few_frames_and_budget_cuts_leave_room() {
        let p = CachePolicy { max_unseen_frames: 2, max_bytes: 1000 };
        let mut c: Lru<u32, u32> = Lru::default();
        c.insert(1, 1, 10, 0);
        assert!(c.evict(0, &p).is_empty(), "the first call scans: nothing stale yet");
        // Stale from frame 3, but the next scan is SCAN_EVERY frames on.
        for f in 1..SCAN_EVERY {
            assert!(c.evict(f, &p).is_empty(), "frame {f}: no scan");
        }
        assert_eq!(c.evict(SCAN_EVERY, &p), vec![1]);
        // Over budget: evict down to three quarters of it, not just under it.
        let mut c: Lru<u32, u32> = Lru::default();
        for k in 0..10 {
            c.insert(k, k, 100, k as u64);
        }
        let out = c.evict(10, &CachePolicy { max_unseen_frames: 1000, max_bytes: 900 });
        assert_eq!(out, vec![0, 1, 2, 3], "oldest first, until 675 bytes or less remain");
        assert_eq!(c.bytes(), 600);
    }

    #[test]
    fn keys_order_deterministically() {
        let a = MeshKey::Fill { path: 2, rule: 0, tol: 0 };
        let b = MeshKey::Stroke { path: 1, space: None, width: 0, cap: 0, join: 0, miter: 0, dash: None, tol: 0 };
        assert!(a < b);
    }
}
