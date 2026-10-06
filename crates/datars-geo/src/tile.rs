//! The slippy-map tile pyramid and the Web-Mercator world space it lives in.
//!
//! Three spaces, and the conversions between them:
//!
//! ```text
//!   lon/lat            world                     pixels
//!  (degrees)   <->   (0..1 square)   · S   ->   (512·2^z world px)
//!                    NW = (0,0), SE = (1,1), y down
//! ```
//!
//! The unit square is the useful intermediate: tiles, cameras and clipping all live in it, so a
//! camera is a point in this space plus a zoom and the same math frames a continent and a street.
//! At integer zoom `z` the world is a `2^z × 2^z` grid of [`TileId`]s (XYZ convention: `x` east from
//! the antimeridian, `y` south from the top — OSM, MapLibre and PMTiles all agree).

use crate::geometry::GeoBbox;
use datars_math::{m, Rect, Vec2};
use serde::{Deserialize, Serialize};

/// World pixels per tile at zoom 0: the world is `512·2^z` px across at zoom `z`.
pub const TILE_SIZE: f64 = 512.0;

/// Web Mercator's latitude limit, `atan(sinh(π))` in degrees: the latitude at which the projected
/// world is exactly square. Beyond it the projection runs to infinity, so it's clamped.
pub const MAX_LAT: f64 = 85.051_128_779_806_59;

/// WGS 84 equatorial radius, the sphere radius Web Mercator uses for metres.
const EARTH_RADIUS_M: f64 = 6_378_137.0;

/// lon/lat (degrees) → Web-Mercator unit square. Latitude is clamped to ±[`MAX_LAT`] so the result
/// stays finite; longitude maps linearly (values outside ±180 land outside 0..1, which is what
/// wrapped geometry wants).
pub fn lonlat_to_world(p: Vec2) -> Vec2 {
    let x = (p.x + 180.0) / 360.0;
    let lat = p.y.clamp(-MAX_LAT, MAX_LAT).to_radians();
    let s = m::sin(lat);
    // y grows downward: north (large lat) → small y.
    let y = 0.5 - m::ln((1.0 + s) / (1.0 - s)) / (4.0 * m::PI);
    Vec2::new(x, y)
}

/// Web-Mercator unit square → lon/lat (degrees).
pub fn world_to_lonlat(p: Vec2) -> Vec2 {
    let lon = p.x * 360.0 - 180.0;
    let lat = m::atan(m::sinh(m::PI * (1.0 - 2.0 * p.y))).to_degrees();
    Vec2::new(lon, lat)
}

/// Width of the whole world in pixels at a (fractional) zoom: `512·2^zoom`.
pub fn world_size_px(zoom: f64) -> f64 {
    TILE_SIZE * m::exp2(zoom)
}

/// The zoom at which the world is `px` pixels across (inverse of [`world_size_px`]).
pub fn zoom_for_world_size(px: f64) -> f64 {
    m::log2(px / TILE_SIZE)
}

/// Ground metres covered by one pixel at latitude `lat` (degrees) and `zoom`.
pub fn meters_per_pixel(lat: f64, zoom: f64) -> f64 {
    m::TAU * EARTH_RADIUS_M * m::cos(lat.to_radians()) / world_size_px(zoom)
}

/// Degrees of longitude per pixel at `zoom`. Used to turn a pixel tolerance into a per-zoom
/// simplification tolerance for lon/lat geometry.
pub fn degrees_per_pixel(zoom: f64) -> f64 {
    360.0 / world_size_px(zoom)
}

/// The address of one tile in the pyramid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// Highest zoom whose tile coordinates fit a `u32` (`2^31` tiles per axis).
const MAX_ZOOM: u8 = 31;

impl TileId {
    pub const fn new(z: u8, x: u32, y: u32) -> TileId {
        TileId { z, x, y }
    }

    /// Tiles along one axis at zoom `z` (`2^z`), as u64 so zoom 31 doesn't overflow.
    fn axis(z: u8) -> u64 {
        1u64 << z
    }

    /// True if the zoom is addressable and `x`, `y` are inside `0..2^z`.
    pub fn is_valid(&self) -> bool {
        self.z <= MAX_ZOOM && (self.x as u64) < Self::axis(self.z) && (self.y as u64) < Self::axis(self.z)
    }

    /// The tile one level up that contains this one (`None` at zoom 0).
    pub fn parent(&self) -> Option<TileId> {
        (self.z > 0).then(|| TileId::new(self.z - 1, self.x >> 1, self.y >> 1))
    }

    /// The ancestor `levels` steps up, clamped at the zoom-0 root.
    pub fn ancestor(&self, levels: u8) -> TileId {
        let dz = levels.min(self.z);
        TileId::new(self.z - dz, self.x >> dz, self.y >> dz)
    }

    /// The four children one level down, in NW, NE, SW, SE order.
    pub fn children(&self) -> [TileId; 4] {
        let (z, x, y) = (self.z + 1, self.x << 1, self.y << 1);
        [TileId::new(z, x, y), TileId::new(z, x + 1, y), TileId::new(z, x, y + 1), TileId::new(z, x + 1, y + 1)]
    }

    /// True if `other` is this tile or one of its descendants.
    pub fn contains(&self, other: &TileId) -> bool {
        if other.z < self.z {
            return false;
        }
        let dz = other.z - self.z;
        if dz >= 32 {
            return self.z == 0;
        }
        (other.x >> dz) == self.x && (other.y >> dz) == self.y
    }

    /// The tile containing a world position (0..1 square) at zoom `z`, clamped into the grid so the
    /// exact SE corner (and anything beyond) maps to the last tile.
    pub fn from_world(p: Vec2, z: u8) -> TileId {
        let z = z.min(MAX_ZOOM);
        let n = Self::axis(z) as f64;
        let cell = |v: f64| if v.is_nan() { 0 } else { (v * n).floor().clamp(0.0, n - 1.0) as u32 };
        TileId::new(z, cell(p.x), cell(p.y))
    }

    /// The tile containing a lon/lat point at zoom `z`.
    pub fn from_lonlat(p: Vec2, z: u8) -> TileId {
        TileId::from_world(lonlat_to_world(p), z)
    }

    /// This tile's extent in world units (0..1 square, y down).
    pub fn world_bounds(&self) -> Rect {
        let n = Self::axis(self.z) as f64;
        Rect::new(self.x as f64 / n, self.y as f64 / n, 1.0 / n, 1.0 / n)
    }

    /// This tile's extent in world pixels at its own zoom (`512·2^z` space).
    pub fn pixel_bounds(&self) -> Rect {
        Rect::new(self.x as f64 * TILE_SIZE, self.y as f64 * TILE_SIZE, TILE_SIZE, TILE_SIZE)
    }

    /// This tile's lon/lat bounding box.
    pub fn lonlat_bounds(&self) -> GeoBbox {
        let r = self.world_bounds();
        // World y grows south, so the rect's top edge is the north edge.
        let nw = world_to_lonlat(Vec2::new(r.x, r.y));
        let se = world_to_lonlat(Vec2::new(r.x1(), r.y1()));
        GeoBbox::new(nw.x, se.y, se.x, nw.y)
    }

    /// The lon/lat of the tile's centre (the centre in world space, so it's inside the tile).
    pub fn center_lonlat(&self) -> Vec2 {
        world_to_lonlat(self.world_bounds().center())
    }

    /// The PMTiles v3 tile id (Hilbert order within each zoom).
    pub fn pmtiles_id(&self) -> u64 {
        crate::pmtiles::zxy_to_tile_id(self.z, self.x, self.y)
    }

    /// Inverse of [`TileId::pmtiles_id`]; `None` past the highest addressable zoom.
    pub fn from_pmtiles_id(id: u64) -> Option<TileId> {
        crate::pmtiles::tile_id_to_zxy(id).map(|(z, x, y)| TileId::new(z, x, y))
    }
}

/// Inclusive tile index range along one axis for world coordinates `[lo, hi]`. Min edges floor;
/// max edges are exclusive, so a range ending exactly on a tile edge doesn't touch the next tile
/// (but a zero-width range still yields the tile it sits in).
fn axis_range(lo: f64, hi: f64, z: u8) -> (u32, u32) {
    let n = TileId::axis(z) as f64;
    let clamp = |v: f64| if v.is_nan() { 0.0 } else { v.clamp(0.0, n - 1.0) };
    let a = clamp((lo * n).floor());
    let b = clamp((hi * n).ceil() - 1.0).max(a);
    (a as u32, b as u32)
}

fn push_range(out: &mut Vec<TileId>, z: u8, xs: (u32, u32), ys: (u32, u32)) {
    for x in xs.0..=xs.1 {
        for y in ys.0..=ys.1 {
            out.push(TileId::new(z, x, y));
        }
    }
}

/// All tiles at zoom `z` overlapping a lon/lat box. A box with `west > east` crosses the
/// antimeridian and covers both edges of the world. Sorted (`z`, `x`, `y`) and deduplicated.
pub fn tiles_covering(bbox: &GeoBbox, z: u8) -> Vec<TileId> {
    let z = z.min(MAX_ZOOM);
    let top = lonlat_to_world(Vec2::new(0.0, bbox.north.max(bbox.south))).y;
    let bottom = lonlat_to_world(Vec2::new(0.0, bbox.south.min(bbox.north))).y;
    let ys = axis_range(top, bottom, z);
    let wx = |lon: f64| (lon.clamp(-180.0, 180.0) + 180.0) / 360.0;
    let mut out = Vec::new();
    if bbox.crosses_antimeridian() {
        push_range(&mut out, z, axis_range(wx(bbox.west), 1.0, z), ys);
        push_range(&mut out, z, axis_range(0.0, wx(bbox.east), z), ys);
    } else {
        push_range(&mut out, z, axis_range(wx(bbox.west), wx(bbox.east), z), ys);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// All tiles at zoom `z` overlapping a world-space rect (0..1 units), clamped to the world.
pub fn tiles_covering_world(rect: Rect, z: u8) -> Vec<TileId> {
    let z = z.min(MAX_ZOOM);
    let mut out = Vec::new();
    push_range(&mut out, z, axis_range(rect.x, rect.x1(), z), axis_range(rect.y, rect.y1(), z));
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn null_island_and_corners() {
        let c = lonlat_to_world(Vec2::new(0.0, 0.0));
        assert!(close(c.x, 0.5, 1e-15) && close(c.y, 0.5, 1e-15));
        let nw = lonlat_to_world(Vec2::new(-180.0, MAX_LAT));
        let se = lonlat_to_world(Vec2::new(180.0, -MAX_LAT));
        assert!(close(nw.x, 0.0, 1e-12) && close(nw.y, 0.0, 1e-9), "{nw:?}");
        assert!(close(se.x, 1.0, 1e-12) && close(se.y, 1.0, 1e-9), "{se:?}");
    }

    #[test]
    fn latitude_clamps() {
        let p = lonlat_to_world(Vec2::new(10.0, 89.99));
        assert!(p.y.is_finite() && close(p.y, 0.0, 1e-9));
        let q = lonlat_to_world(Vec2::new(10.0, -90.0));
        assert!(close(q.y, 1.0, 1e-9));
    }

    #[test]
    fn round_trips() {
        for i in 0..=36 {
            for j in 0..=34 {
                let p = Vec2::new(-180.0 + i as f64 * 10.0, -85.0 + j as f64 * 5.0);
                let q = world_to_lonlat(lonlat_to_world(p));
                assert!(close(p.x, q.x, 1e-9) && close(p.y, q.y, 1e-9), "{p:?} -> {q:?}");
            }
        }
    }

    #[test]
    fn zoom_helpers() {
        assert_eq!(world_size_px(0.0), 512.0);
        assert_eq!(world_size_px(3.0), 4096.0);
        assert!(close(zoom_for_world_size(world_size_px(7.25)), 7.25, 1e-12));
        assert!(close(degrees_per_pixel(0.0), 360.0 / 512.0, 1e-15));
        // Equator, zoom 0: 40075016.686 m / 512 px.
        assert!(close(meters_per_pixel(0.0, 0.0), 78_271.517, 1e-3));
        assert!(close(meters_per_pixel(60.0, 0.0), meters_per_pixel(0.0, 0.0) / 2.0, 1e-6));
    }

    #[test]
    fn stockholm_tile() {
        let t = TileId::from_lonlat(Vec2::new(18.0686, 59.3293), 4);
        assert_eq!((t.z, t.x, t.y), (4, 8, 4));
    }

    #[test]
    fn hierarchy() {
        let t = TileId::new(5, 17, 9);
        for c in t.children() {
            assert_eq!(c.parent(), Some(t));
            assert!(t.contains(&c));
            assert!(!c.contains(&t));
        }
        assert_eq!(TileId::new(0, 0, 0).parent(), None);
        assert_eq!(TileId::new(10, 300, 400).ancestor(3), TileId::new(7, 37, 50));
        assert_eq!(TileId::new(10, 300, 400).ancestor(20), TileId::new(0, 0, 0));
        assert!(TileId::new(2, 1, 1).contains(&TileId::new(6, 16, 16)));
        assert!(!TileId::new(2, 1, 1).contains(&TileId::new(6, 0, 0)));
        assert!(TileId::new(0, 0, 0).contains(&TileId::new(31, 5, 7)));
    }

    #[test]
    fn validity() {
        assert!(TileId::new(0, 0, 0).is_valid());
        assert!(!TileId::new(0, 1, 0).is_valid());
        assert!(TileId::new(31, (1 << 31) - 1, 0).is_valid());
        assert!(!TileId::new(32, 0, 0).is_valid());
    }

    #[test]
    fn bounds_contain_center() {
        for t in [TileId::new(0, 0, 0), TileId::new(4, 9, 4), TileId::new(12, 2200, 1150), TileId::new(18, 140_000, 70_000)] {
            let c = t.center_lonlat();
            assert_eq!(TileId::from_lonlat(c, t.z), t);
            let b = t.lonlat_bounds();
            assert!(b.west < c.x && c.x < b.east && b.south < c.y && c.y < b.north, "{b:?} {c:?}");
        }
        let w = TileId::new(0, 0, 0).lonlat_bounds();
        assert!(close(w.west, -180.0, 1e-9) && close(w.east, 180.0, 1e-9));
        assert!(close(w.north, MAX_LAT, 1e-9) && close(w.south, -MAX_LAT, 1e-9));
        assert_eq!(TileId::new(3, 2, 5).pixel_bounds(), Rect::new(1024.0, 2560.0, 512.0, 512.0));
        assert_eq!(TileId::new(1, 1, 0).world_bounds(), Rect::new(0.5, 0.0, 0.5, 0.5));
    }

    #[test]
    fn covering() {
        assert_eq!(tiles_covering(&GeoBbox::new(-180.0, -85.0, 180.0, 85.0), 1).len(), 4);
        let t = TileId::new(6, 35, 18);
        let b = t.lonlat_bounds();
        assert_eq!(tiles_covering(&b, 6), vec![t]);
        assert_eq!(tiles_covering(&b, 7).len(), 4);
        assert_eq!(tiles_covering_world(t.world_bounds(), 6), vec![t]);
        assert_eq!(tiles_covering_world(Rect::new(0.0, 0.0, 1.0, 1.0), 2).len(), 16);
        // A point covers exactly one tile.
        assert_eq!(tiles_covering(&GeoBbox::new(18.0, 59.0, 18.0, 59.0), 10).len(), 1);
        // Across the antimeridian: both edges of the world, nothing in between.
        let am = tiles_covering(&GeoBbox::new(170.0, -10.0, -170.0, 10.0), 3);
        assert!(am.iter().any(|t| t.x == 0) && am.iter().any(|t| t.x == 7));
        assert!(am.iter().all(|t| t.x == 0 || t.x == 7), "{am:?}");
        assert_eq!(am.len(), 4);
        // Sorted and deduplicated.
        let mut s = am.clone();
        s.sort();
        s.dedup();
        assert_eq!(s, am);
    }
}
