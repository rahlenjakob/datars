//! Camera-aware extraction (docs/09-geo.md): which tiles a story's basemap needs, from the views
//! its cameras settle on and the flights between them.
//!
//! A view is a lon/lat box plus the tile zoom the engine draws it at — `log2(world px / 512)`,
//! rounded, the rule of `datars-engine`'s tile view. Flights are sampled along the same van Wijk–Nuij
//! path the engine flies (`datars_motion::camera`), so the zoom-out between two cities is covered at
//! the zooms it actually passes through, and nothing else is. The result is a [`Cover`]: tile
//! ranges per zoom, the only tiles an extract writes.

use datars_geo::tile::{lonlat_to_world, world_to_lonlat, MAX_LAT, TILE_SIZE};
use datars_geo::{GeoBbox, TileId};
use datars_math::{m, Vec2};
use datars_scene::Camera;
use std::collections::{BTreeMap, BTreeSet};

/// What a camera shows: a lon/lat box `[west, south, east, north]` and the (fractional) tile zoom
/// it's drawn at. `label` names settled views (a scene) in reports; flight samples leave it empty.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Extent {
    pub bbox: [f64; 4],
    pub zoom: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

fn world_box(b: [f64; 4]) -> (Vec2, Vec2) {
    let nw = lonlat_to_world(Vec2::new(b[0], b[3].min(MAX_LAT)));
    let se = lonlat_to_world(Vec2::new(b[2], b[1].max(-MAX_LAT)));
    (nw, se)
}

impl Extent {
    /// The view a `w`×`h` px Web-Mercator map shows when its camera fits `bbox` (contain, no
    /// padding — the importer's `fit` cameras): the box grown to the view's aspect, at the zoom
    /// where it just fits.
    pub fn fit(bbox: [f64; 4], w: f64, h: f64) -> Extent {
        let (nw, se) = world_box(bbox);
        let (bw, bh) = ((se.x - nw.x).max(1e-12), (se.y - nw.y).max(1e-12));
        let world_px = (w / bw).min(h / bh);
        Extent::around((nw + se) * 0.5, world_px, w, h)
    }

    /// The `w`×`h` px view centred on `lon`, `lat` at tile zoom `zoom` (the world `512·2^zoom` px
    /// across).
    pub fn centered(lon: f64, lat: f64, zoom: f64, w: f64, h: f64) -> Extent {
        Extent::around(lonlat_to_world(Vec2::new(lon, lat)), TILE_SIZE * m::exp2(zoom), w, h)
    }

    /// The view centred on world point `c` with the world `world_px` px across.
    fn around(c: Vec2, world_px: f64, w: f64, h: f64) -> Extent {
        let (hw, hh) = (w / world_px / 2.0, h / world_px / 2.0);
        let nw = world_to_lonlat(Vec2::new((c.x - hw).max(0.0), (c.y - hh).max(0.0)));
        let se = world_to_lonlat(Vec2::new((c.x + hw).min(1.0), (c.y + hh).min(1.0)));
        Extent { bbox: [nw.x, se.y, se.x, nw.y], zoom: m::log2(world_px / TILE_SIZE), label: String::new() }
    }

    /// With a label.
    pub fn named(mut self, label: impl Into<String>) -> Extent {
        self.label = label.into();
        self
    }

    /// The camera over the world unit square (content = world units, zoom = world px) — the space
    /// the flight is computed in. Mercator content is an affine image of it, and the van Wijk path
    /// doesn't depend on the scale, so this is the path the engine flies.
    fn camera(&self) -> Camera {
        let (nw, se) = world_box(self.bbox);
        let c = (nw + se) * 0.5;
        Camera { x: c.x, y: c.y, zoom: TILE_SIZE * m::exp2(self.zoom), rotation: 0.0 }
    }

    pub fn geo_bbox(&self) -> GeoBbox {
        GeoBbox::new(self.bbox[0], self.bbox[1], self.bbox[2], self.bbox[3])
    }
}

/// The views a camera passes through flying from `a` to `b` in a `w`×`h` px view: `n` samples
/// strictly between the two (the engine's path: van Wijk–Nuij for long moves, straight otherwise).
pub fn flight(a: &Extent, b: &Extent, w: f64, h: f64, n: usize) -> Vec<Extent> {
    let (ca, cb) = (a.camera(), b.camera());
    (1..=n).map(|i| {
        let c = datars_motion::camera::interpolate_camera(&ca, &cb, i as f64 / (n + 1) as f64, w);
        Extent::around(Vec2::new(c.x, c.y), c.zoom, w, h)
    }).collect()
}

/// How generously a cover is cut.
#[derive(Clone, Copy, Debug)]
pub struct CoverOptions {
    /// Zooms below and above a view's estimated zoom that may be drawn too: view sizes are
    /// estimates (titles, legends), and the engine rounds the zoom — `(0.3, 0.1)` takes the zoom
    /// below as well when the estimate is within 0.3 of rounding down.
    pub zoom_slack: (f64, f64),
    /// The same for settled views (extents with a label): a view that holds still is worth
    /// more zooms (the document drawn at another width than authored) than a passing frame.
    pub settled_slack: (f64, f64),
    /// Each view's box grown by this share of its size on every side.
    pub pad: f64,
    /// Zooms from 0 to this are covered everywhere (a coarse whole-world base: any view outside
    /// the extract still draws land, overzoomed, instead of sea).
    pub world_zoom: u8,
    /// No tiles beyond this zoom (views past it draw its tiles overzoomed).
    pub max_zoom: u8,
}

impl Default for CoverOptions {
    fn default() -> CoverOptions {
        CoverOptions { zoom_slack: (0.3, 0.1), settled_slack: (0.3, 0.1), pad: 0.08, world_zoom: 1, max_zoom: 14 }
    }
}

/// The tiles an extract writes: inclusive ranges `[z, x0, y0, x1, y1]`, disjoint, sorted.
pub type Cover = Vec<[u32; 5]>;

/// The zooms a view of fractional zoom `z` may be drawn at.
pub fn view_zooms(z: f64, o: &CoverOptions) -> std::ops::RangeInclusive<u8> {
    zooms_with(z, o.zoom_slack, o.max_zoom)
}

fn zooms_with(z: f64, slack: (f64, f64), max_zoom: u8) -> std::ops::RangeInclusive<u8> {
    let lo = (z - slack.0).round().clamp(0.0, max_zoom as f64) as u8;
    let hi = (z + slack.1).round().clamp(0.0, max_zoom as f64) as u8;
    lo..=hi
}

/// Tile ranges covering `extents` (views and flight samples), each at the zooms it may be drawn at.
pub fn cover(extents: &[Extent], o: &CoverOptions) -> Cover {
    let mut tiles: BTreeMap<u8, BTreeSet<(u32, u32)>> = BTreeMap::new();
    for z in 0..=o.world_zoom.min(o.max_zoom) {
        let n = 1u32 << z;
        tiles.entry(z).or_default().extend((0..n).flat_map(|x| (0..n).map(move |y| (x, y))));
    }
    for e in extents {
        let b = e.bbox;
        let (dx, dy) = ((b[2] - b[0]) * o.pad, (b[3] - b[1]) * o.pad);
        let grown = GeoBbox::new((b[0] - dx).max(-180.0), (b[1] - dy).max(-MAX_LAT), (b[2] + dx).min(180.0), (b[3] + dy).min(MAX_LAT));
        let slack = if e.label.is_empty() { o.zoom_slack } else { o.settled_slack };
        for z in zooms_with(e.zoom, slack, o.max_zoom) {
            tiles.entry(z).or_default().extend(datars_geo::tiles_covering(&grown, z).into_iter().map(|t| (t.x, t.y)));
        }
    }
    let mut out = Cover::new();
    for (z, set) in tiles {
        out.extend(rects(&set).into_iter().map(|(x0, y0, x1, y1)| [z as u32, x0, y0, x1, y1]));
    }
    out
}

/// A tile set as rectangles: runs along each row, then runs with the same span on consecutive rows
/// merged. Exact (no tile added or lost) and compact for the blobs views make.
fn rects(set: &BTreeSet<(u32, u32)>) -> Vec<(u32, u32, u32, u32)> {
    // Row runs, by row.
    let mut rows: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    let by_row: BTreeSet<(u32, u32)> = set.iter().map(|&(x, y)| (y, x)).collect();
    for (y, x) in by_row {
        let runs = rows.entry(y).or_default();
        match runs.last_mut() {
            Some(r) if r.1 + 1 == x => r.1 = x,
            _ => runs.push((x, x)),
        }
    }
    // Stack identical spans on consecutive rows.
    let mut open: BTreeMap<(u32, u32), (u32, u32)> = BTreeMap::new(); // span → (y0, y1)
    let mut out = Vec::new();
    for (y, runs) in rows {
        let spans: BTreeSet<(u32, u32)> = runs.into_iter().collect();
        let (keep, close): (Vec<_>, Vec<_>) = std::mem::take(&mut open).into_iter().partition(|(s, (_, y1))| *y1 + 1 == y && spans.contains(s));
        out.extend(close.into_iter().map(|((x0, x1), (y0, y1))| (x0, y0, x1, y1)));
        open = keep.into_iter().collect();
        for s in spans {
            open.entry(s).and_modify(|r| r.1 = y).or_insert((y, y));
        }
    }
    out.extend(open.into_iter().map(|((x0, x1), (y0, y1))| (x0, y0, x1, y1)));
    out.sort_by_key(|&(x0, y0, _, _)| (y0, x0));
    out
}

/// Whether a cover holds tile `t`.
pub fn covers(cover: &[[u32; 5]], t: TileId) -> bool {
    cover.iter().any(|r| r[0] == t.z as u32 && (r[1]..=r[3]).contains(&t.x) && (r[2]..=r[4]).contains(&t.y))
}

/// Tiles in a cover.
pub fn tile_count(cover: &[[u32; 5]]) -> u64 {
    cover.iter().map(|r| (r[3] - r[1] + 1) as u64 * (r[4] - r[2] + 1) as u64).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn fit_takes_the_zoom_where_the_box_just_fits() {
        // The whole Mercator square in 512×512 px: zoom 0; in 1024×1024: zoom 1.
        let world = [-180.0, -MAX_LAT, 180.0, MAX_LAT];
        assert!(close(Extent::fit(world, 512.0, 512.0).zoom, 0.0, 1e-9));
        assert!(close(Extent::fit(world, 1024.0, 1024.0).zoom, 1.0, 1e-9));
        // A wide view of a square box: the height decides, the box grows east–west.
        let e = Extent::fit([10.0, -10.0, 20.0, 0.0], 2000.0, 1000.0);
        let (nw, se) = world_box([10.0, -10.0, 20.0, 0.0]);
        assert!(close(e.zoom, m::log2(1000.0 / (se.y - nw.y) / 512.0), 1e-9));
        assert!(e.bbox[0] < 10.0 && e.bbox[2] > 20.0 && close(e.bbox[1], -10.0, 1e-9) && close(e.bbox[3], 0.0, 1e-9), "{e:?}");
    }

    #[test]
    fn centered_views_span_their_pixels() {
        // At zoom 10 the world is 512·1024 px: a 1024 px view spans 360/512 degrees of longitude.
        let e = Extent::centered(18.0, 59.3, 10.0, 1024.0, 600.0);
        assert!(close(e.bbox[2] - e.bbox[0], 360.0 / 512.0, 1e-9));
        assert!(e.bbox[1] < 59.3 && e.bbox[3] > 59.3 && close(e.zoom, 10.0, 1e-12));
        // Round trip: fitting the view's own box gives the view back.
        let f = Extent::fit(e.bbox, 1024.0, 600.0);
        assert!(close(f.zoom, 10.0, 1e-9) && (0..4).all(|i| close(f.bbox[i], e.bbox[i], 1e-9)), "{f:?}");
    }

    #[test]
    fn flights_zoom_out_between_distant_views() {
        let (w, h) = (1000.0, 600.0);
        let stockholm = Extent::centered(18.07, 59.33, 11.0, w, h);
        let lisbon = Extent::centered(-9.15, 38.7, 11.5, w, h);
        let path = flight(&stockholm, &lisbon, w, h, 31);
        assert_eq!(path.len(), 31);
        let lowest = path.iter().map(|e| e.zoom).fold(f64::MAX, f64::min);
        // Both cities fit a 600 px high view at about zoom 3.7: the path zooms out to around there,
        // seven levels below the cities.
        assert!(lowest < 5.0 && lowest > 2.0, "{lowest}");
        // It starts and ends near the cities, at their zooms.
        assert!(path[0].zoom > 9.0 && path[30].zoom > 9.0);
        assert!(path[0].geo_bbox().center().x > 10.0 && path[30].geo_bbox().center().x < -5.0);
        // A short move is a straight one: no zoom below the two ends.
        let near = Extent::centered(18.1, 59.34, 11.0, w, h);
        assert!(flight(&stockholm, &near, w, h, 9).iter().all(|e| e.zoom >= 11.0 - 1e-9));
    }

    #[test]
    fn view_zooms_take_the_neighbour_near_a_rounding_edge() {
        let o = CoverOptions::default();
        assert_eq!(view_zooms(11.0, &o), 11..=11);
        assert_eq!(view_zooms(11.6, &o), 11..=12, "11.3 rounds down: both");
        assert_eq!(view_zooms(11.45, &o), 11..=12, "11.55 rounds up: both");
        assert_eq!(view_zooms(16.2, &o), 14..=14, "clamped to the max zoom");
    }

    #[test]
    fn covers_views_at_their_zooms_and_the_world_below() {
        let o = CoverOptions::default();
        let view = Extent::centered(18.07, 59.33, 11.0, 1000.0, 600.0);
        let c = cover(std::slice::from_ref(&view), &o);
        // The whole world at z0–1 (1 + 4 tiles), the view at z11 only.
        assert!(covers(&c, TileId::new(0, 0, 0)) && covers(&c, TileId::new(1, 0, 1)));
        let zooms: BTreeSet<u32> = c.iter().map(|r| r[0]).collect();
        assert_eq!(zooms, [0, 1, 11].into_iter().collect());
        let centre = TileId::from_lonlat(Vec2::new(18.07, 59.33), 11);
        assert!(covers(&c, centre));
        // About (1000/512 + margin) × (600/512 + margin) tiles at z11: a handful, not a region.
        let z11: u64 = tile_count(&c.iter().filter(|r| r[0] == 11).copied().collect::<Vec<_>>());
        assert!((4..=16).contains(&z11), "{z11}");
        assert!(!covers(&c, TileId::from_lonlat(Vec2::new(18.07, 60.5), 11)), "not far away");
        assert!(!covers(&c, TileId::from_lonlat(Vec2::new(18.07, 59.33), 10)), "not other zooms");
    }

    #[test]
    fn rects_are_exact_and_merged() {
        // An L of tiles: rows 0–1 span x 0–2, row 2 spans x 0 only.
        let set: BTreeSet<(u32, u32)> = [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1), (0, 2)].into_iter().collect();
        let r = rects(&set);
        assert_eq!(r, vec![(0, 0, 2, 1), (0, 2, 0, 2)]);
        let back: BTreeSet<(u32, u32)> = r.iter().flat_map(|&(x0, y0, x1, y1)| (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))).collect();
        assert_eq!(back, set);
    }
}
