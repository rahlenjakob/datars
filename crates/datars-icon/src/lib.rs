//! `datars-icon` — icons as chart material (docs/04-primitives.md §Icons).
//!
//! An icon is a vector drawing read from SVG ([`parse_svg`]) into layers of paths in its own box
//! (its `viewBox`), each filled and/or stroked in an *ink slot*: the node's own ink where the file
//! said `currentColor` (or was drawn in one colour), or one of the inks the author passes for a
//! logo's other colours. Colours never survive as colours: a theme or mode switch repaints an icon
//! like everything else.
//!
//! Where icons come from ([`Source`]): the default set (`lucide:heart`, or just `heart` — Lucide,
//! ISC, vendored under `assets/icons/lucide/`), a project file (`icons/logo.svg`), an `https://`
//! URL, or inline markup or path data in the document. The engine never fetches (P10): a named
//! icon becomes a request the host — or, for published charts, the build step — answers, and a
//! bundle carries only the icons its chart draws ([`Icon::to_json`]), with the set's licence in
//! the manifest.
//!
//! Sans-IO and deterministic: the same bytes give the same layers on every platform.

// The SVG reader (XML, a CSS subset, the SVG subset) is for the tools: the build step, `datars dev`
// and the native hosts' development mode read files and markup with it. The web runtime is built
// without it (`svg` off) — bundles carry their icons already read ([`Icon::to_json`]), and a raw
// document gets the default set as compact files beside the runtime (`datars:icons/…`).
#[cfg(feature = "svg")]
pub mod css;
/// SVG path data (the reader and writers live with `PathData`, in `datars-math`).
pub use datars_math::svgpath as path;
pub mod set;
#[cfg(feature = "svg")]
pub mod svg;
#[cfg(feature = "svg")]
pub mod xml;

pub use datars_math::svgpath::{exact_path_string, parse_svg_path, svg_path_string};
pub use set::{Set, Source, DEFAULT_SET};
#[cfg(feature = "svg")]
pub use svg::parse_svg;

/// Read an icon file's bytes: the compact form ([`Icon::to_json`], what bundles and the web
/// runtime's default set carry), or SVG markup where the reader is compiled in (`svg`).
pub fn read_icon(bytes: &[u8]) -> Result<Icon, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "not text (an icon file is SVG or its compact JSON)".to_string())?;
    if text.trim_start().starts_with('{') {
        return Icon::from_json(text);
    }
    read_svg(text)
}

/// SVG markup read into an icon — or, without the reader (`svg` off: the web runtime), why not.
pub fn read_svg(markup: &str) -> Result<Icon, String> {
    #[cfg(feature = "svg")]
    return parse_svg(markup);
    #[cfg(not(feature = "svg"))]
    {
        let _ = markup;
        Err(NO_SVG_READER.into())
    }
}

/// What a runtime without the SVG reader says of markup it was handed.
pub const NO_SVG_READER: &str = "this runtime reads no SVG: publish the chart or use `datars dev` (they read its icons), or serve `datars icons --compact` output";

use datars_math::{Affine, FillRule, PathData, Rect, Vec2};
use datars_scene::{Cap, IconGlyph, IconInk, IconLayer, IconStroke, Join};
use datars_theme::Ink;
use serde_json::{json, Value};
use std::sync::Arc;

/// An icon's drawing in its own box.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    /// The box the drawing is made in (an SVG's `viewBox`).
    pub view_box: Rect,
    pub layers: Vec<Layer>,
    /// What each ink slot stood for in the file (`#e30613`), in slot order — for authors and
    /// tools deciding which ink to pass; never drawn.
    pub slots: Vec<String>,
    /// The file's `<title>`, if any: a starting point for an accessible label.
    pub title: Option<String>,
    /// What the reader skipped (text, clipping, filters), for diagnostics.
    pub notes: Vec<String>,
}

/// One path, filled and/or stroked, in the icon's box units.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub path: PathData,
    pub fill: Option<IconInk>,
    pub rule: FillRule,
    /// Width in box units.
    pub stroke: Option<IconStroke>,
    pub opacity: f64,
}

impl Icon {
    /// An icon from path data alone (`icon({ d: "M…" })`): filled in the node's ink, or stroked
    /// `width` units wide with round ends (`stroke`), in `view_box` (default: the path's bounds).
    pub fn from_path(d: &str, view_box: Option<Rect>, stroke: Option<f64>) -> Result<Icon, String> {
        let path = parse_svg_path(d);
        if path.is_empty() {
            return Err(format!("no path in `{}`", d.chars().take(40).collect::<String>()));
        }
        let view_box = match view_box.filter(|v| v.w > 0.0 && v.h > 0.0) {
            Some(v) => v,
            None => {
                let b = path.bounds();
                let pad = stroke.unwrap_or(0.0) / 2.0;
                Rect::new(b.x - pad, b.y - pad, b.w + 2.0 * pad, b.h + 2.0 * pad)
            }
        };
        let layer = match stroke.filter(|w| *w > 0.0) {
            Some(w) => Layer { path, fill: None, rule: FillRule::NonZero, stroke: Some(IconStroke { ink: IconInk::Current, width: w, cap: Cap::Round, join: Join::Round }), opacity: 1.0 },
            None => Layer { path, fill: Some(IconInk::Current), rule: FillRule::NonZero, stroke: None, opacity: 1.0 },
        };
        Ok(Icon { view_box, layers: vec![layer], slots: Vec::new(), title: None, notes: Vec::new() })
    }

    /// The transform that puts the box, aspect kept, centred in `target` (SVG's `xMidYMid meet`).
    pub fn fit(&self, target: Rect) -> Affine {
        let vb = self.view_box;
        let s = (target.w / vb.w.max(1e-9)).min(target.h / vb.h.max(1e-9));
        let c = target.center();
        Affine::translate(c.x, c.y).mul(Affine::scale(s, s)).mul(Affine::translate(-(vb.x + vb.w / 2.0), -(vb.y + vb.h / 2.0)))
    }

    /// The scale [`Icon::fit`] draws at in `target` (what stroke widths multiply by).
    pub fn fit_scale(&self, target: Rect) -> f64 {
        (target.w / self.view_box.w.max(1e-9)).min(target.h / self.view_box.h.max(1e-9))
    }

    /// The drawing normalized for instances and markers: its box centred on the origin, the
    /// longer side spanning −1…1, with the inks of its slots.
    pub fn glyph(&self, name: &str, inks: Vec<Ink>) -> IconGlyph {
        let vb = self.view_box;
        let half = (vb.w.max(vb.h) / 2.0).max(1e-9);
        let xf = Affine::scale(1.0 / half, 1.0 / half).mul(Affine::translate(-(vb.x + vb.w / 2.0), -(vb.y + vb.h / 2.0)));
        IconGlyph {
            name: Arc::from(name),
            layers: self
                .layers
                .iter()
                .map(|l| IconLayer {
                    path: Arc::new(l.path.transform(&xf)),
                    fill: l.fill,
                    rule: l.rule,
                    stroke: l.stroke.as_ref().map(|s| IconStroke { width: s.width / half, ..s.clone() }),
                    opacity: l.opacity,
                })
                .collect(),
            inks,
            aspect: vb.w / vb.h.max(1e-9),
        }
    }

    /// How many ink slots its layers use (inks to pass beyond the node's own).
    pub fn slot_count(&self) -> usize {
        let mut n = 0;
        for l in &self.layers {
            for i in l.fill.iter().chain(l.stroke.as_ref().map(|s| &s.ink)) {
                if let IconInk::Slot(k) = i {
                    n = n.max(*k as usize + 1);
                }
            }
        }
        n
    }

    /// The compact form bundles carry (only the drawing: no notes), exact — it reads back to the
    /// same icon, so a published chart draws what its author saw. Its layers are the scene's
    /// glyph layers ([`datars_scene::icon::layer_json`]).
    pub fn to_json(&self) -> String {
        let layers: Vec<Value> = self.layers.iter().map(|l| datars_scene::icon::layer_json(&l.path, l.fill, l.rule, l.stroke.as_ref(), l.opacity)).collect();
        let vb = self.view_box;
        let mut o = json!({ "vb": [vb.x, vb.y, vb.w, vb.h], "layers": layers });
        if !self.slots.is_empty() {
            o["slots"] = json!(self.slots);
        }
        if let Some(t) = &self.title {
            o["title"] = json!(t);
        }
        o.to_string()
    }

    pub fn from_json(s: &str) -> Result<Icon, String> {
        let v: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Icon::from_value(&v)
    }

    pub fn from_value(v: &Value) -> Result<Icon, String> {
        let vb: Vec<f64> = v["vb"].as_array().ok_or("an icon without `vb`")?.iter().filter_map(|x| x.as_f64()).collect();
        if vb.len() != 4 {
            return Err("an icon's `vb` is four numbers".into());
        }
        let layers = v["layers"]
            .as_array()
            .ok_or("an icon without `layers`")?
            .iter()
            .map(|l| {
                let (path, fill, rule, stroke, opacity) = datars_scene::icon::layer_of(l);
                Layer { path, fill, rule, stroke, opacity }
            })
            .collect();
        let slots = v["slots"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(Icon { view_box: Rect::new(vb[0], vb[1], vb[2], vb[3]), layers, slots, title: v["title"].as_str().map(String::from), notes: Vec::new() })
    }

    /// The drawing's layers placed in `target` (see [`Icon::fit`]): each path, and its stroke
    /// width in target units.
    pub fn placed(&self, target: Rect) -> Vec<(PathData, &Layer, f64)> {
        let xf = self.fit(target);
        let s = self.fit_scale(target);
        self.layers.iter().map(|l| (l.path.transform(&xf), l, l.stroke.as_ref().map_or(0.0, |st| st.width * s))).collect()
    }
}

/// Where an anchor name puts a box of `w` × `h` relative to its point: the box's top-left offset
/// (`center`, `top`, `bottom-left`, …; unknown: centred).
pub fn anchor_offset(anchor: &str, w: f64, h: f64) -> Vec2 {
    let (ax, ay) = match anchor {
        "top-left" | "start-top" => (0.0, 0.0),
        "top" => (0.5, 0.0),
        "top-right" | "end-top" => (1.0, 0.0),
        "left" | "start" => (0.0, 0.5),
        "right" | "end" => (1.0, 0.5),
        "bottom-left" | "start-bottom" => (0.0, 1.0),
        "bottom" => (0.5, 1.0),
        "bottom-right" | "end-bottom" => (1.0, 1.0),
        _ => (0.5, 0.5),
    };
    Vec2::new(-ax * w, -ay * h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_fits_centred_with_its_aspect() {
        let i = Icon::from_path("M0 0H20V10H0Z", None, None).unwrap();
        assert_eq!(i.view_box, Rect::new(0.0, 0.0, 20.0, 10.0));
        let xf = i.fit(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(xf.apply(Vec2::new(0.0, 0.0)), Vec2::new(0.0, 2.5));
        assert_eq!(xf.apply(Vec2::new(20.0, 10.0)), Vec2::new(10.0, 7.5));
    }

    #[test]
    fn a_glyph_spans_minus_one_to_one_on_its_longer_side() {
        let i = Icon::from_path("M0 0H24V12H0Z", Some(Rect::new(0.0, 0.0, 24.0, 12.0)), Some(2.0)).unwrap();
        let g = i.glyph("x", Vec::new());
        let b = g.layers[0].path.bounds();
        assert_eq!((b.x, b.y, b.w, b.h), (-1.0, -0.5, 2.0, 1.0));
        assert_eq!(g.layers[0].stroke.as_ref().unwrap().width, 2.0 / 12.0);
        assert_eq!(g.half_extent(), (1.0, 0.5));
    }

    #[test]
    fn a_file_is_read_in_its_compact_form_or_as_markup_where_the_reader_is() {
        let i = Icon::from_path("M0 0H20V10H0Z", None, Some(2.0)).unwrap();
        assert_eq!(read_icon(i.to_json().as_bytes()).unwrap(), i);
        let svg = br#"<svg viewBox="0 0 4 4"><rect width="4" height="4"/></svg>"#;
        #[cfg(feature = "svg")]
        assert_eq!(read_icon(svg).unwrap().view_box, Rect::new(0.0, 0.0, 4.0, 4.0));
        // The web runtime's build: markup is refused, saying what to do instead.
        #[cfg(not(feature = "svg"))]
        assert!(read_icon(svg).unwrap_err().contains("datars dev"));
    }

    #[test]
    fn anchors_place_the_box() {
        assert_eq!(anchor_offset("center", 10.0, 4.0), Vec2::new(-5.0, -2.0));
        assert_eq!(anchor_offset("bottom", 10.0, 4.0), Vec2::new(-5.0, -4.0));
        assert_eq!(anchor_offset("top-left", 10.0, 4.0), Vec2::new(0.0, 0.0));
    }
}
