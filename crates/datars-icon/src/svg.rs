//! SVG → [`Icon`]: the subset real icons and logos use, read the same way on every platform.
//!
//! Read: `svg` (its `viewBox`, or `width`/`height`), `g`, `a`, `switch`, `use` of an element in the
//! same file (and `symbol`), `path`, `rect` (with `rx`/`ry`), `circle`, `ellipse`, `line`,
//! `polyline`, `polygon`; `transform` (matrix, translate, scale, rotate, skewX, skewY); `fill`,
//! `fill-rule`, `fill-opacity`, `stroke`, `stroke-width`, `stroke-opacity`, `stroke-linecap`,
//! `stroke-linejoin`, `opacity`, `display`, `visibility` — as attributes, in `style`, or from a
//! `<style>` sheet of simple rules (see [`crate::css`]).
//!
//! Colours never survive as colours (CLAUDE.md: colours are inks). `currentColor` becomes the
//! node's ink; every other colour becomes a *slot*, numbered in the order the file first uses it,
//! for an ink the author passes (or a palette token). A file drawn in one colour and no
//! `currentColor` — most icon sets' "black" — takes the node's ink, as if it said `currentColor`.
//! Gradients and patterns draw as one slot each.
//!
//! Refused, with a message: scripts, event attributes, `<foreignObject>`, embedded or linked
//! pictures (`<image>`), and any reference outside the file (`href`, `url(…)`, `@import`). An icon
//! is drawn from its own contents only. Skipped, with a note: text (convert it to outlines),
//! clipping, masks, filters, animation.

use crate::css;
use crate::xml::{self, Element};
use crate::{Icon, Layer};
use datars_math::{m, Affine, FillRule, PathData, Rect, Vec2};
use datars_scene::{Cap, IconInk, IconStroke, Join};
use std::collections::BTreeMap;

/// Layers in one icon, at most.
const MAX_LAYERS: usize = 20_000;
/// `use` inside `use`, at most this deep.
const MAX_USE_DEPTH: usize = 8;

/// Elements that are never read: they run code, embed other documents, or fetch.
const REFUSED: &[&str] = &["script", "foreignObject", "image", "iframe", "embed", "object", "audio", "video", "feImage", "handler", "listener"];
/// Elements that draw nothing themselves (their children are referenced, or are metadata).
const SILENT: &[&str] = &["defs", "title", "desc", "metadata", "style", "linearGradient", "radialGradient", "pattern", "clipPath", "mask", "marker", "filter", "symbol", "font", "font-face", "cursor", "view"];

/// Read an SVG file's drawing.
pub fn parse_svg(src: &str) -> Result<Icon, String> {
    let root = xml::parse(src)?;
    if root.local() != "svg" {
        return Err(format!("the root element is <{}>, not <svg>", root.name));
    }
    check(&root)?;
    let mut notes = Vec::new();
    let mut rules = Vec::new();
    collect_styles(&root, &mut rules, &mut notes)?;
    let mut ids = BTreeMap::new();
    index(&root, &mut ids);
    let title = root.elements().find(|e| e.local() == "title").map(|t| t.text().trim().to_string()).filter(|t| !t.is_empty());
    let mut r = Walk { rules, ids, notes, layers: Vec::new(), use_depth: 0 };
    let view_box = root.attr("viewBox").and_then(numbers4).filter(|v| v[2] > 0.0 && v[3] > 0.0).map(|v| Rect::new(v[0], v[1], v[2], v[3]));
    let sized = match (root.attr("width").and_then(|w| length(w, 0.0)), root.attr("height").and_then(|h| length(h, 0.0))) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Rect::new(0.0, 0.0, w, h)),
        _ => None,
    };
    let vb_for_pct = view_box.or(sized).unwrap_or(Rect::new(0.0, 0.0, 100.0, 100.0));
    let st = r.style_of(&root, &St::default(), vb_for_pct)?;
    let op = own_opacity(&r.declared(&root)?);
    r.children(&root, Affine::IDENTITY, &st, op, vb_for_pct)?;
    let Walk { notes, layers, .. } = r;
    let (layers, slots) = assign_slots(layers);
    let layers: Vec<Layer> = layers;
    if layers.is_empty() {
        let why = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
        return Err(format!("the SVG draws nothing{why}"));
    }
    let view_box = match view_box.or(sized) {
        Some(v) => v,
        None => {
            // No box at all: the drawing's own bounds, strokes included.
            let mut b: Option<Rect> = None;
            for l in &layers {
                let pad = l.stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
                let lb = l.path.bounds();
                let lb = Rect::new(lb.x - pad, lb.y - pad, lb.w + 2.0 * pad, lb.h + 2.0 * pad);
                b = Some(b.map_or(lb, |x| x.union(&lb)));
            }
            b.filter(|b| b.w > 0.0 || b.h > 0.0).ok_or("the SVG draws nothing with any extent")?
        }
    };
    Ok(Icon { view_box, layers, slots, title, notes })
}

/// Everything refused, anywhere in the file (also inside `<defs>`: a `use` could reach it).
fn check(e: &Element) -> Result<(), String> {
    let name = e.local();
    if REFUSED.iter().any(|r| r.eq_ignore_ascii_case(name)) {
        return Err(match name {
            "script" | "handler" | "listener" => "the SVG has a script, which is never run; icons are drawings only".to_string(),
            "image" | "feImage" => "the SVG embeds a picture (<image>); draw pictures with `image()`, icons are vector drawings only".to_string(),
            _ => format!("the SVG embeds <{name}>, which isn't read; icons are vector drawings only"),
        });
    }
    for (k, v) in &e.attrs {
        let key = k.to_ascii_lowercase();
        if key.starts_with("on") && key.len() > 2 {
            return Err(format!("<{name}> has an event attribute (`{k}`), which is script; icons are drawings only"));
        }
        if (key == "href" || key == "xlink:href") && name != "a" && !v.trim_start().starts_with('#') {
            return Err(format!("<{name}> refers outside the file (`{k}=\"{v}\"`); icons are drawn from their own contents only"));
        }
        if key == "style" || matches!(key.as_str(), "fill" | "stroke" | "mask" | "clip-path" | "filter" | "marker-start" | "marker-mid" | "marker-end") {
            css::check_value(v)?;
        }
    }
    for c in e.elements() {
        check(c)?;
    }
    Ok(())
}

fn collect_styles(e: &Element, rules: &mut Vec<css::Rule>, notes: &mut Vec<String>) -> Result<(), String> {
    for c in e.elements() {
        if c.local() == "style" {
            let (rs, ns) = css::sheet(&c.text(), rules.len())?;
            rules.extend(rs);
            notes.extend(ns);
        } else {
            collect_styles(c, rules, notes)?;
        }
    }
    Ok(())
}

fn index<'a>(e: &'a Element, ids: &mut BTreeMap<String, &'a Element>) {
    if let Some(id) = e.attr("id") {
        ids.entry(id.to_string()).or_insert(e);
    }
    for c in e.elements() {
        index(c, ids);
    }
}

/// A paint as the file says it.
#[derive(Clone, Debug, PartialEq)]
enum P {
    None,
    Current,
    /// A colour's identity (`#e30613`, `darkred`), with its alpha.
    Color(String, f64),
    /// A gradient or pattern of this file.
    Url(String),
}

/// The inherited style at an element.
#[derive(Clone, Debug)]
struct St {
    fill: P,
    fill_op: f64,
    rule: FillRule,
    stroke: P,
    width: f64,
    stroke_op: f64,
    cap: Cap,
    join: Join,
    visible: bool,
}

impl Default for St {
    fn default() -> St {
        // SVG's initial values: black fill, no stroke, 1 wide, butt caps, miter joins.
        St { fill: P::Color("#000000".into(), 1.0), fill_op: 1.0, rule: FillRule::NonZero, stroke: P::None, width: 1.0, stroke_op: 1.0, cap: Cap::Butt, join: Join::Miter, visible: true }
    }
}

/// A drawn layer before its paints become slots.
struct Raw {
    path: PathData,
    fill: Option<P>,
    rule: FillRule,
    stroke: Option<(P, f64, Cap, Join)>,
    opacity: f64,
}

struct Walk<'a> {
    rules: Vec<css::Rule>,
    ids: BTreeMap<String, &'a Element>,
    notes: Vec<String>,
    layers: Vec<Raw>,
    use_depth: usize,
}

impl<'a> Walk<'a> {
    fn note(&mut self, n: String) {
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    /// The element's declared properties: presentation attributes, then sheet rules by
    /// specificity and order, then its `style` attribute (later wins).
    fn declared(&self, e: &Element) -> Result<BTreeMap<String, String>, String> {
        let mut out = BTreeMap::new();
        for (k, v) in &e.attrs {
            if PROPS.contains(&k.as_str()) {
                out.insert(k.clone(), v.trim().to_string());
            }
        }
        let classes: Vec<&str> = e.attr("class").map(|c| c.split_whitespace().collect()).unwrap_or_default();
        let mut matching: Vec<&css::Rule> = self.rules.iter().filter(|r| r.selector.matches(e.local(), e.attr("id"), &classes)).collect();
        matching.sort_by_key(|r| (r.specificity, r.order));
        for r in matching {
            for (k, v) in &r.decls {
                out.insert(k.clone(), v.clone());
            }
        }
        if let Some(s) = e.attr("style") {
            for (k, v) in css::declarations(s)? {
                out.insert(k, v);
            }
        }
        Ok(out)
    }

    fn style_of(&mut self, e: &Element, parent: &St, vb: Rect) -> Result<St, String> {
        let d = self.declared(e)?;
        let mut st = parent.clone();
        let inherit = |v: &str| v.eq_ignore_ascii_case("inherit");
        for (k, v) in &d {
            if inherit(v) {
                continue;
            }
            match k.as_str() {
                "fill" => {
                    if let Some(p) = self.paint(v) {
                        st.fill = p;
                    }
                }
                "stroke" => {
                    if let Some(p) = self.paint(v) {
                        st.stroke = p;
                    }
                }
                "fill-opacity" => st.fill_op = unit(v).unwrap_or(st.fill_op),
                "stroke-opacity" => st.stroke_op = unit(v).unwrap_or(st.stroke_op),
                "fill-rule" => st.rule = if v == "evenodd" { FillRule::EvenOdd } else { FillRule::NonZero },
                "stroke-width" => st.width = length(v, diag(vb)).unwrap_or(st.width),
                "stroke-linecap" => {
                    st.cap = match v.as_str() {
                        "round" => Cap::Round,
                        "square" => Cap::Square,
                        _ => Cap::Butt,
                    }
                }
                "stroke-linejoin" => {
                    st.join = match v.as_str() {
                        "round" => Join::Round,
                        "bevel" => Join::Bevel,
                        _ => Join::Miter,
                    }
                }
                "visibility" => st.visible = !matches!(v.as_str(), "hidden" | "collapse"),
                "stroke-dasharray" if v != "none" => self.note("dashed strokes draw solid".into()),
                "clip-path" | "mask" if v != "none" => self.note(format!("{k} isn't read: the clipped part draws whole")),
                "filter" if v != "none" => self.note("filters aren't read".into()),
                _ => {}
            }
        }
        Ok(st)
    }

    fn paint(&mut self, v: &str) -> Option<P> {
        let v = v.trim();
        let lower = v.to_ascii_lowercase();
        if lower == "none" || lower == "transparent" {
            return Some(P::None);
        }
        if lower == "currentcolor" {
            return Some(P::Current);
        }
        if let Some(rest) = lower.strip_prefix("url(") {
            let id = rest.split(')').next().unwrap_or("").trim().trim_matches(['"', '\'']).trim_start_matches('#').to_string();
            // `url(#g) red`: the fallback colour only applies when the reference is missing.
            if self.ids.contains_key(&id) {
                let kind = self.ids[&id].local().to_string();
                self.note(format!("<{kind} id=\"{id}\"> draws as one ink (a slot)"));
                return Some(P::Url(id));
            }
            let fallback = v.split(')').nth(1).map(str::trim).unwrap_or("");
            return if fallback.is_empty() { Some(P::None) } else { self.paint(fallback) };
        }
        match colour(&lower) {
            Some((key, a)) => Some(P::Color(key, a)),
            None => {
                self.note(format!("colour `{v}` isn't read; drawn with the inherited paint"));
                None
            }
        }
    }

    fn children(&mut self, e: &Element, ctm: Affine, st: &St, op: f64, vb: Rect) -> Result<(), String> {
        for c in e.elements() {
            self.element(c, ctm, st, op, vb)?;
        }
        Ok(())
    }

    fn element(&mut self, e: &Element, ctm: Affine, parent: &St, parent_op: f64, vb: Rect) -> Result<(), String> {
        let name = e.local().to_string();
        if name.contains(':') {
            return Ok(()); // an editor's own element (sodipodi:namedview, inkscape:…)
        }
        if SILENT.contains(&name.as_str()) {
            return Ok(());
        }
        let d = self.declared(e)?;
        if d.get("display").is_some_and(|v| v == "none") {
            return Ok(());
        }
        let st = self.style_of(e, parent, vb)?;
        let op = parent_op * own_opacity(&d);
        let mut ctm = ctm;
        if let Some(t) = e.attr("transform") {
            ctm = ctm.mul(transform(t).ok_or_else(|| format!("a malformed transform `{t}`"))?);
        }
        let n = |k: &str, rel: f64| e.attr(k).and_then(|v| length(v, rel)).unwrap_or(0.0);
        let path = match name.as_str() {
            "g" | "a" | "switch" => return self.children(e, ctm, &st, op, vb),
            "svg" => {
                // A nested viewport: placed at x, y; its own viewBox scaled into width × height.
                let (x, y) = (n("x", vb.w), n("y", vb.h));
                let mut inner = ctm.mul(Affine::translate(x, y));
                let mut ivb = vb;
                if let Some(v) = e.attr("viewBox").and_then(numbers4).filter(|v| v[2] > 0.0 && v[3] > 0.0) {
                    let (w, h) = (e.attr("width").and_then(|v| length(v, vb.w)).unwrap_or(v[2]), e.attr("height").and_then(|v| length(v, vb.h)).unwrap_or(v[3]));
                    let s = (w / v[2]).min(h / v[3]);
                    inner = inner.mul(Affine::translate((w - v[2] * s) / 2.0, (h - v[3] * s) / 2.0)).mul(Affine::scale(s, s)).mul(Affine::translate(-v[0], -v[1]));
                    ivb = Rect::new(v[0], v[1], v[2], v[3]);
                }
                return self.children(e, inner, &st, op, ivb);
            }
            "use" => return self.use_(e, ctm, &st, op, vb),
            "path" => crate::path::parse_svg_path(e.attr("d").unwrap_or("")),
            "rect" => {
                let (x, y, w, h) = (n("x", vb.w), n("y", vb.h), n("width", vb.w), n("height", vb.h));
                if w <= 0.0 || h <= 0.0 {
                    return Ok(());
                }
                let (rx, ry) = (e.attr("rx").and_then(|v| length(v, vb.w)), e.attr("ry").and_then(|v| length(v, vb.h)));
                let (rx, ry) = match (rx, ry) {
                    (Some(a), Some(b)) => (a, b),
                    (Some(a), None) | (None, Some(a)) => (a, a),
                    (None, None) => (0.0, 0.0),
                };
                rounded_rect(x, y, w, h, rx.clamp(0.0, w / 2.0), ry.clamp(0.0, h / 2.0))
            }
            "circle" => {
                let r = n("r", diag(vb));
                if r <= 0.0 {
                    return Ok(());
                }
                PathData::circle(Vec2::new(n("cx", vb.w), n("cy", vb.h)), r)
            }
            "ellipse" => {
                let (rx, ry) = (n("rx", vb.w), n("ry", vb.h));
                if rx <= 0.0 || ry <= 0.0 {
                    return Ok(());
                }
                PathData::ellipse(Vec2::new(n("cx", vb.w), n("cy", vb.h)), rx, ry)
            }
            "line" => PathData::polyline(&[Vec2::new(n("x1", vb.w), n("y1", vb.h)), Vec2::new(n("x2", vb.w), n("y2", vb.h))]),
            "polyline" | "polygon" => {
                let v = numbers(e.attr("points").unwrap_or(""));
                let pts: Vec<Vec2> = v.as_chunks::<2>().0.iter().map(|c| Vec2::new(c[0], c[1])).collect();
                if pts.len() < 2 {
                    return Ok(());
                }
                if name == "polygon" {
                    PathData::polygon(&pts)
                } else {
                    PathData::polyline(&pts)
                }
            }
            "text" | "tspan" | "textPath" => {
                self.note("text isn't read: convert it to outlines in the editor".into());
                return Ok(());
            }
            "animate" | "animateTransform" | "animateMotion" | "set" => {
                self.note("animation isn't read: the drawing is its first frame".into());
                return Ok(());
            }
            other => {
                self.note(format!("<{other}> isn't read"));
                return Ok(());
            }
        };
        if !st.visible || path.is_empty() {
            return Ok(());
        }
        self.layer(path, &ctm, &st, op, name == "line" || name == "polyline")
    }

    fn use_(&mut self, e: &Element, ctm: Affine, st: &St, op: f64, vb: Rect) -> Result<(), String> {
        let href = e.attr("href").or_else(|| e.attr("xlink:href")).unwrap_or("");
        let id = href.trim().trim_start_matches('#');
        let Some(target) = self.ids.get(id).copied() else {
            self.note(format!("<use> of `#{id}`, which isn't in the file"));
            return Ok(());
        };
        if self.use_depth >= MAX_USE_DEPTH {
            return Err(format!("<use> nested over {MAX_USE_DEPTH} deep (a reference cycle?)"));
        }
        let n = |k: &str, rel: f64| e.attr(k).and_then(|v| length(v, rel)).unwrap_or(0.0);
        let mut at = ctm.mul(Affine::translate(n("x", vb.w), n("y", vb.h)));
        self.use_depth += 1;
        let r = if target.local() == "symbol" {
            // A symbol's own viewBox fills the use's width × height (else it draws at its size).
            if let (Some(v), Some(w), Some(h)) = (target.attr("viewBox").and_then(numbers4), e.attr("width").and_then(|v| length(v, vb.w)), e.attr("height").and_then(|v| length(v, vb.h))) {
                if v[2] > 0.0 && v[3] > 0.0 {
                    let s = (w / v[2]).min(h / v[3]);
                    at = at.mul(Affine::scale(s, s)).mul(Affine::translate(-v[0], -v[1]));
                }
            }
            let tst = self.style_of(target, st, vb)?;
            let top = op * own_opacity(&self.declared(target)?);
            self.children(target, at, &tst, top, vb)
        } else {
            self.element(target, at, st, op, vb)
        };
        self.use_depth -= 1;
        r
    }

    fn layer(&mut self, path: PathData, ctm: &Affine, st: &St, op: f64, open: bool) -> Result<(), String> {
        if self.layers.len() >= MAX_LAYERS {
            return Err(format!("over {MAX_LAYERS} shapes"));
        }
        let path = if *ctm == Affine::IDENTITY { path } else { path.transform(ctm) };
        // A fill of nothing (a line, or a path with no area: `M14 20h12`) paints nothing — and
        // its colour (often SVG's default black) isn't one of the icon's.
        let b = path.bounds();
        let flat = !(b.w > 1e-9 && b.h > 1e-9);
        let fill = (!open && !flat).then(|| st.fill.clone()).filter(|p| *p != P::None);
        let fill_a = st.fill_op * alpha(&st.fill);
        let width = st.width * ctm.scale_factor();
        let stroke = Some(st.stroke.clone()).filter(|p| *p != P::None && width > 0.0 && width.is_finite()).map(|p| (p, width, st.cap, st.join));
        let stroke_a = st.stroke_op * alpha(&st.stroke);
        match (fill, stroke) {
            (None, None) => {}
            // Fill and stroke at different opacities: two layers, the stroke over the fill.
            (Some(f), Some(s)) if fill_a != stroke_a => {
                self.layers.push(Raw { path: path.clone(), fill: Some(f), rule: st.rule, stroke: None, opacity: op * fill_a });
                self.layers.push(Raw { path, fill: None, rule: st.rule, stroke: Some(s), opacity: op * stroke_a });
            }
            (f, s) => {
                let a = if f.is_some() { fill_a } else { stroke_a };
                self.layers.push(Raw { path, fill: f, rule: st.rule, stroke: s, opacity: op * a });
            }
        }
        Ok(())
    }
}

/// Presentation attributes read as properties.
const PROPS: &[&str] = &["fill", "fill-opacity", "fill-rule", "stroke", "stroke-width", "stroke-opacity", "stroke-linecap", "stroke-linejoin", "stroke-dasharray", "opacity", "display", "visibility", "clip-path", "mask", "filter"];

fn own_opacity(d: &BTreeMap<String, String>) -> f64 {
    d.get("opacity").and_then(|v| unit(v)).unwrap_or(1.0)
}

fn alpha(p: &P) -> f64 {
    match p {
        P::Color(_, a) => *a,
        _ => 1.0,
    }
}

/// Paints → icon inks: `currentColor` stays the node's ink; the other colours, in the order the
/// file first uses them, become slots — unless the file is drawn in one colour and never says
/// `currentColor`, when that colour is the node's ink.
fn assign_slots(raw: Vec<Raw>) -> (Vec<Layer>, Vec<String>) {
    let key = |p: &P| match p {
        P::Color(k, _) => Some(k.clone()),
        P::Url(id) => Some(format!("url(#{id})")),
        _ => None,
    };
    let mut order: Vec<String> = Vec::new();
    let mut current = false;
    for r in &raw {
        for p in r.fill.iter().chain(r.stroke.as_ref().map(|s| &s.0)) {
            match key(p) {
                Some(k) if !order.contains(&k) => order.push(k),
                None if *p == P::Current => current = true,
                _ => {}
            }
        }
    }
    let mono = !current && order.len() == 1;
    let ink = |p: &P| -> IconInk {
        match key(p) {
            Some(k) if !mono => IconInk::Slot(order.iter().position(|o| *o == k).unwrap_or(0).min(255) as u8),
            _ => IconInk::Current,
        }
    };
    let layers = raw
        .into_iter()
        .map(|r| Layer {
            fill: r.fill.as_ref().map(ink),
            rule: r.rule,
            stroke: r.stroke.as_ref().map(|(p, w, cap, join)| IconStroke { ink: ink(p), width: *w, cap: *cap, join: *join }),
            path: r.path,
            opacity: r.opacity.clamp(0.0, 1.0),
        })
        .collect();
    (layers, if mono { Vec::new() } else { order })
}

/// A colour's identity and alpha: hex (3, 4, 6 or 8 digits), `rgb()`/`rgba()`, `hsl()`/`hsla()`,
/// or a name (CSS's basic ones spelled as hex, the rest as named).
fn colour(v: &str) -> Option<(String, f64)> {
    let v = v.trim();
    if let Some(h) = v.strip_prefix('#') {
        let hex = |s: &str| u8::from_str_radix(s, 16).ok();
        let (rgb, a) = match h.len() {
            3 | 4 => {
                let d = |i: usize| hex(&h[i..i + 1]).map(|x| x * 17);
                ([d(0)?, d(1)?, d(2)?], if h.len() == 4 { d(3)? as f64 / 255.0 } else { 1.0 })
            }
            6 | 8 => ([hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?], if h.len() == 8 { hex(&h[6..8])? as f64 / 255.0 } else { 1.0 }),
            _ => return None,
        };
        return Some((format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]), a));
    }
    if let Some(args) = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb(")) {
        let parts: Vec<&str> = args.trim_end_matches(')').split([',', ' ', '/']).filter(|s| !s.is_empty()).collect();
        if parts.len() < 3 {
            return None;
        }
        let ch = |s: &str| -> Option<u8> {
            let x = match s.strip_suffix('%') {
                Some(p) => p.parse::<f64>().ok()? * 2.55,
                None => s.parse::<f64>().ok()?,
            };
            Some(x.round().clamp(0.0, 255.0) as u8)
        };
        let a = parts.get(3).and_then(|s| unit(s)).unwrap_or(1.0);
        return Some((format!("#{:02x}{:02x}{:02x}", ch(parts[0])?, ch(parts[1])?, ch(parts[2])?), a));
    }
    if v.starts_with("hsl") {
        return Some((v.replace(' ', ""), 1.0));
    }
    let basic = match v {
        "black" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "green" => "#008000",
        "blue" => "#0000ff",
        "gray" | "grey" => "#808080",
        "yellow" => "#ffff00",
        "orange" => "#ffa500",
        "purple" => "#800080",
        _ => "",
    };
    if !basic.is_empty() {
        return Some((basic.to_string(), 1.0));
    }
    v.chars().all(|c| c.is_ascii_alphabetic()).then(|| (v.to_string(), 1.0)).filter(|(s, _)| !s.is_empty())
}

/// A number, or a percentage of 1.
fn unit(v: &str) -> Option<f64> {
    let v = v.trim();
    let x = match v.strip_suffix('%') {
        Some(p) => p.trim().parse::<f64>().ok()? / 100.0,
        None => v.parse::<f64>().ok()?,
    };
    x.is_finite().then(|| x.clamp(0.0, 1.0))
}

/// A length in user units: unitless or px, pt, pc, mm, cm, in, em (16 px), ex (8 px), or a
/// percentage of `rel`.
fn length(v: &str, rel: f64) -> Option<f64> {
    let v = v.trim();
    let (num, k) = if let Some(p) = v.strip_suffix('%') {
        (p, rel / 100.0)
    } else {
        let units = [("px", 1.0), ("pt", 4.0 / 3.0), ("pc", 16.0), ("mm", 96.0 / 25.4), ("cm", 96.0 / 2.54), ("in", 96.0), ("em", 16.0), ("ex", 8.0)];
        units.iter().find_map(|(u, k)| v.strip_suffix(u).map(|n| (n, *k))).unwrap_or((v, 1.0))
    };
    let x: f64 = num.trim().parse().ok()?;
    (x.is_finite()).then_some(x * k)
}

/// The viewport's normalized diagonal (what a percentage of a radius or a stroke width is of).
fn diag(vb: Rect) -> f64 {
    ((vb.w * vb.w + vb.h * vb.h) / 2.0).sqrt()
}

fn numbers(s: &str) -> Vec<f64> {
    // SVG number lists: separated by whitespace or commas, or by a sign or a second dot.
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        let st = i;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        let mut dot = false;
        while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b'.' && !dot)) {
            dot |= b[i] == b'.';
            i += 1;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            i += 1;
            if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i == st {
            break; // not a number: the list ends here
        }
        match std::str::from_utf8(&b[st..i]).ok().and_then(|t| t.parse::<f64>().ok()) {
            Some(v) => out.push(v),
            None => break,
        }
    }
    out
}

fn numbers4(s: &str) -> Option<[f64; 4]> {
    let v = numbers(s);
    (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
}

/// An SVG transform list.
fn transform(s: &str) -> Option<Affine> {
    let mut xf = Affine::IDENTITY;
    let mut rest = s.trim();
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let name = rest[..open].trim().trim_start_matches(',').trim();
        let close = rest[open..].find(')')? + open;
        let a = numbers(&rest[open + 1..close]);
        let deg = |d: f64| d * m::PI / 180.0;
        let t = match (name, a.len()) {
            ("matrix", 6) => Affine([a[0], a[1], a[2], a[3], a[4], a[5]]),
            ("translate", 1) => Affine::translate(a[0], 0.0),
            ("translate", 2) => Affine::translate(a[0], a[1]),
            ("scale", 1) => Affine::scale(a[0], a[0]),
            ("scale", 2) => Affine::scale(a[0], a[1]),
            ("rotate", 1) => Affine::rotate(deg(a[0])),
            ("rotate", 3) => Affine::translate(a[1], a[2]).mul(Affine::rotate(deg(a[0]))).mul(Affine::translate(-a[1], -a[2])),
            ("skewX", 1) => Affine([1.0, 0.0, m::tan(deg(a[0])), 1.0, 0.0, 0.0]),
            ("skewY", 1) => Affine([1.0, m::tan(deg(a[0])), 0.0, 1.0, 0.0, 0.0]),
            _ => return None,
        };
        xf = xf.mul(t);
        rest = rest[close + 1..].trim_start();
    }
    Some(xf)
}

/// A rectangle with elliptical corners `rx` × `ry`, clockwise from its top edge.
fn rounded_rect(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> PathData {
    if rx <= 0.0 || ry <= 0.0 {
        return PathData::rect(Rect::new(x, y, w, h));
    }
    const K: f64 = 0.552_284_749_830_793_4;
    let (kx, ky) = (rx * K, ry * K);
    let v = Vec2::new;
    let mut p = PathData::new();
    p.move_to(v(x + rx, y));
    p.line_to(v(x + w - rx, y));
    p.cubic_to(v(x + w - rx + kx, y), v(x + w, y + ry - ky), v(x + w, y + ry));
    p.line_to(v(x + w, y + h - ry));
    p.cubic_to(v(x + w, y + h - ry + ky), v(x + w - rx + kx, y + h), v(x + w - rx, y + h));
    p.line_to(v(x + rx, y + h));
    p.cubic_to(v(x + rx - kx, y + h), v(x, y + h - ry + ky), v(x, y + h - ry));
    p.line_to(v(x, y + ry));
    p.cubic_to(v(x, y + ry - ky), v(x + rx - kx, y), v(x + rx, y));
    p.close();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(body: &str) -> String {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\">{body}</svg>")
    }

    #[test]
    fn a_lucide_icon_is_stroked_in_the_nodes_ink() {
        let src = "<!-- @license lucide-static v1 - ISC -->\n<svg class=\"lucide\" xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n  <path d=\"M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8\" />\n  <circle cx=\"12\" cy=\"12\" r=\"3\" fill=\"currentColor\"/>\n</svg>";
        let i = parse_svg(src).unwrap();
        assert_eq!(i.view_box, Rect::new(0.0, 0.0, 24.0, 24.0));
        assert_eq!(i.layers.len(), 2);
        let s = i.layers[0].stroke.as_ref().unwrap();
        assert_eq!((s.ink, s.width, s.cap, s.join), (IconInk::Current, 2.0, Cap::Round, Join::Round));
        assert_eq!(i.layers[0].fill, None);
        assert_eq!(i.layers[1].fill, Some(IconInk::Current));
        assert!(i.slots.is_empty());
    }

    #[test]
    fn a_one_colour_file_takes_the_ink_and_a_logo_gets_slots() {
        // Material-style: no fill attribute at all (SVG's default black).
        let i = parse_svg(&svg("<path d=\"M0 0h10v10z\"/><rect x=\"12\" y=\"0\" width=\"4\" height=\"4\"/>")).unwrap();
        assert!(i.layers.iter().all(|l| l.fill == Some(IconInk::Current)));
        // Two colours (white spelled two ways is one): slots in the order first used.
        let i = parse_svg(&svg("<rect width=\"24\" height=\"24\" fill=\"#E30613\"/><circle cx=\"12\" cy=\"12\" r=\"5\" fill=\"white\"/><circle cx=\"6\" cy=\"6\" r=\"1\" style=\"fill:#FFF\"/>")).unwrap();
        assert_eq!(i.slots, vec!["#e30613".to_string(), "#ffffff".to_string()]);
        assert_eq!(i.layers.iter().map(|l| l.fill).collect::<Vec<_>>(), vec![Some(IconInk::Slot(0)), Some(IconInk::Slot(1)), Some(IconInk::Slot(1))]);
        // currentColor plus a colour: the colour is slot 0, currentColor the ink.
        let i = parse_svg(&svg("<path d=\"M0 0h4v4z\" fill=\"currentColor\"/><path d=\"M5 5h4v4z\" fill=\"rgb(255, 0, 0)\"/>")).unwrap();
        assert_eq!(i.layers[0].fill, Some(IconInk::Current));
        assert_eq!(i.layers[1].fill, Some(IconInk::Slot(0)));
        assert_eq!(i.slots, vec!["#ff0000".to_string()]);
    }

    #[test]
    fn style_sheets_classes_and_inheritance() {
        let src = svg("<style>.a{fill:#123456} #b{fill:#abcdef} rect{stroke:#000}</style><g fill=\"#ff0000\" stroke-width=\"3\"><rect class=\"a\" width=\"5\" height=\"5\"/><rect id=\"b\" class=\"a\" x=\"6\" width=\"5\" height=\"5\"/><circle cx=\"20\" cy=\"20\" r=\"2\"/><path d=\"M0 20h4v4z\" style=\"fill:#123456;fill-rule:evenodd\"/></g>");
        let i = parse_svg(&src).unwrap();
        // .a, then #b over .a, then the group's red inherited, then an inline style.
        assert_eq!(i.slots, vec!["#123456".to_string(), "#000000".to_string(), "#abcdef".to_string(), "#ff0000".to_string()]);
        assert_eq!(i.layers[0].stroke.as_ref().unwrap().width, 3.0);
        assert_eq!(i.layers[3].rule, FillRule::EvenOdd);
    }

    #[test]
    fn transforms_are_baked_into_the_paths_and_widths() {
        let i = parse_svg(&svg("<g transform=\"translate(10 0) scale(2)\"><path d=\"M0 0L1 0\" stroke=\"#000\" stroke-width=\"1.5\" fill=\"none\"/></g><rect transform=\"rotate(90 12 12)\" x=\"0\" y=\"11\" width=\"24\" height=\"2\"/>")).unwrap();
        let b = i.layers[0].path.bounds();
        assert_eq!((b.x, b.w), (10.0, 2.0));
        assert_eq!(i.layers[0].stroke.as_ref().unwrap().width, 3.0);
        let r = i.layers[1].path.bounds();
        assert!((r.x - 11.0).abs() < 1e-9 && (r.h - 24.0).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn every_shape_element() {
        let i = parse_svg(&svg("<rect x=\"1\" y=\"1\" width=\"10\" height=\"6\" rx=\"2\" ry=\"1\"/><circle cx=\"5\" cy=\"5\" r=\"2\"/><ellipse cx=\"5\" cy=\"5\" rx=\"3\" ry=\"1\"/><line x1=\"0\" y1=\"0\" x2=\"4\" y2=\"4\" stroke=\"#000\"/><polyline points=\"0,0 4,4 8,0\" fill=\"none\" stroke=\"#000\"/><polygon points=\"0 0 4 4 8 0\"/><path d=\"M0 0h1\"/>")).unwrap();
        // The last path has no area and no stroke: it paints nothing, and isn't a layer.
        assert_eq!(i.layers.len(), 6);
        // A line has no fill, whatever is inherited.
        assert_eq!(i.layers[3].fill, None);
        assert!(i.layers[3].stroke.is_some());
        let r = i.layers[0].path.bounds();
        assert_eq!((r.x, r.y, r.w, r.h), (1.0, 1.0, 10.0, 6.0));
    }

    #[test]
    fn use_references_and_symbols() {
        let i = parse_svg(&svg("<defs><path id=\"p\" d=\"M0 0h2v2h-2z\"/><symbol id=\"s\" viewBox=\"0 0 1 1\"><rect width=\"1\" height=\"1\"/></symbol></defs><use href=\"#p\" x=\"5\"/><use xlink:href=\"#s\" x=\"10\" y=\"10\" width=\"4\" height=\"4\"/>")).unwrap();
        assert_eq!(i.layers.len(), 2, "defs draw only where used");
        assert_eq!(i.layers[0].path.bounds().x, 5.0);
        let b = i.layers[1].path.bounds();
        assert_eq!((b.x, b.w), (10.0, 4.0));
        // A use cycle is refused, not followed for ever.
        assert!(parse_svg(&svg("<g id=\"a\"><use href=\"#a\"/></g>")).unwrap_err().contains("deep"));
    }

    #[test]
    fn scripts_and_outside_references_are_refused() {
        for (body, says) in [
            ("<script>alert(1)</script>", "script"),
            ("<rect width=\"1\" height=\"1\" onclick=\"x()\"/>", "event attribute"),
            ("<image href=\"photo.png\" width=\"4\" height=\"4\"/>", "picture"),
            ("<use href=\"https://example.com/sprite.svg#a\"/>", "outside"),
            ("<rect width=\"1\" height=\"1\" fill=\"url(https://example.com/x.svg#g)\"/>", "outside"),
            ("<style>@import url(x.css);</style><rect width=\"1\" height=\"1\"/>", "import"),
            ("<foreignObject><div/></foreignObject>", "foreignObject"),
            ("<defs><script>x</script></defs><rect width=\"1\" height=\"1\"/>", "script"),
        ] {
            let e = parse_svg(&svg(body)).unwrap_err();
            assert!(e.contains(says), "{body}: {e}");
        }
        // A link is not a fetch: <a href> draws its content.
        assert!(parse_svg(&svg("<a href=\"https://example.com\"><rect width=\"1\" height=\"1\"/></a>")).is_ok());
    }

    #[test]
    fn what_is_skipped_is_said() {
        let i = parse_svg(&svg("<text x=\"0\" y=\"10\">Hi</text><rect width=\"4\" height=\"4\" clip-path=\"url(#c)\"/><clipPath id=\"c\"><rect width=\"2\" height=\"2\"/></clipPath>")).unwrap();
        assert_eq!(i.layers.len(), 1);
        assert!(i.notes.iter().any(|n| n.contains("text")), "{:?}", i.notes);
        assert!(i.notes.iter().any(|n| n.contains("clip-path")), "{:?}", i.notes);
        assert!(parse_svg(&svg("<text>only words</text>")).unwrap_err().contains("draws nothing"));
    }

    #[test]
    fn boxes_from_width_and_height_or_the_drawing() {
        let i = parse_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"48px\" height=\"32\"><rect width=\"48\" height=\"32\"/></svg>").unwrap();
        assert_eq!(i.view_box, Rect::new(0.0, 0.0, 48.0, 32.0));
        let i = parse_svg("<svg xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"2\" y=\"3\" width=\"4\" height=\"5\"/></svg>").unwrap();
        assert_eq!(i.view_box, Rect::new(2.0, 3.0, 4.0, 5.0));
    }

    #[test]
    fn opacities_multiply_down_and_split_fill_from_stroke() {
        let i = parse_svg(&svg("<g opacity=\"0.5\"><rect width=\"4\" height=\"4\" fill=\"#000\" fill-opacity=\"0.5\" stroke=\"#000\"/></g>")).unwrap();
        assert_eq!(i.layers.len(), 2);
        assert_eq!((i.layers[0].opacity, i.layers[1].opacity), (0.25, 0.5));
        assert!(i.layers[0].stroke.is_none() && i.layers[1].fill.is_none());
    }

    #[test]
    fn the_same_file_reads_the_same_everywhere() {
        let src = svg("<g transform=\"rotate(33 12 12) skewX(10)\"><path d=\"M3 3a9 9 0 1 0 18 0\" stroke=\"#000\" fill=\"none\"/></g>");
        let a = parse_svg(&src).unwrap();
        let b = parse_svg(&src).unwrap();
        assert_eq!(a, b);
        assert_eq!(crate::Icon::from_json(&a.to_json()).unwrap(), a);
    }
}
