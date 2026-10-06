//! Gradient ramps: each distinct stop list becomes one 256-texel row of a shared atlas texture;
//! the fragment shader computes `t` from the gradient geometry and samples its row.
//!
//! Stops interpolate in straight-alpha sRGB (`Color::lerp_srgb`), the same space the colours are
//! written in and the CPU reference blends in; beyond the end stops the end colours extend (pad).

use datars_color::Color;
use datars_math::Hash64;
use std::collections::BTreeMap;

/// Texels per ramp row.
pub const RAMP_W: u32 = 256;

/// One ramp row: `RAMP_W` straight-alpha RGBA8 texels.
pub fn ramp(stops: &[(f32, Color)]) -> Vec<u8> {
    let mut s: Vec<(f32, Color)> = stops.iter().map(|&(t, c)| (if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) }, c)).collect();
    s.sort_by(|a, b| a.0.total_cmp(&b.0)); // stable: equal offsets keep their order (hard edges)
    let mut out = Vec::with_capacity(RAMP_W as usize * 4);
    for i in 0..RAMP_W {
        let t = i as f32 / (RAMP_W - 1) as f32;
        let c = match s.as_slice() {
            [] => Color::BLACK,
            [only] => only.1,
            _ => {
                let k = s.iter().position(|st| st.0 > t).unwrap_or(s.len());
                if k == 0 {
                    s[0].1
                } else if k == s.len() {
                    s[s.len() - 1].1
                } else {
                    let (a, b) = (s[k - 1], s[k]);
                    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 1.0 };
                    a.1.lerp_srgb(b.1, f as f64)
                }
            }
        };
        out.extend_from_slice(&c.to_rgba8());
    }
    out
}

pub fn stops_hash(stops: &[(f32, Color)]) -> u64 {
    let mut h = Hash64::new();
    for (t, c) in stops {
        h.f32(*t);
        for v in [c.r, c.g, c.b, c.a] {
            h.f32(v);
        }
    }
    h.finish()
}

struct Row {
    row: u32,
    last_used: u64,
    data: Vec<u8>,
}

/// Row allocation for the ramp atlas (the texture itself lives with the GPU resources).
#[derive(Default)]
pub struct RampRows {
    rows: BTreeMap<u64, Row>,
    free: Vec<u32>,
    next: u32,
    /// Rows allocated this frame whose texels still need uploading.
    pub dirty: Vec<u32>,
}

impl RampRows {
    /// The row for `stops`, allocating (and marking dirty) on first use.
    pub fn row_for(&mut self, stops: &[(f32, Color)], frame: u64) -> u32 {
        let key = stops_hash(stops);
        if let Some(r) = self.rows.get_mut(&key) {
            r.last_used = frame;
            return r.row;
        }
        let row = self.free.pop().unwrap_or_else(|| {
            self.next += 1;
            self.next - 1
        });
        self.rows.insert(key, Row { row, last_used: frame, data: ramp(stops) });
        self.dirty.push(row);
        row
    }
    /// Rows the atlas needs (the highest allocated row + 1).
    pub fn needed(&self) -> u32 {
        self.next
    }
    /// Texels of `row`, if allocated.
    pub fn data(&self, row: u32) -> Option<&[u8]> {
        self.rows.values().find(|r| r.row == row).map(|r| r.data.as_slice())
    }
    /// Every allocated row (for re-uploading after the atlas grows).
    pub fn all(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.rows.values().map(|r| r.row).collect();
        v.sort_unstable();
        v
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    /// Free rows unused for more than `max_unseen` frames.
    pub fn evict(&mut self, frame: u64, max_unseen: u64) {
        let stale: Vec<u64> = self.rows.iter().filter(|(_, r)| frame.saturating_sub(r.last_used) > max_unseen).map(|(k, _)| *k).collect();
        for k in stale {
            if let Some(r) = self.rows.remove(&k) {
                self.free.push(r.row);
            }
        }
        self.free.sort_unstable_by(|a, b| b.cmp(a)); // reuse low rows first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_interpolates_and_pads() {
        let r = ramp(&[(0.25, Color::rgb8(0, 0, 0)), (0.75, Color::rgb8(255, 0, 0))]);
        assert_eq!(r.len(), RAMP_W as usize * 4);
        assert_eq!(&r[0..4], &[0, 0, 0, 255], "before the first stop: first colour");
        let last = (RAMP_W as usize - 1) * 4;
        assert_eq!(&r[last..last + 4], &[255, 0, 0, 255], "after the last stop: last colour");
        let mid = (RAMP_W as usize / 2) * 4;
        assert!((r[mid] as i32 - 128).abs() <= 2, "midpoint is half red: {}", r[mid]);
    }

    #[test]
    fn rows_are_shared_reused_and_freed() {
        let a = [(0.0, Color::BLACK), (1.0, Color::WHITE)];
        let b = [(0.0, Color::WHITE), (1.0, Color::BLACK)];
        let mut rows = RampRows::default();
        assert_eq!(rows.row_for(&a, 1), 0);
        assert_eq!(rows.row_for(&b, 1), 1);
        assert_eq!(rows.row_for(&a, 2), 0, "same stops, same row");
        assert_eq!(rows.dirty, vec![0, 1]);
        rows.evict(10, 5);
        assert_eq!(rows.len(), 0);
        assert_eq!(rows.row_for(&b, 11), 0, "freed rows are reused, lowest first");
        assert_eq!(rows.needed(), 2);
    }
}
