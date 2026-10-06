//! `datars-render` — the display list (docs/11-rendering.md).
//!
//! `flatten(scene, theme)` walks a resolved scene with accumulated transforms, opacity and clips, and
//! produces backend-neutral ops in paint order. Inks resolve against the theme here — the latest
//! possible moment — so theme and mode switches never re-resolve scenes (docs/18-themes.md).
//! Coordinates are CSS px; backends multiply by the device pixel ratio.

mod flatten;
mod trim;

pub use flatten::{bounds, flatten, flatten_cached, FlattenCache};
pub use trim::trim_path;

use datars_color::Color;
use datars_math::{Affine, FillRule, Hash64, PathData, PathEl, Rect, Vec2};
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto, Sym};
use std::sync::Arc;

/// A path plus its content hash (resource identity for GPU caches), and whether it is all
/// straight segments (its fill doesn't depend on a curve tolerance: a GPU mesh of it serves every
/// zoom).
#[derive(Clone, Debug, PartialEq)]
pub struct SharedPath {
    pub path: Arc<PathData>,
    pub hash: u64,
    pub straight: bool,
}

impl SharedPath {
    pub fn new(path: PathData) -> SharedPath {
        let (hash, straight) = digest(&path);
        SharedPath { path: Arc::new(path), hash, straight }
    }
    pub fn from_arc(path: Arc<PathData>) -> SharedPath {
        let (hash, straight) = digest(&path);
        SharedPath { path, hash, straight }
    }
    /// A path the scene shares frame after frame (a map tile's feature is the same allocation in
    /// every frame of a camera flight): hashed once, not per frame — hashing every vertex of a
    /// street-level tile each frame cost as much as drawing it. The memo holds the allocation, so
    /// its address can't be reused by another path while the entry lives.
    ///
    /// A path nothing but the scene holds was made for this frame (a morph's in-between outline)
    /// and won't be seen again: it's hashed and not kept. Kept, a morph's outlines filled the memo
    /// within seconds and it freed them all in one frame — 30–50 ms in a browser, mid-transition.
    pub fn from_shared(path: &Arc<PathData>) -> SharedPath {
        if Arc::strong_count(path) == 1 {
            return SharedPath::from_arc(path.clone());
        }
        let key = Arc::as_ptr(path) as usize;
        let (hash, straight) = DIGESTS.with(|memo| {
            let mut memo = memo.borrow_mut();
            if let Some((p, d)) = memo.paths.get(&key) {
                if Arc::ptr_eq(p, path) {
                    return *d;
                }
            }
            let d = digest(path);
            // Every so often, the entries whose paths nothing else holds any more go (a few
            // thousand frees at most, not the whole memo at once); all-live and full, it restarts.
            if memo.paths.len() >= memo.swept + DIGESTS_SWEEP {
                memo.paths.retain(|_, (p, _)| Arc::strong_count(p) > 1);
                if memo.paths.len() >= DIGESTS_MAX {
                    memo.paths.clear();
                }
                memo.swept = memo.paths.len();
            }
            memo.paths.insert(key, (path.clone(), d));
            d
        });
        SharedPath { path: path.clone(), hash, straight }
    }
}

/// Entries in the path-digest memo past which it starts again (bounds the paths it keeps alive),
/// and new entries between sweeps of the dead ones.
const DIGESTS_MAX: usize = 16_384;
const DIGESTS_SWEEP: usize = 2_048;

struct Digests {
    paths: std::collections::BTreeMap<usize, (Arc<PathData>, (u64, bool))>,
    /// Entries after the last sweep.
    swept: usize,
}

thread_local! {
    static DIGESTS: std::cell::RefCell<Digests> = const { std::cell::RefCell::new(Digests { paths: std::collections::BTreeMap::new(), swept: 0 }) };
}

pub fn path_hash(p: &PathData) -> u64 {
    digest(p).0
}

/// The path's content hash, and whether it has no curves.
fn digest(p: &PathData) -> (u64, bool) {
    let mut h = Hash64::new();
    let mut straight = true;
    for e in &p.els {
        match *e {
            PathEl::Move { p } => {
                h.u8(0);
                h.f64(p.x);
                h.f64(p.y);
            }
            PathEl::Line { p } => {
                h.u8(1);
                h.f64(p.x);
                h.f64(p.y);
            }
            PathEl::Quad { c, p } => {
                straight = false;
                h.u8(2);
                for q in [c, p] {
                    h.f64(q.x);
                    h.f64(q.y);
                }
            }
            PathEl::Cubic { c1, c2, p } => {
                straight = false;
                h.u8(3);
                for q in [c1, c2, p] {
                    h.f64(q.x);
                    h.f64(q.y);
                }
            }
            PathEl::Close => h.u8(4),
        }
    }
    (h.finish(), straight)
}

/// A paint with colours resolved. Gradient geometry is in the op's local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum DPaint {
    Solid(Color),
    Linear { p0: Vec2, p1: Vec2, stops: Vec<(f32, Color)> },
    Radial { c: Vec2, r: f64, stops: Vec<(f32, Color)> },
}

impl DPaint {
    pub fn representative(&self) -> Color {
        match self {
            DPaint::Solid(c) => *c,
            DPaint::Linear { stops, .. } | DPaint::Radial { stops, .. } => stops.first().map(|s| s.1).unwrap_or(Color::BLACK),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    /// Final width in CSS px (non-scaling strokes already accounted for).
    pub width_px: f64,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f64,
    /// Dash pattern in CSS px.
    pub dash: Option<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Fill { path: SharedPath, xf: Affine, paint: DPaint, rule: FillRule, opacity: f32 },
    Stroke { path: SharedPath, xf: Affine, style: StrokeStyle, paint: DPaint, opacity: f32 },
    /// Positioned glyphs. `origin` is in CSS px; glyph offsets and size are multiplied by `scale`.
    /// `text`: the source string, on a text node's first run — for vector exports (SVG/PDF labels,
    /// search, selection); rasterizers ignore it.
    Glyphs { font: Sym, size: f64, glyphs: Arc<[GlyphPos]>, origin: Vec2, scale: f64, rotate: f64, color: Color, halo: Option<(Color, f64)>, text: Option<Arc<str>> },
    Instances(InstancesOp),
    Image { asset: Sym, rect: Rect, xf: Affine, opacity: f32 },
    PushClip { path: SharedPath, xf: Affine, rule: FillRule },
    PopClip,
    PushLayer { opacity: f32, blend: Blend },
    PopLayer,
}

/// Instanced marks with resolved colours. Positions go through `xf`; sizes are multiplied by
/// `size_scale` (1 for screen-sized instances, the transform's scale otherwise).
#[derive(Clone, Debug, PartialEq)]
pub struct InstancesOp {
    pub proto: Proto,
    pub xf: Affine,
    pub x: Arc<[f64]>,
    pub y: Arc<[f64]>,
    pub size: Arc<[f64]>,
    pub w: Option<Arc<[f64]>>,
    pub h: Option<Arc<[f64]>>,
    pub fill: Arc<[Color]>,
    /// Each instance's own opacity (the set's, not its groups').
    pub opacity: Arc<[f32]>,
    /// The opacity of the groups around the set, for all of it at once: a tile fading in or a
    /// level crossfading changes this, not the columns, so a renderer keeps what it uploaded.
    pub alpha: f32,
    pub stroke: Option<(Color, f64)>,
    pub size_scale: f64,
}

impl InstancesOp {
    /// Instance `i`'s opacity as drawn: its own times the groups' (`alpha`).
    pub fn opacity_at(&self, i: usize) -> f32 {
        let own = self.opacity.get(i).copied().unwrap_or(1.0);
        if self.alpha == 1.0 { own } else { own * self.alpha }
    }
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
    /// The device-space path of instance `i` (for backends without instancing).
    pub fn instance_path(&self, i: usize) -> PathData {
        let c = self.xf.apply(Vec2::new(self.x[i], self.y[i]));
        match &self.proto {
            Proto::Symbol { symbol } => {
                let s = self.size.get(i).copied().unwrap_or(3.0) * self.size_scale;
                datars_scene::Geom::Symbol { kind: *symbol, x: c.x, y: c.y, size: s }.to_path()
            }
            Proto::Rect => {
                let w = self.w.as_ref().and_then(|w| w.get(i)).copied().unwrap_or(1.0);
                let h = self.h.as_ref().and_then(|h| h.get(i)).copied().unwrap_or(1.0);
                let r = datars_scene::Geom::rect(self.x[i], self.y[i], w, h).to_path();
                r.transform(&self.xf)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayList {
    pub width: f64,
    pub height: f64,
    pub background: Color,
    pub ops: Vec<Op>,
}

impl DisplayList {
    /// A stable hash of the whole display list (P1).
    pub fn hash(&self) -> u64 {
        let mut h = Hash64::new();
        h.f64(self.width);
        h.f64(self.height);
        hash_color(&mut h, self.background);
        for op in &self.ops {
            hash_op(&mut h, op);
        }
        h.finish()
    }
}

fn hash_color(h: &mut Hash64, c: Color) {
    h.f32(c.r);
    h.f32(c.g);
    h.f32(c.b);
    h.f32(c.a);
}

/// An f32's bits, NaN canonicalized and -0 folded into +0 (as `Hash64::f32` does).
fn canon32(v: f32) -> u32 {
    if v.is_nan() {
        0x7fc0_0000
    } else if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

fn hash_xf(h: &mut Hash64, xf: &Affine) {
    for v in xf.0 {
        h.f64(v);
    }
}

fn hash_paint(h: &mut Hash64, p: &DPaint) {
    match p {
        DPaint::Solid(c) => {
            h.u8(0);
            hash_color(h, *c)
        }
        DPaint::Linear { p0, p1, stops } => {
            h.u8(1);
            for v in [p0.x, p0.y, p1.x, p1.y] {
                h.f64(v);
            }
            for (t, c) in stops {
                h.f32(*t);
                hash_color(h, *c);
            }
        }
        DPaint::Radial { c, r, stops } => {
            h.u8(2);
            for v in [c.x, c.y, *r] {
                h.f64(v);
            }
            for (t, c) in stops {
                h.f32(*t);
                hash_color(h, *c);
            }
        }
    }
}

fn hash_op(h: &mut Hash64, op: &Op) {
    match op {
        Op::Fill { path, xf, paint, rule, opacity } => {
            h.u8(1);
            h.u64(path.hash);
            hash_xf(h, xf);
            hash_paint(h, paint);
            h.u8(*rule as u8);
            h.f32(*opacity);
        }
        Op::Stroke { path, xf, style, paint, opacity } => {
            h.u8(2);
            h.u64(path.hash);
            hash_xf(h, xf);
            h.f64(style.width_px);
            h.u8(style.cap as u8);
            h.u8(style.join as u8);
            h.f64(style.miter_limit);
            match &style.dash {
                Some(d) => {
                    h.u64(d.len() as u64 + 1);
                    for v in d {
                        h.f64(*v);
                    }
                }
                None => h.u64(0),
            }
            hash_paint(h, paint);
            h.f32(*opacity);
        }
        Op::Glyphs { font, size, glyphs, origin, scale, rotate, color, halo, text } => {
            h.u8(3);
            h.str(font);
            h.str(text.as_deref().unwrap_or(""));
            h.f64(*size);
            for g in glyphs.iter() {
                h.u32(g.id as u32);
                h.f32(g.x);
                h.f32(g.y);
            }
            h.f64(origin.x);
            h.f64(origin.y);
            h.f64(*scale);
            h.f64(*rotate);
            hash_color(h, *color);
            if let Some((c, w)) = halo {
                hash_color(h, *c);
                h.f64(*w);
            }
        }
        Op::Instances(i) => {
            h.u8(4);
            hash_xf(h, &i.xf);
            match &i.proto {
                Proto::Symbol { symbol } => {
                    h.u8(0);
                    h.u8(*symbol as u8);
                }
                Proto::Rect => h.u8(1),
            }
            for col in [&i.w, &i.h] {
                match col {
                    Some(c) => {
                        h.u64(c.len() as u64 + 1);
                        for v in c.iter() {
                            h.f64(*v);
                        }
                    }
                    None => h.u64(0),
                }
            }
            if let Some((c, w)) = i.stroke {
                hash_color(h, c);
                h.f64(w);
            }
            h.u64(i.len() as u64);
            // Word at a time: a frame can hold hundreds of thousands of instances.
            let f32w = |a: f32, b: f32| (canon32(a) as u64) << 32 | canon32(b) as u64;
            for k in 0..i.len() {
                h.f64_word(i.x[k]);
                h.f64_word(i.y[k]);
                h.f64_word(i.size.get(k).copied().unwrap_or(0.0));
                if let Some(c) = i.fill.get(k) {
                    h.word(f32w(c.r, c.g));
                    h.word(f32w(c.b, c.a));
                }
                h.word(canon32(i.opacity.get(k).copied().unwrap_or(1.0)) as u64);
            }
            h.f64(i.size_scale);
        }
        Op::Image { asset, rect, xf, opacity } => {
            h.u8(5);
            h.str(asset);
            for v in [rect.x, rect.y, rect.w, rect.h] {
                h.f64(v);
            }
            hash_xf(h, xf);
            h.f32(*opacity);
        }
        Op::PushClip { path, xf, rule } => {
            h.u8(6);
            h.u64(path.hash);
            hash_xf(h, xf);
            h.u8(*rule as u8);
        }
        Op::PopClip => h.u8(7),
        Op::PushLayer { opacity, blend } => {
            h.u8(8);
            h.f32(*opacity);
            h.u8(*blend as u8);
        }
        Op::PopLayer => h.u8(9),
    }
}

/// Something that can draw a display list. Implemented by the CPU reference rasterizer, the SVG
/// writer and the wgpu renderer.
pub trait Backend {
    type Output;
    type Error;
    fn render(&mut self, list: &DisplayList, fonts: &dyn GlyphSource) -> Result<Self::Output, Self::Error>;
}

/// Glyph outlines for backends (implemented by `datars-text`'s font database). Outlines are in font
/// units scaled to `size` px, y down, origin at the glyph's baseline origin.
pub trait GlyphSource {
    fn outline(&self, font: &str, glyph: u16, size: f64) -> Option<PathData>;
}

/// A glyph source with no fonts (tests, text-free scenes).
pub struct NoGlyphs;
impl GlyphSource for NoGlyphs {
    fn outline(&self, _: &str, _: u16, _: f64) -> Option<PathData> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Vec2;

    #[test]
    fn a_shared_path_hashes_like_any_other_and_keeps_its_hash() {
        let p = PathData::polyline(&[Vec2::new(0.0, 0.0), Vec2::new(10.0, 5.0), Vec2::new(20.0, 0.0)]);
        let arc = Arc::new(p.clone());
        let tile = arc.clone(); // held elsewhere too (a tile cache): remembered
        let a = SharedPath::from_shared(&arc);
        assert_eq!(a.hash, path_hash(&p), "the same hash as computed afresh");
        assert!(Arc::ptr_eq(&a.path, &arc), "no copy");
        assert_eq!(SharedPath::from_shared(&arc).hash, a.hash, "remembered");
        let kept = |p: &Arc<PathData>| DIGESTS.with(|m| m.borrow().paths.get(&(Arc::as_ptr(p) as usize)).is_some_and(|(q, _)| Arc::ptr_eq(p, q)));
        assert!(kept(&tile));
        // Another allocation with other content never borrows a remembered hash.
        let q = Arc::new(PathData::polyline(&[Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0)]));
        let _q2 = q.clone();
        assert_eq!(SharedPath::from_shared(&q).hash, path_hash(&q));
        // Only the scene holds it (a morph's in-between outline): hashed the same, not kept.
        let fresh = Arc::new(PathData::polyline(&[Vec2::new(3.0, 1.0), Vec2::new(4.0, 7.0)]));
        assert_eq!(SharedPath::from_shared(&fresh).hash, path_hash(&fresh));
        assert!(!kept(&fresh), "a frame's own path isn't remembered");
    }
}
