//! `datars-render-svg` — a display list as a standalone SVG 1.1 document (docs/11-rendering.md):
//! vector export, the no-JS fallback, and a human-readable view of a frame.
//!
//! The output is deterministic (same list → same bytes): ids come from counters prefixed with a
//! slice of the display-list hash (so several documents can be inlined in one HTML page without
//! id clashes), numbers are rounded to 3 decimals (6 for the linear part of matrices, so large
//! zooms keep their precision) with no trailing zeros, and elements are written in op order.
//!
//! Mapping (matching `datars-render-cpu` semantics):
//! - `Fill` → `<path d>` with `transform="matrix(…)"` and `fill-rule`; gradients go in `<defs>`
//!   with `gradientUnits="userSpaceOnUse"` in the path's local space.
//! - `Stroke` → `<path d>` with coordinates already transformed to CSS px, because the display
//!   list's `width_px` is a device-space width (uniform under non-uniform transforms); gradient
//!   strokes carry the op transform as `gradientTransform`.
//! - `PushClip`/`PopClip` → `<clipPath>` in `<defs>` + `<g clip-path>`; nested clips nest groups.
//! - `PushLayer`/`PopLayer` → `<g opacity>`, with `mix-blend-mode` (CSS) for Multiply/Screen.
//! - `Instances` → `<circle>`, `<rect>` or `<path>` per instance (colour and opacity each).
//! - `Glyphs` → one `<g>` per run holding a `<use>` per glyph; each distinct outline (font, glyph)
//!   is written once in `<defs>`, in integer units at 1000 per em (≈0.01 px at label sizes), and the
//!   run's group scales it — text-heavy charts repeat digits and letters constantly.
//!   A halo is the same outlines stroked (round joins) underneath. The run's source text becomes
//!   the group's `aria-label` (and `role="img"`), so the export stays readable by assistive tech.
//! - `Image` → an empty `<g data-image="…"/>` marker: images aren't supported yet.
//!
//! Clips and layers must nest properly (as `datars_render::flatten` produces them): a pop closes
//! the innermost open group of its kind and any group opened after it.

use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, PathEl, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, InstancesOp, Op, StrokeStyle};
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto, SymbolKind};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Output options. The default is an exact export; [`SvgOptions::poster`] trades invisible detail for
/// size (bundle posters are downloaded before anything else).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvgOptions {
    /// Decimals for coordinates.
    pub decimals: u32,
    /// Screen-space simplification tolerance for straight-line paths, in CSS px (0 = off). Paths
    /// are then written in CSS px with relative commands, and parts smaller than the tolerance are
    /// dropped.
    pub simplify_px: f64,
    /// Units per em of glyph outlines in `<defs>` (whole units): 1000 is ~0.01 px at label sizes;
    /// posters use 250 (under 0.1 px at title sizes, and shorter numbers).
    pub glyph_em: f64,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions { decimals: 3, simplify_px: 0.0, glyph_em: GLYPH_EM }
    }
}

impl SvgOptions {
    /// For posters at the size they're drawn — a placeholder until the runtime takes over, and the
    /// T0 chart: whole-pixel coordinates, detail below half a pixel removed, coarser glyph units (a
    /// world map's poster is 14 KB gzipped instead of 33).
    pub fn poster() -> SvgOptions {
        SvgOptions { decimals: 0, simplify_px: 0.5, glyph_em: 250.0 }
    }
}

/// Write `list` as an SVG document whose `viewBox` is the list's size in CSS px.
pub fn to_svg(list: &DisplayList, glyphs: &dyn GlyphSource) -> String {
    to_svg_with(list, glyphs, &SvgOptions::default())
}

/// [`to_svg`] with options.
pub fn to_svg_with(list: &DisplayList, glyphs: &dyn GlyphSource, opts: &SvgOptions) -> String {
    let mut w = Writer { prefix: format!("d{:07x}", list.hash() >> 36), defs: String::new(), body: String::new(), open: Vec::new(), depth: 0, next_id: 0, glyph_ids: BTreeMap::new(), opts: *opts, stroke: None };
    if list.background.a > 0.0 {
        w.line(&format!("<rect width=\"{}\" height=\"{}\"{}/>", num(list.width), num(list.height), color_attrs("fill", list.background, 1.0)));
    }
    for op in &list.ops {
        w.op(op, glyphs);
    }
    w.flush_stroke();
    while w.open.pop().is_some() {
        w.depth -= 1;
        w.line("</g>");
    }
    let mut out = String::with_capacity(w.body.len() + w.defs.len() + 256);
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" width=\"{w_}\" height=\"{h_}\" viewBox=\"0 0 {w_} {h_}\">",
        w_ = num(list.width.max(0.0)),
        h_ = num(list.height.max(0.0))
    );
    if !w.defs.is_empty() {
        out.push_str("<defs>\n");
        out.push_str(&w.defs);
        out.push_str("</defs>\n");
    }
    out.push_str(&w.body);
    out.push_str("</svg>\n");
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Clip,
    Layer,
}

struct Writer {
    prefix: String,
    defs: String,
    body: String,
    /// Open clip and layer groups, innermost last.
    open: Vec<Group>,
    /// Nesting depth of all open elements (for indentation).
    depth: usize,
    next_id: u32,
    /// Glyph outlines already in `<defs>`: (font, glyph id) → element id.
    glyph_ids: BTreeMap<(String, u16), Option<String>>,
    opts: SvgOptions,
    /// An opaque stroke not written yet — (attributes, path data) — that the next strokes with the
    /// same attributes join as subpaths: an axis's gridlines are one element, not eight.
    stroke: Option<(String, String)>,
}

impl Writer {
    fn id(&mut self, kind: char) -> String {
        self.next_id += 1;
        format!("{}-{kind}{}", self.prefix, self.next_id - 1)
    }

    /// One body line, indented by group depth.
    fn line(&mut self, s: &str) {
        for _ in 0..self.depth {
            self.body.push_str("  ");
        }
        self.body.push_str(s);
        self.body.push('\n');
    }

    fn close(&mut self, kind: Group) {
        if let Some(i) = self.open.iter().rposition(|g| *g == kind) {
            while self.open.len() > i {
                self.open.pop();
                self.depth -= 1;
                self.line("</g>");
            }
        }
    }

    /// Write the stroke waiting for companions, if any.
    fn flush_stroke(&mut self) {
        if let Some((attrs, d)) = self.stroke.take() {
            self.line(&format!("<path d=\"{d}\"{attrs}/>"));
        }
    }

    fn op(&mut self, op: &Op, glyphs: &dyn GlyphSource) {
        // Only a stroke can join the waiting one; anything else is painted after it.
        if !matches!(op, Op::Stroke { .. }) {
            self.flush_stroke();
        }
        match op {
            Op::Fill { path, xf, paint, rule, opacity } => {
                // Simplified output is in CSS px (the tolerance is a screen distance); gradients
                // keep the exact path so their local space stays meaningful.
                let simplify = self.opts.simplify_px > 0.0 && matches!(paint, DPaint::Solid(_));
                let (d, xf_attr) = if simplify {
                    (rel_d(&simplify_path(&path.path.transform(xf), self.opts.simplify_px), self.opts.decimals), String::new())
                } else {
                    (path_d(&path.path), transform_attr(xf))
                };
                if d.is_empty() {
                    return;
                }
                let fill = self.paint_attrs("fill", paint, *opacity, None);
                let rule = if *rule == FillRule::EvenOdd { " fill-rule=\"evenodd\"" } else { "" };
                self.line(&format!("<path d=\"{d}\"{xf_attr}{fill}{rule}/>"));
            }
            Op::Stroke { path, xf, style, paint, opacity } => {
                if !(style.width_px > 0.0 && style.width_px.is_finite()) {
                    return;
                }
                let device = path.path.transform(xf);
                let d = if self.opts.simplify_px > 0.0 { rel_d(&simplify_path(&device, self.opts.simplify_px), self.opts.decimals) } else { path_d(&device) };
                if d.is_empty() {
                    return;
                }
                let attrs = format!(" fill=\"none\"{}{}", self.paint_attrs("stroke", paint, *opacity, Some(xf)), stroke_attrs(style));
                // Opaque solid strokes painted one after another with the same attributes are one
                // path: overlaps of one opaque colour look the same either way, and dash patterns
                // restart at every subpath. Translucent ones would blend their overlaps differently.
                let opaque = matches!(paint, DPaint::Solid(c) if c.a >= 1.0) && *opacity >= 1.0;
                match &mut self.stroke {
                    Some((a, pending)) if opaque && *a == attrs => pending.push_str(&d),
                    _ => {
                        self.flush_stroke();
                        if opaque {
                            self.stroke = Some((attrs, d));
                        } else {
                            self.line(&format!("<path d=\"{d}\"{attrs}/>"));
                        }
                    }
                }
            }
            Op::Glyphs { font, size, glyphs: g, origin, scale, rotate, color, halo, text } => self.glyphs(glyphs, font, *size, g, *origin, *scale, *rotate, *color, *halo, text.as_deref()),
            Op::Instances(i) => self.instances(i),
            Op::Image { asset, .. } => self.line(&format!("<g data-image=\"{}\"/>", esc(asset))),
            Op::PushClip { path, xf, rule } => {
                let id = self.id('c');
                let rule = if *rule == FillRule::EvenOdd { " clip-rule=\"evenodd\"" } else { "" };
                let _ = writeln!(self.defs, "<clipPath id=\"{id}\"><path d=\"{}\"{}{rule}/></clipPath>", path_d(&path.path), transform_attr(xf));
                self.line(&format!("<g clip-path=\"url(#{id})\">"));
                self.open.push(Group::Clip);
                self.depth += 1;
            }
            Op::PopClip => self.close(Group::Clip),
            Op::PushLayer { opacity, blend } => {
                let mut s = String::from("<g");
                if *opacity < 1.0 {
                    let _ = write!(s, " opacity=\"{}\"", num(opacity.max(0.0) as f64));
                }
                match blend {
                    Blend::Normal => {}
                    Blend::Multiply => s.push_str(" style=\"mix-blend-mode:multiply\""),
                    Blend::Screen => s.push_str(" style=\"mix-blend-mode:screen\""),
                }
                s.push('>');
                self.line(&s);
                self.open.push(Group::Layer);
                self.depth += 1;
            }
            Op::PopLayer => self.close(Group::Layer),
        }
    }

    /// `fill="…"`/`stroke="…"` plus opacity for a paint; gradients are added to `<defs>`.
    /// `gradient_xf` is set when the geometry was pre-transformed (strokes).
    fn paint_attrs(&mut self, attr: &str, paint: &DPaint, opacity: f32, gradient_xf: Option<&Affine>) -> String {
        let (open, stops) = match paint {
            DPaint::Solid(c) => return color_attrs(attr, *c, opacity),
            DPaint::Linear { stops, .. } | DPaint::Radial { stops, .. } if stops.is_empty() => return format!(" {attr}=\"none\""),
            DPaint::Linear { p0, p1, stops } => {
                let id = self.id('g');
                (format!("<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"", num(p0.x), num(p0.y), num(p1.x), num(p1.y)), (id, stops, "linearGradient"))
            }
            DPaint::Radial { c, r, stops } => {
                let id = self.id('g');
                (format!("<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\"", num(c.x), num(c.y), num(r.max(0.0))), (id, stops, "radialGradient"))
            }
        };
        let (id, stops, tag) = stops;
        self.defs.push_str(&open);
        if let Some(xf) = gradient_xf.filter(|x| !x.is_identity()) {
            let _ = write!(self.defs, " gradientTransform=\"{}\"", matrix(xf));
        }
        self.defs.push_str(">\n");
        let mut last = 0.0f64;
        for (t, c) in stops {
            // SVG clamps offsets to [0, 1] and to at least the previous one, as the CPU does.
            let t = if t.is_nan() { 0.0 } else { (*t as f64).clamp(0.0, 1.0).max(last) };
            last = t;
            let _ = write!(self.defs, "  <stop offset=\"{}\" stop-color=\"{}\"", num(t), hex(*c));
            if c.a < 1.0 {
                let _ = write!(self.defs, " stop-opacity=\"{}\"", num(c.a.max(0.0) as f64));
            }
            self.defs.push_str("/>\n");
        }
        let _ = writeln!(self.defs, "</{tag}>");
        let mut s = format!(" {attr}=\"url(#{id})\"");
        if opacity < 1.0 {
            let _ = write!(s, " {attr}-opacity=\"{}\"", num(opacity.max(0.0) as f64));
        }
        s
    }

    fn instances(&mut self, op: &InstancesOp) {
        let n = op.x.len().min(op.y.len());
        let stroke = op.stroke.filter(|(_, w)| *w > 0.0 && w.is_finite());
        // Instances that draw nothing — a line's hover points are transparent with radius 0 — are
        // left out: they'd be most of a line chart's poster.
        let circle = matches!(op.proto, Proto::Symbol { symbol: SymbolKind::Circle });
        let visible = |i: usize| {
            let alpha = op.opacity_at(i);
            let fill = op.fill.get(i).or(op.fill.first()).map_or(1.0, |c| c.a);
            let inked = fill * alpha > 0.0 || stroke.is_some_and(|(c, _)| c.a * alpha > 0.0);
            inked && !(circle && op.size.get(i).copied().unwrap_or(3.0) * op.size_scale == 0.0)
        };
        if !(0..n).any(visible) {
            return;
        }
        if self.opts.simplify_px > 0.0 && n > 64 && op.stroke.is_none() && circle {
            self.dense_dots(op, n);
            return;
        }
        let mut g = String::from("<g");
        if let Some((c, w)) = stroke {
            let _ = write!(g, " stroke=\"{}\" stroke-width=\"{}\"", hex(c), num(w));
        }
        g.push('>');
        self.line(&g);
        self.depth += 1;
        for i in (0..n).filter(|&i| visible(i)) {
            let fill = op.fill.get(i).or(op.fill.first()).copied().unwrap_or(Color::BLACK);
            let alpha = op.opacity_at(i);
            let mut attrs = color_attrs("fill", fill, alpha);
            if let Some((c, _)) = stroke {
                let a = c.a * alpha.clamp(0.0, 1.0);
                if a < 1.0 {
                    let _ = write!(attrs, " stroke-opacity=\"{}\"", num(a.max(0.0) as f64));
                }
            }
            let path = op.instance_path(i);
            let el = match op.proto {
                Proto::Symbol { symbol: SymbolKind::Circle } => {
                    let c = op.xf.apply(Vec2::new(op.x[i], op.y[i]));
                    let r = op.size.get(i).copied().unwrap_or(3.0) * op.size_scale;
                    format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{attrs}/>", num(c.x), num(c.y), num(r.abs()))
                }
                Proto::Symbol { symbol: SymbolKind::Square } => rect_el(&path, &attrs),
                Proto::Rect if op.xf.0[1] == 0.0 && op.xf.0[2] == 0.0 => rect_el(&path, &attrs),
                _ => format!("<path d=\"{}\"{attrs}/>", path_d(&path)),
            };
            self.line(&el);
        }
        self.depth -= 1;
        self.line("</g>");
    }

    /// Many circles, compactly (posters): each run of consecutive dots with the same colour,
    /// opacity and radius is one path of zero-length round-capped segments (`m dx dy h0`), about
    /// ten bytes a dot instead of fifty. Runs keep the stacking order; when the styles are
    /// interleaved (runs of a dot or two), dots are grouped by style instead — at poster density
    /// the stacking between groups doesn't show. Within a path, overlapping translucent dots don't
    /// compound (fine for a poster).
    fn dense_dots(&mut self, op: &InstancesOp, n: usize) {
        let dec = self.opts.decimals;
        let scale = 10f64.powi(dec as i32);
        let q = |v: f64| if v.is_finite() { (v * scale).round() as i64 } else { 0 };
        let style = |k: usize| {
            let fill = op.fill.get(k).or(op.fill.first()).copied().unwrap_or(Color::BLACK);
            let alpha = (op.opacity_at(k) * 100.0).round() as i32;
            let r = q(op.size.get(k).copied().unwrap_or(3.0) * op.size_scale);
            (hex(fill), (fill.a * 100.0).round() as i32, alpha, r)
        };
        // Dots with no colour or no size draw nothing.
        let mut order: Vec<usize> = (0..n).filter(|&k| matches!(style(k), (_, a, o, r) if a > 0 && o > 0 && r != 0)).collect();
        let n = order.len();
        let runs = 1 + (1..n).filter(|&k| style(order[k]) != style(order[k - 1])).count();
        if runs * 8 > n {
            order.sort_by_key(|&k| style(k)); // stable: data order within each style
        }
        let mut i = 0;
        while i < n {
            let s = style(order[i]);
            let first = order[i];
            let mut j = i;
            let mut d = String::new();
            let mut cur = (0i64, 0i64);
            while j < n && style(order[j]) == s {
                let k = order[j];
                let c = op.xf.apply(Vec2::new(op.x[k], op.y[k]));
                let p = (q(c.x), q(c.y));
                let (dx, dy) = if d.is_empty() { p } else { (p.0 - cur.0, p.1 - cur.1) };
                d.push(if d.is_empty() { 'M' } else { 'm' });
                d.push_str(&fmt(dx as f64 / scale, dec));
                let ys = fmt(dy as f64 / scale, dec);
                if !ys.starts_with('-') {
                    d.push(' ');
                }
                d.push_str(&ys);
                d.push_str("h0");
                cur = p;
                j += 1;
            }
            let fill = op.fill.get(first).or(op.fill.first()).copied().unwrap_or(Color::BLACK);
            let alpha = op.opacity_at(first);
            let r = s.3 as f64 / scale;
            self.line(&format!("<path d=\"{d}\" fill=\"none\"{} stroke-width=\"{}\" stroke-linecap=\"round\"/>", color_attrs("stroke", fill, alpha), fmt(2.0 * r.abs(), dec)));
            i = j;
        }
    }

    /// The `<defs>` id of a glyph outline (at [`GLYPH_EM`] units per em, shared by every size),
    /// writing it on first use. None: no outline (a space).
    fn glyph_def(&mut self, src: &dyn GlyphSource, font: &str, id: u16) -> Option<String> {
        let key = (font.to_string(), id);
        if let Some(v) = self.glyph_ids.get(&key) {
            return v.clone();
        }
        let d = src.outline(font, id, self.opts.glyph_em).map(|p| glyph_d(&p)).filter(|d| !d.is_empty());
        let v = d.map(|d| {
            let el = self.id('g');
            let _ = writeln!(self.defs, "<path id=\"{el}\" d=\"{d}\"/>");
            el
        });
        self.glyph_ids.insert(key, v.clone());
        v
    }

    #[allow(clippy::too_many_arguments)]
    fn glyphs(&mut self, src: &dyn GlyphSource, font: &str, size: f64, glyphs: &[GlyphPos], origin: Vec2, scale: f64, rotate: f64, color: Color, halo: Option<(Color, f64)>, text: Option<&str>) {
        if !(size > 0.0 && size.is_finite()) {
            return;
        }
        // The run group scales em units to px; glyph positions are written in em units.
        let k = size / self.opts.glyph_em;
        let uses: Vec<String> = glyphs
            .iter()
            .filter_map(|g| {
                let id = self.glyph_def(src, font, g.id)?;
                let (x, y) = (fmt(g.x as f64 / k, 0), fmt(g.y as f64 / k, 0));
                let pos = match (x.as_str(), y.as_str()) {
                    ("0", "0") => String::new(),
                    ("0", _) => format!(" y=\"{y}\""),
                    (_, "0") => format!(" x=\"{x}\""),
                    _ => format!(" x=\"{x}\" y=\"{y}\""),
                };
                Some(format!("<use xlink:href=\"#{id}\"{pos}/>"))
            })
            .collect();
        if uses.is_empty() {
            return;
        }
        let mut base = Affine::translate(origin.x, origin.y);
        if rotate != 0.0 {
            base = base.mul(Affine::rotate(rotate));
        }
        let base = base.mul(Affine::scale(scale * k, scale * k));
        let head = format!("<g{}", transform_attr(&base));
        if let Some((hc, hw)) = halo.filter(|(_, w)| *w > 0.0) {
            self.line(&format!("{head} aria-hidden=\"true\" fill=\"none\"{} stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">", color_attrs("stroke", hc, 1.0), fmt(hw / k, 1)));
            self.run(&uses);
        }
        let label = text.map(|t| format!(" role=\"img\" aria-label=\"{}\"", esc(t))).unwrap_or_default();
        self.line(&format!("{head}{label}{}>", color_attrs("fill", color, 1.0)));
        self.run(&uses);
    }

    fn run(&mut self, paths: &[String]) {
        self.depth += 1;
        for p in paths {
            self.line(p);
        }
        self.depth -= 1;
        self.line("</g>");
    }
}

fn rect_el(path: &PathData, attrs: &str) -> String {
    let b = path.bounds();
    format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{attrs}/>", num(b.x), num(b.y), num(b.w.max(0.0)), num(b.h.max(0.0)))
}

/// Stroke geometry attributes, omitting SVG defaults.
fn stroke_attrs(s: &StrokeStyle) -> String {
    let mut a = format!(" stroke-width=\"{}\"", num(s.width_px));
    match s.cap {
        Cap::Butt => {}
        Cap::Round => a.push_str(" stroke-linecap=\"round\""),
        Cap::Square => a.push_str(" stroke-linecap=\"square\""),
    }
    match s.join {
        Join::Miter => {}
        Join::Round => a.push_str(" stroke-linejoin=\"round\""),
        Join::Bevel => a.push_str(" stroke-linejoin=\"bevel\""),
    }
    if s.join == Join::Miter && s.miter_limit != 4.0 && s.miter_limit.is_finite() {
        let _ = write!(a, " stroke-miterlimit=\"{}\"", num(s.miter_limit.max(1.0)));
    }
    if let Some(d) = &s.dash {
        let valid = !d.is_empty() && d.iter().all(|v| v.is_finite() && *v >= 0.0) && d.iter().any(|v| *v > 0.0);
        if valid {
            let list: Vec<String> = d.iter().map(|v| num(*v)).collect();
            let _ = write!(a, " stroke-dasharray=\"{}\"", list.join(" "));
        }
    }
    a
}

/// `fill="#rrggbb"` (or stroke) plus `fill-opacity` when not opaque.
fn color_attrs(attr: &str, c: Color, opacity: f32) -> String {
    let a = if c.a.is_nan() || opacity.is_nan() { 0.0 } else { (c.a.clamp(0.0, 1.0) * opacity.clamp(0.0, 1.0)) as f64 };
    let mut s = format!(" {attr}=\"{}\"", hex(c));
    let o = num(a);
    if o != "1" {
        let _ = write!(s, " {attr}-opacity=\"{o}\"");
    }
    s
}

/// `#rrggbb`, ignoring alpha (SVG 1.1 has no 8-digit colours; alpha goes in `*-opacity`).
fn hex(c: Color) -> String {
    Color { a: 1.0, ..c }.to_hex()
}

fn transform_attr(xf: &Affine) -> String {
    if xf.is_identity() {
        String::new()
    } else {
        format!(" transform=\"{}\"", matrix(xf))
    }
}

fn matrix(xf: &Affine) -> String {
    let [a, b, c, d, e, f] = xf.0;
    format!("matrix({} {} {} {} {} {})", fmt(a, 6), fmt(b, 6), fmt(c, 6), fmt(d, 6), num(e), num(f))
}

/// SVG path data, compact: `M1 2L3-4Z`.
pub fn path_d(p: &PathData) -> String {
    let mut d = String::new();
    let pt = |d: &mut String, cmd: char, pts: &[Vec2]| {
        d.push(cmd);
        let mut first = true;
        for q in pts {
            for v in [q.x, q.y] {
                let s = num(v);
                if !first && !s.starts_with('-') {
                    d.push(' ');
                }
                d.push_str(&s);
                first = false;
            }
        }
    };
    for e in &p.els {
        match *e {
            PathEl::Move { p } => pt(&mut d, 'M', &[p]),
            PathEl::Line { p } => pt(&mut d, 'L', &[p]),
            PathEl::Quad { c, p } => pt(&mut d, 'Q', &[c, p]),
            PathEl::Cubic { c1, c2, p } => pt(&mut d, 'C', &[c1, c2, p]),
            PathEl::Close => d.push('Z'),
        }
    }
    d
}

/// Douglas–Peucker over the straight-line subpaths of `p` with tolerance `tol`; subpaths with
/// curves pass through; closed subpaths whose box is smaller than `tol` in both directions vanish
/// (islands below a pixel).
fn simplify_path(p: &PathData, tol: f64) -> PathData {
    let mut out = PathData::new();
    let mut i = 0;
    while i < p.els.len() {
        let PathEl::Move { p: start } = p.els[i] else {
            out.els.push(p.els[i]);
            i += 1;
            continue;
        };
        let mut j = i + 1;
        let mut pts = vec![start];
        let mut lines_only = true;
        let mut closed = false;
        while j < p.els.len() {
            match p.els[j] {
                PathEl::Move { .. } => break,
                PathEl::Line { p } => pts.push(p),
                PathEl::Close => {
                    closed = true;
                    j += 1;
                    break;
                }
                _ => lines_only = false,
            }
            j += 1;
        }
        if !lines_only {
            out.els.extend_from_slice(&p.els[i..j]);
        } else {
            let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for q in &pts {
                x0 = x0.min(q.x);
                y0 = y0.min(q.y);
                x1 = x1.max(q.x);
                y1 = y1.max(q.y);
            }
            if !(closed && x1 - x0 < tol && y1 - y0 < tol) {
                let kept = douglas_peucker(&pts, tol);
                out.els.push(PathEl::Move { p: kept[0] });
                for q in &kept[1..] {
                    out.els.push(PathEl::Line { p: *q });
                }
                if closed {
                    out.els.push(PathEl::Close);
                }
            }
        }
        i = j;
    }
    out
}

fn douglas_peucker(pts: &[Vec2], tol: f64) -> Vec<Vec2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
        let len = (dx * dx + dy * dy).sqrt();
        let mut best = (0.0, a);
        for (k, q) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = if len > 0.0 { ((q.x - pa.x) * dy - (q.y - pa.y) * dx).abs() / len } else { ((q.x - pa.x).powi(2) + (q.y - pa.y).powi(2)).sqrt() };
            if d > best.0 {
                best = (d, k);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| *q).collect()
}

/// Path data with relative commands at `decimals` precision; deltas come from rounded absolute
/// positions, so errors don't accumulate.
fn rel_d(p: &PathData, decimals: u32) -> String {
    let scale = 10f64.powi(decimals as i32);
    let r = |v: f64| if v.is_finite() { (v * scale).round() as i64 } else { 0 };
    let mut d = Compact::new(move |v| fmt(v as f64 / scale, decimals));
    let (mut cur, mut start) = ((0i64, 0i64), (0i64, 0i64));
    for e in &p.els {
        match *e {
            PathEl::Move { p } => {
                let q = (r(p.x), r(p.y));
                if d.is_empty() {
                    d.cmd('M', &[q.0, q.1]);
                } else {
                    d.cmd('m', &[q.0 - cur.0, q.1 - cur.1]);
                }
                cur = q;
                start = q;
            }
            PathEl::Line { p } => {
                let q = (r(p.x), r(p.y));
                if q != cur {
                    d.line(q.0 - cur.0, q.1 - cur.1);
                    cur = q;
                }
            }
            PathEl::Quad { c, p } => {
                let (cq, q) = ((r(c.x), r(c.y)), (r(p.x), r(p.y)));
                d.cmd('q', &[cq.0 - cur.0, cq.1 - cur.1, q.0 - cur.0, q.1 - cur.1]);
                cur = q;
            }
            PathEl::Cubic { c1, c2, p } => {
                let (a, b, q) = ((r(c1.x), r(c1.y)), (r(c2.x), r(c2.y)), (r(p.x), r(p.y)));
                d.cmd('c', &[a.0 - cur.0, a.1 - cur.1, b.0 - cur.0, b.1 - cur.1, q.0 - cur.0, q.1 - cur.1]);
                cur = q;
            }
            PathEl::Close => {
                d.close();
                cur = start;
            }
        }
    }
    d.out
}

/// Relative path data in its shortest usual form: a command letter only where it changes (after
/// `m`, bare pairs are lines already), `h`/`v` for axis-aligned lines, no space before a minus.
/// The same shapes as spelling everything out, in fewer bytes — posters are mostly this.
struct Compact<F: Fn(i64) -> String> {
    out: String,
    last: Option<char>,
    num: F,
}

impl<F: Fn(i64) -> String> Compact<F> {
    fn new(num: F) -> Self {
        Compact { out: String::new(), last: None, num }
    }
    fn is_empty(&self) -> bool {
        self.out.is_empty()
    }
    fn cmd(&mut self, cmd: char, vals: &[i64]) {
        let implied = self.last == Some(cmd) || (cmd == 'l' && self.last == Some('m'));
        if !implied {
            self.out.push(cmd);
        }
        for (i, v) in vals.iter().enumerate() {
            let s = (self.num)(*v);
            if (i > 0 || implied) && !s.starts_with('-') {
                self.out.push(' ');
            }
            self.out.push_str(&s);
        }
        // After an implied line the pen is still "in" lines, not in the move.
        self.last = Some(if cmd == 'l' && self.last == Some('m') { 'm' } else { cmd });
    }
    fn line(&mut self, dx: i64, dy: i64) {
        match (dx, dy) {
            (_, 0) => self.cmd('h', &[dx]),
            (0, _) => self.cmd('v', &[dy]),
            _ => self.cmd('l', &[dx, dy]),
        }
    }
    fn close(&mut self) {
        self.out.push('z');
        self.last = None;
    }
}

/// Glyph outline path data in integer units, relative after the first point (`M474 0l-433-1236…`,
/// compact: see [`Compact`]; `t` where a quadratic's control reflects the last one):
/// outlines are defined at [`GLYPH_EM`] units per em, where a unit is ~0.01 px at label sizes, and
/// relative integers are the shortest exact-enough form (they compress well too).
fn glyph_d(p: &PathData) -> String {
    let mut d = Compact::new(|v| v.to_string());
    let mut cur = (0i64, 0i64);
    let mut start = (0i64, 0i64);
    // The last quadratic's control point: TrueType outlines imply on-curve points halfway between
    // controls, so the next control is often its exact reflection — a `t` with no control at all.
    let mut ctrl: Option<(i64, i64)> = None;
    let r = |v: f64| if v.is_finite() { v.round() as i64 } else { 0 };
    for e in &p.els {
        let was = ctrl.take();
        match *e {
            PathEl::Move { p } => {
                let q = (r(p.x), r(p.y));
                if d.is_empty() {
                    d.cmd('M', &[q.0, q.1]);
                } else {
                    d.cmd('m', &[q.0 - cur.0, q.1 - cur.1]);
                }
                cur = q;
                start = q;
            }
            PathEl::Line { p } => {
                let q = (r(p.x), r(p.y));
                d.line(q.0 - cur.0, q.1 - cur.1);
                cur = q;
            }
            PathEl::Quad { c, p } => {
                let (cq, q) = ((r(c.x), r(c.y)), (r(p.x), r(p.y)));
                if was.is_some_and(|w| (2 * cur.0 - w.0, 2 * cur.1 - w.1) == cq) {
                    d.cmd('t', &[q.0 - cur.0, q.1 - cur.1]);
                } else {
                    d.cmd('q', &[cq.0 - cur.0, cq.1 - cur.1, q.0 - cur.0, q.1 - cur.1]);
                }
                ctrl = Some(cq);
                cur = q;
            }
            PathEl::Cubic { c1, c2, p } => {
                let (a, b, q) = ((r(c1.x), r(c1.y)), (r(c2.x), r(c2.y)), (r(p.x), r(p.y)));
                d.cmd('c', &[a.0 - cur.0, a.1 - cur.1, b.0 - cur.0, b.1 - cur.1, q.0 - cur.0, q.1 - cur.1]);
                cur = q;
            }
            PathEl::Close => {
                d.close();
                cur = start;
            }
        }
    }
    d.out
}

/// Units per em of glyph definitions in `<defs>`.
pub const GLYPH_EM: f64 = 1000.0;

/// A number with at most 3 decimals and no trailing zeros (`1.5`, `-0.25`, `3`); non-finite → `0`.
pub fn num(v: f64) -> String {
    fmt(v, 3)
}

/// A number with at most `decimals` (≤ 9) decimals, no trailing zeros, no `-0`.
fn fmt(v: f64, decimals: u32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let scale = 10u64.pow(decimals) as f64;
    let r = (v * scale).round();
    if r == 0.0 {
        return "0".into();
    }
    if r.abs() >= 9.0e15 {
        return format!("{}", v.round()); // beyond exact integers: whole units are plenty
    }
    let n = r as i64;
    let a = n.unsigned_abs();
    let unit = 10u64.pow(decimals);
    let mut s = String::new();
    if n < 0 {
        s.push('-');
    }
    let _ = write!(s, "{}", a / unit);
    let frac = a % unit;
    if frac != 0 {
        let digits = format!("{:0width$}", frac, width = decimals as usize);
        s.push('.');
        s.push_str(digits.trim_end_matches('0'));
    }
    s
}

/// Escape text for XML attribute values and content.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Characters XML 1.0 forbids are dropped.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(-0.25), "-0.25");
        assert_eq!(num(0.1234), "0.123");
        assert_eq!(num(2.0005), "2.001");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(f64::NAN), "0");
        assert_eq!(num(1e20), "100000000000000000000");
        assert_eq!(fmt(0.0000125, 6), "0.000013");
        assert_eq!(num(123456.7899), "123456.79");
    }

    #[test]
    fn path_data_is_compact() {
        let mut p = PathData::new();
        p.move_to(Vec2::new(1.0, 2.0)).line_to(Vec2::new(3.0, -4.0)).quad_to(Vec2::new(5.5, 6.0), Vec2::new(-7.0, 8.0)).close();
        assert_eq!(path_d(&p), "M1 2L3-4Q5.5 6-7 8Z");
    }

    #[test]
    fn escaping() {
        assert_eq!(esc("a<b & \"c\" 'd'>"), "a&lt;b &amp; &quot;c&quot; &apos;d&apos;&gt;");
        assert_eq!(esc("x\u{1}y"), "xy");
    }
}

#[cfg(test)]
mod poster_tests {
    use super::*;

    #[test]
    fn poster_paths_drop_subpixel_detail_and_islands() {
        let mut p = PathData::new();
        // A wiggly edge: every other point 0.1 px off the line.
        p.move_to(Vec2::new(0.0, 0.0));
        for k in 1..100 {
            p.line_to(Vec2::new(k as f64, if k % 2 == 0 { 0.0 } else { 0.1 }));
        }
        p.line_to(Vec2::new(100.0, 50.0));
        p.close();
        // A 0.2 px island.
        p.move_to(Vec2::new(200.0, 200.0));
        p.line_to(Vec2::new(200.2, 200.0));
        p.line_to(Vec2::new(200.2, 200.2));
        p.close();
        let s = simplify_path(&p, 0.35);
        let lines = s.els.iter().filter(|e| matches!(e, PathEl::Line { .. })).count();
        assert!(lines <= 3, "{lines} lines left");
        assert_eq!(s.els.iter().filter(|e| matches!(e, PathEl::Move { .. })).count(), 1, "the island is gone");
        assert_eq!(rel_d(&s, 1), "M0 0l99 0.1 1 49.9z", "the corner at (99, 0.1) stays");
    }

    #[test]
    fn compact_path_data_spells_the_same_outline_shorter() {
        // A box and a TrueType-style run of quadratics whose controls reflect through the shared
        // on-curve points: `h`/`v`, one `l` for a run of lines, `t` for reflected controls.
        let mut p = PathData::new();
        p.move_to(Vec2::new(0.0, 0.0));
        p.line_to(Vec2::new(10.0, 0.0));
        p.line_to(Vec2::new(10.0, 10.0));
        p.line_to(Vec2::new(5.0, 12.0));
        p.line_to(Vec2::new(0.0, 10.0));
        p.close();
        p.move_to(Vec2::new(20.0, 0.0));
        p.quad_to(Vec2::new(25.0, -5.0), Vec2::new(30.0, 0.0));
        p.quad_to(Vec2::new(35.0, 5.0), Vec2::new(40.0, 0.0));
        p.quad_to(Vec2::new(44.0, -3.0), Vec2::new(48.0, 0.0));
        p.close();
        assert_eq!(glyph_d(&p), "M0 0h10v10l-5 2-5-2zm20 0q5-5 10 0t10 0q4-3 8 0z");
        assert_eq!(rel_d(&p, 0), "M0 0h10v10l-5 2-5-2zm20 0q5-5 10 0 5 5 10 0 4-3 8 0z");
    }
}

#[cfg(test)]
mod dense_tests {
    use super::*;
    use datars_render::DisplayList;

    #[test]
    fn poster_dots_are_runs_of_round_caps() {
        let n = 100;
        let op = InstancesOp {
            proto: Proto::Symbol { symbol: SymbolKind::Circle },
            xf: Affine::IDENTITY,
            x: (0..n).map(|i| i as f64).collect::<Vec<_>>().into(),
            y: (0..n).map(|i| (i % 7) as f64).collect::<Vec<_>>().into(),
            size: vec![2.0; n].into(),
            w: None,
            h: None,
            fill: (0..n).map(|i| if i < 60 { Color::rgb8(255, 0, 0) } else { Color::rgb8(0, 0, 255) }).collect::<Vec<_>>().into(),
            opacity: vec![1.0; n].into(),
            alpha: 1.0,
            stroke: None,
            size_scale: 1.0,
        };
        let list = DisplayList { width: 100.0, height: 10.0, background: Color::WHITE, ops: vec![datars_render::Op::Instances(op)] };
        let svg = to_svg_with(&list, &datars_render::NoGlyphs, &SvgOptions::poster());
        assert_eq!(svg.matches("<circle").count(), 0);
        assert_eq!(svg.matches("stroke-linecap=\"round\"").count(), 2, "two runs (red, then blue): {svg}");
        assert!(svg.contains("stroke-width=\"4\""), "diameter");
        assert!(svg.contains("M0 0h0m1 1h0"), "{svg}");
        // The exact export keeps circles.
        assert_eq!(to_svg(&list, &datars_render::NoGlyphs).matches("<circle").count(), n);
    }

    #[test]
    fn instances_that_draw_nothing_are_left_out() {
        // A line's hover points: transparent, radius 0 — and one visible dot among them.
        let n = 100;
        let op = InstancesOp {
            proto: Proto::Symbol { symbol: SymbolKind::Circle },
            xf: Affine::IDENTITY,
            x: (0..n).map(|i| i as f64).collect::<Vec<_>>().into(),
            y: vec![5.0; n].into(),
            size: (0..n).map(|i| if i == 7 { 3.0 } else { 0.0 }).collect::<Vec<_>>().into(),
            w: None,
            h: None,
            fill: (0..n).map(|i| if i == 7 { Color::rgb8(255, 0, 0) } else { Color::TRANSPARENT }).collect::<Vec<_>>().into(),
            opacity: vec![1.0; n].into(),
            alpha: 1.0,
            stroke: None,
            size_scale: 1.0,
        };
        let list = DisplayList { width: 100.0, height: 10.0, background: Color::WHITE, ops: vec![datars_render::Op::Instances(op.clone())] };
        assert_eq!(to_svg(&list, &datars_render::NoGlyphs).matches("<circle").count(), 1);
        let poster = to_svg_with(&list, &datars_render::NoGlyphs, &SvgOptions::poster());
        assert!(poster.contains("M7 5h0\"") && poster.matches("<path").count() == 1, "{poster}");
        let hidden = InstancesOp { size: vec![0.0; n].into(), ..op };
        let list = DisplayList { ops: vec![datars_render::Op::Instances(hidden)], ..list };
        assert!(!to_svg(&list, &datars_render::NoGlyphs).contains("<g>"), "nothing to draw, no group");
    }
}
