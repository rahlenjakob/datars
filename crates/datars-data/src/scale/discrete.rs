//! Discrete-domain scales (band, point, ordinal) and discretizing scales (quantize, quantile,
//! threshold).

use crate::num::{quantile_sorted, sorted_values};
use crate::value::Value;
use datars_theme::Ink;
use serde::{Deserialize, Serialize};

fn half() -> f64 {
    0.5
}

/// Evenly spaced bands for categories (d3's `scaleBand`). Paddings are fractions of the step:
/// `padding_inner` between bands, `padding_outer` before the first and after the last; `align`
/// (0…1) places the bands within leftover space; `round` snaps step and positions to whole pixels.
/// Maps a value to its band's start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub domain: Vec<Value>,
    pub range: [f64; 2],
    #[serde(default)]
    pub padding_inner: f64,
    #[serde(default)]
    pub padding_outer: f64,
    #[serde(default = "half")]
    pub align: f64,
    #[serde(default)]
    pub round: bool,
}

/// Where the bands sit: first start, step, bandwidth, and whether the range runs backwards.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BandLayout {
    pub start: f64,
    pub step: f64,
    pub bandwidth: f64,
    pub reverse: bool,
    pub n: usize,
}

impl BandLayout {
    /// The start of band `i`.
    pub fn position(&self, i: usize) -> f64 {
        let j = if self.reverse { self.n - 1 - i } else { i };
        self.start + self.step * j as f64
    }
    /// The band whose slot (band plus half the gaps around it) holds `px`.
    pub fn index_at(&self, px: f64) -> Option<usize> {
        if self.n == 0 || self.step.is_nan() || self.step <= 0.0 {
            return None;
        }
        let gap = self.step - self.bandwidth;
        let k = ((px - self.start + gap / 2.0) / self.step).floor();
        if k < 0.0 || k >= self.n as f64 {
            return None;
        }
        let k = k as usize;
        Some(if self.reverse { self.n - 1 - k } else { k })
    }
}

impl Band {
    pub fn new(domain: Vec<Value>, range: [f64; 2]) -> Band {
        Band { domain, range, padding_inner: 0.0, padding_outer: 0.0, align: 0.5, round: false }
    }

    pub(crate) fn layout(&self) -> BandLayout {
        let n = self.domain.len();
        let reverse = self.range[1] < self.range[0];
        let (start, stop) = if reverse { (self.range[1], self.range[0]) } else { (self.range[0], self.range[1]) };
        let pi = self.padding_inner.clamp(0.0, 1.0);
        let po = self.padding_outer.max(0.0);
        let mut step = (stop - start) / (n as f64 - pi + po * 2.0).max(1.0);
        if self.round {
            step = step.floor();
        }
        let mut start = start + (stop - start - step * (n as f64 - pi)) * self.align.clamp(0.0, 1.0);
        let mut bandwidth = step * (1.0 - pi);
        if self.round {
            start = start.round();
            bandwidth = bandwidth.round();
        }
        BandLayout { start, step, bandwidth, reverse, n }
    }

    /// Distance between the starts of adjacent bands.
    pub fn step(&self) -> f64 {
        self.layout().step
    }
    /// Width of each band.
    pub fn bandwidth(&self) -> f64 {
        self.layout().bandwidth
    }
    pub fn index_of(&self, v: &Value) -> Option<usize> {
        self.domain.iter().position(|d| d == v)
    }
}

/// Evenly spaced points for categories (d3's `scalePoint`): a band scale with zero-width bands;
/// `padding` is the outer padding in steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub domain: Vec<Value>,
    pub range: [f64; 2],
    #[serde(default)]
    pub padding: f64,
    #[serde(default = "half")]
    pub align: f64,
    #[serde(default)]
    pub round: bool,
}

impl Point {
    pub fn new(domain: Vec<Value>, range: [f64; 2]) -> Point {
        Point { domain, range, padding: 0.0, align: 0.5, round: false }
    }
    pub(crate) fn as_band(&self) -> Band {
        Band { domain: self.domain.clone(), range: self.range, padding_inner: 1.0, padding_outer: self.padding, align: self.align, round: self.round }
    }
    pub fn step(&self) -> f64 {
        self.as_band().step()
    }
}

/// The outputs of discrete scales: numbers or inks (JSON: an array of numbers or of ink strings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Outputs {
    Num(Vec<f64>),
    Ink(Vec<Ink>),
}

impl Outputs {
    pub fn len(&self) -> usize {
        match self {
            Outputs::Num(v) => v.len(),
            Outputs::Ink(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// `n` evenly spaced positions along a theme palette used as a ramp (0, 1/(n−1), …, 1).
    pub fn ramp(palette: &str, n: usize) -> Outputs {
        let name = palette.trim_start_matches('$');
        Outputs::Ink((0..n).map(|i| Ink::ramp(name, if n > 1 { i as f64 / (n - 1) as f64 } else { 0.5 })).collect())
    }
    /// The first `n` colours of a theme palette.
    pub fn palette(palette: &str, n: usize) -> Outputs {
        let name = palette.trim_start_matches('$');
        Outputs::Ink((0..n).map(|i| Ink::palette(name, i as u32)).collect())
    }
    /// Output `i` as a number (inks: the index itself).
    pub(crate) fn num(&self, i: usize) -> f64 {
        match self {
            Outputs::Num(v) => v.get(i).copied().unwrap_or(f64::NAN),
            Outputs::Ink(_) => i as f64,
        }
    }
    pub(crate) fn ink(&self, i: usize) -> Option<Ink> {
        match self {
            Outputs::Ink(v) => v.get(i).cloned(),
            Outputs::Num(_) => None,
        }
    }
}

/// An explicit mapping from domain values to outputs, cycling the outputs when the domain is
/// longer (d3's `scaleOrdinal`). Values outside the domain map to `unknown` (inks) or NaN.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ordinal {
    pub domain: Vec<Value>,
    pub range: Outputs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown: Option<Ink>,
}

impl Ordinal {
    pub(crate) fn output_index(&self, i: usize) -> Option<usize> {
        (!self.range.is_empty()).then(|| i % self.range.len())
    }
}

/// A continuous domain cut into `range.len()` equal intervals (d3's `scaleQuantize`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantize {
    pub domain: [f64; 2],
    pub range: Outputs,
}

impl Quantize {
    /// The inner boundaries (n − 1 of them).
    pub fn thresholds(&self) -> Vec<f64> {
        let n = self.range.len();
        let [a, b] = self.domain;
        (1..n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
    }
}

/// Buckets holding equal numbers of samples (d3's `scaleQuantile`). Stores the computed
/// thresholds (not the samples) and the sample extent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantile {
    pub thresholds: Vec<f64>,
    pub extent: [f64; 2],
    pub range: Outputs,
}

impl Quantile {
    /// Thresholds at the i/n quantiles of the non-null samples, for n = `range.len()`.
    pub fn from_samples(samples: &[f64], range: Outputs) -> Quantile {
        let s = sorted_values(samples);
        let n = range.len();
        let thresholds = (1..n).map(|i| quantile_sorted(&s, i as f64 / n as f64)).collect();
        let extent = match (s.first(), s.last()) {
            (Some(&a), Some(&b)) => [a, b],
            _ => [f64::NAN, f64::NAN],
        };
        Quantile { thresholds, extent, range }
    }
}

/// Explicit boundaries: values below `thresholds[0]` get output 0, values in
/// [thresholds[i−1], thresholds[i]) get output i (d3's `scaleThreshold`). `range` has one more
/// entry than `thresholds`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    pub thresholds: Vec<f64>,
    pub range: Outputs,
}

/// The bucket of `x` among ascending `thresholds` (bisect right: a value equal to a threshold
/// goes above it). None for NaN.
pub(crate) fn bucket(thresholds: &[f64], x: f64) -> Option<usize> {
    (!x.is_nan()).then(|| thresholds.partition_point(|&t| t <= x))
}

/// The value range of bucket `i` (open ends use `lo` / `hi`).
pub(crate) fn bucket_extent(thresholds: &[f64], i: usize, lo: f64, hi: f64) -> [f64; 2] {
    let a = if i == 0 { lo } else { thresholds[i - 1] };
    let b = thresholds.get(i).copied().unwrap_or(hi);
    [a, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_layout_matches_d3() {
        let dom: Vec<Value> = ["a", "b", "c"].iter().map(|s| Value::str(s)).collect();
        let mut b = Band::new(dom.clone(), [0.0, 120.0]);
        assert_eq!((b.step(), b.bandwidth()), (40.0, 40.0));
        b.padding_inner = 0.2;
        b.padding_outer = 0.1;
        // step = 120 / (3 − 0.2 + 0.2) = 40; bandwidth = 32; start = (120 − 40 × 2.8) / 2 = 4.
        let l = b.layout();
        assert_eq!((l.step, l.bandwidth, l.start), (40.0, 32.0, 4.0));
        assert_eq!(l.index_at(4.0 + 40.0 + 1.0), Some(1));
        assert_eq!(l.index_at(-10.0), None);
        let p = Point::new(dom, [0.0, 100.0]);
        assert_eq!(p.step(), 50.0);
    }

    #[test]
    fn buckets() {
        assert_eq!(bucket(&[10.0, 20.0], 5.0), Some(0));
        assert_eq!(bucket(&[10.0, 20.0], 10.0), Some(1));
        assert_eq!(bucket(&[10.0, 20.0], 25.0), Some(2));
        assert_eq!(bucket(&[10.0, 20.0], f64::NAN), None);
        let q = Quantile::from_samples(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], Outputs::Num(vec![0.0, 1.0, 2.0, 3.0]));
        assert_eq!(q.thresholds, vec![2.75, 4.5, 6.25]);
        assert_eq!(Quantize { domain: [0.0, 1.0], range: Outputs::Num(vec![0.0; 4]) }.thresholds(), vec![0.25, 0.5, 0.75]);
    }
}
