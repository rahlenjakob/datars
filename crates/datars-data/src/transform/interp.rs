//! Values at a data time — the table behind ranking races and time-animated maps.

use super::group_rows;
use crate::table::{Column, ColumnType, Table};
use crate::DataError;

/// For each distinct `key_col` value (first appearance), `value_col` linearly interpolated at
/// data time `t` along `time_col` (numbers, or dates as days — fractional days interpolate between
/// dates), clamped to the first / last observation outside the observed range. Rows with a null
/// time or value are ignored; a key with no observations gets null.
///
/// Output: one row per key, keyed by `key_col`, with the input's columns in order — `time_col` set
/// to `t` (a number), `value_col` the interpolated number, and every other column carried from the
/// key's latest observation at or before `t` (its first one when `t` is earlier).
pub fn interpolate_at(table: &Table, key_col: &str, time_col: &str, value_col: &str, t: f64) -> Result<Table, DataError> {
    let time = table.require(time_col)?;
    if !matches!(time.ty(), ColumnType::Num | ColumnType::Date) {
        return Err(DataError::TypeMismatch { column: time_col.into(), expected: "num or date".into(), found: time.ty().name().into() });
    }
    let value = table.require(value_col)?;
    if value.ty() != ColumnType::Num {
        return Err(DataError::TypeMismatch { column: value_col.into(), expected: "num".into(), found: value.ty().name().into() });
    }
    let groups = group_rows(table, &[key_col])?;
    let mut carry = Vec::with_capacity(groups.len());
    let mut out = Vec::with_capacity(groups.len());
    for (_, rows) in &groups {
        let mut obs: Vec<(f64, f64, usize)> = rows.iter().map(|&i| (time.f64_at(i), value.f64_at(i), i)).filter(|o| !o.0.is_nan() && !o.1.is_nan()).collect();
        obs.sort_by(|a, b| datars_math::total_cmp(a.0, b.0)); // stable: equal times keep row order
        let (v, row) = match (obs.first(), obs.last()) {
            (None, _) | (_, None) => (f64::NAN, rows[0]),
            (Some(f), _) if t <= f.0 => (f.1, f.2),
            (_, Some(l)) if t >= l.0 => (l.1, l.2),
            _ => {
                // The last observation at or before t, and the next one after it.
                let j = obs.iter().rposition(|o| o.0 <= t).expect("t is inside the observed range");
                let (a, b) = (obs[j], obs[j + 1]);
                let f = (t - a.0) / (b.0 - a.0);
                (a.1 + (b.1 - a.1) * f, a.2)
            }
        };
        out.push(v);
        carry.push(row);
    }
    let columns = table
        .columns
        .iter()
        .map(|(n, c)| {
            let col = if n == time_col {
                Column::Num(vec![t; groups.len()])
            } else if n == value_col {
                Column::Num(out.clone())
            } else {
                c.take(&carry)
            };
            (n.clone(), col)
        })
        .collect();
    Ok(Table { name: table.name.clone(), columns, key: vec![key_col.to_string()], version: table.version })
}
