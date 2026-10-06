//! `datars data profile`: what a data file holds before it is charted — column types, missing
//! values, cardinality, ranges, the columns that could key its rows (P3: identity drives joins and
//! transitions), and hints about what each column is for. Heuristics, stated as suggestions.

use datars_data::{Column, CsvOptions, Table};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Profile {
    pub rows: usize,
    pub columns: Vec<ColumnProfile>,
    /// Columns (or pairs) whose values identify every row: candidate `key`s.
    pub keys: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ColumnProfile {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: &'static str,
    pub nulls: usize,
    pub distinct: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<serde_json::Value>,
    /// The most frequent values of a text column, with counts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub top: Vec<(String, usize)>,
    pub hints: Vec<String>,
}

/// Where a column's values might join: named id sets (`"countries"` → the atlas's ISO codes).
pub type IdSets = BTreeMap<String, BTreeSet<String>>;

/// Profile CSV or JSON bytes (records or columns), read the way documents read them.
pub fn profile_bytes(name: &str, bytes: &[u8], ids: &IdSets) -> Result<Profile, String> {
    let looks_json = bytes.iter().find(|b| !b.is_ascii_whitespace()).is_some_and(|b| *b == b'[' || *b == b'{');
    let t = if looks_json {
        datars_data::read_json(bytes).map_err(|e| e.to_string())?
    } else {
        datars_data::read_csv(bytes, &CsvOptions { name: name.to_string(), ..Default::default() }).map_err(|e| e.to_string())?
    };
    Ok(profile(&t, ids))
}

/// A value's identity for counting distinct values (numbers by bit pattern, -0 as 0).
fn ident(c: &Column, i: usize) -> Option<String> {
    if c.is_null(i) {
        return None;
    }
    Some(match c {
        Column::Num(v) => format!("{}", if v[i] == 0.0 { 0.0 } else { v[i] }),
        Column::Str(v) => v[i].as_deref().unwrap_or("").to_string(),
        Column::Bool(v) => v[i].to_string(),
        Column::Date(v) => v[i].unwrap_or(0).to_string(),
    })
}

pub fn profile(t: &Table, ids: &IdSets) -> Profile {
    let rows = t.len();
    let columns: Vec<ColumnProfile> = t.columns.iter().map(|(name, c)| column(name, c, rows, ids)).collect();

    // Candidate keys: single columns without gaps or repeats (not measurements), else pairs.
    let idents: Vec<Vec<Option<String>>> = t.columns.iter().map(|(_, c)| (0..rows).map(|i| ident(c, i)).collect()).collect();
    let keyish = |ci: usize| {
        let (_, c) = &t.columns[ci];
        match c {
            Column::Num(v) => v.iter().all(|x| x.fract() == 0.0),
            Column::Bool(_) => false,
            _ => true,
        }
    };
    let unique = |cols: &[usize]| {
        let mut seen = BTreeSet::new();
        (0..rows).all(|i| {
            let parts: Option<Vec<&String>> = cols.iter().map(|&c| idents[c][i].as_ref()).collect();
            parts.is_some_and(|p| seen.insert(p))
        })
    };
    let mut keys: Vec<Vec<String>> = (0..t.columns.len()).filter(|&c| keyish(c) && rows > 0 && unique(&[c])).map(|c| vec![t.columns[c].0.clone()]).collect();
    if keys.is_empty() && rows > 0 {
        let cands: Vec<usize> = (0..t.columns.len()).filter(|&c| keyish(c) && columns[c].nulls == 0).collect();
        'outer: for (i, &a) in cands.iter().enumerate() {
            for &b in &cands[i + 1..] {
                if unique(&[a, b]) {
                    keys.push(vec![t.columns[a].0.clone(), t.columns[b].0.clone()]);
                    if keys.len() >= 3 {
                        break 'outer;
                    }
                }
            }
        }
    }
    Profile { rows, columns, keys }
}

fn column(name: &str, c: &Column, rows: usize, ids: &IdSets) -> ColumnProfile {
    let nulls = c.null_count();
    let present = rows - nulls;
    let distinct_set: BTreeSet<String> = (0..rows).filter_map(|i| ident(c, i)).collect();
    let distinct = distinct_set.len();
    let lname = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lname == *w || lname.split(|ch: char| !ch.is_alphanumeric()).any(|p| p == *w));
    let mut hints = Vec::new();
    let (mut min, mut max, mut top) = (None, None, Vec::new());
    match c {
        Column::Num(v) => {
            let vals: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
            if let (Some(lo), Some(hi)) = (vals.iter().copied().reduce(f64::min), vals.iter().copied().reduce(f64::max)) {
                min = Some(lo.into());
                max = Some(hi.into());
                let ints = vals.iter().all(|x| x.fract() == 0.0);
                if has(&["lat", "latitude"]) && lo >= -90.0 && hi <= 90.0 {
                    hints.push("latitude: with a longitude column, points on a map (`symbols` in a geo view)".into());
                } else if has(&["lon", "lng", "long", "longitude"]) && lo >= -180.0 && hi <= 180.0 {
                    hints.push("longitude: with a latitude column, points on a map (`symbols` in a geo view)".into());
                } else if ints && (has(&["year", "yr"]) || (lo >= 1800.0 && hi <= 2100.0 && distinct > 1)) {
                    hints.push("years as numbers: read them as dates (`years_as_dates`) for a time axis, or keep a linear scale".into());
                } else if lo >= 0.0 && hi <= 1.0 && !ints {
                    hints.push("fractions 0–1: format as percent (`.0%`)".into());
                } else if lo < 0.0 && hi > 0.0 {
                    hints.push("signed: bars from a zero baseline, or a diverging colour scale around 0".into());
                } else if ints && distinct <= 12 && distinct < present {
                    hints.push(format!("{distinct} distinct integers: an ordinal? (a band axis)"));
                }
                let pos: Vec<f64> = vals.iter().copied().filter(|x| *x > 0.0).collect();
                if let (Some(plo), Some(phi)) = (pos.iter().copied().reduce(f64::min), pos.iter().copied().reduce(f64::max)) {
                    if lo > 0.0 && phi / plo >= 1000.0 {
                        hints.push(format!("spans {} orders of magnitude: consider a log scale", datars_math::m::log10(phi / plo).floor()));
                    }
                }
                if has(&["pct", "percent", "percentage", "share"]) && lo >= 0.0 && hi <= 100.0 && hi > 1.0 {
                    hints.push("percentages 0–100: a share of a whole (stacked bars, a waffle, a pie for few parts)".into());
                }
            }
        }
        Column::Str(v) => {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for s in v.iter().flatten() {
                *counts.entry(s).or_default() += 1;
            }
            let mut by_count: Vec<(&str, usize)> = counts.into_iter().collect();
            by_count.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            // Frequencies say something only when values repeat.
            if by_count.first().is_some_and(|(_, n)| *n > 1) {
                top = by_count.iter().take(5).map(|(s, n)| (s.to_string(), *n)).collect();
            }
            if nulls == 0 && distinct == rows && rows > 1 {
                hints.push("unique: a key — rows keep their identity through filters, joins and transitions".into());
            } else if distinct <= 12 {
                hints.push(format!("category ({distinct} values): colour, a band axis or small multiples"));
            } else {
                hints.push(format!("{distinct} categories: more than colours can tell apart — top N plus “other”, a sorted bar list, or search"));
            }
            for (set, known) in ids {
                let hit = distinct_set.iter().filter(|s| known.contains(*s)).count();
                if hit > 0 && hit * 10 >= distinct * 8 {
                    let missing: Vec<&String> = distinct_set.iter().filter(|s| !known.contains(*s)).take(5).collect();
                    let tail = if missing.is_empty() { String::new() } else { format!(" (not found: {})", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) };
                    hints.push(format!("{hit} of {distinct} values are `{set}` ids: join the atlas by this column{tail}"));
                }
            }
        }
        Column::Date(v) => {
            let mut days: Vec<i32> = v.iter().flatten().copied().collect();
            days.sort_unstable();
            days.dedup();
            if let (Some(lo), Some(hi)) = (days.first(), days.last()) {
                min = serde_json::to_value(datars_data::Value::Date(*lo)).ok();
                max = serde_json::to_value(datars_data::Value::Date(*hi)).ok();
            }
            let gap = days.windows(2).map(|w| w[1] - w[0]).min();
            let grain = match gap {
                Some(1) => "daily",
                Some(7) => "weekly",
                Some(28..=31) => "monthly",
                Some(89..=92) => "quarterly",
                Some(365 | 366) => "yearly",
                Some(_) => "irregular",
                None => "a single date",
            };
            hints.push(format!("time ({grain}): a time axis; lines for trends, a scrubbed program for change"));
        }
        Column::Bool(_) => hints.push("flag: filter, highlight or split by it".into()),
    }
    if nulls > 0 {
        hints.push(format!("{nulls} missing ({:.0}%): marks skip them — filter or impute explicitly", nulls as f64 * 100.0 / rows.max(1) as f64));
    }
    ColumnProfile { name: name.to_string(), ty: c.ty().name(), nulls, distinct, min, max, top, hints }
}

/// The profile as text for a terminal.
pub fn to_text(p: &Profile) -> String {
    let mut s = format!("{} rows, {} columns\n", p.rows, p.columns.len());
    for c in &p.columns {
        let range = match (&c.min, &c.max) {
            (Some(a), Some(b)) => format!("  {a} … {b}"),
            _ => String::new(),
        };
        s += &format!("\n{}  {}  {} distinct{}{}\n", c.name, c.ty, c.distinct, if c.nulls > 0 { format!(", {} missing", c.nulls) } else { String::new() }, range);
        if !c.top.is_empty() {
            s += &format!("  top: {}\n", c.top.iter().map(|(v, n)| format!("{v} ({n})")).collect::<Vec<_>>().join(", "));
        }
        for h in &c.hints {
            s += &format!("  → {h}\n");
        }
    }
    s += "\n";
    if p.keys.is_empty() {
        s += "keys: none — no column or pair identifies every row (add an id, or rows fall back to their index)\n";
    } else {
        s += &format!("keys: {}\n", p.keys.iter().map(|k| format!("[{}]", k.join(", "))).collect::<Vec<_>>().join(" or "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_table_is_profiled_with_its_key_and_hints() {
        let csv = b"country,year,share,note\nSWE,2020,0.6,\nSWE,2021,0.62,x\nNOR,2020,0.7,\nNOR,2021,0.71,\nXXX,2021,0.1,\n";
        let ids: IdSets = BTreeMap::from([("countries".to_string(), ["SWE", "NOR", "FIN", "DNK"].iter().map(|s| s.to_string()).collect())]);
        let p = profile_bytes("t", csv, &ids).unwrap();
        assert_eq!(p.rows, 5);
        assert_eq!(p.keys, vec![vec!["country".to_string(), "year".to_string()]], "no single column is unique; the pair is");
        let col = |n: &str| p.columns.iter().find(|c| c.name == n).unwrap();
        assert!(col("year").hints.iter().any(|h| h.starts_with("years as numbers")), "{:?}", col("year").hints);
        assert!(col("share").hints.iter().any(|h| h.starts_with("fractions")));
        assert!(!col("country").hints.iter().any(|h| h.contains("`countries` ids")), "2 of 3 is below the 80% bar");
        assert_eq!(col("note").nulls, 4);
        assert!(col("note").hints.iter().any(|h| h.starts_with("4 missing")));
        assert_eq!(col("country").top[0], ("NOR".to_string(), 2));
        let text = to_text(&p);
        assert!(text.contains("keys: [country, year]"), "{text}");
    }

    #[test]
    fn unique_columns_are_keys_and_atlas_ids_are_recognised() {
        let json = br#"{"id": ["SWE", "NOR", "FIN", "DNK", "ISL"], "v": [1, 20, 300, 4000, 50000]}"#;
        let ids: IdSets = BTreeMap::from([("countries".to_string(), ["SWE", "NOR", "FIN", "DNK", "ISL"].iter().map(|s| s.to_string()).collect())]);
        let p = profile_bytes("t", json, &ids).unwrap();
        assert_eq!(p.keys[0], vec!["id".to_string()]);
        let id = &p.columns[0];
        assert!(id.hints.iter().any(|h| h.starts_with("unique")));
        assert!(id.hints.iter().any(|h| h.starts_with("5 of 5 values are `countries` ids")), "{:?}", id.hints);
        assert!(p.columns[1].hints.iter().any(|h| h.contains("orders of magnitude")), "{:?}", p.columns[1].hints);
    }
}
