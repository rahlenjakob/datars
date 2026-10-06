//! Transforms: pure, deterministic functions from tables to new tables (docs/06 §Transforms).
//!
//! Every transform returns a fresh table and never mutates its input. Row order is always defined
//! (first appearance for groups, stable sorts with nulls last), sums use a fixed pairwise tree,
//! and sampling is seeded — so the same inputs give the same bytes on every target.
//!
//! Keys follow the rows: row-preserving transforms keep the input key; `aggregate` and `pivot` key
//! the output by their group columns; others document what they do.

mod aggregate;
mod bin;
mod interp;
mod reshape;
mod window;

pub use aggregate::{aggregate, Agg, AggOp};
pub use bin::{bin_num, bin_text, bin_time, BinSpec, TextPart, TimeBin};
pub use interp::interpolate_at;
pub use reshape::{join, pivot, unpivot, JoinKind};
pub use window::{window, window_with, WindowOp, WindowOpts};

use crate::num::pairwise_sum;
use crate::table::{Column, ColumnType, Table};
use crate::value::Value;
use crate::DataError;
use datars_math::Rng;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The rows where `mask` is true (in order). The mask must have one entry per row.
pub fn filter(t: &Table, mask: &[bool]) -> Result<Table, DataError> {
    if mask.len() != t.len() {
        return Err(DataError::LengthMismatch { column: "(filter mask)".into(), expected: t.len(), found: mask.len() });
    }
    let idx: Vec<usize> = mask.iter().enumerate().filter(|(_, &m)| m).map(|(i, _)| i).collect();
    Ok(t.take_rows(&idx))
}

/// A row predicate on one column (`column op value`), for building filter masks without an
/// expression engine.
#[derive(Clone, Debug, PartialEq)]
pub enum Pred {
    Eq(Value),
    Ne(Value),
    In(Vec<Value>),
    NotIn(Vec<Value>),
    /// Numeric (or date-as-days) comparisons; nulls never match.
    Gt(f64),
    Ge(f64),
    Lt(f64),
    Le(f64),
    /// Inclusive range.
    Between(f64, f64),
    /// Case-insensitive substring of the value's text.
    Contains(String),
    IsNull,
    NotNull,
}

/// A mask of the rows where `column` satisfies `pred`.
pub fn mask(t: &Table, column: &str, pred: &Pred) -> Result<Vec<bool>, DataError> {
    let c = t.require(column)?;
    let needle = match pred {
        Pred::Contains(s) => s.to_lowercase(),
        _ => String::new(),
    };
    Ok((0..t.len())
        .map(|i| {
            let x = c.f64_at(i);
            match pred {
                Pred::Eq(v) => c.get(i) == *v,
                Pred::Ne(v) => c.get(i) != *v,
                Pred::In(vs) => vs.contains(&c.get(i)),
                Pred::NotIn(vs) => !vs.contains(&c.get(i)),
                Pred::Gt(v) => x > *v,
                Pred::Ge(v) => x >= *v,
                Pred::Lt(v) => x < *v,
                Pred::Le(v) => x <= *v,
                Pred::Between(a, b) => x >= *a && x <= *b,
                Pred::Contains(_) => !c.is_null(i) && c.get(i).to_string().to_lowercase().contains(&needle),
                Pred::IsNull => c.is_null(i),
                Pred::NotNull => !c.is_null(i),
            }
        })
        .collect())
}

/// `filter(t, mask(t, column, pred))`.
pub fn filter_where(t: &Table, column: &str, pred: &Pred) -> Result<Table, DataError> {
    filter(t, &mask(t, column, pred)?)
}

/// The table with `column` added (or replaced in place). The column must have one value per row.
pub fn derive(t: &Table, name: &str, column: Column) -> Result<Table, DataError> {
    t.clone().with_column(name, column)
}

/// Stable sort by columns (`true` = descending). Total order: numbers by value, strings by code
/// point, false < true, dates by day; nulls (and NaN) last in either direction; ties keep their
/// input order.
pub fn sort(t: &Table, by: &[(&str, bool)]) -> Result<Table, DataError> {
    let idx = sorted_indices(t, &(0..t.len()).collect::<Vec<_>>(), by)?;
    Ok(t.take_rows(&idx))
}

/// `rows` stably sorted by `by` (see [`sort`]).
pub(crate) fn sorted_indices(t: &Table, rows: &[usize], by: &[(&str, bool)]) -> Result<Vec<usize>, DataError> {
    let cols: Vec<(&Column, bool)> = by.iter().map(|(c, d)| Ok((t.require(c)?, *d))).collect::<Result<_, DataError>>()?;
    let mut idx = rows.to_vec();
    idx.sort_by(|&a, &b| cmp_rows(&cols, a, b));
    Ok(idx)
}

/// Compare two rows over columns with directions; nulls last regardless of direction.
pub(crate) fn cmp_rows(cols: &[(&Column, bool)], a: usize, b: usize) -> Ordering {
    for &(c, desc) in cols {
        let o = match (c.is_null(a), c.is_null(b)) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => {
                let o = cmp_cells(c, a, b);
                if desc {
                    o.reverse()
                } else {
                    o
                }
            }
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

fn cmp_cells(c: &Column, a: usize, b: usize) -> Ordering {
    match c {
        Column::Num(v) => datars_math::total_cmp(v[a], v[b]),
        Column::Str(v) => v[a].cmp(&v[b]),
        Column::Bool(v) => v[a].cmp(&v[b]),
        Column::Date(v) => v[a].cmp(&v[b]),
    }
}

/// A group: its key values and its rows (in row order).
pub(crate) type Group = (Vec<Value>, Vec<usize>);

/// Groups of rows by the values of `cols`, in order of first appearance: (group key values,
/// rows). Null is a group value like any other.
pub(crate) fn group_rows(t: &Table, cols: &[&str]) -> Result<Vec<Group>, DataError> {
    let cs: Vec<&Column> = cols.iter().map(|c| t.require(c)).collect::<Result<_, _>>()?;
    let mut index: BTreeMap<Vec<Value>, usize> = BTreeMap::new();
    let mut groups: Vec<Group> = Vec::new();
    for i in 0..t.len() {
        let k: Vec<Value> = cs.iter().map(|c| c.get(i)).collect();
        match index.get(&k) {
            Some(&g) => groups[g].1.push(i),
            None => {
                index.insert(k.clone(), groups.len());
                groups.push((k, vec![i]));
            }
        }
    }
    Ok(groups)
}

/// The `n` rows with the largest `by` (descending, ties in input order, nulls never ranked above
/// numbers). With `other_label`, the remaining rows are folded into one final row: string
/// columns get the label, numeric columns the (pairwise) sum of the folded rows, dates null,
/// booleans false. Without it, the rest is dropped. No fold happens when nothing is left over.
pub fn top_n(t: &Table, by: &str, n: usize, other_label: Option<&str>) -> Result<Table, DataError> {
    t.require(by)?;
    let idx = sorted_indices(t, &(0..t.len()).collect::<Vec<_>>(), &[(by, true)])?;
    let (top, rest) = idx.split_at(n.min(idx.len()));
    let mut out = t.take_rows(top);
    let (Some(label), false) = (other_label, rest.is_empty()) else { return Ok(out) };
    let label: Arc<str> = Arc::from(label);
    for (name, col) in &mut out.columns {
        let src = t.column(name).expect("same columns");
        match (col, src) {
            (Column::Num(v), Column::Num(s)) => {
                let xs: Vec<f64> = rest.iter().map(|&i| s[i]).filter(|x| !x.is_nan()).collect();
                v.push(pairwise_sum(&xs));
            }
            (Column::Str(v), _) => v.push(Some(label.clone())),
            (Column::Bool(v), _) => v.push(false),
            (Column::Date(v), _) => v.push(None),
            _ => unreachable!("column types are preserved"),
        }
    }
    Ok(out)
}

/// Concatenate tables. Columns are matched by name (first table's order, then new names); a
/// column missing from a table is null there; the same name with different types is an error.
/// The key is kept when every table declares the same key.
pub fn union(tables: &[&Table]) -> Result<Table, DataError> {
    let Some(first) = tables.first() else { return Ok(Table::default()) };
    let mut names: Vec<(String, ColumnType)> = Vec::new();
    for t in tables {
        for (n, c) in &t.columns {
            match names.iter().find(|x| x.0 == *n) {
                Some((_, ty)) if *ty != c.ty() => {
                    return Err(DataError::TypeMismatch { column: n.clone(), expected: ty.name().into(), found: c.ty().name().into() })
                }
                Some(_) => {}
                None => names.push((n.clone(), c.ty())),
            }
        }
    }
    let mut columns = Vec::with_capacity(names.len());
    for (n, ty) in &names {
        let mut acc = Column::nulls(*ty, 0);
        for t in tables {
            let part = t.column(n).cloned().unwrap_or_else(|| Column::nulls(*ty, t.len()));
            acc = acc.concat(&part).expect("types checked");
        }
        columns.push((n.clone(), acc));
    }
    let key = if tables.iter().all(|t| t.key == first.key) { first.key.clone() } else { Vec::new() };
    Ok(Table { name: first.name.clone(), columns, key, version: tables.iter().map(|t| t.version).max().unwrap_or(0) })
}

/// `n` rows drawn without replacement using a seeded [`Rng`] (partial Fisher–Yates), kept in their
/// original order. Same seed → same rows on every target. `n ≥ len` returns every row.
pub fn sample(t: &Table, n: usize, seed: u64) -> Table {
    let len = t.len();
    if n >= len {
        return t.clone();
    }
    let mut idx: Vec<usize> = (0..len).collect();
    let mut rng = Rng::new(seed);
    for i in 0..n {
        let j = i + rng.below((len - i) as u64) as usize;
        idx.swap(i, j);
    }
    let mut chosen = idx[..n].to_vec();
    chosen.sort_unstable();
    t.take_rows(&chosen)
}

/// A unique output column name: `base`, else `base_2`, `base_3`, …
pub(crate) fn unique_name(taken: &[String], base: &str) -> String {
    if !taken.iter().any(|n| n == base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base}_{k}")).find(|n| !taken.contains(n)).expect("some suffix is free")
}
