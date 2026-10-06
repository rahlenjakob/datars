//! Binning: numbers into intervals, dates into calendar units, text into parts (before/after a
//! separator, first word, first letter). Bins are typed values — a month bin is the date of the
//! month's first day, a weekday bin is 0 (Monday) … 6 — so time scales, sorting and locale-aware
//! labels work downstream; nothing here formats text.

use crate::date;
use crate::num::{tick_step, Step};
use crate::table::{Column, ColumnType, Table};
use crate::DataError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// How to cut a number line.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinSpec {
    /// Intervals of this width, anchored at 0 (`[20, 30)`).
    Step(f64),
    /// This many equal intervals spanning the data's extent (the maximum joins the last bin).
    Count(usize),
    /// About this many intervals with a nice 1/2/5×10ᵏ width, anchored at multiples of it.
    Nice(usize),
}

/// Adds `as_name` (each value's bin start) and `{as_name}_end` (its end). Bins are half-open
/// `[start, end)`, except the last `Count` bin which includes the maximum. Nulls stay null.
pub fn bin_num(t: &Table, column: &str, spec: BinSpec, as_name: &str) -> Result<Table, DataError> {
    let c = t.require(column)?;
    if c.ty() != ColumnType::Num {
        return Err(DataError::TypeMismatch { column: column.into(), expected: "num".into(), found: c.ty().name().into() });
    }
    let v = c.as_num().expect("checked");
    let finite = || v.iter().copied().filter(|x| x.is_finite());
    let lo = finite().fold(f64::INFINITY, f64::min);
    let hi = finite().fold(f64::NEG_INFINITY, f64::max);
    if let BinSpec::Step(s) = spec {
        if !(s > 0.0 && s.is_finite()) {
            return Err(DataError::Invalid(format!("bin_num: step {s} must be a positive number")));
        }
    }
    let (start, end): (Vec<f64>, Vec<f64>) = match spec {
        BinSpec::Count(n) => {
            if n == 0 {
                return Err(DataError::Invalid("bin_num: Count(0) makes no bins".into()));
            }
            let w = (hi - lo) / n as f64;
            v.iter()
                .map(|&x| {
                    if !x.is_finite() {
                        return (f64::NAN, f64::NAN);
                    }
                    if w.is_nan() || w <= 0.0 {
                        return (lo, hi);
                    }
                    let i = (((x - lo) / w).floor() as usize).min(n - 1);
                    let b = |i: usize| if i == n { hi } else { lo + w * i as f64 };
                    (b(i), b(i + 1))
                })
                .unzip()
        }
        _ => {
            let step = match spec {
                BinSpec::Step(s) => Step::from_size(s),
                BinSpec::Nice(n) => tick_step(lo, hi, n.max(1) as f64).unwrap_or(Step::Mul(1.0)),
                BinSpec::Count(_) => unreachable!(),
            };
            v.iter()
                .map(|&x| {
                    if !x.is_finite() {
                        return (f64::NAN, f64::NAN);
                    }
                    let k = step.floor_index(x);
                    (step.at(k), step.at(k + 1.0))
                })
                .unzip()
        }
    };
    t.clone().with_column(as_name, Column::Num(start))?.with_column(&format!("{as_name}_end"), Column::Num(end))
}

/// Calendar units for [`bin_time`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBin {
    /// January 1st of the year (a date).
    Year,
    /// First day of the quarter (a date).
    Quarter,
    /// First day of the month (a date).
    Month,
    /// Monday of the ISO week (a date).
    Week,
    /// The day itself (a date).
    Day,
    /// 0 = Monday … 6 = Sunday, whatever the week (a number): weekly rhythm.
    Weekday,
    /// 1 … 12, whatever the year (a number): seasonality.
    MonthOfYear,
}

/// Adds `as_name`: each date's bin (see [`TimeBin`]). The column must be a date column.
pub fn bin_time(t: &Table, column: &str, unit: TimeBin, as_name: &str) -> Result<Table, DataError> {
    let c = t.require(column)?;
    let Some(d) = c.as_date() else {
        return Err(DataError::TypeMismatch { column: column.into(), expected: "date".into(), found: c.ty().name().into() });
    };
    let start = |x: i32| -> i32 {
        let (y, m, _) = date::ymd_from_date(x);
        match unit {
            TimeBin::Year => date::date_from_ymd(y, 1, 1),
            TimeBin::Quarter => date::date_from_ymd(y, (m - 1) / 3 * 3 + 1, 1),
            TimeBin::Month => date::date_from_ymd(y, m, 1),
            TimeBin::Week => Some(date::start_of_week(x)),
            _ => Some(x),
        }
        .expect("valid calendar date")
    };
    let col = match unit {
        TimeBin::Weekday => Column::Num(d.iter().map(|x| x.map_or(f64::NAN, |x| date::weekday(x) as f64)).collect()),
        TimeBin::MonthOfYear => Column::Num(d.iter().map(|x| x.map_or(f64::NAN, |x| date::ymd_from_date(x).1 as f64)).collect()),
        _ => Column::Date(d.iter().map(|x| x.map(start)).collect()),
    };
    t.clone().with_column(as_name, col)
}

/// Parts of a text value for [`bin_text`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPart {
    /// Text after the first separator (`after("@")` on an email → the domain). No separator →
    /// the whole value.
    After(String),
    /// Text before the first separator (`before(",")` on "Uppsala, Sweden" → "Uppsala").
    Before(String),
    /// The first whitespace-separated word.
    FirstWord,
    /// The first letter, upper-cased.
    FirstLetter,
}

/// Adds `as_name`: each string's part (trimmed; empty → null). The column must be text.
pub fn bin_text(t: &Table, column: &str, part: TextPart, as_name: &str) -> Result<Table, DataError> {
    let c = t.require(column)?;
    let Some(s) = c.as_str() else {
        return Err(DataError::TypeMismatch { column: column.into(), expected: "str".into(), found: c.ty().name().into() });
    };
    let mut pool: BTreeMap<String, Arc<str>> = BTreeMap::new();
    let out = s
        .iter()
        .map(|v| {
            let v = v.as_deref()?.trim();
            let p: String = match &part {
                TextPart::After(sep) => v.find(sep.as_str()).map_or(v, |i| &v[i + sep.len()..]).trim().to_string(),
                TextPart::Before(sep) => v.find(sep.as_str()).map_or(v, |i| &v[..i]).trim().to_string(),
                TextPart::FirstWord => v.split_whitespace().next().unwrap_or("").to_string(),
                TextPart::FirstLetter => v.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default(),
            };
            if p.is_empty() {
                return None;
            }
            Some(pool.entry(p.clone()).or_insert_with(|| Arc::from(p.as_str())).clone())
        })
        .collect();
    t.clone().with_column(as_name, Column::Str(out))
}
