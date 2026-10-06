//! Grouped aggregation.

use super::group_rows;
use crate::num::{pairwise_sum, quantile_sorted};
use crate::table::{Column, ColumnType, Table};
use crate::value::Value;
use crate::DataError;
use serde::{Deserialize, Serialize};

/// What to compute per group.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggOp {
    /// Rows in the group; with a column, its non-null values.
    Count,
    /// Pairwise sum of non-null values (0 for none).
    Sum,
    /// Mean of non-null values (null for none).
    Mean,
    Median,
    /// Quantile p ∈ [0, 1] (linear between closest ranks, R-7).
    Quantile(f64),
    /// Smallest / largest non-null value, keeping the column's type (strings by code point).
    Min,
    Max,
    /// Number of distinct non-null values.
    Distinct,
    /// First / last non-null value in row order, keeping the column's type.
    First,
    Last,
}

/// One output column: `op` over `column`, named `as_name`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Agg {
    pub op: AggOp,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(rename = "as")]
    pub as_name: String,
}

impl Agg {
    pub fn new(op: AggOp, column: &str, as_name: &str) -> Agg {
        Agg { op, column: Some(column.to_string()), as_name: as_name.to_string() }
    }
    /// Rows per group.
    pub fn count(as_name: &str) -> Agg {
        Agg { op: AggOp::Count, column: None, as_name: as_name.to_string() }
    }
    pub fn sum(column: &str, as_name: &str) -> Agg {
        Agg::new(AggOp::Sum, column, as_name)
    }
    pub fn mean(column: &str, as_name: &str) -> Agg {
        Agg::new(AggOp::Mean, column, as_name)
    }
}

/// One row per distinct combination of `group_by` values, in order of first appearance, with the
/// group columns followed by one column per [`Agg`]. The output is keyed by `group_by`. With no
/// group columns the whole table is one group (one row, even for an empty table).
pub fn aggregate(t: &Table, group_by: &[&str], aggs: &[Agg]) -> Result<Table, DataError> {
    let groups = if group_by.is_empty() { vec![(Vec::new(), (0..t.len()).collect())] } else { group_rows(t, group_by)? };
    let first_rows: Vec<usize> = groups.iter().filter_map(|g| g.1.first().copied()).collect();
    let mut columns: Vec<(String, Column)> = Vec::new();
    for g in group_by {
        columns.push((g.to_string(), t.require(g)?.take(&first_rows)));
    }
    for a in aggs {
        if columns.iter().any(|c| c.0 == a.as_name) {
            return Err(DataError::Invalid(format!("aggregate: output column \"{}\" is named twice", a.as_name)));
        }
        let col = match &a.column {
            Some(c) => Some(t.require(c)?),
            None => None,
        };
        let need_col = || col.ok_or_else(|| DataError::Invalid(format!("aggregate: {:?} needs a column (for \"{}\")", a.op, a.as_name)));
        let numeric = || -> Result<&Column, DataError> {
            let c = need_col()?;
            if c.ty() == ColumnType::Str {
                return Err(DataError::TypeMismatch { column: a.column.clone().unwrap_or_default(), expected: "num".into(), found: "str".into() });
            }
            Ok(c)
        };
        let values = |c: &Column, rows: &[usize]| -> Vec<f64> { rows.iter().map(|&i| c.f64_at(i)).filter(|x| !x.is_nan()).collect() };
        let out = match a.op {
            AggOp::Count => Column::Num(
                groups.iter().map(|(_, rows)| col.map_or(rows.len(), |c| rows.iter().filter(|&&i| !c.is_null(i)).count()) as f64).collect(),
            ),
            AggOp::Distinct => {
                let c = need_col()?;
                Column::Num(
                    groups
                        .iter()
                        .map(|(_, rows)| {
                            let mut vs: Vec<Value> = rows.iter().map(|&i| c.get(i)).filter(|v| !v.is_null()).collect();
                            vs.sort();
                            vs.dedup();
                            vs.len() as f64
                        })
                        .collect(),
                )
            }
            AggOp::Sum => {
                let c = numeric()?;
                Column::Num(groups.iter().map(|(_, rows)| pairwise_sum(&values(c, rows))).collect())
            }
            AggOp::Mean => {
                let c = numeric()?;
                Column::Num(
                    groups
                        .iter()
                        .map(|(_, rows)| {
                            let v = values(c, rows);
                            if v.is_empty() {
                                f64::NAN
                            } else {
                                pairwise_sum(&v) / v.len() as f64
                            }
                        })
                        .collect(),
                )
            }
            AggOp::Median | AggOp::Quantile(_) => {
                let p = if let AggOp::Quantile(p) = a.op { p } else { 0.5 };
                if !(0.0..=1.0).contains(&p) {
                    return Err(DataError::Invalid(format!("aggregate: quantile {p} is outside [0, 1]")));
                }
                let c = numeric()?;
                Column::Num(
                    groups
                        .iter()
                        .map(|(_, rows)| {
                            let mut v = values(c, rows);
                            v.sort_by(|x, y| datars_math::total_cmp(*x, *y));
                            quantile_sorted(&v, p)
                        })
                        .collect(),
                )
            }
            AggOp::Min | AggOp::Max | AggOp::First | AggOp::Last => {
                let c = need_col()?;
                let vals: Vec<Value> = groups
                    .iter()
                    .map(|(_, rows)| {
                        let mut it = rows.iter().map(|&i| c.get(i)).filter(|v| !v.is_null());
                        match a.op {
                            AggOp::Min => it.min(),
                            AggOp::Max => it.max(),
                            AggOp::First => it.next(),
                            _ => it.next_back(),
                        }
                        .unwrap_or(Value::Null)
                    })
                    .collect();
                Column::from_values(c.ty(), &vals)
            }
        };
        columns.push((a.as_name.clone(), out));
    }
    Ok(Table { name: t.name.clone(), columns, key: group_by.iter().map(|g| g.to_string()).collect(), version: t.version })
}
