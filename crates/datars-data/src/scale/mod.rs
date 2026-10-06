//! Scales: data values → positions, sizes or inks (docs/04 §Scales).
//!
//! One serde-able [`Scale`] enum covers continuous (linear, log, sqrt, pow, symlog, time), discrete
//! (band, point, ordinal), discretizing (quantize, quantile, threshold) and colour (sequential,
//! diverging, categorical, piecewise) scales, with one API:
//!
//! - [`Scale::map_num`] — a number: a position for positional scales; for colour scales the ramp
//!   position t ∈ [0, 1] (sequential, diverging, piecewise) or the palette index (categorical, and
//!   ordinal / discretizing scales with ink outputs) — what a colour key needs.
//! - [`Scale::map_ink`] — an [`Ink`] (palette references stay late-bound). Anything the scale
//!   can't map (null, an unknown key without an `unknown` ink, a non-colour scale) gets `$muted`;
//!   [`Scale::try_map_ink`] returns `None` instead.
//! - [`Scale::invert`] — back from a number (a pixel, or a ramp position for colour scales).
//! - [`Scale::ticks`] — nice 1/2/5×10ᵏ ticks, log ticks, calendar-aware time ticks, or the domain
//!   of discrete scales, each with a [`LabelHint`] (formatting lives in `datars-text`).
//! - [`Scale::nice`], domain / range accessors, [`Scale::lerp`] for animated rescales, and
//!   [`Scale::from_data`] to compute domains from a column.
//!
//! Time scales work on dates as days since 1970-01-01 ([`TimeUnit::Days`], the unit of
//! [`crate::Column::Date`]) or on UTC timestamps in milliseconds ([`TimeUnit::Millis`]).

mod color;
mod continuous;
mod discrete;
pub mod domain;
pub mod ticks;

pub use color::{Categorical, Colors, Diverging, Piecewise, Sequential};
pub use continuous::{Continuous, Transform};
pub use discrete::{Band, Ordinal, Outputs, Point, Quantile, Quantize, Threshold};
pub use ticks::{LabelHint, Tick, TimeInterval};

/// The names docs/dev/contracts.md uses for the colour scales.
pub type ColorSeq = Sequential;
pub type ColorDiv = Diverging;
pub type ColorCat = Categorical;

/// What [`Scale::map`] produces: an ink from colour scales, a number from the rest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Mapped {
    Num(f64),
    Ink(Ink),
}

impl Mapped {
    pub fn num(&self) -> Option<f64> {
        match self {
            Mapped::Num(v) => Some(*v),
            Mapped::Ink(_) => None,
        }
    }
    pub fn ink(&self) -> Option<&Ink> {
        match self {
            Mapped::Ink(i) => Some(i),
            Mapped::Num(_) => None,
        }
    }
}

use crate::num;
use crate::table::Column;
use crate::value::Value;
use datars_theme::Ink;
use discrete::{bucket, bucket_extent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ticks::MS_DAY;

/// The unit of a time scale's domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeUnit {
    /// Days since 1970-01-01 (date columns). Ticks fall on whole days or coarser.
    #[default]
    Days,
    /// Milliseconds since 1970-01-01T00:00Z. Ticks can be hours, minutes, seconds.
    Millis,
}

/// A scale. JSON: internally tagged by `"type"`, e.g.
/// `{"type": "linear", "domain": [0, 100], "range": [0, 500]}`,
/// `{"type": "categorical", "domain": ["S", "M"], "palette": "categorical"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Scale {
    Linear(Continuous),
    Log {
        base: f64,
        #[serde(flatten)]
        scale: Continuous,
    },
    Sqrt(Continuous),
    Pow {
        exponent: f64,
        #[serde(flatten)]
        scale: Continuous,
    },
    Symlog {
        constant: f64,
        #[serde(flatten)]
        scale: Continuous,
    },
    Time {
        #[serde(default)]
        unit: TimeUnit,
        #[serde(flatten)]
        scale: Continuous,
    },
    Band(Band),
    Point(Point),
    Ordinal(Ordinal),
    Quantize(Quantize),
    Quantile(Quantile),
    Threshold(Threshold),
    Sequential(Sequential),
    Diverging(Diverging),
    Categorical(Categorical),
    Piecewise(Piecewise),
}

/// What [`Scale::from_data`] should build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleKind {
    Linear,
    Log,
    Sqrt,
    Symlog,
    Time,
    Band,
    Point,
    Ordinal,
    Quantize,
    Quantile,
    Sequential,
    Diverging,
    Categorical,
}

/// Options for [`Scale::from_data_with`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DomainOptions {
    /// Stretch numeric domains to include zero.
    pub zero: bool,
    /// Extend the domain to nice values for about this many ticks.
    pub nice: Option<usize>,
    /// Palette for colour scales (default `categorical`, `sequential` or `diverging`).
    pub palette: Option<String>,
}

fn muted() -> Ink {
    Ink::token("muted")
}

impl Scale {
    // ---- constructors ------------------------------------------------------------------------

    pub fn linear(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Linear(Continuous::new(domain, range))
    }
    pub fn log(base: f64, domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Log { base, scale: Continuous::new(domain, range) }
    }
    pub fn sqrt(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Sqrt(Continuous::new(domain, range))
    }
    pub fn pow(exponent: f64, domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Pow { exponent, scale: Continuous::new(domain, range) }
    }
    pub fn symlog(constant: f64, domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Symlog { constant, scale: Continuous::new(domain, range) }
    }
    /// A time scale over dates (days since 1970-01-01).
    pub fn time(domain_days: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Time { unit: TimeUnit::Days, scale: Continuous::new(domain_days, range) }
    }
    /// A time scale over UTC timestamps in milliseconds.
    pub fn time_ms(domain_ms: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::Time { unit: TimeUnit::Millis, scale: Continuous::new(domain_ms, range) }
    }
    pub fn band(domain: Vec<Value>, range: [f64; 2]) -> Scale {
        Scale::Band(Band::new(domain, range))
    }
    pub fn point(domain: Vec<Value>, range: [f64; 2]) -> Scale {
        Scale::Point(Point::new(domain, range))
    }
    pub fn ordinal(domain: Vec<Value>, range: Outputs) -> Scale {
        Scale::Ordinal(Ordinal { domain, range, unknown: None })
    }
    pub fn quantize(domain: [f64; 2], range: Outputs) -> Scale {
        Scale::Quantize(Quantize { domain, range })
    }
    pub fn quantile(samples: &[f64], range: Outputs) -> Scale {
        Scale::Quantile(Quantile::from_samples(samples, range))
    }
    pub fn threshold(thresholds: Vec<f64>, range: Outputs) -> Scale {
        Scale::Threshold(Threshold { thresholds, range })
    }
    pub fn sequential(domain: [f64; 2], colors: Colors) -> Scale {
        Scale::Sequential(Sequential { domain, colors, transform: Transform::Linear, clamp: true })
    }
    /// `domain = [low, mid, high]`.
    pub fn diverging(domain: [f64; 3], colors: Colors) -> Scale {
        Scale::Diverging(Diverging { domain, colors, transform: Transform::Linear, clamp: true })
    }
    pub fn categorical(domain: Vec<Value>, palette: &str) -> Scale {
        Scale::Categorical(Categorical { domain, palette: palette.trim_start_matches('$').to_string(), overrides: Vec::new(), unknown: None })
    }
    pub fn piecewise(stops: Vec<(f64, Ink)>) -> Scale {
        Scale::Piecewise(Piecewise::new(stops))
    }

    // ---- builders ----------------------------------------------------------------------------

    /// Clamp continuous outputs to the range (continuous, sequential, diverging scales).
    pub fn with_clamp(mut self, clamp: bool) -> Scale {
        match &mut self {
            Scale::Sequential(s) => s.clamp = clamp,
            Scale::Diverging(s) => s.clamp = clamp,
            s => {
                if let Some(c) = s.cont_mut() {
                    c.clamp = clamp;
                }
            }
        }
        self
    }
    /// Band paddings (fractions of the step); for point scales `outer` is the padding.
    pub fn with_padding(mut self, inner: f64, outer: f64) -> Scale {
        match &mut self {
            Scale::Band(b) => {
                b.padding_inner = inner;
                b.padding_outer = outer;
            }
            Scale::Point(p) => p.padding = outer,
            _ => {}
        }
        self
    }
    pub fn with_align(mut self, align: f64) -> Scale {
        match &mut self {
            Scale::Band(b) => b.align = align,
            Scale::Point(p) => p.align = align,
            _ => {}
        }
        self
    }
    pub fn with_round(mut self, round: bool) -> Scale {
        match &mut self {
            Scale::Band(b) => b.round = round,
            Scale::Point(p) => p.round = round,
            _ => {}
        }
        self
    }
    /// Pin an ink to a key (categorical scales).
    pub fn with_override(mut self, key: impl Into<Value>, ink: Ink) -> Scale {
        if let Scale::Categorical(c) = &mut self {
            let key = key.into();
            c.overrides.retain(|(k, _)| *k != key);
            c.overrides.push((key, ink));
        }
        self
    }
    /// The ink for keys outside the domain (categorical and ordinal scales).
    pub fn with_unknown(mut self, ink: Ink) -> Scale {
        match &mut self {
            Scale::Categorical(c) => c.unknown = Some(ink),
            Scale::Ordinal(o) => o.unknown = Some(ink),
            _ => {}
        }
        self
    }
    /// The transform of a sequential or diverging colour scale (e.g. log colour ramps).
    pub fn with_transform(mut self, tf: Transform) -> Scale {
        match &mut self {
            Scale::Sequential(s) => s.transform = tf,
            Scale::Diverging(s) => s.transform = tf,
            _ => {}
        }
        self
    }
    /// Steps between piecewise stops instead of blending.
    pub fn with_stepped(mut self, stepped: bool) -> Scale {
        if let Scale::Piecewise(p) = &mut self {
            p.stepped = stepped;
        }
        self
    }
    pub fn with_domain_extent(mut self, d: [f64; 2]) -> Scale {
        self.set_domain_extent(d);
        self
    }
    pub fn with_domain_values(mut self, d: Vec<Value>) -> Scale {
        self.set_domain_values(d);
        self
    }
    pub fn with_range(mut self, r: [f64; 2]) -> Scale {
        self.set_range(r);
        self
    }

    // ---- structure -----------------------------------------------------------------------------

    pub fn kind_name(&self) -> &'static str {
        match self {
            Scale::Linear(_) => "linear",
            Scale::Log { .. } => "log",
            Scale::Sqrt(_) => "sqrt",
            Scale::Pow { .. } => "pow",
            Scale::Symlog { .. } => "symlog",
            Scale::Time { .. } => "time",
            Scale::Band(_) => "band",
            Scale::Point(_) => "point",
            Scale::Ordinal(_) => "ordinal",
            Scale::Quantize(_) => "quantize",
            Scale::Quantile(_) => "quantile",
            Scale::Threshold(_) => "threshold",
            Scale::Sequential(_) => "sequential",
            Scale::Diverging(_) => "diverging",
            Scale::Categorical(_) => "categorical",
            Scale::Piecewise(_) => "piecewise",
        }
    }

    /// The transform and domain/range of a continuous scale.
    pub fn continuous(&self) -> Option<(Transform, &Continuous)> {
        Some(match self {
            Scale::Linear(c) => (Transform::Linear, c),
            Scale::Log { base, scale } => (Transform::Log { base: *base }, scale),
            Scale::Sqrt(c) => (Transform::Sqrt, c),
            Scale::Pow { exponent, scale } => (Transform::Pow { exponent: *exponent }, scale),
            Scale::Symlog { constant, scale } => (Transform::Symlog { constant: *constant }, scale),
            Scale::Time { scale, .. } => (Transform::Linear, scale),
            _ => return None,
        })
    }
    fn cont_mut(&mut self) -> Option<&mut Continuous> {
        match self {
            Scale::Linear(c) | Scale::Sqrt(c) => Some(c),
            Scale::Log { scale, .. } | Scale::Pow { scale, .. } | Scale::Symlog { scale, .. } | Scale::Time { scale, .. } => Some(scale),
            _ => None,
        }
    }
    /// Whether [`Scale::map_ink`] is meaningful.
    pub fn is_color(&self) -> bool {
        match self {
            Scale::Sequential(_) | Scale::Diverging(_) | Scale::Categorical(_) | Scale::Piecewise(_) => true,
            Scale::Ordinal(Ordinal { range, .. }) | Scale::Quantize(Quantize { range, .. }) | Scale::Quantile(Quantile { range, .. }) | Scale::Threshold(Threshold { range, .. }) => {
                matches!(range, Outputs::Ink(_))
            }
            _ => false,
        }
    }

    /// The numeric domain [low, high] (continuous, quantize, quantile, colour ramps, piecewise).
    pub fn domain_extent(&self) -> Option<[f64; 2]> {
        if let Some((_, c)) = self.continuous() {
            return Some(c.domain);
        }
        match self {
            Scale::Quantize(q) => Some(q.domain),
            Scale::Quantile(q) => Some(q.extent),
            Scale::Threshold(t) => Some([*t.thresholds.first()?, *t.thresholds.last()?]),
            Scale::Sequential(s) => Some(s.domain),
            Scale::Diverging(d) => Some([d.domain[0], d.domain[2]]),
            Scale::Piecewise(p) => Some([p.stops.first()?.0, p.stops.last()?.0]),
            _ => None,
        }
    }

    /// Set the numeric domain. Diverging scales keep their midpoint; piecewise stops are rescaled
    /// proportionally; quantile and threshold scales are unchanged (their cuts are explicit).
    pub fn set_domain_extent(&mut self, d: [f64; 2]) {
        if let Some(c) = self.cont_mut() {
            c.domain = d;
            return;
        }
        match self {
            Scale::Quantize(q) => q.domain = d,
            Scale::Sequential(s) => s.domain = d,
            Scale::Diverging(s) => s.domain = [d[0], s.domain[1], d[1]],
            Scale::Piecewise(p) => {
                if let (Some(a), Some(b)) = (p.stops.first().map(|s| s.0), p.stops.last().map(|s| s.0)) {
                    for s in &mut p.stops {
                        let t = if b > a { (s.0 - a) / (b - a) } else { 0.0 };
                        s.0 = datars_math::lerp(d[0], d[1], t);
                    }
                    p.stops.sort_by(|x, y| datars_math::total_cmp(x.0, y.0));
                }
            }
            _ => {}
        }
    }

    /// The discrete domain (band, point, ordinal, categorical).
    pub fn domain_values(&self) -> Option<&[Value]> {
        match self {
            Scale::Band(b) => Some(&b.domain),
            Scale::Point(p) => Some(&p.domain),
            Scale::Ordinal(o) => Some(&o.domain),
            Scale::Categorical(c) => Some(&c.domain),
            _ => None,
        }
    }
    pub fn set_domain_values(&mut self, d: Vec<Value>) {
        match self {
            Scale::Band(b) => b.domain = d,
            Scale::Point(p) => p.domain = d,
            Scale::Ordinal(o) => o.domain = d,
            Scale::Categorical(c) => c.domain = d,
            _ => {}
        }
    }

    /// The output interval of positional scales (continuous, band, point).
    pub fn range(&self) -> Option<[f64; 2]> {
        if let Some((_, c)) = self.continuous() {
            return Some(c.range);
        }
        match self {
            Scale::Band(b) => Some(b.range),
            Scale::Point(p) => Some(p.range),
            _ => None,
        }
    }
    pub fn set_range(&mut self, r: [f64; 2]) {
        if let Some(c) = self.cont_mut() {
            c.range = r;
            return;
        }
        match self {
            Scale::Band(b) => b.range = r,
            Scale::Point(p) => p.range = r,
            _ => {}
        }
    }

    /// Width of a band (0 for other scales, including point scales).
    pub fn bandwidth(&self) -> f64 {
        match self {
            Scale::Band(b) => b.bandwidth(),
            _ => 0.0,
        }
    }
    /// Distance between adjacent bands / points (0 for other scales).
    pub fn step(&self) -> f64 {
        match self {
            Scale::Band(b) => b.step(),
            Scale::Point(p) => p.step(),
            _ => 0.0,
        }
    }

    // ---- mapping -------------------------------------------------------------------------------

    /// A value on this scale's number line: dates are days (or ms for millisecond time scales).
    fn x_of(&self, v: &Value) -> f64 {
        match (self, v) {
            (Scale::Time { unit: TimeUnit::Millis, .. }, Value::Date(d)) => *d as f64 * MS_DAY,
            _ => v.as_f64().unwrap_or(f64::NAN),
        }
    }

    /// Map a value to what this scale is for: an ink for colour scales ([`Scale::is_color`]), a
    /// number otherwise.
    pub fn map(&self, v: impl Into<Value>) -> Mapped {
        if self.is_color() {
            Mapped::Ink(self.map_ink(v))
        } else {
            Mapped::Num(self.map_num(v))
        }
    }

    /// Map a value to a number (see the module docs for what the number means per kind). NaN when
    /// the value can't be mapped.
    pub fn map_num(&self, v: impl Into<Value>) -> f64 {
        self.map_value(&v.into())
    }

    /// Fast path for numeric input.
    pub fn map_f64(&self, x: f64) -> f64 {
        match self.continuous() {
            Some((tf, c)) => continuous::map(tf, c, x),
            None => self.map_value(&Value::Num(x)),
        }
    }

    fn map_value(&self, v: &Value) -> f64 {
        if let Some((tf, c)) = self.continuous() {
            return continuous::map(tf, c, self.x_of(v));
        }
        let x = self.x_of(v);
        match self {
            Scale::Band(b) => b.index_of(v).map_or(f64::NAN, |i| b.layout().position(i)),
            Scale::Point(p) => {
                let b = p.as_band();
                b.index_of(v).map_or(f64::NAN, |i| b.layout().position(i))
            }
            Scale::Ordinal(o) => o.domain.iter().position(|d| d == v).and_then(|i| o.output_index(i)).map_or(f64::NAN, |j| o.range.num(j)),
            Scale::Quantize(q) => bucket(&q.thresholds(), x).map_or(f64::NAN, |i| q.range.num(i)),
            Scale::Quantile(q) => bucket(&q.thresholds, x).map_or(f64::NAN, |i| q.range.num(i)),
            Scale::Threshold(t) => bucket(&t.thresholds, x).map_or(f64::NAN, |i| t.range.num(i)),
            Scale::Sequential(s) => s.position(x),
            Scale::Diverging(d) => d.position(x),
            Scale::Categorical(c) => c.index_of(v).map_or(f64::NAN, |i| i as f64),
            Scale::Piecewise(p) => p.position(x),
            _ => unreachable!("continuous scales handled above"),
        }
    }

    /// Map a value to an ink, or `None` when this scale can't (a non-colour scale, a null, a
    /// value a ramp can't place). Categorical and ordinal scales give unknown keys their `unknown`
    /// ink (categorical: `$muted` by default).
    pub fn try_map_ink(&self, v: impl Into<Value>) -> Option<Ink> {
        let v = v.into();
        let x = self.x_of(&v);
        match self {
            Scale::Ordinal(o) => match o.domain.iter().position(|d| *d == v) {
                Some(i) => o.output_index(i).and_then(|j| o.range.ink(j)),
                None if matches!(o.range, Outputs::Ink(_)) => o.unknown.clone(),
                None => None,
            },
            Scale::Quantize(q) => bucket(&q.thresholds(), x).and_then(|i| q.range.ink(i)),
            Scale::Quantile(q) => bucket(&q.thresholds, x).and_then(|i| q.range.ink(i)),
            Scale::Threshold(t) => bucket(&t.thresholds, x).and_then(|i| t.range.ink(i)),
            Scale::Sequential(s) => Some(s.position(x)).filter(|t| !t.is_nan()).map(|t| s.colors.at(t)),
            Scale::Diverging(d) => Some(d.position(x)).filter(|t| !t.is_nan()).map(|t| d.colors.at(t)),
            Scale::Categorical(c) => c.ink(&v).or_else(|| Some(c.unknown.clone().unwrap_or_else(muted))),
            Scale::Piecewise(p) => p.ink(x),
            _ => None,
        }
    }

    /// [`Scale::try_map_ink`], with `$muted` for anything unmappable.
    pub fn map_ink(&self, v: impl Into<Value>) -> Ink {
        self.try_map_ink(v).unwrap_or_else(muted)
    }

    /// Map a whole column (discrete lookups use an index built once).
    pub fn map_column(&self, c: &Column) -> Vec<f64> {
        let n = c.len();
        if let Some((tf, cs)) = self.continuous() {
            return (0..n).map(|i| continuous::map(tf, cs, self.x_of(&c.get(i)))).collect();
        }
        let positions: Option<(Vec<f64>, &[Value])> = match self {
            Scale::Band(b) => {
                let l = b.layout();
                Some(((0..b.domain.len()).map(|i| l.position(i)).collect(), &b.domain))
            }
            Scale::Point(p) => {
                let l = p.as_band().layout();
                Some(((0..p.domain.len()).map(|i| l.position(i)).collect(), &p.domain))
            }
            Scale::Ordinal(o) => Some(((0..o.domain.len()).map(|i| o.output_index(i).map_or(f64::NAN, |j| o.range.num(j))).collect(), &o.domain)),
            Scale::Categorical(k) => Some(((0..k.domain.len()).map(|i| i as f64).collect(), &k.domain)),
            _ => None,
        };
        match positions {
            Some((out, domain)) => {
                let index = index_of(domain);
                (0..n).map(|i| index.get(&c.get(i)).map_or(f64::NAN, |&j| out[j])).collect()
            }
            None => (0..n).map(|i| self.map_value(&c.get(i))).collect(),
        }
    }

    /// Map a whole column to inks (see [`Scale::map_ink`]).
    pub fn map_column_ink(&self, c: &Column) -> Vec<Ink> {
        if let Scale::Categorical(k) = self {
            let index = index_of(&k.domain);
            let unknown = k.unknown.clone().unwrap_or_else(muted);
            return (0..c.len())
                .map(|i| {
                    let v = c.get(i);
                    match k.overrides.iter().find(|(key, _)| *key == v) {
                        Some((_, ink)) => ink.clone(),
                        None => index.get(&v).map_or_else(|| unknown.clone(), |&j| Ink::palette(&k.palette, j as u32)),
                    }
                })
                .collect();
        }
        (0..c.len()).map(|i| self.map_ink(c.get(i))).collect()
    }

    /// Back from a number to a domain value: a pixel for positional scales (band / point: the
    /// category whose slot holds it), a ramp position t for sequential / diverging / piecewise
    /// scales, an index for categorical scales, the middle of the matching bucket for quantize /
    /// quantile / threshold scales with numeric outputs. Continuous scales return numbers (days or
    /// ms for time scales).
    pub fn invert(&self, px: f64) -> Option<Value> {
        if px.is_nan() {
            return None;
        }
        if let Some((tf, c)) = self.continuous() {
            let x = continuous::invert(tf, c, px);
            return x.is_finite().then_some(Value::Num(x));
        }
        let num = |x: f64| x.is_finite().then_some(Value::Num(x));
        let nearest = |r: &Outputs| -> Option<usize> {
            let Outputs::Num(v) = r else { return None };
            (0..v.len()).filter(|&i| !v[i].is_nan()).min_by(|&a, &b| datars_math::total_cmp((v[a] - px).abs(), (v[b] - px).abs()))
        };
        let mid = |[a, b]: [f64; 2]| -> f64 {
            match (a.is_finite(), b.is_finite()) {
                (true, true) => (a + b) / 2.0,
                (true, false) => a,
                (false, true) => b,
                _ => f64::NAN,
            }
        };
        match self {
            Scale::Band(b) => b.layout().index_at(px).map(|i| b.domain[i].clone()),
            Scale::Point(p) => p.as_band().layout().index_at(px).map(|i| p.domain[i].clone()),
            Scale::Ordinal(o) => {
                let j = nearest(&o.range)?;
                o.domain.iter().enumerate().find(|(i, _)| o.output_index(*i) == Some(j)).map(|(_, v)| v.clone())
            }
            Scale::Quantize(q) => num(mid(bucket_extent(&q.thresholds(), nearest(&q.range)?, q.domain[0], q.domain[1]))),
            Scale::Quantile(q) => num(mid(bucket_extent(&q.thresholds, nearest(&q.range)?, q.extent[0], q.extent[1]))),
            Scale::Threshold(t) => num(mid(bucket_extent(&t.thresholds, nearest(&t.range)?, f64::NEG_INFINITY, f64::INFINITY))),
            Scale::Sequential(s) => {
                let t = if s.clamp { px.clamp(0.0, 1.0) } else { px };
                num(s.transform.denormalize(t, s.domain))
            }
            Scale::Diverging(d) => num(d.value_at(if d.clamp { px.clamp(0.0, 1.0) } else { px })),
            Scale::Categorical(c) => {
                let i = px.round();
                (i >= 0.0 && (i as usize) < c.domain.len()).then(|| c.domain[i as usize].clone())
            }
            Scale::Piecewise(p) => {
                let (a, b) = (p.stops.first()?.0, p.stops.last()?.0);
                num(datars_math::lerp(a, b, px.clamp(0.0, 1.0)))
            }
            _ => None,
        }
    }

    /// [`Scale::invert`] as a number (dates as days).
    pub fn invert_num(&self, px: f64) -> Option<f64> {
        self.invert(px).and_then(|v| v.as_f64())
    }

    // ---- ticks and nice ------------------------------------------------------------------------

    /// About `count` ticks (see the module docs).
    pub fn ticks(&self, count: usize) -> Vec<Tick> {
        let category = |d: &[Value]| d.iter().map(|v| Tick { value: v.clone(), label_hint: LabelHint::Category }).collect();
        let at = |xs: &[f64]| xs.iter().map(|&x| Tick { value: Value::Num(x), label_hint: LabelHint::Number { decimals: ticks::decimals_of(x) } }).collect();
        match self {
            Scale::Log { base, scale } => ticks::log_ticks(*base, scale.domain[0], scale.domain[1], count),
            Scale::Time { unit, scale } => {
                let k = if *unit == TimeUnit::Days { MS_DAY } else { 1.0 };
                ticks::time_ticks_ms(scale.domain[0] * k, scale.domain[1] * k, count, *unit == TimeUnit::Days)
                    .into_iter()
                    .map(|(ms, label_hint)| Tick { value: if *unit == TimeUnit::Days { Value::Date((ms / MS_DAY) as i32) } else { Value::Num(ms) }, label_hint })
                    .collect()
            }
            Scale::Linear(c) | Scale::Sqrt(c) => ticks::linear_ticks(c.domain[0], c.domain[1], count),
            Scale::Pow { scale: c, .. } | Scale::Symlog { scale: c, .. } => ticks::linear_ticks(c.domain[0], c.domain[1], count),
            Scale::Band(b) => slot_ticks(&b.domain, count).unwrap_or_else(|| category(&b.domain)),
            Scale::Point(p) => slot_ticks(&p.domain, count).unwrap_or_else(|| category(&p.domain)),
            Scale::Ordinal(o) => category(&o.domain),
            Scale::Categorical(c) => category(&c.domain),
            Scale::Quantize(q) => ticks::linear_ticks(q.domain[0], q.domain[1], count),
            Scale::Quantile(q) => at(&q.thresholds),
            Scale::Threshold(t) => at(&t.thresholds),
            Scale::Sequential(s) => match s.transform {
                Transform::Log { base } => ticks::log_ticks(base, s.domain[0], s.domain[1], count),
                _ => ticks::linear_ticks(s.domain[0], s.domain[1], count),
            },
            Scale::Diverging(d) => ticks::linear_ticks(d.domain[0], d.domain[2], count),
            Scale::Piecewise(p) => at(&p.stops.iter().map(|s| s.0).collect::<Vec<_>>()),
        }
    }

    /// Extend the domain to nice values for about `count` ticks: multiples of the tick step
    /// (linear-ish scales, quantize, colour ramps), whole powers of the base (log), calendar
    /// boundaries (time). Discrete scales are returned unchanged.
    pub fn nice(mut self, count: usize) -> Scale {
        let lin = |d: [f64; 2]| -> [f64; 2] {
            let (a, b) = num::nice_domain(d[0], d[1], count);
            [a, b]
        };
        match &mut self {
            Scale::Log { base, scale } => {
                let base = *base;
                let [a, b] = scale.domain;
                let neg = a < 0.0 && b < 0.0;
                let (lo, hi) = if neg { (-b.max(a), -b.min(a)) } else { (a.min(b), a.max(b)) };
                if lo > 0.0 {
                    let logs = |x: f64| continuous::log(x, base);
                    let pows = |e: f64| datars_math::m::pow(base, e);
                    let mut nlo = pows(logs(lo).floor());
                    let mut nhi = pows(logs(hi).ceil());
                    // Guard against log rounding pushing an exact power one decade out.
                    if pows(logs(lo).round()) == lo {
                        nlo = lo;
                    }
                    if pows(logs(hi).round()) == hi {
                        nhi = hi;
                    }
                    let (x, y) = if neg { (-nhi, -nlo) } else { (nlo, nhi) };
                    scale.domain = if a <= b { [x, y] } else { [y, x] };
                }
            }
            Scale::Time { unit, scale } => {
                let k = if *unit == TimeUnit::Days { MS_DAY } else { 1.0 };
                let (a, b) = ticks::nice_time_ms(scale.domain[0] * k, scale.domain[1] * k, count, *unit == TimeUnit::Days);
                scale.domain = [a / k, b / k];
            }
            Scale::Linear(c) | Scale::Sqrt(c) => c.domain = lin(c.domain),
            Scale::Pow { scale: c, .. } | Scale::Symlog { scale: c, .. } => c.domain = lin(c.domain),
            Scale::Quantize(q) => q.domain = lin(q.domain),
            Scale::Sequential(s) => {
                if s.transform.is_linear() {
                    s.domain = lin(s.domain)
                }
            }
            Scale::Diverging(d) => {
                let [a, b] = lin([d.domain[0], d.domain[2]]);
                d.domain = [a, d.domain[1], b];
            }
            _ => {}
        }
        self
    }

    // ---- interpolation -------------------------------------------------------------------------

    /// The scale part-way from `a` to `b` (animated rescale): domains and ranges interpolate
    /// (continuous domains in transformed space, so log scales zoom geometrically); discrete scales
    /// interpolate their range and paddings when their domains are equal. Scales that can't
    /// interpolate (different kinds, parameters or domains) switch at t = 0.5. Exact at the ends:
    /// `lerp(a, b, 0) == a`, `lerp(a, b, 1) == b`.
    pub fn lerp(a: &Scale, b: &Scale, t: f64) -> Scale {
        if t <= 0.0 || t.is_nan() {
            return a.clone();
        }
        if t >= 1.0 {
            return b.clone();
        }
        let l = |x: f64, y: f64| datars_math::lerp(x, y, t);
        let l2 = |x: [f64; 2], y: [f64; 2]| [l(x[0], y[0]), l(x[1], y[1])];
        let lv = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(p, q)| l(*p, *q)).collect::<Vec<f64>>();
        let snap = || if t < 0.5 { a.clone() } else { b.clone() };
        match (a, b) {
            (Scale::Linear(x), Scale::Linear(y)) => Scale::Linear(continuous::lerp(Transform::Linear, x, y, t)),
            (Scale::Sqrt(x), Scale::Sqrt(y)) => Scale::Sqrt(continuous::lerp(Transform::Sqrt, x, y, t)),
            (Scale::Log { base: p, scale: x }, Scale::Log { base: q, scale: y }) if p == q => {
                Scale::Log { base: *p, scale: continuous::lerp(Transform::Log { base: *p }, x, y, t) }
            }
            (Scale::Pow { exponent: p, scale: x }, Scale::Pow { exponent: q, scale: y }) if p == q => {
                Scale::Pow { exponent: *p, scale: continuous::lerp(Transform::Pow { exponent: *p }, x, y, t) }
            }
            (Scale::Symlog { constant: p, scale: x }, Scale::Symlog { constant: q, scale: y }) if p == q => {
                Scale::Symlog { constant: *p, scale: continuous::lerp(Transform::Symlog { constant: *p }, x, y, t) }
            }
            (Scale::Time { unit: p, scale: x }, Scale::Time { unit: q, scale: y }) if p == q => {
                Scale::Time { unit: *p, scale: continuous::lerp(Transform::Linear, x, y, t) }
            }
            (Scale::Band(x), Scale::Band(y)) if x.domain == y.domain => Scale::Band(Band {
                domain: y.domain.clone(),
                range: l2(x.range, y.range),
                padding_inner: l(x.padding_inner, y.padding_inner),
                padding_outer: l(x.padding_outer, y.padding_outer),
                align: l(x.align, y.align),
                round: if t < 0.5 { x.round } else { y.round },
            }),
            (Scale::Point(x), Scale::Point(y)) if x.domain == y.domain => Scale::Point(Point {
                domain: y.domain.clone(),
                range: l2(x.range, y.range),
                padding: l(x.padding, y.padding),
                align: l(x.align, y.align),
                round: if t < 0.5 { x.round } else { y.round },
            }),
            (Scale::Ordinal(x), Scale::Ordinal(y)) if x.domain == y.domain => match (&x.range, &y.range) {
                (Outputs::Num(p), Outputs::Num(q)) if p.len() == q.len() => Scale::Ordinal(Ordinal { domain: y.domain.clone(), range: Outputs::Num(lv(p, q)), unknown: y.unknown.clone() }),
                _ => snap(),
            },
            (Scale::Quantize(x), Scale::Quantize(y)) if x.range == y.range => Scale::Quantize(Quantize { domain: l2(x.domain, y.domain), range: y.range.clone() }),
            (Scale::Quantile(x), Scale::Quantile(y)) if x.range == y.range && x.thresholds.len() == y.thresholds.len() => {
                Scale::Quantile(Quantile { thresholds: lv(&x.thresholds, &y.thresholds), extent: l2(x.extent, y.extent), range: y.range.clone() })
            }
            (Scale::Threshold(x), Scale::Threshold(y)) if x.range == y.range && x.thresholds.len() == y.thresholds.len() => {
                Scale::Threshold(Threshold { thresholds: lv(&x.thresholds, &y.thresholds), range: y.range.clone() })
            }
            (Scale::Sequential(x), Scale::Sequential(y)) if x.colors == y.colors && x.transform == y.transform => {
                let c = continuous::lerp(x.transform, &Continuous::new(x.domain, [0.0, 1.0]), &Continuous::new(y.domain, [0.0, 1.0]), t);
                Scale::Sequential(Sequential { domain: c.domain, colors: y.colors.clone(), transform: y.transform, clamp: if t < 0.5 { x.clamp } else { y.clamp } })
            }
            (Scale::Diverging(x), Scale::Diverging(y)) if x.colors == y.colors && x.transform == y.transform => Scale::Diverging(Diverging {
                domain: [l(x.domain[0], y.domain[0]), l(x.domain[1], y.domain[1]), l(x.domain[2], y.domain[2])],
                colors: y.colors.clone(),
                transform: y.transform,
                clamp: if t < 0.5 { x.clamp } else { y.clamp },
            }),
            (Scale::Piecewise(x), Scale::Piecewise(y))
                if x.stepped == y.stepped && x.stops.len() == y.stops.len() && x.stops.iter().zip(&y.stops).all(|(p, q)| p.1 == q.1) =>
            {
                Scale::Piecewise(Piecewise { stops: x.stops.iter().zip(&y.stops).map(|(p, q)| (l(p.0, q.0), q.1.clone())).collect(), stepped: y.stepped })
            }
            _ => snap(),
        }
    }

    // ---- from data -----------------------------------------------------------------------------

    /// A scale of `kind` with its domain computed from `column` (range [0, 1]; set it with
    /// [`Scale::with_range`]). See [`Scale::from_data_with`].
    pub fn from_data(column: &Column, kind: ScaleKind) -> Scale {
        Scale::from_data_with(column, kind, &DomainOptions::default())
    }

    /// A scale of `kind` with its domain from `column`: the extent of the non-null values
    /// (positive values for log; optionally stretched to zero and niced) for numeric kinds;
    /// distinct values in first-appearance order for discrete kinds. Colour kinds use the default
    /// palettes (`sequential`, `diverging`, `categorical`) unless `opts.palette` is set;
    /// quantize / quantile get five ramp steps. Diverging domains centre on zero when the data
    /// crosses it, else on the middle of the extent.
    pub fn from_data_with(column: &Column, kind: ScaleKind, opts: &DomainOptions) -> Scale {
        let ext = || {
            let e = domain::extent(column).unwrap_or([0.0, 1.0]);
            if opts.zero {
                domain::include_zero(e)
            } else {
                e
            }
        };
        let pal = |default: &str| opts.palette.clone().unwrap_or_else(|| default.to_string());
        let unit = [0.0, 1.0];
        let s = match kind {
            ScaleKind::Linear => Scale::linear(ext(), unit),
            ScaleKind::Sqrt => Scale::sqrt(ext(), unit),
            ScaleKind::Symlog => Scale::symlog(1.0, ext(), unit),
            ScaleKind::Log => Scale::log(10.0, domain::positive_extent(column).unwrap_or([1.0, 10.0]), unit),
            ScaleKind::Time => Scale::time(domain::extent(column).unwrap_or([0.0, 1.0]), unit),
            ScaleKind::Band => Scale::band(domain::slots(column), unit),
            ScaleKind::Point => Scale::point(domain::slots(column), unit),
            ScaleKind::Ordinal => {
                let d = domain::distinct(column);
                let n = d.len();
                Scale::ordinal(d, Outputs::Num((0..n).map(|i| i as f64).collect()))
            }
            ScaleKind::Quantize => Scale::quantize(ext(), Outputs::ramp(&pal("sequential"), 5)),
            ScaleKind::Quantile => Scale::quantile(&column.to_f64(), Outputs::ramp(&pal("sequential"), 5)),
            ScaleKind::Sequential => Scale::sequential(ext(), Colors::palette(&pal("sequential"))),
            ScaleKind::Diverging => {
                let [a, b] = domain::extent(column).unwrap_or([-1.0, 1.0]);
                let mid = if a < 0.0 && b > 0.0 { 0.0 } else { (a + b) / 2.0 };
                Scale::diverging([a, mid, b], Colors::palette(&pal("diverging")))
            }
            ScaleKind::Categorical => Scale::categorical(domain::distinct(column), &pal("categorical")),
        };
        match opts.nice {
            Some(n) => s.nice(n),
            None => s,
        }
    }
}

/// Calendar ticks for a band or point scale whose slots are dates in calendar order (see
/// [`ticks::slot_time_ticks`]); `None` for any other domain (every category is a tick).
fn slot_ticks(domain: &[Value], count: usize) -> Option<Vec<Tick>> {
    let days: Vec<i32> = domain.iter().map(|v| if let Value::Date(d) = v { Some(*d) } else { None }).collect::<Option<_>>()?;
    if days.is_empty() || days.windows(2).any(|w| w[0] >= w[1]) {
        return None;
    }
    Some(ticks::slot_time_ticks(&days, count).into_iter().map(|(i, label_hint)| Tick { value: domain[i].clone(), label_hint }).collect())
}

/// Domain value → index (first occurrence wins).
fn index_of(domain: &[Value]) -> BTreeMap<Value, usize> {
    let mut m = BTreeMap::new();
    for (i, v) in domain.iter().enumerate() {
        m.entry(v.clone()).or_insert(i);
    }
    m
}
