//! Live sources' arrivals (docs/06 §Live data): what a new version of a source does to the rows
//! already there, and which rows it keeps.
//!
//! A `snapshot` source is replaced by every arrival (keys absent now exit). An `upsert` or
//! `append` source takes arrivals as *rows*: a feed sends what is new — the trade that just
//! happened, the minute that just closed — not the whole table again. Rows merge by key (a known
//! key's row is replaced where it stands, so its mark moves rather than exits and enters; a new
//! key joins at the end), then the source's retention drops what is too old: the last `window`
//! rows, and the rows within `span` seconds of the newest. Rows that leave exit with animation
//! like any other, because the scene is resolved from the merged table as usual.
//!
//! Pure functions of tables (no clock: "the newest" is the newest row, not the time now), so a
//! recorded feed replays to the same rows on every target.

use datars_data::{Column, Table};
use datars_ir::LiveDecl;
use datars_scene::Key;
use std::collections::BTreeMap;

/// Does this source take arrivals as rows to merge (`upsert` / `append`), not whole tables?
pub fn merges(live: Option<&LiveDecl>) -> bool {
    live.is_some_and(|l| matches!(l.mode.as_str(), "upsert" | "append"))
}

/// `fresh` rows merged into `old` by `key` (none: appended as they are), then retained.
pub fn merge(old: Option<&Table>, fresh: Table, key: &[String], live: &LiveDecl) -> Result<Table, String> {
    let all = match old {
        Some(o) if !o.is_empty() => datars_data::transform::union(&[o, &fresh]).map_err(|e| e.to_string())?,
        _ => fresh,
    };
    let merged = if key.is_empty() { all } else { by_key(&all, key)? };
    Ok(retain(merged, key, live))
}

/// One row per key: where the key first stood, with the values it arrived with last.
fn by_key(t: &Table, key: &[String]) -> Result<Table, String> {
    let mut keyed = t.clone();
    keyed.key = key.to_vec();
    for k in key {
        keyed.require(k).map_err(|e| e.to_string())?;
    }
    let mut at: BTreeMap<Key, usize> = BTreeMap::new();
    let mut order: Vec<usize> = Vec::with_capacity(t.len());
    for i in 0..keyed.len() {
        match at.get(&keyed.row_key(i)) {
            Some(&slot) => order[slot] = i,
            None => {
                at.insert(keyed.row_key(i), order.len());
                order.push(i);
            }
        }
    }
    let out = keyed.take_rows(&order);
    out.validate_keys().map_err(|e| e.to_string())?;
    Ok(out)
}

/// The rows a live source keeps: within `span` seconds of the newest on its `on` column, and the
/// last `window` of those.
pub fn retain(t: Table, key: &[String], live: &LiveDecl) -> Table {
    let mut keep: Vec<usize> = (0..t.len()).collect();
    if let Some(span) = live.span.filter(|s| s.is_finite() && *s >= 0.0) {
        if let Some(secs) = live.on.as_deref().or(key.last().map(String::as_str)).and_then(|c| t.column(c)).and_then(seconds) {
            let newest = secs.iter().copied().filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
            if newest.is_finite() {
                // Rows without a time stay: retention drops what is old, not what is unknown.
                keep.retain(|&i| !secs[i].is_finite() || secs[i] >= newest - span);
            }
        }
    }
    if let Some(n) = live.window {
        if keep.len() > n {
            keep.drain(..keep.len() - n);
        }
    }
    if keep.len() == t.len() {
        t
    } else {
        t.take_rows(&keep)
    }
}

/// A column's values in seconds: dates are days, numbers millisecond timestamps (as a time scale
/// reads them). Text has no time.
fn seconds(c: &Column) -> Option<Vec<f64>> {
    match c {
        Column::Date(v) => Some(v.iter().map(|d| d.map_or(f64::NAN, |d| d as f64 * 86_400.0)).collect()),
        Column::Num(v) => Some(v.iter().map(|ms| ms / 1000.0).collect()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_data::Column;

    fn live(mode: &str, window: Option<usize>, span: Option<f64>) -> LiveDecl {
        LiveDecl { every: 0.0, mode: mode.into(), window, span, on: None }
    }

    fn rows(sym: &[&str], t: &[f64], p: &[f64]) -> Table {
        Table::from_columns("prices", vec![("sym", Column::strs(sym)), ("t", Column::Num(t.to_vec())), ("p", Column::Num(p.to_vec()))]).unwrap()
    }

    fn key() -> Vec<String> {
        vec!["sym".into(), "t".into()]
    }

    #[test]
    fn upserts_replace_in_place_and_new_keys_join_at_the_end() {
        let old = rows(&["A", "A"], &[0.0, 60_000.0], &[10.0, 11.0]);
        let fresh = rows(&["A", "A"], &[60_000.0, 120_000.0], &[11.5, 12.0]);
        let t = merge(Some(&old), fresh, &key(), &live("upsert", None, None)).unwrap();
        assert_eq!(t.num("t").unwrap(), &[0.0, 60_000.0, 120_000.0]);
        assert_eq!(t.num("p").unwrap(), &[10.0, 11.5, 12.0], "the open minute took its latest price");
        assert_eq!(t.key, key());
    }

    #[test]
    fn a_batch_may_repeat_a_key_and_the_last_row_wins() {
        // Trades within one minute arrive together: the minute's price is the last trade's.
        let fresh = rows(&["A", "A", "A"], &[0.0, 0.0, 0.0], &[1.0, 2.0, 3.0]);
        let t = merge(None, fresh, &key(), &live("append", None, None)).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.num("p").unwrap(), &[3.0]);
    }

    #[test]
    fn append_without_a_key_just_appends() {
        let old = rows(&["A"], &[0.0], &[1.0]);
        let t = merge(Some(&old), rows(&["A"], &[0.0], &[2.0]), &[], &live("append", None, None)).unwrap();
        assert_eq!(t.num("p").unwrap(), &[1.0, 2.0]);
    }

    #[test]
    fn a_window_keeps_the_last_rows() {
        let old = rows(&["A", "A", "A"], &[0.0, 1.0, 2.0], &[1.0, 2.0, 3.0]);
        let t = merge(Some(&old), rows(&["A"], &[3.0], &[4.0]), &key(), &live("append", Some(2), None)).unwrap();
        assert_eq!(t.num("p").unwrap(), &[3.0, 4.0]);
    }

    #[test]
    fn a_span_keeps_the_rows_near_the_newest() {
        // Yesterday's session leaves once today's first minute arrives (span: 8 hours).
        let h = 3_600_000.0;
        let old = rows(&["A", "A", "B"], &[0.0, 6.0 * h, 6.0 * h], &[1.0, 2.0, 5.0]);
        let fresh = rows(&["A"], &[24.0 * h], &[3.0]);
        let mut l = live("upsert", None, Some(8.0 * 3600.0));
        l.on = Some("t".into());
        let t = merge(Some(&old), fresh, &key(), &l).unwrap();
        assert_eq!(t.num("p").unwrap(), &[3.0]);
        // Within the span everything stays, whichever series it belongs to.
        let t = retain(rows(&["A", "B"], &[0.0, 7.0 * h], &[1.0, 2.0]), &key(), &l);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn spans_read_dates_as_days() {
        let t = Table::from_columns("d", vec![("day", Column::Date(vec![Some(0), Some(5), Some(9), None]))]).unwrap();
        let mut l = live("append", None, Some(5.0 * 86_400.0));
        l.on = Some("day".into());
        let kept = retain(t, &[], &l);
        assert_eq!(kept.date("day").unwrap(), &[Some(5), Some(9), None], "rows without a date stay");
    }

    #[test]
    fn snapshot_sources_do_not_merge() {
        assert!(!merges(Some(&live("snapshot", None, None))));
        assert!(merges(Some(&live("upsert", None, None))));
        assert!(!merges(None));
    }
}
