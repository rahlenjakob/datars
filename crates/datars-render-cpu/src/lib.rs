//! `datars-render-cpu` — the deterministic reference rasterizer (docs/11-rendering.md).
//!
//! Renders a [`DisplayList`] to an RGBA8 [`Pixmap`] whose bytes are identical on every target
//! (x86-64, arm64, wasm32): it is the anchor of the determinism contract (P1), the renderer behind
//! pixel goldens, posters and server images, and how tests and agents *see* a frame without a GPU
//! or a browser.
//!
//! How determinism is kept:
//! - Geometry is flattened (0.1 device px) and stroked in f64 using only IEEE basic operations
//!   (+, −, ×, ÷, √, floor/round) plus `datars_math::m` for round joins and caps; Rust never fuses
//!   or reorders float ops, so these are bit-identical everywhere.
//! - Coverage is computed in 24.8 fixed point with integer accumulation ([`raster`]): exact-area
//!   anti-aliasing, non-zero and even-odd fill rules.
//! - Compositing is integer arithmetic on premultiplied RGBA8 with one correctly rounded division
//!   by 255 per result; the output is converted to straight alpha at the end.
//! - Single-threaded, no hash maps, no time, no IO.
//!
//! Semantics: fills close open subpaths; strokes are built in device space with
//! `StrokeStyle::width_px × dpr` (the display list already folds the transform into the width),
//! so widths are uniform under non-uniform transforms. Gradients interpolate in premultiplied
//! sRGB and clamp outside their stops. Clips intersect as coverage masks. Layers are offscreen
//! buffers composited with opacity and Normal, Multiply or Screen blending. Glyph halos are the
//! glyph outlines stroked (width = halo width × scale, round joins) beneath the fill.
//!
//! Not supported yet: **images** (`Op::Image` draws nothing), dash offsets (the display list has
//! no field for one; [`stroke::StrokeParams::dash_offset`] is ready for it).

mod canvas;
mod paint;
mod pixel;
pub mod raster;
pub mod stroke;

use datars_math::Hash64;
use datars_render::{DisplayList, GlyphSource};

/// Largest pixmap side in pixels; larger (or non-finite) sizes are clamped.
pub const MAX_SIZE: u32 = 1 << 15;

/// An RGBA8 image, straight (non-premultiplied) alpha, row-major, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Pixmap {
    /// A PNG encoding (8-bit RGBA). An empty pixmap encodes as a 1×1 transparent image, since PNG
    /// has no zero-sized images.
    pub fn to_png(&self) -> Vec<u8> {
        let (w, h, data): (u32, u32, &[u8]) = if self.width == 0 || self.height == 0 { (1, 1, &[0, 0, 0, 0]) } else { (self.width, self.height, &self.data) };
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().expect("PNG header for a valid size");
            wr.write_image_data(data).expect("PNG data matches the header");
        }
        out
    }

    /// A stable hash of size and pixels (the golden-test identity of a frame).
    pub fn hash(&self) -> u64 {
        let mut h = Hash64::new();
        h.u32(self.width);
        h.u32(self.height);
        h.bytes(&self.data);
        h.finish()
    }

    /// The straight-alpha RGBA at (x, y); transparent outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let o = (y as usize * self.width as usize + x as usize) * 4;
        [self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]
    }
}

/// Rasterize `list` at device pixel ratio `dpr`: the pixmap is ⌈width·dpr⌉ × ⌈height·dpr⌉,
/// cleared to `list.background`, with every op drawn in order. A non-positive or non-finite `dpr`
/// is treated as 1.
pub fn render(list: &DisplayList, glyphs: &dyn GlyphSource, dpr: f64) -> Pixmap {
    let dpr = if dpr > 0.0 && dpr.is_finite() { dpr } else { 1.0 };
    let side = |v: f64| {
        let s = (v * dpr).ceil();
        if s > 0.0 {
            (s.min(MAX_SIZE as f64)) as u32
        } else {
            0 // also NaN
        }
    };
    let (w, h) = (side(list.width), side(list.height));
    let mut cx = canvas::Canvas::new(w, h, dpr, list.background, glyphs);
    if w > 0 && h > 0 {
        for op in &list.ops {
            cx.op(op);
        }
    }
    cx.finish()
}
