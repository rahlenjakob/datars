//! `datars-render-pdf` — a display list as one vector PDF page, for print and reports (the
//! "static" outputs next to PNG and SVG).
//!
//! - Geometry is written in CSS px under a top-left, y-down page transform; the page is sized in
//!   points at 72 dpi-equivalent (1 CSS px = 0.75 pt), so print sizes match the browser's.
//! - Text is drawn as glyph outlines (the engine's own layout, identical to every other backend);
//!   each run's source string is also written as invisible text, so the PDF is searchable and
//!   copyable (Latin-1 text; other scripts stay outlines only).
//! - Layers (`PushLayer`) become transparency groups, composited with their opacity and blend
//!   mode — the same compositing the CPU reference does; clips nest with `q … W n … Q`.
//! - Gradients become axial/radial shadings; instances expand to their paths.
//!
//! Deterministic: fixed number formatting, objects in creation order, compressed with fixed
//! settings — the same list gives the same bytes on every machine.

use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, PathEl, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, Op, StrokeStyle};
use datars_scene::{Blend, Cap, Join};
use std::fmt::Write as _;
use std::io::Write as _;

/// 1 CSS px in PDF points.
const PT: f64 = 0.75;

/// Numbers with at most 3 decimals, no trailing zeros.
fn n(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

struct Writer<'a> {
    glyphs: &'a dyn GlyphSource,
    /// Content streams: the page, then one per open transparency group.
    stack: Vec<String>,
    /// Finished objects (body, in id order from 1).
    objects: Vec<Vec<u8>>,
    /// Graphics states by (fill alpha, stroke alpha, blend) → name.
    gstates: Vec<((u32, u32, Blend), String)>,
    patterns: Vec<(String, usize)>,
    xobjects: Vec<(String, usize)>,
    /// Open layers: (opacity, blend).
    layers: Vec<(f32, Blend)>,
    width: f64,
    height: f64,
    uses_text: bool,
}

impl Writer<'_> {
    fn out(&mut self) -> &mut String {
        self.stack.last_mut().expect("a content stream")
    }

    fn object(&mut self, body: Vec<u8>) -> usize {
        self.objects.push(body);
        self.objects.len() // ids start at 1
    }

    /// A reserved id whose body is filled in later.
    fn reserve(&mut self) -> usize {
        self.object(Vec::new())
    }

    fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
        let _ = z.write_all(data);
        let packed = z.finish().unwrap_or_default();
        let mut out = format!("<< {dict} /Filter /FlateDecode /Length {} >>\nstream\n", packed.len()).into_bytes();
        out.extend_from_slice(&packed);
        out.extend_from_slice(b"\nendstream");
        out
    }

    fn gstate(&mut self, fill: f32, stroke: f32, blend: Blend) -> String {
        let key = ((fill.clamp(0.0, 1.0) * 1000.0).round() as u32, (stroke.clamp(0.0, 1.0) * 1000.0).round() as u32, blend);
        if let Some((_, name)) = self.gstates.iter().find(|(k, _)| *k == key) {
            return name.clone();
        }
        let name = format!("G{}", self.gstates.len());
        self.gstates.push((key, name.clone()));
        name
    }

    /// Set the fill (or stroke) colour and its alpha for the next painting operator.
    fn paint(&mut self, paint: &DPaint, opacity: f32, stroke: bool) {
        match paint {
            DPaint::Solid(c) => {
                let a = c.a * opacity;
                let (fa, sa) = if stroke { (1.0, a) } else { (a, 1.0) };
                if a < 1.0 {
                    let g = self.gstate(fa, sa, Blend::Normal);
                    let _ = writeln!(self.out(), "/{g} gs");
                }
                let op = if stroke { "RG" } else { "rg" };
                let _ = writeln!(self.out(), "{} {} {} {op}", n(c.r as f64), n(c.g as f64), n(c.b as f64));
            }
            DPaint::Linear { .. } | DPaint::Radial { .. } => {
                let p = self.shading(paint);
                if opacity < 1.0 {
                    let g = if stroke { self.gstate(1.0, opacity, Blend::Normal) } else { self.gstate(opacity, 1.0, Blend::Normal) };
                    let _ = writeln!(self.out(), "/{g} gs");
                }
                let (cs, sc) = if stroke { ("CS", "SCN") } else { ("cs", "scn") };
                let _ = writeln!(self.out(), "/Pattern {cs} /{p} {sc}");
            }
        }
    }

    /// A shading pattern for a gradient (stops stitched from linear segments; stop alphas are
    /// dropped — PDF shadings are opaque).
    fn shading(&mut self, paint: &DPaint) -> String {
        let (kind, coords, stops) = match paint {
            DPaint::Linear { p0, p1, stops } => (2, format!("{} {} {} {}", n(p0.x), n(p0.y), n(p1.x), n(p1.y)), stops),
            DPaint::Radial { c, r, stops } => (3, format!("{} {} 0 {} {} {}", n(c.x), n(c.y), n(c.x), n(c.y), n(*r)), stops),
            DPaint::Solid(_) => unreachable!("solid paints need no shading"),
        };
        let rgb = |c: &Color| format!("{} {} {}", n(c.r as f64), n(c.g as f64), n(c.b as f64));
        let function = if stops.len() < 2 {
            let c = stops.first().map(|s| s.1).unwrap_or(Color::BLACK);
            format!("<< /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [{}] /N 1 >>", rgb(&c), rgb(&c))
        } else {
            let segs: Vec<String> = stops.windows(2).map(|w| format!("<< /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [{}] /N 1 >>", rgb(&w[0].1), rgb(&w[1].1))).collect();
            let bounds: Vec<String> = stops[1..stops.len() - 1].iter().map(|s| n(s.0 as f64)).collect();
            let encode: Vec<&str> = segs.iter().map(|_| "0 1").collect();
            format!("<< /FunctionType 3 /Domain [{} {}] /Functions [{}] /Bounds [{}] /Encode [{}] >>", n(stops[0].0 as f64), n(stops[stops.len() - 1].0 as f64), segs.join(" "), bounds.join(" "), encode.join(" "))
        };
        // The pattern lives in page space: our content runs under the flip, so the pattern
        // matrix applies it too.
        let body = format!("<< /PatternType 2 /Matrix [{pt} 0 0 {npt} 0 {h}] /Shading << /ShadingType {kind} /ColorSpace /DeviceRGB /Coords [{coords}] /Function {function} /Extend [true true] >> >>", pt = n(PT), npt = n(-PT), h = n(self.height * PT));
        let id = self.object(body.into_bytes());
        let name = format!("P{}", self.patterns.len());
        self.patterns.push((name.clone(), id));
        name
    }

    fn path(&mut self, p: &PathData) {
        let mut cur = Vec2::ZERO;
        let mut s = String::new();
        for e in &p.els {
            match *e {
                PathEl::Move { p } => {
                    let _ = writeln!(s, "{} {} m", n(p.x), n(p.y));
                    cur = p;
                }
                PathEl::Line { p } => {
                    let _ = writeln!(s, "{} {} l", n(p.x), n(p.y));
                    cur = p;
                }
                PathEl::Quad { c, p } => {
                    let (c1, c2) = (cur + (c - cur) * (2.0 / 3.0), p + (c - p) * (2.0 / 3.0));
                    let _ = writeln!(s, "{} {} {} {} {} {} c", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(p.x), n(p.y));
                    cur = p;
                }
                PathEl::Cubic { c1, c2, p } => {
                    let _ = writeln!(s, "{} {} {} {} {} {} c", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(p.x), n(p.y));
                    cur = p;
                }
                PathEl::Close => s.push_str("h\n"),
            }
        }
        self.out().push_str(&s);
    }

    fn fill(&mut self, p: &PathData, paint: &DPaint, rule: FillRule, opacity: f32) {
        if p.els.is_empty() {
            return;
        }
        self.out().push_str("q\n");
        self.paint(paint, opacity, false);
        self.path(p);
        self.out().push_str(if rule == FillRule::EvenOdd { "f*\nQ\n" } else { "f\nQ\n" });
    }

    fn stroke(&mut self, p: &PathData, style: &StrokeStyle, paint: &DPaint, opacity: f32) {
        if p.els.is_empty() || style.width_px <= 0.0 {
            return;
        }
        self.out().push_str("q\n");
        self.paint(paint, opacity, true);
        let cap = match style.cap {
            Cap::Butt => 0,
            Cap::Round => 1,
            Cap::Square => 2,
        };
        let join = match style.join {
            Join::Miter => 0,
            Join::Round => 1,
            Join::Bevel => 2,
        };
        let _ = writeln!(self.out(), "{} w {cap} J {join} j {} M", n(style.width_px), n(style.miter_limit));
        if let Some(d) = style.dash.as_ref().filter(|d| !d.is_empty()) {
            let _ = writeln!(self.out(), "[{}] 0 d", d.iter().map(|x| n(*x)).collect::<Vec<_>>().join(" "));
        }
        self.path(p);
        self.out().push_str("S\nQ\n");
    }

    #[allow(clippy::too_many_arguments)]
    fn glyphs(&mut self, font: &str, size: f64, glyphs: &[datars_scene::GlyphPos], origin: Vec2, scale: f64, rotate: f64, color: Color, halo: Option<(Color, f64)>, text: Option<&str>) {
        let mut base = Affine::translate(origin.x, origin.y);
        if rotate != 0.0 {
            base = base.mul(Affine::rotate(rotate));
        }
        let base = base.mul(Affine::scale(scale, scale));
        let mut path = PathData::new();
        for g in glyphs {
            if let Some(o) = self.glyphs.outline(font, g.id, size) {
                path.els.extend(o.transform(&base.mul(Affine::translate(g.x as f64, g.y as f64))).els);
            }
        }
        if let Some((hc, hw)) = halo.filter(|h| h.1 > 0.0) {
            let style = StrokeStyle { width_px: hw * scale.abs(), cap: Cap::Round, join: Join::Round, miter_limit: 4.0, dash: None };
            self.stroke(&path, &style, &DPaint::Solid(hc), 1.0);
        }
        self.fill(&path, &DPaint::Solid(color), FillRule::NonZero, 1.0);
        // Invisible text over the outlines: search, copy, screen readers of the PDF.
        if let Some(t) = text.filter(|t| !t.trim().is_empty()) {
            let latin: String = t.chars().filter_map(|c| if (c as u32) < 256 { Some(c) } else { None }).collect();
            if !latin.trim().is_empty() {
                self.uses_text = true;
                let esc: String = latin.chars().map(|c| match c {
                    '(' | ')' | '\\' => format!("\\{c}"),
                    c if (c as u32) < 32 || (c as u32) > 126 => format!("\\{:03o}", c as u32),
                    c => c.to_string(),
                }).collect();
                // Under the page flip, text needs its own flip back (a -1 y scale in Tm).
                let fs = size * scale;
                let _ = writeln!(self.out(), "BT 3 Tr /F1 {} Tf 1 0 0 -1 {} {} Tm ({esc}) Tj ET", n(fs), n(origin.x), n(origin.y));
            }
        }
    }

    fn push_layer(&mut self, opacity: f32, blend: Blend) {
        self.layers.push((opacity, blend));
        self.stack.push(String::new());
    }

    fn pop_layer(&mut self) {
        let (Some((opacity, blend)), Some(content)) = (self.layers.pop(), self.stack.pop()) else { return };
        if self.stack.is_empty() {
            self.stack.push(content); // unbalanced list: keep drawing on the page
            return;
        }
        // A transparency group in page space (the flip is inside, like the page's own content).
        let (w, h) = (self.width * PT, self.height * PT);
        let dict = format!("/Type /XObject /Subtype /Form /BBox [0 0 {} {}] /Group << /S /Transparency /CS /DeviceRGB >> /Resources RES", n(w), n(h));
        let flip = format!("{} 0 0 {} 0 {} cm\n", n(PT), n(-PT), n(h));
        let id = self.object(Vec::new());
        // Resources are shared (the page's dictionary), patched in when the file is written.
        self.objects[id - 1] = format!("__STREAM__{}\n{}", dict, flip + &content).into_bytes();
        let name = format!("X{}", self.xobjects.len());
        self.xobjects.push((name.clone(), id));
        let g = self.gstate(opacity, opacity, blend);
        let _ = writeln!(self.out(), "q /{g} gs");
        // The form draws in page space; undo the flip for the Do (and restore after).
        let height = self.height;
        let _ = writeln!(self.out(), "{} 0 0 {} 0 {} cm /{name} Do Q", n(1.0 / PT), n(-1.0 / PT), n(height));
    }
}

/// The display list as a one-page PDF.
pub fn to_pdf(list: &DisplayList, glyphs: &dyn GlyphSource) -> Vec<u8> {
    to_pdf_pages(std::slice::from_ref(list), glyphs)
}

/// Display lists as the pages of one PDF, in order — a story's steps as a handout, each page the
/// size of its list. Graphics states, gradients, groups and the search-text font are resources
/// shared by every page. One list gives exactly the bytes of [`to_pdf`].
pub fn to_pdf_pages(lists: &[DisplayList], glyphs: &dyn GlyphSource) -> Vec<u8> {
    let mut w = Writer { glyphs, stack: vec![String::new()], objects: Vec::new(), gstates: Vec::new(), patterns: Vec::new(), xobjects: Vec::new(), layers: Vec::new(), width: 0.0, height: 0.0, uses_text: false };
    let catalog = w.reserve();
    let pages = w.reserve();
    let mut written = Vec::with_capacity(lists.len());
    for list in lists {
        let page = w.reserve();
        let content_id = draw_page(&mut w, list);
        written.push((page, content_id, list.width * PT, list.height * PT));
    }

    // Resources shared by the page and its groups.
    let blend_name = |b: Blend| match b {
        Blend::Normal => "Normal",
        Blend::Multiply => "Multiply",
        Blend::Screen => "Screen",
    };
    let gs: Vec<String> = w.gstates.iter().map(|((f, s, b), name)| format!("/{name} << /ca {} /CA {} /BM /{} >>", n(*f as f64 / 1000.0), n(*s as f64 / 1000.0), blend_name(*b))).collect();
    let pats: Vec<String> = w.patterns.iter().map(|(name, id)| format!("/{name} {id} 0 R")).collect();
    let xos: Vec<String> = w.xobjects.iter().map(|(name, id)| format!("/{name} {id} 0 R")).collect();
    let font = if w.uses_text { "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >> >>" } else { "" };
    let resources = format!("<< /ExtGState << {} >> /Pattern << {} >> /XObject << {} >> {font} >>", gs.join(" "), pats.join(" "), xos.join(" "));
    // One dictionary for every page and group: an indirect object, written once.
    let resources = format!("{} 0 R", w.object(resources.into_bytes()));
    // Groups were written with a placeholder for the shared resources: make them real streams.
    for (_, id) in w.xobjects.clone() {
        let raw = String::from_utf8(std::mem::take(&mut w.objects[id - 1])).unwrap_or_default();
        let raw = raw.trim_start_matches("__STREAM__");
        let (dict, body) = raw.split_once('\n').unwrap_or((raw, ""));
        w.objects[id - 1] = Writer::stream(&dict.replace("RES", &resources), body.as_bytes());
    }
    w.objects[catalog - 1] = format!("<< /Type /Catalog /Pages {pages} 0 R >>").into_bytes();
    let kids: Vec<String> = written.iter().map(|(page, ..)| format!("{page} 0 R")).collect();
    w.objects[pages - 1] = format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), written.len()).into_bytes();
    for (page, content_id, pw, ph) in written {
        w.objects[page - 1] = format!("<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 {} {}] /Resources {resources} /Contents {content_id} 0 R >>", n(pw), n(ph)).into_bytes();
    }

    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(w.objects.len());
    for (i, body) in w.objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", w.objects.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n", w.objects.len() + 1).as_bytes());
    out
}

/// One page's content stream (drawn under the page flip); returns its object id.
fn draw_page(w: &mut Writer, list: &DisplayList) -> usize {
    w.width = list.width;
    w.height = list.height;
    w.stack = vec![String::new()];
    w.layers.clear();
    let _ = writeln!(w.out(), "{} 0 0 {} 0 {} cm", n(PT), n(-PT), n(list.height * PT));
    if list.background.a > 0.0 {
        let bg = list.background;
        let rect = PathData { els: vec![PathEl::Move { p: Vec2::ZERO }, PathEl::Line { p: Vec2::new(list.width, 0.0) }, PathEl::Line { p: Vec2::new(list.width, list.height) }, PathEl::Line { p: Vec2::new(0.0, list.height) }, PathEl::Close] };
        w.fill(&rect, &DPaint::Solid(bg), FillRule::NonZero, 1.0);
    }
    let mut clips = 0usize;
    for op in &list.ops {
        match op {
            Op::Fill { path, xf, paint, rule, opacity } => w.fill(&path.path.transform(xf), paint, *rule, *opacity),
            Op::Stroke { path, xf, style, paint, opacity } => w.stroke(&path.path.transform(xf), style, paint, *opacity),
            Op::Glyphs { font, size, glyphs: g, origin, scale, rotate, color, halo, text } => w.glyphs(font, *size, g, *origin, *scale, *rotate, *color, *halo, text.as_deref()),
            Op::Instances(i) => {
                for k in 0..i.len() {
                    let p = i.instance_path(k);
                    let fill = i.fill.get(k).or(i.fill.first()).copied().unwrap_or(Color::BLACK);
                    let op = i.opacity.get(k).or(i.opacity.first()).copied().unwrap_or(1.0) * i.alpha;
                    w.fill(&p, &DPaint::Solid(fill), FillRule::NonZero, op);
                    if let Some((sc, sw)) = i.stroke {
                        w.stroke(&p, &StrokeStyle { width_px: sw, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None }, &DPaint::Solid(sc), op);
                    }
                }
            }
            Op::Image { .. } => {}
            Op::PushClip { path, xf, rule } => {
                clips += 1;
                w.out().push_str("q\n");
                w.path(&path.path.transform(xf));
                w.out().push_str(if *rule == FillRule::EvenOdd { "W* n\n" } else { "W n\n" });
            }
            Op::PopClip => {
                if clips > 0 {
                    clips -= 1;
                    w.out().push_str("Q\n");
                }
            }
            Op::PushLayer { opacity, blend } => w.push_layer(*opacity, *blend),
            Op::PopLayer => w.pop_layer(),
        }
    }
    while !w.layers.is_empty() {
        w.pop_layer();
    }
    let content = w.stack.pop().unwrap_or_default();
    w.object(Writer::stream("", content.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_render::{NoGlyphs, SharedPath};

    fn rect(x: f64, y: f64, w: f64, h: f64) -> SharedPath {
        SharedPath::new(PathData { els: vec![PathEl::Move { p: Vec2::new(x, y) }, PathEl::Line { p: Vec2::new(x + w, y) }, PathEl::Line { p: Vec2::new(x + w, y + h) }, PathEl::Line { p: Vec2::new(x, y + h) }, PathEl::Close] })
    }

    #[test]
    fn a_page_with_paths_layers_and_clips_is_well_formed_and_deterministic() {
        let red = Color::rgb8(220, 30, 40);
        let list = DisplayList {
            width: 200.0,
            height: 100.0,
            background: Color::WHITE,
            ops: vec![
                Op::PushClip { path: rect(0.0, 0.0, 150.0, 100.0), xf: Affine::IDENTITY, rule: FillRule::NonZero },
                Op::Fill { path: rect(10.0, 10.0, 50.0, 50.0), xf: Affine::IDENTITY, paint: DPaint::Solid(red), rule: FillRule::NonZero, opacity: 1.0 },
                Op::PushLayer { opacity: 0.5, blend: Blend::Multiply },
                Op::Fill { path: rect(40.0, 20.0, 50.0, 50.0), xf: Affine::translate(5.0, 0.0), paint: DPaint::Linear { p0: Vec2::new(40.0, 0.0), p1: Vec2::new(90.0, 0.0), stops: vec![(0.0, red), (0.5, Color::WHITE), (1.0, Color::BLACK)] }, rule: FillRule::NonZero, opacity: 1.0 },
                Op::PopLayer,
                Op::Stroke { path: rect(100.0, 10.0, 40.0, 40.0), xf: Affine::IDENTITY, style: StrokeStyle { width_px: 2.0, cap: Cap::Round, join: Join::Round, miter_limit: 4.0, dash: Some(vec![4.0, 2.0]) }, paint: DPaint::Solid(Color::BLACK), opacity: 0.5 },
                Op::PopClip,
            ],
        };
        let pdf = to_pdf(&list, &NoGlyphs);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 150 75]"), "200×100 px = 150×75 pt");
        assert!(text.contains("/Group << /S /Transparency"), "the layer is a transparency group");
        assert!(text.contains("/BM /Multiply") && text.contains("/ca 0.5"));
        assert!(text.contains("/ShadingType 2") && text.contains("/FunctionType 3"), "a stitched axial gradient");
        assert!(text.trim_end().ends_with("%%EOF"));
        // Every xref offset points at its object (bytes: the streams are compressed).
        let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).rposition(|w| w == needle);
        let sx = find(&pdf, b"startxref\n").unwrap() + 10;
        let xref_at: usize = String::from_utf8_lossy(&pdf[sx..]).lines().next().unwrap().parse().unwrap();
        let table = String::from_utf8_lossy(&pdf[xref_at..]).into_owned();
        let entries: Vec<usize> = table.lines().skip(3).take_while(|l| l.ends_with(" n ")).map(|l| l[..10].parse().unwrap()).collect();
        assert!(entries.len() >= 5);
        for (i, off) in entries.iter().enumerate() {
            assert!(pdf[*off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "object {}", i + 1);
        }
        assert_eq!(to_pdf(&list, &NoGlyphs), pdf, "same list, same bytes");
    }

    #[test]
    fn several_lists_make_one_page_each_with_their_own_sizes() {
        let page = |w: f64, h: f64, c: Color| DisplayList { width: w, height: h, background: Color::WHITE, ops: vec![Op::Fill { path: rect(10.0, 10.0, 20.0, 20.0), xf: Affine::IDENTITY, paint: DPaint::Solid(c), rule: FillRule::NonZero, opacity: 0.5 }] };
        let lists = [page(200.0, 100.0, Color::BLACK), page(400.0, 400.0, Color::rgb8(220, 30, 40)), page(200.0, 100.0, Color::BLACK)];
        let pdf = to_pdf_pages(&lists, &NoGlyphs);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 3"), "three pages");
        assert_eq!(text.matches("/Type /Page ").count(), 3);
        assert!(text.contains("/MediaBox [0 0 150 75]") && text.contains("/MediaBox [0 0 300 300]"), "each page its own size");
        assert_eq!(text.matches("/ca 0.5").count(), 1, "graphics states are shared between pages");
        assert_eq!(to_pdf_pages(&lists[..1], &NoGlyphs), to_pdf(&lists[0], &NoGlyphs), "one list: the same bytes as to_pdf");
        assert_eq!(to_pdf_pages(&lists, &NoGlyphs), pdf, "deterministic");
    }
}
