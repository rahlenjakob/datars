//! Scale declarations → scales (datars-data), resolved in the context of a layout box.

use crate::env::str_of;
use crate::resolve::{Cx, Resolver};
use datars_data::scale::{Colors, DomainOptions, LabelHint, Outputs, ScaleKind, TimeInterval};
use datars_data::{Column, Scale, Value as DValue};
use datars_expr::Value;
use datars_ir::{Prop, ScaleDecl};
use datars_math::{Hash64, Rect};
use datars_theme::Ink;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct EngineScale {
    pub scale: Scale,
    pub time: bool,
}

pub(crate) fn to_d(v: &Value) -> DValue {
    match v {
        Value::Null => DValue::Null,
        Value::Num(n) => DValue::Num(*n),
        Value::Str(s) => DValue::Str(s.clone()),
        Value::Bool(b) => DValue::Bool(*b),
    }
}

pub(crate) fn from_d(v: &DValue) -> Value {
    match v {
        DValue::Null => Value::Null,
        DValue::Bool(b) => Value::Bool(*b),
        DValue::Num(n) => Value::Num(*n),
        DValue::Date(d) => Value::Num(*d as f64),
        DValue::Str(s) => Value::Str(s.clone()),
    }
}

impl EngineScale {
    pub fn map_value(&self, v: &Value) -> Value {
        if self.scale.is_color() {
            return Value::Str(Arc::from(self.scale.map_ink(to_d(v)).to_string().as_str()));
        }
        let dv = match (v, self.time) {
            (Value::Num(n), true) => DValue::Date(n.round() as i32),
            // An ISO date written in a document (`span({ from: "2024-03-01" })`) is a date too.
            (Value::Str(s), true) => datars_data::date::parse_date(s).map_or_else(|| to_d(v), DValue::Date),
            _ => to_d(v),
        };
        Value::Num(self.scale.map_num(dv))
    }
    pub fn ink(&self, v: &Value) -> Ink {
        self.scale.map_ink(to_d(v))
    }
    pub fn bandwidth(&self) -> f64 {
        self.scale.bandwidth()
    }
    pub fn step(&self) -> f64 {
        self.scale.step()
    }
    pub fn invert(&self, px: f64) -> f64 {
        self.scale.invert_num(px).unwrap_or(f64::NAN)
    }
    pub fn range_min(&self) -> f64 {
        self.scale.range().map(|r| r[0].min(r[1])).unwrap_or(0.0)
    }
    pub fn range_max(&self) -> f64 {
        self.scale.range().map(|r| r[0].max(r[1])).unwrap_or(0.0)
    }
    pub fn domain_values(&self) -> Vec<Value> {
        self.scale.domain_values().map(|d| d.iter().map(from_d).collect()).unwrap_or_default()
    }
    /// Ticks with formatted labels. `count` 0 = from the range length.
    /// A value as a reader should see it in a tooltip or label: a date in full on a time scale,
    /// a number with at most two decimals (grouped from 10,000), a category as itself.
    pub fn label(&self, v: &Value, locale: &str) -> String {
        match (self.time, v) {
            (true, Value::Num(n)) => datars_text::format::date(*n as i64, "%-d %b %Y", locale),
            (false, Value::Num(n)) => {
                let spec = if n.abs() >= 10_000.0 { ",.2~f" } else { ".2~f" };
                datars_text::format::number(*n, spec, locale)
            }
            (_, other) => str_of(other),
        }
    }

    pub fn ticks(&self, count: usize, locale: &str) -> Vec<TickOut> {
        let len = (self.range_max() - self.range_min()).abs();
        let n = if count > 0 { count } else { ((len / 64.0).round() as usize).clamp(2, 10) };
        let ticks = self.scale.ticks(n);
        // Digit grouping only once some tick needs it (≥ 10,000): years and 4-digit values read
        // better ungrouped (Chicago, AP and most European style guides agree), and one axis stays
        // consistent.
        let group = ticks.iter().any(|t| t.value.as_f64().is_some_and(|v| v.abs() >= 10_000.0));
        let grouping = if group { "," } else { "" };
        ticks
            .into_iter()
            .map(|t| {
                let v = from_d(&t.value);
                let (kind, major) = match t.label_hint {
                    LabelHint::Number { .. } => ("number", false),
                    LabelHint::Log { major, .. } => ("log", major),
                    LabelHint::Time { interval, boundary, .. } => ("time", boundary > interval),
                    LabelHint::Category => ("category", false),
                };
                let label = match t.label_hint {
                    LabelHint::Number { decimals } | LabelHint::Log { decimals, .. } => datars_text::format::number(t.value.as_f64().unwrap_or(f64::NAN), &format!("{grouping}.{decimals}f"), locale),
                    LabelHint::Time { interval, boundary, .. } => {
                        let spec = match (interval, boundary) {
                            (TimeInterval::Year, _) | (_, TimeInterval::Year) => "%Y",
                            (TimeInterval::Quarter | TimeInterval::Month, _) => "%b",
                            _ => datars_text::locale::get(locale).day_month,
                        };
                        datars_text::format::date(t.value.as_f64().unwrap_or(0.0) as i64, spec, locale)
                    }
                    LabelHint::Category => str_of(&v),
                };
                TickOut { value: v, label, kind, major }
            })
            .collect()
    }
}

/// A tick as templates see it (`repeat({ ticks })` rows: `d.value`, `d.label`, `d.kind`,
/// `d.major`, plus `d.index` and `d.pos`).
pub struct TickOut {
    pub value: Value,
    pub label: String,
    /// `"number"`, `"log"`, `"time"` or `"category"`. Category ticks come one per category and
    /// an axis thins their labels when they crowd; the others are already spaced for the axis's
    /// length (calendar ticks over dates on a band scale too).
    pub kind: &'static str,
    /// A tick that starts a larger unit than its neighbours: a log decade, a year among months.
    pub major: bool,
}

/// Scales visible at a point in the template: a chain of scopes.
#[derive(Default)]
pub struct Scope {
    pub parent: Option<Rc<Scope>>,
    pub scales: BTreeMap<String, EngineScale>,
    pub fingerprint: u64,
}

impl Scope {
    pub fn get(&self, name: &str) -> Option<&EngineScale> {
        self.scales.get(name).or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }
}

fn palette_name(v: &serde_json::Value) -> Option<String> {
    v.as_str().and_then(|s| s.strip_prefix('$')).map(String::from)
}

/// Build the scales declared on a template node.
pub(crate) fn build_scope(r: &Resolver, decls: &BTreeMap<String, ScaleDecl>, cx: &Cx, boxes: Option<&BTreeMap<String, Rect>>) -> Scope {
    let mut h = Hash64::new();
    h.u64(cx.scope.fingerprint);
    h.f64(cx.box_w);
    h.f64(cx.box_h);
    let mut scales = BTreeMap::new();
    for (name, d) in decls {
        match build(r, d, cx, boxes) {
            Ok(s) => {
                h.str(name);
                h.str(&serde_json::to_string(&s.scale).unwrap_or_default());
                scales.insert(name.clone(), s);
            }
            Err(e) => r.diag(format!("scale `{name}`: {e}")),
        }
    }
    Scope { parent: Some(cx.scope.clone()), scales, fingerprint: h.finish() }
}

/// The domain's data: one or more columns, or literal values.
fn domain_column(r: &Resolver, d: &ScaleDecl, cx: &Cx) -> Result<(Column, bool), String> {
    let dom = &d.domain;
    if let Some(arr) = dom.as_array().or_else(|| dom.get("values").and_then(|v| v.as_array())) {
        let nums: Option<Vec<f64>> = arr.iter().map(|v| v.as_f64()).collect();
        return Ok(match nums {
            Some(n) => (Column::Num(n), false),
            None => (Column::Str(arr.iter().map(|v| Some(Arc::from(v.as_str().map(String::from).unwrap_or_else(|| v.to_string()).as_str()))).collect()), false),
        });
    }
    let table = dom.get("data").and_then(|t| t.as_str()).ok_or("domain needs `data` + `field`, `fields` or `expr`, or a literal list")?;
    let t = r.table_for(table, cx).ok_or_else(|| format!("unknown table `{table}`"))?;
    // `@group` (the current group's rows, e.g. one small multiple) has no stored original.
    let original = if table == "@group" { t.clone() } else { crate::tables::get_original(r, table, cx).ok_or_else(|| format!("unknown table `{table}`"))? };
    if let Some(f) = dom.get("field").and_then(|f| f.as_str()) {
        let is_date = matches!(original.column(f), Some(Column::Date(_)));
        // Categorical colours are identities: a filter must not reassign them, so their domain is
        // the field's values in the root source when it has the field (positions do re-lay out).
        if matches!(d.ty.as_str(), "categorical" | "ordinal") {
            if let Some(Column::Str(v)) = crate::tables::root_source(r, table).and_then(|s| s.column(f).cloned()) {
                return Ok((Column::Str(v), false));
            }
        }
        let col = t.column(f).ok_or_else(|| format!("no column `{f}` in `{table}`"))?.clone();
        return Ok((col, is_date));
    }
    if let Some(fs) = dom.get("fields").and_then(|f| f.as_array()) {
        let mut all = Vec::new();
        for f in fs.iter().filter_map(|f| f.as_str()) {
            if let Some(Column::Num(v)) = t.column(f) {
                all.extend_from_slice(v);
            }
        }
        return Ok((Column::Num(all), false));
    }
    if let Some(e) = dom.get("expr").and_then(|e| e.as_str()) {
        return Ok((Column::Num(r.column_values(&Prop::expr(e), &t, cx)), false));
    }
    Err("domain needs `field`, `fields` or `expr`".into())
}

fn range(r: &Resolver, d: &ScaleDecl, cx: &Cx, boxes: Option<&BTreeMap<String, Rect>>) -> [f64; 2] {
    let (w, h) = (cx.box_w, cx.box_h);
    match &d.range {
        serde_json::Value::String(s) => match s.as_str() {
            "width" => [0.0, w],
            "height" => [0.0, h],
            "-height" => [h, 0.0],
            "-width" => [w, 0.0],
            _ => [0.0, w],
        },
        serde_json::Value::Array(a) if a.len() == 2 && !a.iter().any(|x| x.as_str().is_some_and(|s| s.starts_with('#') || s.starts_with('$'))) => {
            [r.num(&Prop(a[0].clone()), cx, 0.0), r.num(&Prop(a[1].clone()), cx, w)]
        }
        serde_json::Value::Object(o) if o.contains_key("box") => {
            let id = o["box"].as_str().unwrap_or("");
            let axis = o.get("axis").and_then(|a| a.as_str()).unwrap_or("x");
            let b = boxes.and_then(|m| m.get(id)).copied().unwrap_or(Rect::new(0.0, 0.0, w, h));
            match axis {
                "x" => [0.0, b.w],
                "-x" => [b.w, 0.0],
                "y" => [0.0, b.h],
                _ => [b.h, 0.0],
            }
        }
        _ => [0.0, w],
    }
}

fn build(r: &Resolver, d: &ScaleDecl, cx: &Cx, boxes: Option<&BTreeMap<String, Rect>>) -> Result<EngineScale, String> {
    let (col, is_date) = domain_column(r, d, cx)?;
    let rng = range(r, d, cx, boxes);
    let len = (rng[1] - rng[0]).abs();
    let nice = if d.nice { Some(((len / 64.0).round() as usize).clamp(2, 10)) } else { None };
    let ty = if is_date && matches!(d.ty.as_str(), "linear" | "time") { "time" } else { d.ty.as_str() };
    // An explicit domain is exact: no zero, no nice rounding.
    let explicit = d.domain.is_array();
    let nice = if explicit { None } else { nice };
    let opts = DomainOptions { zero: d.zero && !explicit, nice, palette: palette_name(&d.range) };
    let colors = || -> Colors {
        match &d.range {
            serde_json::Value::Array(a) => Colors::Stops(a.iter().filter_map(|x| x.as_str()).filter_map(datars_color::Color::parse).collect()),
            v => Colors::Palette(palette_name(v).unwrap_or_else(|| if ty == "diverging" { "diverging".into() } else { "sequential".into() })),
        }
    };
    let mut scale = match ty {
        "linear" | "sqrt" | "log" | "symlog" | "time" | "pow" => {
            let kind = match ty {
                "sqrt" => ScaleKind::Sqrt,
                "log" => ScaleKind::Log,
                "symlog" => ScaleKind::Symlog,
                "time" => ScaleKind::Time,
                _ => ScaleKind::Linear,
            };
            let col = if is_date { col } else { Column::Num(col.to_f64()) };
            let s = Scale::from_data_with(&col, kind, &opts);
            // `pow`: the linear scale's domain (zero, nice) through `|x|^exponent`. An exponent of
            // 0 would collapse every value onto one: linear instead, and say so.
            let exponent = d.exponent.filter(|_| ty == "pow");
            if let Some(bad) = exponent.filter(|x| !x.is_finite() || *x == 0.0) {
                r.diag(format!("pow scale: exponent {bad} maps every value to one place; using 1"));
            }
            let s = match (s, exponent.filter(|x| x.is_finite() && *x != 0.0)) {
                (Scale::Linear(c), Some(exponent)) => Scale::Pow { exponent, scale: c },
                (s, _) => s,
            };
            s.with_range(rng)
        }
        "band" | "point" => {
            let kind = if ty == "band" { ScaleKind::Band } else { ScaleKind::Point };
            let p = d.padding.unwrap_or(0.1);
            // Dates reach expressions as day numbers; slots keep them dates (calendar order and
            // calendar ticks).
            let col = if is_date { Column::Date(col.to_f64().iter().map(|x| x.is_finite().then(|| x.round() as i32)).collect()) } else { col };
            Scale::from_data_with(&col, kind, &opts).with_range(rng).with_padding(p, p / 2.0)
        }
        "ordinal" => Scale::from_data_with(&col, ScaleKind::Ordinal, &opts),
        "quantize" => Scale::from_data_with(&col, ScaleKind::Quantize, &opts),
        "quantile" => Scale::from_data_with(&col, ScaleKind::Quantile, &opts),
        "categorical" => {
            let mut s = Scale::from_data_with(&col, ScaleKind::Categorical, &opts);
            // Key metadata colours (party colours are data) override the palette.
            for (k, meta) in &r.doc.keys {
                if let Some(ink) = meta.color.as_deref().and_then(Ink::parse) {
                    s = s.with_override(DValue::Str(Arc::from(k.as_str())), ink);
                }
            }
            s
        }
        "sequential" => {
            let ext = extent(&col);
            Scale::sequential(ext, colors())
        }
        "diverging" => {
            let ext = extent(&col);
            let mid = d.params.get("mid").and_then(|m| m.as_f64()).unwrap_or(0.0);
            let m = (ext[0] - mid).abs().max((ext[1] - mid).abs());
            Scale::diverging([mid - m, mid, mid + m], colors())
        }
        "piecewise" => {
            let stops = d.params.get("stops");
            match stops {
                Some(serde_json::Value::String(s)) => datars_data::scale::Piecewise::parse(s).map(Scale::Piecewise).ok_or("bad piecewise stops")?,
                Some(serde_json::Value::Array(a)) => Scale::piecewise(a.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, Ink::parse(p.get(1)?.as_str()?)?))).collect()),
                _ => return Err("piecewise needs `stops`".into()),
            }
        }
        "threshold" => {
            let th: Vec<f64> = d.params.get("thresholds").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
            let inks: Vec<Ink> = d.range.as_array().map(|a| a.iter().filter_map(|x| x.as_str()).filter_map(Ink::parse).collect()).unwrap_or_default();
            Scale::threshold(th, Outputs::Ink(inks))
        }
        other => return Err(format!("unknown scale type `{other}`")),
    };
    if let Some(ov) = d.params.get("domain_override").and_then(|v| v.as_array()) {
        if let (Some(a), Some(b)) = (ov.first().and_then(|v| v.as_f64()), ov.get(1).and_then(|v| v.as_f64())) {
            scale = scale.with_domain_extent([a, b]);
        }
    }
    if d.params.get("clamp").and_then(|c| c.as_bool()).unwrap_or(false) {
        scale = scale.with_clamp(true);
    }
    // Dates one per band are time too (trading days without gaps): expressions hand dates over as
    // numbers, and labels write them as dates.
    Ok(EngineScale { scale, time: ty == "time" || (is_date && matches!(ty, "band" | "point")) })
}

fn extent(c: &Column) -> [f64; 2] {
    let v = c.to_f64();
    let lo = v.iter().copied().filter(|x| x.is_finite()).fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    if lo.is_finite() && hi.is_finite() { [lo, hi] } else { [0.0, 1.0] }
}
