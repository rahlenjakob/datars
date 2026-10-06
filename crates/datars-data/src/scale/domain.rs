//! Domains from data: extents, zero inclusion, distinct values in first-appearance order.

use crate::table::Column;
use crate::value::Value;
use std::collections::BTreeSet;

/// [min, max] of the column's non-null values on a number line (dates as days); `None` when
/// there are none.
pub fn extent(column: &Column) -> Option<[f64; 2]> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in 0..column.len() {
        let x = column.f64_at(i);
        if x.is_finite() {
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo <= hi).then_some([lo, hi])
}

/// [min, max] of the strictly positive values (log-scale domains).
pub fn positive_extent(column: &Column) -> Option<[f64; 2]> {
    let v: Vec<f64> = column.to_f64().into_iter().filter(|x| x.is_finite() && *x > 0.0).collect();
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo <= hi).then_some([lo, hi])
}

/// Stretch an extent to include zero (lengths measured from a zero baseline need it).
pub fn include_zero(e: [f64; 2]) -> [f64; 2] {
    [e[0].min(0.0), e[1].max(0.0)]
}

/// The slots of a band or point scale: distinct values in order of first appearance — except
/// dates, which take calendar order. Dates one per slot are a time axis without gaps (trading
/// days: no room for weekends and holidays); a table sorted by ticker, then date, must not put a
/// date the first ticker lacks after all the others.
pub fn slots(column: &Column) -> Vec<Value> {
    let mut d = distinct(column);
    if matches!(column, Column::Date(_)) {
        d.sort();
    }
    d
}

/// Distinct non-null values in order of first appearance.
pub fn distinct(column: &Column) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for v in column.values() {
        if !v.is_null() && seen.insert(v.clone()) {
            out.push(v);
        }
    }
    out
}
