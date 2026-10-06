//! Axis ticks: "nice" numbers (1, 2, 5 × 10ⁿ, as in d3-array) and calendar-aware date ticks.

use crate::date::{civil_from_days, days_from_civil, weekday};
use datars_math::m;
use serde::{Deserialize, Serialize};

const E10: f64 = 7.0710678118654755; // √50
const E5: f64 = 3.1622776601683795; // √10
const E2: f64 = std::f64::consts::SQRT_2;

/// 10^p for integer `p`, exact where representable (repeated multiplication, no libm rounding).
fn pow10(p: f64) -> f64 {
    if p.abs() <= 22.0 {
        let mut v = 1.0;
        for _ in 0..(p.abs() as i32) {
            v *= 10.0;
        }
        if p < 0.0 {
            1.0 / v
        } else {
            v
        }
    } else {
        m::pow(10.0, p)
    }
}

/// d3's tickSpec: tick indices `i1..=i2` and an increment (negative = the inverse of the step, so
/// fractional ticks are computed as `i / k` and come out as clean decimals like 0.3).
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
    let power = m::log10(step).floor();
    let error = step / pow10(power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, inc);
    if power < 0.0 {
        let k = pow10(-power) / factor;
        i1 = (start * k).round();
        i2 = (stop * k).round();
        if i1 / k < start {
            i1 += 1.0;
        }
        if i2 / k > stop {
            i2 -= 1.0;
        }
        inc = -k;
    } else {
        let k = pow10(power) * factor;
        i1 = (start / k).round();
        i2 = (stop / k).round();
        if i1 * k < start {
            i1 += 1.0;
        }
        if i2 * k > stop {
            i2 -= 1.0;
        }
        inc = k;
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

fn usable(lo: f64, hi: f64, count: usize) -> bool {
    lo.is_finite() && hi.is_finite() && count > 0
}

/// About `count` evenly spaced "nice" values (multiples of 1, 2 or 5 × 10ⁿ) inside `[lo, hi]`,
/// in the direction `lo → hi` (descending when `hi < lo`). Fractional ticks are computed as
/// `i / k`, so they print cleanly (0.1, 0.2, 0.3 — not 0.30000000000000004).
///
/// Empty for non-finite bounds or `count == 0`; `[lo]` when `lo == hi`. Same results as
/// `d3.ticks`.
///
/// ```
/// # use datars_algo::nice_ticks;
/// assert_eq!(nice_ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
/// assert_eq!(nice_ticks(0.0, 1.0, 5), vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
/// ```
pub fn nice_ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    if !usable(lo, hi, count) {
        return Vec::new();
    }
    if lo == hi {
        return vec![lo];
    }
    let reverse = hi < lo;
    let (i1, i2, inc) = if reverse { tick_spec(hi, lo, count as f64) } else { tick_spec(lo, hi, count as f64) };
    if i1.is_nan() || i2.is_nan() || i2 < i1 || !inc.is_finite() || inc == 0.0 {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0).min(1e6) as usize;
    let at = |i: f64| if inc < 0.0 { i / -inc } else { i * inc };
    (0..n).map(|k| if reverse { at(i2 - k as f64) } else { at(i1 + k as f64) }).collect()
}

/// The positive step [`nice_ticks`] would use for `[lo, hi]` and `count` (0 when unusable).
pub fn tick_step(lo: f64, hi: f64, count: usize) -> f64 {
    if !usable(lo, hi, count) || lo == hi {
        return 0.0;
    }
    let (a, b) = if hi < lo { (hi, lo) } else { (lo, hi) };
    let inc = tick_spec(a, b, count as f64).2;
    if inc < 0.0 {
        1.0 / -inc
    } else {
        inc
    }
}

/// Extends `[lo, hi]` outward to nice round bounds for about `count` ticks (d3's `linear.nice`):
/// repeatedly snaps both ends to the tick step until the step stops changing. Keeps the direction
/// of the input. Non-finite input comes back unchanged; `lo == hi` too.
///
/// ```
/// # use datars_algo::nice_domain;
/// assert_eq!(nice_domain(0.13, 9.7, 10), (0.0, 10.0));
/// ```
pub fn nice_domain(lo: f64, hi: f64, count: usize) -> (f64, f64) {
    if !usable(lo, hi, count) || lo == hi {
        return (lo, hi);
    }
    let reverse = hi < lo;
    let (mut start, mut stop) = if reverse { (hi, lo) } else { (lo, hi) };
    let mut prestep = f64::NAN;
    for _ in 0..10 {
        let step = tick_spec(start, stop, count as f64).2;
        if step == prestep {
            break;
        } else if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else if step < 0.0 {
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        } else {
            break;
        }
        prestep = step;
    }
    if reverse {
        (stop, start)
    } else {
        (start, stop)
    }
}

/// Upper bound on the number of date ticks returned, whatever `count` asks for.
const MAX_TICKS: usize = 1_000_000;

/// Calendar units, finest first (so `Ord` compares granularity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TimeUnit {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

/// Average lengths in days, for choosing an interval.
const YEAR_DAYS: f64 = 365.2425;
const MONTH_DAYS: f64 = YEAR_DAYS / 12.0;
const INTERVALS: [(TimeUnit, u32, f64); 6] = [
    (TimeUnit::Day, 1, 1.0),
    (TimeUnit::Day, 2, 2.0),
    (TimeUnit::Week, 1, 7.0),
    (TimeUnit::Month, 1, MONTH_DAYS),
    (TimeUnit::Quarter, 1, 3.0 * MONTH_DAYS),
    (TimeUnit::Year, 1, YEAR_DAYS),
];

/// The tick interval `(unit, step)` for about `count` ticks over `[lo_day, hi_day]`, chosen like
/// d3's time scale: the candidate (1 or 2 days, a week, a month, a quarter, a year) whose length is
/// nearest the target spacing by ratio, or a nice number of years (2, 5, 10, 20, 50, …) beyond.
pub fn time_tick_interval(lo_day: i32, hi_day: i32, count: usize) -> (TimeUnit, u32) {
    let span = (hi_day as f64 - lo_day as f64).abs();
    let target = span / count.max(1) as f64;
    let i = INTERVALS.iter().position(|iv| iv.2 > target).unwrap_or(INTERVALS.len());
    if i == INTERVALS.len() {
        let years = tick_step(0.0, span / YEAR_DAYS, count.max(1)).max(1.0).round();
        return (TimeUnit::Year, years.min(u32::MAX as f64) as u32);
    }
    if i == 0 {
        return (TimeUnit::Day, 1);
    }
    let (a, b) = (INTERVALS[i - 1], INTERVALS[i]);
    let pick = if target / a.2 < b.2 / target { a } else { b };
    (pick.0, pick.1)
}

/// The coarsest calendar boundary `day` falls on: 1 January → `Year`, the first of January, April,
/// July or October → `Quarter`, any other first of a month → `Month`, a Monday → `Week`, else `Day`.
/// This is what a multi-scale date formatter keys on ("2024", "Apr", "Mar 4", …).
pub fn time_unit_of(day: i32) -> TimeUnit {
    let (_, mo, d) = civil_from_days(day);
    if d == 1 {
        if mo == 1 {
            TimeUnit::Year
        } else if (mo - 1) % 3 == 0 {
            TimeUnit::Quarter
        } else {
            TimeUnit::Month
        }
    } else if weekday(day) == 0 {
        TimeUnit::Week
    } else {
        TimeUnit::Day
    }
}

/// Calendar-aware ticks over `[lo_day, hi_day]` (days since 1970-01-01, inclusive, either order;
/// output ascending), about `count` of them: every day or every other day of the month, Mondays
/// (ISO weeks), first days of months or quarters, or 1 January of every n-th year (years divisible
/// by n). Each tick comes with [`time_unit_of`] its day — the coarsest boundary it falls on — so a
/// formatter can print years at year boundaries and months at month boundaries.
///
/// Empty when `count == 0`; a single tick when `lo_day == hi_day`.
pub fn time_ticks(lo_day: i32, hi_day: i32, count: usize) -> Vec<(i32, TimeUnit)> {
    if count == 0 {
        return Vec::new();
    }
    let (lo, hi) = if hi_day < lo_day { (hi_day, lo_day) } else { (lo_day, hi_day) };
    if lo == hi {
        return vec![(lo, time_unit_of(lo))];
    }
    let (unit, step) = time_tick_interval(lo, hi, count);
    let step = step.max(1);
    let mut days: Vec<i32> = Vec::new();
    match unit {
        TimeUnit::Day => {
            // (A day step is only chosen when about `count` ticks fit, so this loop is short unless
            // `count` itself is absurd; MAX_TICKS bounds that case.)
            for d in lo..=hi {
                if days.len() >= MAX_TICKS {
                    break;
                }
                if step == 1 || (civil_from_days(d).2 - 1) % step == 0 {
                    days.push(d);
                }
            }
        }
        TimeUnit::Week => {
            let first = lo as i64 + (7 - weekday(lo) as i64) % 7;
            let mut d = first;
            while d <= hi as i64 && days.len() < MAX_TICKS {
                days.push(d as i32);
                d += 7 * step as i64;
            }
        }
        TimeUnit::Month | TimeUnit::Quarter => {
            let months = if unit == TimeUnit::Quarter { 3 * step } else { step };
            let (y0, m0, _) = civil_from_days(lo);
            let (y1, m1, _) = civil_from_days(hi);
            let last = y1 as i64 * 12 + (m1 as i64 - 1);
            let mut idx = y0 as i64 * 12 + (m0 as i64 - 1);
            while idx <= last {
                let (y, mo) = (idx.div_euclid(12) as i32, (idx.rem_euclid(12) + 1) as u32);
                let d = days_from_civil(y, mo, 1);
                if d >= lo && d <= hi && (mo - 1) % months == 0 {
                    days.push(d);
                }
                idx += 1;
            }
        }
        TimeUnit::Year => {
            // Iterate years, not days: the range can span millions of years.
            let (y0, _, _) = civil_from_days(lo);
            let (y1, _, _) = civil_from_days(hi);
            let s = step as i64;
            let mut y = (y0 as i64).div_euclid(s) * s;
            while y <= y1 as i64 {
                let d = days_from_civil(y as i32, 1, 1);
                if d >= lo && d <= hi {
                    days.push(d);
                }
                y += s;
            }
        }
    }
    days.into_iter().map(|d| (d, time_unit_of(d))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_tick_examples() {
        assert_eq!(nice_ticks(0.0, 100.0, 5), vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(nice_ticks(0.0, 32.0, 6), vec![0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0]);
        assert_eq!(nice_ticks(-3.3, 7.1, 10), vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(nice_ticks(0.1, 0.35, 5), vec![0.1, 0.15, 0.2, 0.25, 0.3, 0.35]);
        assert_eq!(nice_ticks(0.0, 0.3, 3), vec![0.0, 0.1, 0.2, 0.3], "clean decimals");
        assert_eq!(nice_ticks(10.0, 0.0, 2), vec![10.0, 5.0, 0.0], "reversed");
        assert_eq!(nice_ticks(1e6, 5e6, 4), vec![1e6, 2e6, 3e6, 4e6, 5e6]);
        assert_eq!(nice_ticks(5.0, 5.0, 3), vec![5.0]);
        assert!(nice_ticks(0.0, f64::NAN, 5).is_empty());
        assert!(nice_ticks(0.0, 1.0, 0).is_empty());
        assert_eq!(nice_ticks(0.0, 1e-12, 1), vec![0.0, 1e-12]);
    }

    #[test]
    fn steps_and_domains() {
        assert_eq!(tick_step(0.0, 10.0, 5), 2.0);
        assert_eq!(tick_step(0.0, 1.0, 10), 0.1);
        assert_eq!(tick_step(10.0, 0.0, 5), 2.0);
        assert_eq!(nice_domain(0.5, 9.5, 10), (0.0, 10.0));
        assert_eq!(nice_domain(-0.87, 0.93, 5), (-1.0, 1.0));
        assert_eq!(nice_domain(9.7, 0.13, 10), (10.0, 0.0), "keeps direction");
        assert_eq!(nice_domain(3.0, 3.0, 10), (3.0, 3.0));
        assert_eq!(nice_domain(12.3, 987.0, 5), (0.0, 1000.0));
    }

    #[test]
    fn time_intervals() {
        let d = |y, m, dd| days_from_civil(y, m, dd);
        assert_eq!(time_tick_interval(d(2024, 1, 1), d(2024, 1, 8), 7), (TimeUnit::Day, 1));
        assert_eq!(time_tick_interval(d(2024, 1, 1), d(2024, 3, 1), 8), (TimeUnit::Week, 1));
        assert_eq!(time_tick_interval(d(2024, 1, 1), d(2025, 1, 1), 12), (TimeUnit::Month, 1));
        assert_eq!(time_tick_interval(d(2020, 1, 1), d(2024, 1, 1), 12), (TimeUnit::Quarter, 1));
        assert_eq!(time_tick_interval(d(1900, 1, 1), d(2020, 1, 1), 10), (TimeUnit::Year, 10));
    }

    #[test]
    fn time_tick_examples() {
        let d = |y, m, dd| days_from_civil(y, m, dd);
        let t = time_ticks(d(2023, 11, 15), d(2024, 5, 20), 6);
        let expect: Vec<(i32, TimeUnit)> = vec![
            (d(2023, 12, 1), TimeUnit::Month),
            (d(2024, 1, 1), TimeUnit::Year),
            (d(2024, 2, 1), TimeUnit::Month),
            (d(2024, 3, 1), TimeUnit::Month),
            (d(2024, 4, 1), TimeUnit::Quarter),
            (d(2024, 5, 1), TimeUnit::Month),
        ];
        assert_eq!(t, expect);
        let years = time_ticks(d(1901, 3, 1), d(2019, 6, 1), 6);
        assert_eq!(years.first().unwrap().0, d(1920, 1, 1));
        assert!(years.iter().all(|&(day, u)| u == TimeUnit::Year && civil_from_days(day).0 % 20 == 0));
        let weeks = time_ticks(d(2024, 1, 3), d(2024, 2, 20), 7);
        assert!(weeks.iter().all(|&(day, _)| weekday(day) == 0));
        assert_eq!(weeks[0].0, d(2024, 1, 8));
        let days2 = time_ticks(d(2024, 1, 1), d(2024, 1, 20), 10);
        assert!(days2.iter().all(|&(day, _)| civil_from_days(day).2 % 2 == 1));
        assert_eq!(time_ticks(d(2024, 1, 1), d(2024, 1, 1), 5), vec![(d(2024, 1, 1), TimeUnit::Year)]);
        assert!(time_ticks(0, 100, 0).is_empty());
        let rev = time_ticks(d(2024, 5, 20), d(2023, 11, 15), 6);
        assert_eq!(rev, expect, "either order");
    }
}
