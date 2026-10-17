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

pub mod css;
pub mod path;
pub mod set;
pub mod svg;
pub mod xml;

pub use path::{exact_path_string, parse_svg_path, svg_path_string};
pub use set::{Set, Source, DEFAULT_SET};
pub use svg::parse_svg;

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
    /// same icon, so a published chart draws what its author saw.
    pub fn to_json(&self) -> String {
        let ink = |i: &IconInk| match i {
            IconInk::Current => json!("c"),
            IconInk::Slot(k) => json!(k),
        };
        let layers: Vec<Value> = self
            .layers
            .iter()
            .map(|l| {
                let mut o = serde_json::Map::new();
                o.insert("d".into(), exact_path_string(&l.path).into());
                if let Some(f) = &l.fill {
                    o.insert("fill".into(), ink(f));
                }
                if l.rule == FillRule::EvenOdd {
                    o.insert("rule".into(), "evenodd".into());
                }
                if let Some(s) = &l.stroke {
                    o.insert("stroke".into(), json!({ "ink": ink(&s.ink), "w": s.width, "cap": s.cap, "join": s.join }));
                }
                if l.opacity != 1.0 {
                    o.insert("o".into(), l.opacity.into());
                }
                Value::Object(o)
            })
            .collect();
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
        let ink = |x: &Value| -> Option<IconInk> {
            match x {
                Value::String(s) if s == "c" => Some(IconInk::Current),
                Value::Number(n) => n.as_u64().map(|k| IconInk::Slot(k.min(255) as u8)),
                _ => None,
            }
        };
        let mut layers = Vec::new();
        for l in v["layers"].as_array().ok_or("an icon without `layers`")? {
            let stroke = l.get("stroke").filter(|s| s.is_object()).map(|s| IconStroke {
                ink: ink(&s["ink"]).unwrap_or(IconInk::Current),
                width: s["w"].as_f64().unwrap_or(1.0),
                cap: serde_json::from_value(s["cap"].clone()).unwrap_or_default(),
                join: serde_json::from_value(s["join"].clone()).unwrap_or_default(),
            });
            layers.push(Layer {
                path: parse_svg_path(l["d"].as_str().unwrap_or("")),
                fill: l.get("fill").and_then(ink),
                rule: if l["rule"].as_str() == Some("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero },
                stroke,
                opacity: l["o"].as_f64().unwrap_or(1.0),
            });
        }
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
    fn anchors_place_the_box() {
        assert_eq!(anchor_offset("center", 10.0, 4.0), Vec2::new(-5.0, -2.0));
        assert_eq!(anchor_offset("bottom", 10.0, 4.0), Vec2::new(-5.0, -4.0));
        assert_eq!(anchor_offset("top-left", 10.0, 4.0), Vec2::new(0.0, 0.0));
    }
}
