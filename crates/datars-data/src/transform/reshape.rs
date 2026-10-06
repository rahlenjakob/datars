//! Joins and long ↔ wide reshaping.

use super::{group_rows, unique_name};
use crate::table::{Column, ColumnType, Table};
use crate::value::Value;
use crate::DataError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinKind {
    /// Only left rows with a match.
    Inner,
    /// Every left row; right columns are null where nothing matches.
    Left,
}

/// Join on columns present in both tables (same names, same types — a text code never silently
/// fails to match a number). Output: left rows in order, each followed by its matches in right
/// order; left columns, then right columns except `on` (a clashing name gets the right table's name
/// as a suffix, or `_right`). Null keys never match. The output keeps the left key when each left
/// row matches at most one right row; otherwise the left key plus the right key's columns.
///
/// A side with no rows has nothing to mismatch: its key columns' types are whatever an empty
/// derivation guessed (a filter that matched nothing, a `derive` over zero rows), and the join is
/// still well defined — an inner join is empty, a left join keeps every left row.
pub fn join(left: &Table, right: &Table, on: &[&str], kind: JoinKind) -> Result<Table, DataError> {
    if on.is_empty() {
        return Err(DataError::Invalid("join: `on` names no columns".into()));
    }
    let mut lc = Vec::new();
    let mut rc = Vec::new();
    let empty = left.is_empty() || right.is_empty();
    for k in on {
        let (a, b) = (left.require(k)?, right.require(k)?);
        if a.ty() != b.ty() && !empty {
            return Err(DataError::TypeMismatch { column: format!("{k} (right table)"), expected: a.ty().name().into(), found: b.ty().name().into() });
        }
        lc.push(a);
        rc.push(b);
    }
    let key_at = |cs: &[&Column], i: usize| -> Option<Vec<Value>> {
        let k: Vec<Value> = cs.iter().map(|c| c.get(i)).collect();
        (!k.iter().any(Value::is_null)).then_some(k)
    };
    let mut index: BTreeMap<Vec<Value>, Vec<usize>> = BTreeMap::new();
    for j in 0..right.len() {
        if let Some(k) = key_at(&rc, j) {
            index.entry(k).or_default().push(j);
        }
    }
    let unique_right = index.values().all(|v| v.len() == 1);
    let (mut li, mut ri): (Vec<usize>, Vec<Option<usize>>) = (Vec::new(), Vec::new());
    for i in 0..left.len() {
        match key_at(&lc, i).and_then(|k| index.get(&k)) {
            Some(ms) => {
                for &j in ms {
                    li.push(i);
                    ri.push(Some(j));
                }
            }
            None if kind == JoinKind::Left => {
                li.push(i);
                ri.push(None);
            }
            None => {}
        }
    }
    let mut columns: Vec<(String, Column)> = left.columns.iter().map(|(n, c)| (n.clone(), c.take(&li))).collect();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    for (n, c) in &right.columns {
        if on.contains(&n.as_str()) {
            continue;
        }
        let taken: Vec<String> = columns.iter().map(|c| c.0.clone()).collect();
        let base = if taken.contains(n) { format!("{n}_{}", if right.name.is_empty() { "right" } else { &right.name }) } else { n.clone() };
        let name = unique_name(&taken, &base);
        renamed.insert(n.clone(), name.clone());
        columns.push((name, c.take_opt(&ri)));
    }
    let mut key = left.key.clone();
    if !unique_right {
        key.extend(right.key.iter().filter_map(|k| renamed.get(k).cloned()));
    }
    let mut out = Table { name: left.name.clone(), columns, key, version: left.version.max(right.version) };
    if out.validate_keys().is_err() {
        out.key.clear();
    }
    Ok(out)
}

/// Long → wide: one row per distinct `index` combination (first appearance), one column per
/// distinct `names_from` value (first appearance; named by its text), holding `values_from`
/// (same type; null where a combination is missing). A repeated (index, name) pair is an error —
/// aggregate first. The output is keyed by `index`.
pub fn pivot(t: &Table, index: &[&str], names_from: &str, values_from: &str) -> Result<Table, DataError> {
    let names = t.require(names_from)?;
    let values = t.require(values_from)?;
    let groups = group_rows(t, index)?;
    let mut name_order: Vec<Value> = Vec::new();
    let mut name_index: BTreeMap<Value, usize> = BTreeMap::new();
    for i in 0..t.len() {
        let v = names.get(i);
        if v.is_null() {
            return Err(DataError::Invalid(format!("pivot: \"{names_from}\" is empty at row {i} (every row needs a column name)")));
        }
        if !name_index.contains_key(&v) {
            name_index.insert(v.clone(), name_order.len());
            name_order.push(v);
        }
    }
    let mut cells: Vec<Vec<Option<usize>>> = vec![vec![None; groups.len()]; name_order.len()];
    for (g, (_, rows)) in groups.iter().enumerate() {
        for &i in rows {
            let c = name_index[&names.get(i)];
            if let Some(prev) = cells[c][g] {
                return Err(DataError::Invalid(format!(
                    "pivot: rows {prev} and {i} both give ({}) a value for \"{}\" — aggregate first",
                    groups[g].0.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
                    name_order[c]
                )));
            }
            cells[c][g] = Some(i);
        }
    }
    let first_rows: Vec<usize> = groups.iter().map(|g| g.1[0]).collect();
    let mut columns: Vec<(String, Column)> = index.iter().map(|n| Ok((n.to_string(), t.require(n)?.take(&first_rows)))).collect::<Result<_, DataError>>()?;
    for (c, name) in name_order.iter().enumerate() {
        let name = name.to_string();
        if columns.iter().any(|x| x.0 == name) {
            return Err(DataError::Invalid(format!("pivot: a new column \"{name}\" clashes with an index column")));
        }
        columns.push((name, values.take_opt(&cells[c])));
    }
    Ok(Table { name: t.name.clone(), columns, key: index.iter().map(|s| s.to_string()).collect(), version: t.version })
}

/// Wide → long: for every row and every column in `value_columns` (all columns not in `id` when
/// empty), a row with the `id` columns, `names_to` = the column's name (text) and `values_to` = its
/// value. Value columns must share a type. Rows are row-major (all of row 0's columns, then row 1…).
/// Keyed by the input key (or `id`) plus `names_to` when that is unique.
pub fn unpivot(t: &Table, id: &[&str], value_columns: &[&str], names_to: &str, values_to: &str) -> Result<Table, DataError> {
    for c in id {
        t.require(c)?;
    }
    let vcols: Vec<&str> = if value_columns.is_empty() {
        t.columns.iter().map(|c| c.0.as_str()).filter(|c| !id.contains(c)).collect()
    } else {
        value_columns.to_vec()
    };
    let cols: Vec<&Column> = vcols.iter().map(|c| t.require(c)).collect::<Result<_, _>>()?;
    let ty = cols.first().map_or(ColumnType::Num, |c| c.ty());
    if let Some((n, c)) = vcols.iter().zip(&cols).find(|(_, c)| c.ty() != ty) {
        return Err(DataError::TypeMismatch { column: n.to_string(), expected: ty.name().into(), found: c.ty().name().into() });
    }
    let k = vcols.len();
    let rows: Vec<usize> = (0..t.len()).flat_map(|i| std::iter::repeat_n(i, k)).collect();
    let mut columns: Vec<(String, Column)> = id.iter().map(|c| (c.to_string(), t.require(c).expect("checked").take(&rows))).collect();
    let labels: Vec<Arc<str>> = vcols.iter().map(|c| Arc::from(*c)).collect();
    columns.push((names_to.to_string(), Column::Str((0..rows.len()).map(|r| Some(labels[r % k].clone())).collect())));
    let vals: Vec<Value> = (0..rows.len()).map(|r| cols[r % k].get(rows[r])).collect();
    columns.push((values_to.to_string(), Column::from_values(ty, &vals)));
    let base: Vec<String> = if !t.key.is_empty() && t.key.iter().all(|c| id.contains(&c.as_str())) { t.key.clone() } else { id.iter().map(|s| s.to_string()).collect() };
    let mut out = Table::from_columns(&t.name, columns)?;
    out.version = t.version;
    out.key = base.into_iter().chain([names_to.to_string()]).collect();
    if out.validate_keys().is_err() {
        out.key.clear();
    }
    Ok(out)
}
