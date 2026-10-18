//! An icon as scenes carry it where one drawing repeats: the prototype of icon instances (a
//! pictogram's thousand people, map markers) and stroke markers. Layers in a box centred on the
//! origin whose longer side spans −1…1, so an instance's `size` scales it like a symbol's radius.
//! (A single `icon` node resolves to plain shapes instead; see the engine.)
//!
//! Colours stay inks (late-bound, P1/themes): a layer paints in the instance's own colour
//! (`Current`, what an SVG's `currentColor` becomes) or in one of the glyph's `inks`.

use crate::paint::{Cap, Join};
use datars_math::svgpath::{exact_path_string, parse_svg_path};
use datars_math::{FillRule, PathData};
use datars_theme::Ink;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};
use std::sync::Arc;

// Glyphs travel in the compact form icons do everywhere (bundles' icons chunks, the web runtime's
// default set, baked scenes): a layer is `{"d", "fill", "rule", "stroke": {"ink", "w", "cap",
// "join"}, "o"}`, path data as exact SVG text and inks as `"c"` or a slot number. Read and written
// by hand, through `serde_json::Value`: derived code for these types would be a few KB of the web
// runtime for the one place they're read.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconInk {
    /// The instance's fill (the stroke's paint, for a marker).
    Current,
    /// `inks[i]` of the glyph.
    Slot(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IconStroke {
    pub ink: IconInk,
    /// In the glyph's units (the box spans 2).
    pub width: f64,
    pub cap: Cap,
    pub join: Join,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IconLayer {
    pub path: Arc<PathData>,
    pub fill: Option<IconInk>,
    pub rule: FillRule,
    pub stroke: Option<IconStroke>,
    pub opacity: f64,
}

/// An icon's drawing, normalized: what an instance or a marker repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct IconGlyph {
    /// Where it came from (`lucide:heart`, `logo.svg`): its identity, for caches and exports.
    pub name: Arc<str>,
    pub layers: Vec<IconLayer>,
    /// The inks of its other colours (a logo's brand red), by slot.
    pub inks: Vec<Ink>,
    /// Width ÷ height of its box (the longer side spans 2 units): hit tests and bounds.
    pub aspect: f64,
}

fn ink_json(i: &IconInk) -> Value {
    match i {
        IconInk::Current => json!("c"),
        IconInk::Slot(k) => json!(k),
    }
}

fn ink_of(v: &Value) -> Option<IconInk> {
    match v {
        Value::String(s) if s == "c" => Some(IconInk::Current),
        Value::Number(n) => n.as_u64().map(|k| IconInk::Slot(k.min(255) as u8)),
        _ => None,
    }
}

/// A layer in the compact form (exact: it reads back to the same layer).
pub fn layer_json(path: &PathData, fill: Option<IconInk>, rule: FillRule, stroke: Option<&IconStroke>, opacity: f64) -> Value {
    let mut o = Map::new();
    o.insert("d".into(), exact_path_string(path).into());
    if let Some(f) = &fill {
        o.insert("fill".into(), ink_json(f));
    }
    if rule == FillRule::EvenOdd {
        o.insert("rule".into(), "evenodd".into());
    }
    if let Some(s) = stroke {
        let cap = match s.cap {
            Cap::Butt => "butt",
            Cap::Round => "round",
            Cap::Square => "square",
        };
        let join = match s.join {
            Join::Miter => "miter",
            Join::Round => "round",
            Join::Bevel => "bevel",
        };
        o.insert("stroke".into(), json!({ "ink": ink_json(&s.ink), "w": s.width, "cap": cap, "join": join }));
    }
    if opacity != 1.0 {
        o.insert("o".into(), opacity.into());
    }
    Value::Object(o)
}

/// A layer read from the compact form: path, fill, rule, stroke, opacity.
pub fn layer_of(l: &Value) -> (PathData, Option<IconInk>, FillRule, Option<IconStroke>, f64) {
    let stroke = l.get("stroke").filter(|s| s.is_object()).map(|s| IconStroke {
        ink: ink_of(&s["ink"]).unwrap_or(IconInk::Current),
        width: s["w"].as_f64().unwrap_or(1.0),
        cap: match s["cap"].as_str() {
            Some("round") => Cap::Round,
            Some("square") => Cap::Square,
            _ => Cap::Butt,
        },
        join: match s["join"].as_str() {
            Some("round") => Join::Round,
            Some("bevel") => Join::Bevel,
            _ => Join::Miter,
        },
    });
    let rule = if l["rule"].as_str() == Some("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero };
    (parse_svg_path(l["d"].as_str().unwrap_or("")), l.get("fill").and_then(ink_of), rule, stroke, l["o"].as_f64().unwrap_or(1.0))
}

impl IconGlyph {
    /// The compact form: `{"name", "layers", "inks", "aspect"}`.
    pub fn to_json(&self) -> Value {
        let layers: Vec<Value> = self.layers.iter().map(|l| layer_json(&l.path, l.fill, l.rule, l.stroke.as_ref(), l.opacity)).collect();
        let mut o = json!({ "name": &*self.name, "layers": layers, "aspect": self.aspect });
        if !self.inks.is_empty() {
            o["inks"] = Value::Array(self.inks.iter().map(|i| Value::String(i.to_string())).collect());
        }
        o
    }

    pub fn from_json(v: &Value) -> Result<IconGlyph, String> {
        let layers = v["layers"].as_array().ok_or("an icon glyph without `layers`")?;
        Ok(IconGlyph {
            name: Arc::from(v["name"].as_str().unwrap_or("")),
            layers: layers
                .iter()
                .map(|l| {
                    let (path, fill, rule, stroke, opacity) = layer_of(l);
                    IconLayer { path: Arc::new(path), fill, rule, stroke, opacity }
                })
                .collect(),
            inks: v["inks"].as_array().map(|a| a.iter().filter_map(|i| i.as_str().and_then(Ink::parse)).collect()).unwrap_or_default(),
            aspect: v["aspect"].as_f64().unwrap_or(1.0),
        })
    }
}

impl Serialize for IconGlyph {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}

impl<'de> Deserialize<'de> for IconGlyph {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<IconGlyph, D::Error> {
        IconGlyph::from_json(&Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl IconGlyph {
    /// Half the box's width and height, in glyph units (the longer is 1).
    pub fn half_extent(&self) -> (f64, f64) {
        let a = if self.aspect.is_finite() && self.aspect > 0.0 { self.aspect } else { 1.0 };
        if a >= 1.0 {
            (1.0, 1.0 / a)
        } else {
            (a, 1.0)
        }
    }
}

impl IconGlyph {
    /// The whole drawing as one path at `c`, `s` px per glyph unit, and how to paint it as one
    /// shape: whether any layer fills, and the first stroke (its width in px) — so an icon can
    /// travel as a single shape (a pictogram's unit flying to a bar and back) and still look like
    /// itself at either end.
    pub fn as_shape(&self, c: datars_math::Vec2, s: f64) -> (PathData, bool, Option<IconStroke>) {
        let xf = datars_math::Affine::translate(c.x, c.y).mul(datars_math::Affine::scale(s, s));
        let mut p = PathData::new();
        for l in &self.layers {
            p.extend(&l.path.transform(&xf));
        }
        let fills = self.layers.iter().any(|l| l.fill.is_some());
        let stroke = self.layers.iter().find_map(|l| l.stroke.clone()).map(|st| IconStroke { width: st.width * s, ..st });
        (p, fills, stroke)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_reads_back_from_its_compact_form() {
        let g = IconGlyph {
            name: Arc::from("lucide:heart"),
            layers: vec![
                IconLayer { path: Arc::new(parse_svg_path("M-1 -0.5L1 0.25Z")), fill: Some(IconInk::Slot(1)), rule: FillRule::EvenOdd, stroke: None, opacity: 0.5 },
                IconLayer { path: Arc::new(parse_svg_path("M0 0C0.1 0.2 0.3 0.4 1 1")), fill: None, rule: FillRule::NonZero, stroke: Some(IconStroke { ink: IconInk::Current, width: 1.0 / 12.0, cap: Cap::Round, join: Join::Bevel }), opacity: 1.0 },
            ],
            inks: vec![Ink::parse("$accent").unwrap()],
            aspect: 0.75,
        };
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<IconGlyph>(&text).unwrap(), g);
        assert!(text.contains("\"d\":\"M") && text.contains("\"cap\":\"round\""), "{text}");
    }
}
