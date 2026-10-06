//! PMTiles tile ids: every tile is keyed by one `u64` — the number of tiles at all lower zooms plus
//! the tile's position along a Hilbert curve at its own zoom. The Hilbert curve keeps spatially
//! near tiles near in id order, so directories delta-encode and compress well and range reads for
//! a viewport stay local.

/// Highest zoom whose coordinates fit `u32` and whose ids fit `u64`.
const MAX_ZOOM: u8 = 31;

/// Tiles at all zooms below `z`: `(4^z − 1) / 3`.
fn base(z: u8) -> u64 {
    (0..z as u64).map(|t| 1u64 << (2 * t)).sum()
}

/// `(z, x, y)` → PMTiles tile id. Tiles beyond zoom 31 have no id; they map to `u64::MAX`, which
/// no archive contains.
pub fn zxy_to_tile_id(z: u8, x: u32, y: u32) -> u64 {
    if z > MAX_ZOOM {
        return u64::MAX;
    }
    base(z) + xy_to_d(z, x as u64, y as u64)
}

/// PMTiles tile id → `(z, x, y)`; `None` for ids past zoom 31.
pub fn tile_id_to_zxy(id: u64) -> Option<(u8, u32, u32)> {
    let mut acc = 0u64;
    for z in 0..=MAX_ZOOM {
        let n = 1u64 << (2 * z as u64);
        if id - acc < n {
            let (x, y) = d_to_xy(z, id - acc);
            return Some((z, x as u32, y as u32));
        }
        acc += n;
    }
    None
}

/// Hilbert `xy2d` on a `2^z` grid.
fn xy_to_d(z: u8, mut x: u64, mut y: u64) -> u64 {
    let n = 1u64 << z;
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        rotate(n, &mut x, &mut y, rx, ry);
        s /= 2;
    }
    d
}

/// Hilbert `d2xy` on a `2^z` grid.
fn d_to_xy(z: u8, mut d: u64) -> (u64, u64) {
    let n = 1u64 << z;
    let (mut x, mut y) = (0u64, 0u64);
    let mut s = 1u64;
    while s < n {
        let rx = 1 & (d / 2);
        let ry = 1 & (d ^ rx);
        rotate(s, &mut x, &mut y, rx, ry);
        x += s * rx;
        y += s * ry;
        d /= 4;
        s *= 2;
    }
    (x, y)
}

#[inline]
fn rotate(n: u64, x: &mut u64, y: &mut u64, rx: u64, ry: u64) {
    if ry == 0 {
        if rx == 1 {
            *x = n.wrapping_sub(1).wrapping_sub(*x);
            *y = n.wrapping_sub(1).wrapping_sub(*y);
        }
        std::mem::swap(x, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_reference_ids() {
        assert_eq!(zxy_to_tile_id(0, 0, 0), 0);
        assert_eq!(zxy_to_tile_id(1, 0, 0), 1);
        assert_eq!(zxy_to_tile_id(1, 0, 1), 2);
        assert_eq!(zxy_to_tile_id(1, 1, 1), 3);
        assert_eq!(zxy_to_tile_id(1, 1, 0), 4);
        assert_eq!(zxy_to_tile_id(2, 0, 0), 5);
        assert_eq!(zxy_to_tile_id(32, 0, 0), u64::MAX);
    }

    #[test]
    fn round_trips_across_zooms() {
        for z in 0..=MAX_ZOOM {
            let n = 1u64 << z;
            let step = (n / 7).max(1);
            let mut x = 0;
            while x < n {
                let mut y = 0;
                while y < n {
                    let id = zxy_to_tile_id(z, x as u32, y as u32);
                    assert_eq!(tile_id_to_zxy(id), Some((z, x as u32, y as u32)), "z{z} {x},{y}");
                    y += step;
                }
                x += step;
            }
            let last = (n - 1) as u32;
            assert_eq!(tile_id_to_zxy(zxy_to_tile_id(z, last, last)), Some((z, last, last)));
        }
        assert_eq!(tile_id_to_zxy(base(32)), None);
        assert_eq!(tile_id_to_zxy(u64::MAX), None);
    }

    #[test]
    fn ids_are_a_bijection_within_a_zoom() {
        let z = 5u8;
        let n = 1u32 << z;
        let mut ids: Vec<u64> = (0..n).flat_map(|y| (0..n).map(move |x| zxy_to_tile_id(z, x, y))).collect();
        ids.sort_unstable();
        let expect: Vec<u64> = (base(z)..base(z + 1)).collect();
        assert_eq!(ids, expect);
    }
}
