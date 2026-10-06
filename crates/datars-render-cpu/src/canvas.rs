//! Executes display-list ops against a layer stack (premultiplied RGBA8 buffers) and a clip stack
//! (coverage masks). Single-threaded and allocation-light: one rasterizer and one stroke outline
//! are reused for every op.

use crate::paint::Shader;
use crate::pixel::{self, div255, unit};
use crate::raster::{IRect, Rasterizer};
use crate::stroke::{self, Outline, StrokeParams};
use crate::Pixmap;
use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, Rect, Vec2};
use datars_render::{DPaint, GlyphSource, InstancesOp, Op, StrokeStyle};
use datars_scene::{Blend, Cap, GlyphPos, Join};

/// Curve flattening tolerance in device pixels.
pub const TOLERANCE: f64 = 0.1;

struct Layer {
    buf: Vec<u8>,
    opacity: f32,
    blend: Blend,
    /// Pixels drawn into this layer so far (bounds compositing on pop).
    dirty: IRect,
}

struct Clip {
    /// Coverage per pixel (row-major, full pixmap size); empty when `bbox` is empty.
    mask: Vec<u8>,
    /// Where the mask is non-zero.
    bbox: IRect,
}

pub struct Canvas<'a> {
    w: i32,
    h: i32,
    dpr: f64,
    glyphs: &'a dyn GlyphSource,
    layers: Vec<Layer>,
    clips: Vec<Clip>,
    rast: Rasterizer,
    outline: Outline,
    /// Zeroed buffers from popped layers (RGBA) and clips (masks), reused by later pushes.
    spare_layers: Vec<Vec<u8>>,
    spare_masks: Vec<Vec<u8>>,
}

/// Zero the rows of `buf` (row-major, `bpp` bytes per pixel, `w` pixels wide) inside `r`.
fn clear_rect(buf: &mut [u8], w: usize, bpp: usize, r: IRect) {
    if r.is_empty() {
        return;
    }
    for y in r.y0 as usize..r.y1 as usize {
        buf[(y * w + r.x0 as usize) * bpp..(y * w + r.x1 as usize) * bpp].fill(0);
    }
}

impl<'a> Canvas<'a> {
    pub fn new(w: u32, h: u32, dpr: f64, background: Color, glyphs: &'a dyn GlyphSource) -> Canvas<'a> {
        let bg = pixel::premul(background, 1.0);
        let mut buf = vec![0u8; w as usize * h as usize * 4];
        if bg != [0, 0, 0, 0] {
            for px in buf.chunks_mut(4) {
                px.copy_from_slice(&bg);
            }
        }
        let full = IRect::new(0, 0, w as i32, h as i32);
        Canvas {
            w: w as i32,
            h: h as i32,
            dpr,
            glyphs,
            layers: vec![Layer { buf, opacity: 1.0, blend: Blend::Normal, dirty: full }],
            clips: Vec::new(),
            rast: Rasterizer::new(w, h),
            outline: Outline::default(),
            spare_layers: Vec::new(),
            spare_masks: Vec::new(),
        }
    }

    pub fn op(&mut self, op: &Op) {
        match op {
            Op::Fill { path, xf, paint, rule, opacity } => self.fill(&path.path, xf, paint, *rule, *opacity),
            Op::Stroke { path, xf, style, paint, opacity } => self.stroke(&path.path, xf, style, paint, *opacity),
            Op::Glyphs { font, size, glyphs, origin, scale, rotate, color, halo, .. } => {
                self.glyphs(font, *size, glyphs, *origin, *scale, *rotate, *color, *halo)
            }
            Op::Instances(i) => self.instances(i),
            // Images are not supported yet: they draw nothing (documented in the crate docs).
            Op::Image { .. } => {}
            Op::PushClip { path, xf, rule } => self.push_clip(&path.path, xf, *rule),
            Op::PopClip => self.pop_clip(),
            Op::PushLayer { opacity, blend } => self.push_layer(*opacity, *blend),
            Op::PopLayer => self.pop_layer(),
        }
    }

    /// Composite any unbalanced layers and convert to straight alpha.
    pub fn finish(mut self) -> Pixmap {
        while self.layers.len() > 1 {
            self.pop_layer();
        }
        let mut data = self.layers.pop().map(|l| l.buf).unwrap_or_default();
        for px in data.chunks_mut(4) {
            let s = pixel::unpremul(px);
            px.copy_from_slice(&s);
        }
        Pixmap { width: self.w as u32, height: self.h as u32, data }
    }

    /// Local → device: the op transform, then the device pixel ratio.
    fn device(&self, xf: &Affine) -> Affine {
        Affine::scale(self.dpr, self.dpr).mul(*xf)
    }

    fn clip_rect(&self) -> IRect {
        match self.clips.last() {
            Some(c) => c.bbox,
            None => IRect::new(0, 0, self.w, self.h),
        }
    }

    /// Whether device-space bounds `b`, grown by `pad`, can touch the current clip.
    fn visible(&self, b: Rect, pad: f64) -> bool {
        let c = self.clip_rect();
        if c.is_empty() || b.is_empty() {
            return false;
        }
        !(b.x - pad >= c.x1 as f64 || b.y - pad >= c.y1 as f64 || b.x1() + pad <= c.x0 as f64 || b.y1() + pad <= c.y0 as f64)
    }

    /// `path` in device space, flattened — or `None` if it can't touch the clip even grown by
    /// `pad` px.
    fn flatten_visible(&self, path: &PathData, dev: &Affine, pad: f64) -> Option<Vec<(Vec<Vec2>, bool)>> {
        let moved;
        let p = if dev.is_identity() {
            path
        } else {
            moved = path.transform(dev);
            &moved
        };
        self.visible(p.bounds(), pad).then(|| p.flatten(TOLERANCE))
    }

    fn fill(&mut self, path: &PathData, xf: &Affine, paint: &DPaint, rule: FillRule, opacity: f32) {
        let dev = self.device(xf);
        let Some(shader) = Shader::new(paint, &dev, opacity) else { return };
        let Some(polys) = self.flatten_visible(path, &dev, 1.0) else { return };
        self.fill_polys(&polys, rule, &shader);
    }

    fn fill_polys(&mut self, polys: &[(Vec<Vec2>, bool)], rule: FillRule, shader: &Shader) {
        self.rast.reset();
        for (pts, _) in polys {
            self.rast.polygon(pts); // fills close open subpaths
        }
        self.paint(rule, shader);
    }

    /// Device-space stroke parameters; `None` if the stroke is invisible (zero or bad width).
    fn stroke_params(&self, style: &StrokeStyle) -> Option<StrokeParams> {
        let k = self.dpr;
        let params = StrokeParams {
            width: style.width_px * k,
            cap: style.cap,
            join: style.join,
            miter_limit: style.miter_limit,
            dash: style.dash.as_ref().map(|d| d.iter().map(|v| v * k).collect()),
            dash_offset: 0.0,
            tolerance: TOLERANCE,
        };
        (params.width > 0.0 && params.width.is_finite()).then_some(params)
    }

    /// How far a stroke can reach beyond its path (miters, square caps), plus a pixel.
    fn stroke_pad(params: &StrokeParams) -> f64 {
        params.width * 0.5 * params.miter_limit.max(1.5) + 1.0
    }

    fn stroke(&mut self, path: &PathData, xf: &Affine, style: &StrokeStyle, paint: &DPaint, opacity: f32) {
        let dev = self.device(xf);
        let Some(shader) = Shader::new(paint, &dev, opacity) else { return };
        let Some(params) = self.stroke_params(style) else { return };
        let Some(polys) = self.flatten_visible(path, &dev, Self::stroke_pad(&params)) else { return };
        self.stroke_polys(&polys, &params, &shader);
    }

    fn stroke_polys(&mut self, polys: &[(Vec<Vec2>, bool)], params: &StrokeParams, shader: &Shader) {
        self.outline.clear();
        stroke::stroke(polys, params, &mut self.outline);
        self.rast.reset();
        for c in self.outline.contours() {
            self.rast.polygon(c);
        }
        self.paint(FillRule::NonZero, shader);
    }

    /// Composite the rasterizer's coverage, through the clip, with `shader` onto the top layer.
    fn paint(&mut self, rule: FillRule, shader: &Shader) {
        let clip = self.clip_rect();
        if clip.is_empty() || self.rast.is_empty() {
            return;
        }
        let w = self.w as usize;
        let Some(layer) = self.layers.last_mut() else { return };
        let mask = self.clips.last().map(|c| c.mask.as_slice());
        let mut dirty = IRect::EMPTY;
        let buf = &mut layer.buf;
        self.rast.fill(rule, clip, |y, x0, cov| {
            blit(buf, w, y, x0, cov, mask, shader);
            dirty = dirty.union(&IRect::new(x0, y, x0 + cov.len() as i32, y + 1));
        });
        layer.dirty = layer.dirty.union(&dirty);
    }

    fn push_clip(&mut self, path: &PathData, xf: &Affine, rule: FillRule) {
        let parent = self.clip_rect();
        let dev = self.device(xf);
        self.rast.reset();
        for (pts, _) in path.transform(&dev).flatten(TOLERANCE) {
            self.rast.polygon(&pts);
        }
        let mut clip = Clip { mask: Vec::new(), bbox: IRect::EMPTY };
        if !parent.is_empty() && !self.rast.is_empty() {
            let w = self.w as usize;
            let mut mask = self.spare_masks.pop().unwrap_or_else(|| vec![0u8; w * self.h as usize]);
            let pm = self.clips.last().map(|c| c.mask.as_slice());
            let mut bbox = IRect::EMPTY;
            self.rast.fill(rule, parent, |y, x0, cov| {
                let row = y as usize * w;
                let (mut lo, mut hi) = (usize::MAX, 0);
                for (i, &c) in cov.iter().enumerate() {
                    if c == 0 {
                        continue;
                    }
                    let x = x0 as usize + i;
                    let v = match pm {
                        Some(p) => div255(c as u32 * p[row + x] as u32) as u8,
                        None => c,
                    };
                    if v != 0 {
                        mask[row + x] = v;
                        lo = lo.min(x);
                        hi = x;
                    }
                }
                if lo <= hi {
                    bbox = bbox.union(&IRect::new(lo as i32, y, hi as i32 + 1, y + 1));
                }
            });
            if bbox.is_empty() {
                self.spare_masks.push(mask); // nothing was written: still zero
            } else {
                clip = Clip { mask, bbox };
            }
        }
        self.clips.push(clip);
    }

    fn pop_clip(&mut self) {
        if let Some(mut c) = self.clips.pop() {
            if !c.mask.is_empty() {
                clear_rect(&mut c.mask, self.w as usize, 1, c.bbox);
                self.spare_masks.push(c.mask);
            }
        }
    }

    fn push_layer(&mut self, opacity: f32, blend: Blend) {
        let n = self.w as usize * self.h as usize * 4;
        let buf = self.spare_layers.pop().unwrap_or_else(|| vec![0u8; n]);
        self.layers.push(Layer { buf, opacity, blend, dirty: IRect::EMPTY });
    }

    fn pop_layer(&mut self) {
        if self.layers.len() < 2 {
            return; // unbalanced PopLayer: ignore
        }
        let Some(mut top) = self.layers.pop() else { return };
        let w = self.w as usize;
        self.composite(&top);
        clear_rect(&mut top.buf, w, 4, top.dirty);
        self.spare_layers.push(top.buf);
    }

    /// Blend a popped layer's drawn region onto the layer below it.
    fn composite(&mut self, top: &Layer) {
        let Some(parent) = self.layers.last_mut() else { return };
        let k = (unit(top.opacity) * 255.0 + 0.5).floor() as u32;
        let r = top.dirty;
        if k == 0 || r.is_empty() {
            return;
        }
        let w = self.w as usize;
        for y in r.y0..r.y1 {
            let row = y as usize * w;
            for x in r.x0 as usize..r.x1 as usize {
                let o = (row + x) * 4;
                let s = [top.buf[o], top.buf[o + 1], top.buf[o + 2], top.buf[o + 3]];
                if s[3] == 0 {
                    continue;
                }
                let s = if k == 255 { s } else { pixel::scale(s, k) };
                pixel::blend(&mut parent.buf[o..o + 4], s, top.blend);
            }
        }
        parent.dirty = parent.dirty.union(&r);
    }

    #[allow(clippy::too_many_arguments)]
    fn glyphs(&mut self, font: &str, size: f64, glyphs: &[GlyphPos], origin: Vec2, scale: f64, rotate: f64, color: Color, halo: Option<(Color, f64)>) {
        // origin + rotate(scale · (glyph offset + outline))
        let mut base = Affine::translate(origin.x, origin.y);
        if rotate != 0.0 {
            base = base.mul(Affine::rotate(rotate));
        }
        let base = base.mul(Affine::scale(scale, scale));
        let mut path = PathData::new();
        for g in glyphs {
            if let Some(o) = self.glyphs.outline(font, g.id, size) {
                path.extend(&o.transform(&base.mul(Affine::translate(g.x as f64, g.y as f64))));
            }
        }
        if path.is_empty() {
            return;
        }
        if let Some((hc, hw)) = halo {
            if hw > 0.0 {
                let style = StrokeStyle { width_px: hw * scale.abs(), cap: Cap::Round, join: Join::Round, miter_limit: 4.0, dash: None };
                self.stroke(&path, &Affine::IDENTITY, &style, &DPaint::Solid(hc), 1.0);
            }
        }
        self.fill(&path, &Affine::IDENTITY, &DPaint::Solid(color), FillRule::NonZero, 1.0);
    }

    /// Instances draw exactly as their `instance_path` would: fill, then the optional stroke, one
    /// instance at a time in order (so later instances cover earlier ones' strokes).
    fn instances(&mut self, op: &InstancesOp) {
        // Same steps as `fill` + `stroke` of each instance path, but flattened once per instance.
        let dev = self.device(&Affine::IDENTITY);
        let stroke = op.stroke.and_then(|(c, w)| {
            let style = StrokeStyle { width_px: w, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None };
            self.stroke_params(&style).map(|p| (c, p))
        });
        let pad = stroke.as_ref().map_or(1.0, |(_, p)| Self::stroke_pad(p));
        let n = op.x.len().min(op.y.len());
        for i in 0..n {
            let fill = op.fill.get(i).or(op.fill.first()).copied().unwrap_or(Color::BLACK);
            let alpha = op.opacity_at(i);
            let Some(polys) = self.flatten_visible(&op.instance_path(i), &dev, pad) else { continue };
            if let Some(shader) = Shader::new(&DPaint::Solid(fill), &dev, alpha) {
                self.fill_polys(&polys, FillRule::NonZero, &shader);
            }
            if let Some((c, params)) = &stroke {
                if let Some(shader) = Shader::new(&DPaint::Solid(*c), &dev, alpha) {
                    self.stroke_polys(&polys, params, &shader);
                }
            }
        }
    }
}

/// Source-over one row of coverage through the optional clip mask.
#[inline]
fn blit(buf: &mut [u8], w: usize, y: i32, x0: i32, cov: &[u8], mask: Option<&[u8]>, shader: &Shader) {
    let row = y as usize * w;
    let solid = match shader {
        Shader::Solid(p) => Some(*p),
        _ => None,
    };
    for (i, &c) in cov.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let x = x0 as usize + i;
        let mut k = c as u32;
        if let Some(m) = mask {
            let mv = m[row + x] as u32;
            if mv == 0 {
                continue;
            }
            if mv != 255 {
                k = div255(k * mv);
                if k == 0 {
                    continue;
                }
            }
        }
        let src = match solid {
            Some(p) => p,
            None => shader.at(x as i32, y),
        };
        let src = if k == 255 { src } else { pixel::scale(src, k) };
        let o = (row + x) * 4;
        pixel::over(&mut buf[o..o + 4], src);
    }
}
