//! Which tiles a view shows, and at what zoom: from the transform that takes projected content
//! to the screen (camera included) and the screen rectangle the view clips to.
//!
//! The zoom is the one at which Web-Mercator tiles would appear `tile_size` px across given the
//! view's scale where it's looking: for Mercator that's exact (the world is `2πk·s` px wide); for
//! other projections it comes from the local scale at the centre of the view, so an Equal Earth
//! world map and a SWEREF city map both ask for sensibly detailed tiles.

use datars_geo::tile::{lonlat_to_world, world_to_lonlat};
use datars_geo::{Projection, TileId};
use datars_math::{m, Affine, Rect, Vec2};

/// Tiles one view may draw at once; beyond this the data zoom steps down.
const MAX_TILES: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Visible {
    /// The zoom styles are evaluated at (the camera's; may exceed the archive's max zoom).
    pub zoom: u8,
    /// The zoom tiles are fetched at (clamped to the archive).
    pub data_zoom: u8,
    /// Tiles at `data_zoom` covering the view, in (x, y) order.
    pub tiles: Vec<TileId>,
    /// World → screen for the Mercator case (exact tile squares, snapped clips).
    pub world_to_screen: Option<Affine>,
}

/// World (Web-Mercator unit square) → projected content, when the projection makes it affine.
pub(crate) fn world_to_content(proj: &Projection) -> Option<Affine> {
    proj.tile_transform(TileId::new(0, 0, 0), 1.0)
}

fn content_of_world(proj: &Projection, w: Vec2) -> Option<Vec2> {
    proj.forward(world_to_lonlat(w))
}

/// What a view shows of the world: the part of the Web-Mercator unit square it covers and the
/// fractional zoom at which a tile would span `tile_size` px there (before rounding and clamping to
/// an archive) — what an archive must hold for it (`Engine::tile_views`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Extent {
    pub world: Rect,
    pub zoom: f64,
}

pub(crate) fn extent(proj: &Projection, xf: &Affine, clip: Rect, tile_size: f64) -> Option<Extent> {
    let (world, zf, _) = world_view(proj, xf, clip, tile_size)?;
    Some(Extent { world, zoom: zf })
}

pub(crate) fn visible(proj: &Projection, xf: &Affine, clip: Rect, tile_size: f64, zooms: (u8, u8)) -> Option<Visible> {
    let (b, zf, w2s) = world_view(proj, xf, clip, tile_size)?;
    let zoom = zf.round().clamp(0.0, 24.0) as u8;
    let mut data_zoom = zoom.clamp(zooms.0, zooms.1.max(zooms.0));
    loop {
        let tiles = covering(b, data_zoom);
        if tiles.len() <= MAX_TILES || data_zoom == 0 {
            return Some(Visible { zoom, data_zoom, tiles, world_to_screen: w2s });
        }
        data_zoom -= 1;
    }
}

/// The world rectangle a view covers (inside the unit square), its fractional tile zoom, and world
/// → screen when that's affine.
fn world_view(proj: &Projection, xf: &Affine, clip: Rect, tile_size: f64) -> Option<(Rect, f64, Option<Affine>)> {
    if clip.is_empty() || clip.w <= 0.0 || clip.h <= 0.0 {
        return None;
    }
    let inv = xf.inverse()?;
    let (wbox, world_px, w2s) = match world_to_content(proj) {
        Some(w2c) => {
            let w2s = xf.mul(w2c);
            let s2w = w2s.inverse()?;
            let corners = [Vec2::new(clip.x, clip.y), Vec2::new(clip.x1(), clip.y), Vec2::new(clip.x, clip.y1()), Vec2::new(clip.x1(), clip.y1())];
            let b = corners.iter().fold(Rect::empty(), |r, c| r.include(s2w.apply(*c)));
            (b, w2s.scale_factor(), Some(w2s))
        }
        None => {
            // Sample the view on a grid; keep the points that land on the globe.
            let n = 8;
            let mut b = Rect::empty();
            let mut centre: Option<Vec2> = None;
            for i in 0..=n {
                for j in 0..=n {
                    let s = Vec2::new(clip.x + clip.w * i as f64 / n as f64, clip.y + clip.h * j as f64 / n as f64);
                    let c = inv.apply(s);
                    let Some(ll) = proj.inverse(c) else { continue };
                    // Inverse can return a point for content outside the projection's domain.
                    let Some(back) = proj.forward(ll) else { continue };
                    if (back - c).len() > 1e-6 * (1.0 + c.len()) {
                        continue;
                    }
                    b = b.include(lonlat_to_world(ll));
                    if i == n / 2 && j == n / 2 {
                        centre = Some(ll);
                    }
                }
            }
            if b.is_empty() {
                return None;
            }
            // Sampling can miss slivers between samples: grow by one cell.
            let b = b.inset(-(b.w.max(b.h) / n as f64));
            let c = centre.map(lonlat_to_world).unwrap_or(b.center());
            (b, local_world_px(proj, xf, c)?, None)
        }
    };
    let b = Rect::from_points(Vec2::new(wbox.x.max(0.0), wbox.y.max(0.0)), Vec2::new(wbox.x1().min(1.0), wbox.y1().min(1.0)));
    if b.w <= 0.0 || b.h <= 0.0 || !world_px.is_finite() || world_px <= 0.0 {
        return None;
    }
    Some((b, m::log2(world_px / tile_size.max(16.0)), w2s))
}

/// Screen px per world unit around world point `w` (geometric mean of the two axes).
fn local_world_px(proj: &Projection, xf: &Affine, w: Vec2) -> Option<f64> {
    let e = 1e-7;
    let p0 = content_of_world(proj, w)?;
    let px = content_of_world(proj, Vec2::new(w.x + e, w.y))?;
    let py = content_of_world(proj, Vec2::new(w.x, w.y + e))?;
    let dx = xf.apply_vec(px - p0).len();
    let dy = xf.apply_vec(py - p0).len();
    let v = (dx * dy).sqrt() / e;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Tiles at `z` covering a world rectangle (inside the unit square).
fn covering(b: Rect, z: u8) -> Vec<TileId> {
    let n = (1u64 << z) as f64;
    let cell = |v: f64| (v * n).floor().clamp(0.0, n - 1.0) as u32;
    // Max edges are exclusive: a view ending exactly on a tile edge doesn't take the next tile.
    let last = |v: f64| ((v * n).ceil() - 1.0).clamp(0.0, n - 1.0) as u32;
    let (x0, x1, y0, y1) = (cell(b.x), last(b.x1()).max(cell(b.x)), cell(b.y), last(b.y1()).max(cell(b.y)));
    let mut out = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push(TileId::new(z, x, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_geo::{Fit, GeoBbox};

    #[test]
    fn mercator_world_view_takes_the_world_tile() {
        // The whole world fit into 512×512: one z0 tile at 512 px.
        let p = Projection::web_mercator().fit_extent(Fit::Bbox(GeoBbox::new(-180.0, -85.0511, 180.0, 85.0511)), Rect::new(0.0, 0.0, 512.0, 512.0));
        let v = visible(&p, &Affine::IDENTITY, Rect::new(0.0, 0.0, 512.0, 512.0), 512.0, (0, 14)).unwrap();
        assert_eq!((v.zoom, v.data_zoom), (0, 0));
        assert_eq!(v.tiles, vec![TileId::new(0, 0, 0)]);
        // Zoom the camera 16× into Stockholm: z4, the 2×2 tiles around it at most.
        let s = p.forward(Vec2::new(18.07, 59.33)).unwrap();
        let cam = datars_scene::Camera { x: s.x, y: s.y, zoom: 16.0, rotation: 0.0 }.transform(Rect::new(0.0, 0.0, 512.0, 512.0));
        let v = visible(&p, &cam, Rect::new(0.0, 0.0, 512.0, 512.0), 512.0, (0, 14)).unwrap();
        assert_eq!(v.zoom, 4);
        assert!(v.tiles.contains(&TileId::from_lonlat(Vec2::new(18.07, 59.33), 4)) && v.tiles.len() <= 4, "{:?}", v.tiles);
        // Beyond the archive: fetched at its max zoom, styled at the camera's.
        let v = visible(&p, &cam, Rect::new(0.0, 0.0, 512.0, 512.0), 512.0, (0, 3)).unwrap();
        assert_eq!((v.zoom, v.data_zoom), (4, 3));
    }

    #[test]
    fn other_projections_use_the_local_scale() {
        let p = Projection::equal_earth().fit_extent(Fit::Bbox(GeoBbox::new(10.0, 55.0, 25.0, 69.0)), Rect::new(0.0, 0.0, 600.0, 600.0));
        let v = visible(&p, &Affine::IDENTITY, Rect::new(0.0, 0.0, 600.0, 600.0), 512.0, (0, 14)).unwrap();
        // About 15° of longitude across 600 px at ~62°N ≈ a z4–z5 Mercator view.
        assert!((4..=5).contains(&v.zoom), "{}", v.zoom);
        assert!(v.tiles.contains(&TileId::from_lonlat(Vec2::new(18.0, 62.0), v.data_zoom)));
        assert!(v.world_to_screen.is_none());
    }

    #[test]
    fn tile_counts_are_capped() {
        let p = Projection::web_mercator().fit_extent(Fit::Bbox(GeoBbox::new(-180.0, -85.0, 180.0, 85.0)), Rect::new(0.0, 0.0, 4000.0, 4000.0));
        // An archive starting at z6 viewed at z3: 4096 tiles would cover it; the cap steps down.
        let v = visible(&p, &Affine::IDENTITY, Rect::new(0.0, 0.0, 4000.0, 4000.0), 512.0, (6, 14)).unwrap();
        assert!(v.tiles.len() <= MAX_TILES && v.data_zoom < 6);
    }
}
