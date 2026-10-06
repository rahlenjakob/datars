//! Calendar heatmap cells: one panel per year, a column per week, a row per weekday.

use crate::date::{civil_from_days, day_of_year, days_from_civil, weekday};
use crate::util::mag;
use datars_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// The first day of the week (row 0 of a calendar panel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeekStart {
    #[default]
    Monday,
    Sunday,
}

impl WeekStart {
    /// Row (0–6) of an ISO weekday (0 = Monday).
    fn row(self, iso: u32) -> u32 {
        match self {
            WeekStart::Monday => iso,
            WeekStart::Sunday => (iso + 1) % 7,
        }
    }
}

/// One day's cell in a [`calendar`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalendarCell {
    /// Calendar year of the day.
    pub year: i32,
    /// Year panel: `year − first year in the input` (so panels stack in year order).
    pub panel: usize,
    /// Week column within the year (0–53): week 0 holds 1 January.
    pub col: usize,
    /// Weekday row (0–6) counted from `week_start`.
    pub row: usize,
    /// The cell in panel-local coordinates: `(col · cell, row · cell, cell, cell)`. Offset each panel
    /// yourself (e.g. `y += panel · (7 · cell + gap)`).
    pub rect: Rect,
}

/// Calendar-heatmap cells for `days` (days since 1970-01-01), indexed like the input.
///
/// Each year is a panel of up to 54 week columns × 7 weekday rows, the classic GitHub-style grid:
/// column = week of the year counted so that the week containing 1 January is column 0, row = day
/// of the week from `week_start`. Days needn't be sorted, contiguous or unique.
pub fn calendar(days: &[i32], cell: f64, week_start: WeekStart) -> Vec<CalendarCell> {
    let cell = mag(cell);
    let first_year = days.iter().map(|&d| civil_from_days(d).0).min().unwrap_or(0);
    days.iter()
        .map(|&d| {
            let (year, _, _) = civil_from_days(d);
            let (col, row) = week_col_row(d, week_start);
            CalendarCell {
                year,
                panel: (year as i64 - first_year as i64) as usize,
                col,
                row,
                rect: Rect::new(col as f64 * cell, row as f64 * cell, cell, cell),
            }
        })
        .collect()
}

/// `(week column, weekday row)` of a day within its year's panel.
fn week_col_row(d: i32, week_start: WeekStart) -> (usize, usize) {
    let (year, _, _) = civil_from_days(d);
    let jan1 = days_from_civil(year, 1, 1);
    let offset = week_start.row(weekday(jan1));
    let col = (day_of_year(d) + offset) / 7;
    (col as usize, week_start.row(weekday(d)) as usize)
}

/// The outline of `month` (1–12) of `year` in a calendar panel with square cells of side `cell`:
/// the stepped polygon d3's calendar examples draw between months (panel-local coordinates,
/// implicitly closed).
pub fn calendar_month_outline(year: i32, month: u32, cell: f64, week_start: WeekStart) -> Vec<Vec2> {
    let month = month.clamp(1, 12);
    let c = mag(cell);
    let d0 = days_from_civil(year, month, 1);
    // Month 13 rolls over to January of the next year inside `days_from_civil` (in i64).
    let d1 = days_from_civil(year, month + 1, 1).saturating_sub(1);
    let (w0, r0) = week_col_row(d0, week_start);
    let (w1, r1) = week_col_row(d1, week_start);
    let (w0, r0, w1, r1) = (w0 as f64, r0 as f64, w1 as f64, r1 as f64);
    vec![
        Vec2::new((w0 + 1.0) * c, r0 * c),
        Vec2::new(w0 * c, r0 * c),
        Vec2::new(w0 * c, 7.0 * c),
        Vec2::new(w1 * c, 7.0 * c),
        Vec2::new(w1 * c, (r1 + 1.0) * c),
        Vec2::new((w1 + 1.0) * c, (r1 + 1.0) * c),
        Vec2::new((w1 + 1.0) * c, 0.0),
        Vec2::new((w0 + 1.0) * c, 0.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::path::signed_area;

    #[test]
    fn cells_by_week_and_weekday() {
        // 2024-01-01 was a Monday.
        let jan1 = days_from_civil(2024, 1, 1);
        let days: Vec<i32> = (jan1..jan1 + 366).collect();
        let cells = calendar(&days, 10.0, WeekStart::Monday);
        assert_eq!((cells[0].col, cells[0].row), (0, 0));
        assert_eq!((cells[6].col, cells[6].row), (0, 6), "Sunday closes week 0");
        assert_eq!((cells[7].col, cells[7].row), (1, 0));
        assert_eq!(cells[7].rect, Rect::new(10.0, 0.0, 10.0, 10.0));
        let last = cells.last().unwrap();
        assert_eq!((last.col, last.row), (52, 1), "2024-12-31 was a Tuesday");
        // Unique cells within the year.
        let mut seen: Vec<(usize, usize)> = cells.iter().map(|c| (c.col, c.row)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 366);
        let sun = calendar(&days[..1], 10.0, WeekStart::Sunday);
        assert_eq!((sun[0].col, sun[0].row), (0, 1));
    }

    #[test]
    fn panels_follow_years() {
        let d = [days_from_civil(2022, 6, 1), days_from_civil(2020, 1, 1), days_from_civil(2021, 12, 31)];
        let cells = calendar(&d, 1.0, WeekStart::Monday);
        assert_eq!(cells.iter().map(|c| c.panel).collect::<Vec<_>>(), vec![2, 0, 1]);
        assert_eq!(cells[0].year, 2022);
        assert!(calendar(&[], 1.0, WeekStart::Monday).is_empty());
        assert!(cells[2].col <= 53);
    }

    #[test]
    fn month_outlines_partition_the_year() {
        let area: f64 = (1..=12).map(|m| signed_area(&calendar_month_outline(2023, m, 1.0, WeekStart::Monday)).abs()).sum();
        assert!((area - 365.0).abs() < 1e-9, "{area}");
    }
}
