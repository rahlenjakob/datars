//! Tick generation: d3-compatible linear and log ticks, and calendar-aware time ticks (UTC, ISO
//! weeks starting on Monday). Values only — labels are formatted by `datars-text` from the
//! [`LabelHint`] each tick carries.

use crate::date;
use crate::num::{self, Step};
use crate::value::Value;
use datars_math::m;
use serde::{Deserialize, Serialize};

/// A tick: a domain value plus what a formatter needs to label it well.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tick {
    pub value: Value,
    pub label_hint: LabelHint,
}

impl Tick {
    /// The tick on a number line (dates as days); NaN for categories that aren't numbers.
    pub fn num(&self) -> f64 {
        self.value.as_f64().unwrap_or(f64::NAN)
    }
}

/// How a tick wants to be labelled (the text crate turns this into a format).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LabelHint {
    /// A number; `decimals` digits after the point are enough to tell the ticks apart.
    Number { decimals: u8 },
    /// A log-scale tick. `major` ticks are exact powers of the base (label these; minor ticks
    /// usually go unlabelled).
    Log { decimals: u8, major: bool },
    /// A calendar tick spaced every `step` `interval`s. `boundary` is the largest calendar unit
    /// the tick starts (January 1st → `Year`), so a formatter can write "2024" there and "Feb"
    /// elsewhere.
    Time { interval: TimeInterval, step: u32, boundary: TimeInterval },
    /// A discrete domain value, labelled as itself.
    Category,
}

/// Calendar units, smallest to largest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInterval {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

/// Nice linear ticks over a domain (either order), labelled with the step's precision.
pub fn linear_ticks(d0: f64, d1: f64, count: usize) -> Vec<Tick> {
    let decimals = num::tick_step(d0, d1, count.max(1) as f64).map_or(0, Step::decimals);
    num::ticks(d0, d1, count).into_iter().map(|v| Tick { value: Value::Num(v), label_hint: LabelHint::Number { decimals } }).collect()
}

/// Digits after the point in the shortest representation of `v` (capped at 17).
pub fn decimals_of(v: f64) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    let s = num::fmt_num(v.abs());
    s.split_once('.').map_or(0, |(_, f)| f.len().min(17) as u8)
}

/// d3's log ticks: with an integer base and few decades, every k·baseⁱ (k = 1 … base−1) inside the
/// domain; otherwise powers of the base, thinned to about `count`. Negative domains mirror.
pub fn log_ticks(base: f64, d0: f64, d1: f64, count: usize) -> Vec<Tick> {
    let reverse = d1 < d0;
    let (mut u, mut v) = if reverse { (d1, d0) } else { (d0, d1) };
    let negative = v < 0.0;
    if negative {
        (u, v) = (-v, -u);
    }
    if u.is_nan() || u <= 0.0 || !v.is_finite() || base.is_nan() || base <= 1.0 || count == 0 {
        return Vec::new();
    }
    let logs = |x: f64| super::continuous::log(x, base);
    let pows = |i: f64| m::pow(base, i);
    let (i, j) = (logs(u), logs(v));
    let n = count as f64;
    let mut z: Vec<Tick> = Vec::new();
    if base.fract() == 0.0 && j - i < n {
        let (i0, j0) = (i.floor() as i32, j.ceil() as i32);
        'outer: for e in i0..=j0 {
            for k in 1..(base as u32) {
                let t = if e < 0 { k as f64 / pows(-e as f64) } else { k as f64 * pows(e as f64) };
                if t < u {
                    continue;
                }
                if t > v {
                    break 'outer;
                }
                z.push(Tick { value: Value::Num(t), label_hint: LabelHint::Log { decimals: decimals_of(t), major: k == 1 } });
            }
        }
        // Fewer than three can't be read as a scale (a price between 9 and 16 would get 9 and
        // 10): linear ticks then, as d3 does when there are too few for the count.
        if z.len() * 2 < count || z.len() < 3 {
            z = linear_ticks(u, v, count.max(3));
        }
    } else {
        let k = (j - i).min(n).max(1.0) as usize;
        z = num::ticks(i, j, k)
            .into_iter()
            .map(|e| {
                let t = pows(e);
                Tick { value: Value::Num(t), label_hint: LabelHint::Log { decimals: decimals_of(t), major: e.fract() == 0.0 } }
            })
            .collect();
    }
    if negative {
        for t in &mut z {
            t.value = Value::Num(-t.num());
        }
        z.reverse();
    }
    if reverse {
        z.reverse();
    }
    z
}

pub(crate) const MS_SECOND: f64 = 1000.0;
pub(crate) const MS_MINUTE: f64 = 60_000.0;
pub(crate) const MS_HOUR: f64 = 3_600_000.0;
pub(crate) const MS_DAY: f64 = 86_400_000.0;

/// d3's tick intervals: (unit, step, approximate duration in ms).
const INTERVALS: [(TimeInterval, u32, f64); 18] = [
    (TimeInterval::Second, 1, MS_SECOND),
    (TimeInterval::Second, 5, 5.0 * MS_SECOND),
    (TimeInterval::Second, 15, 15.0 * MS_SECOND),
    (TimeInterval::Second, 30, 30.0 * MS_SECOND),
    (TimeInterval::Minute, 1, MS_MINUTE),
    (TimeInterval::Minute, 5, 5.0 * MS_MINUTE),
    (TimeInterval::Minute, 15, 15.0 * MS_MINUTE),
    (TimeInterval::Minute, 30, 30.0 * MS_MINUTE),
    (TimeInterval::Hour, 1, MS_HOUR),
    (TimeInterval::Hour, 3, 3.0 * MS_HOUR),
    (TimeInterval::Hour, 6, 6.0 * MS_HOUR),
    (TimeInterval::Hour, 12, 12.0 * MS_HOUR),
    (TimeInterval::Day, 1, MS_DAY),
    (TimeInterval::Day, 2, 2.0 * MS_DAY),
    (TimeInterval::Week, 1, 7.0 * MS_DAY),
    (TimeInterval::Month, 1, 30.0 * MS_DAY),
    (TimeInterval::Quarter, 1, 90.0 * MS_DAY),
    (TimeInterval::Year, 1, 365.0 * MS_DAY),
];

/// The calendar interval (unit, step) for about `count` ticks over [a, b] ms (a ≤ b). With
/// `whole_days`, nothing finer than a day.
pub fn time_interval(a: f64, b: f64, count: usize, whole_days: bool) -> (TimeInterval, u32) {
    let target = (b - a).abs() / count.max(1) as f64;
    let table: &[(TimeInterval, u32, f64)] = if whole_days { &INTERVALS[12..] } else { &INTERVALS };
    let i = table.partition_point(|x| x.2 <= target);
    if i == table.len() {
        let years = num::tick_step(a / (365.0 * MS_DAY), b / (365.0 * MS_DAY), count.max(1) as f64).map_or(1.0, Step::size);
        return (TimeInterval::Year, years.round().max(1.0) as u32);
    }
    if i == 0 {
        if whole_days {
            return (TimeInterval::Day, 1);
        }
        let ms = num::tick_step(a, b, count.max(1) as f64).map_or(1.0, Step::size);
        return (TimeInterval::Millisecond, ms.round().max(1.0) as u32);
    }
    let (lo, hi) = (table[i - 1], table[i]);
    let pick = if target / lo.2 < hi.2 / target { lo } else { hi };
    (pick.0, pick.1)
}

fn day_of(ms: f64) -> i64 {
    (ms / MS_DAY).floor() as i64
}

fn ms_of_day(d: i64) -> f64 {
    d as f64 * MS_DAY
}

/// Month index (years × 12 + month − 1) of a day.
fn month_index(d: i64) -> i64 {
    let (y, mo, _) = date::ymd_from_date(d as i32);
    y as i64 * 12 + mo as i64 - 1
}

fn day_of_month_index(mi: i64) -> i64 {
    date::days_from_civil(mi.div_euclid(12), mi.rem_euclid(12) + 1, 1)
}

fn fixed_ms(unit: TimeInterval, step: u32) -> Option<f64> {
    let k = step.max(1) as f64;
    match unit {
        TimeInterval::Millisecond => Some(k),
        TimeInterval::Second => Some(k * MS_SECOND),
        TimeInterval::Minute => Some(k * MS_MINUTE),
        TimeInterval::Hour => Some(k * MS_HOUR),
        _ => None,
    }
}

/// The last interval boundary at or before `ms`.
pub fn floor_time(unit: TimeInterval, step: u32, ms: f64) -> f64 {
    let k = step.max(1) as i64;
    if let Some(len) = fixed_ms(unit, step) {
        return (ms / len).floor() * len;
    }
    let d = day_of(ms);
    match unit {
        TimeInterval::Day => {
            // Days whose day-of-month − 1 is a multiple of the step (d3's `day.every`).
            let mut x = d;
            while (date::ymd_from_date(x as i32).2 as i64 - 1) % k != 0 {
                x -= 1;
            }
            ms_of_day(x)
        }
        TimeInterval::Week => ms_of_day(date::start_of_week(d as i32) as i64),
        TimeInterval::Month | TimeInterval::Quarter => {
            let k = if unit == TimeInterval::Quarter { 3 * k } else { k };
            ms_of_day(day_of_month_index(month_index(d).div_euclid(k) * k))
        }
        TimeInterval::Year => {
            let (y, _, _) = date::ymd_from_date(d as i32);
            ms_of_day(date::days_from_civil((y as i64).div_euclid(k) * k, 1, 1))
        }
        _ => unreachable!("fixed-length units handled above"),
    }
}

/// The next interval boundary strictly after boundary `b`.
fn next_time(unit: TimeInterval, step: u32, b: f64) -> f64 {
    let k = step.max(1) as i64;
    if let Some(len) = fixed_ms(unit, step) {
        return b + len;
    }
    let d = day_of(b);
    match unit {
        TimeInterval::Day => {
            let mut x = d + 1;
            while (date::ymd_from_date(x as i32).2 as i64 - 1) % k != 0 {
                x += 1;
            }
            ms_of_day(x)
        }
        TimeInterval::Week => ms_of_day(d + 7 * k),
        TimeInterval::Month => ms_of_day(day_of_month_index(month_index(d) + k)),
        TimeInterval::Quarter => ms_of_day(day_of_month_index(month_index(d) + 3 * k)),
        TimeInterval::Year => {
            let (y, _, _) = date::ymd_from_date(d as i32);
            ms_of_day(date::days_from_civil(y as i64 + k, 1, 1))
        }
        _ => unreachable!(),
    }
}

/// The first interval boundary at or after `ms`.
pub fn ceil_time(unit: TimeInterval, step: u32, ms: f64) -> f64 {
    let f = floor_time(unit, step, ms);
    if f >= ms {
        f
    } else {
        next_time(unit, step, f)
    }
}

/// The largest calendar unit a timestamp starts.
pub fn boundary_of(ms: f64) -> TimeInterval {
    let r = ms - (ms / MS_DAY).floor() * MS_DAY;
    if r != 0.0 {
        return if r % MS_HOUR == 0.0 {
            TimeInterval::Hour
        } else if r % MS_MINUTE == 0.0 {
            TimeInterval::Minute
        } else if r % MS_SECOND == 0.0 {
            TimeInterval::Second
        } else {
            TimeInterval::Millisecond
        };
    }
    let (_, mo, d) = date::ymd_from_date(day_of(ms) as i32);
    match (mo, d) {
        (1, 1) => TimeInterval::Year,
        (4 | 7 | 10, 1) => TimeInterval::Quarter,
        (_, 1) => TimeInterval::Month,
        _ => TimeInterval::Day,
    }
}

/// Calendar ticks over [a, b] ms (either order) for about `count` ticks, as (ms, hint).
pub fn time_ticks_ms(a: f64, b: f64, count: usize, whole_days: bool) -> Vec<(f64, LabelHint)> {
    if !a.is_finite() || !b.is_finite() || count == 0 {
        return Vec::new();
    }
    let reverse = b < a;
    let (lo, hi) = if reverse { (b, a) } else { (a, b) };
    let (unit, step) = time_interval(lo, hi, count, whole_days);
    let mut out = Vec::new();
    let mut t = ceil_time(unit, step, lo);
    while t <= hi && out.len() < 10_000 {
        out.push((t, LabelHint::Time { interval: unit, step, boundary: boundary_of(t) }));
        t = next_time(unit, step, t);
    }
    if reverse {
        out.reverse();
    }
    out
}

/// Intervals for dates that sit one per slot, finest first (see [`slot_time_ticks`]).
const SLOT_INTERVALS: [(TimeInterval, u32); 12] = [
    (TimeInterval::Day, 1),
    (TimeInterval::Week, 1),
    (TimeInterval::Month, 1),
    (TimeInterval::Quarter, 1),
    (TimeInterval::Month, 6),
    (TimeInterval::Year, 1),
    (TimeInterval::Year, 2),
    (TimeInterval::Year, 5),
    (TimeInterval::Year, 10),
    (TimeInterval::Year, 20),
    (TimeInterval::Year, 50),
    (TimeInterval::Year, 100),
];

/// Ticks for strictly increasing dates (days) drawn one per slot — trading days on a band scale,
/// where weekends and holidays take no room, so calendar time runs unevenly along the axis. A
/// tick marks the first date of each calendar interval present (the first trading day of a month:
/// a Monday holiday doesn't lose the month its label); the first date only when it starts one
/// itself. The interval is the finest whose ticks are nowhere closer than about `len / count`
/// slots (0.6 of it; 0.8 for the wider day and week labels), so labels never crowd where months
/// are short. Returns (index into `days`,
/// hint); a tick's `boundary` is the largest unit that changed since the previous date (the first
/// trading day of January starts a year even when it's the 2nd).
pub fn slot_time_ticks(days: &[i32], count: usize) -> Vec<(usize, LabelHint)> {
    let n = days.len();
    if n == 0 || count == 0 {
        return Vec::new();
    }
    // Day and week labels ("27 Jan") are wider than month and year ones ("Feb", "2026").
    let need = |unit: TimeInterval| (n as f64 / count as f64 * if unit <= TimeInterval::Week { 0.8 } else { 0.6 }).max(1.0);
    let floor = |unit: TimeInterval, step: u32, d: i32| floor_time(unit, step, d as f64 * MS_DAY);
    let at = |unit: TimeInterval, step: u32| -> Vec<usize> {
        (0..n).filter(|&i| if i == 0 { floor(unit, step, days[0]) == days[0] as f64 * MS_DAY } else { floor(unit, step, days[i]) != floor(unit, step, days[i - 1]) }).collect()
    };
    let closest = |t: &[usize]| t.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(usize::MAX);
    let last = SLOT_INTERVALS[SLOT_INTERVALS.len() - 1];
    let (pick, idx) = SLOT_INTERVALS.iter().map(|&(u, s)| ((u, s), at(u, s))).find(|((u, _), t)| closest(t) as f64 >= need(*u)).unwrap_or_else(|| (last, at(last.0, last.1)));
    let changed = |i: usize| -> TimeInterval {
        if i == 0 {
            return boundary_of(days[0] as f64 * MS_DAY);
        }
        let (y0, m0, _) = date::ymd_from_date(days[i - 1]);
        let (y1, m1, _) = date::ymd_from_date(days[i]);
        if y0 != y1 {
            TimeInterval::Year
        } else if (m0 - 1) / 3 != (m1 - 1) / 3 {
            TimeInterval::Quarter
        } else if m0 != m1 {
            TimeInterval::Month
        } else {
            TimeInterval::Day
        }
    };
    idx.into_iter().map(|i| (i, LabelHint::Time { interval: pick.0, step: pick.1, boundary: changed(i) })).collect()
}

/// Extend [a, b] ms to the boundaries of the interval chosen for `count` ticks.
pub fn nice_time_ms(a: f64, b: f64, count: usize, whole_days: bool) -> (f64, f64) {
    let reverse = b < a;
    let (lo, hi) = if reverse { (b, a) } else { (a, b) };
    let (unit, step) = time_interval(lo, hi, count, whole_days);
    let (lo, hi) = (floor_time(unit, step, lo), ceil_time(unit, step, hi));
    if reverse {
        (hi, lo)
    } else {
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, mo: u32, d: u32) -> f64 {
        date::date_from_ymd(y, mo, d).unwrap() as f64 * MS_DAY
    }

    #[test]
    fn interval_choice() {
        assert_eq!(time_interval(day(2020, 1, 1), day(2023, 1, 1), 10, true), (TimeInterval::Quarter, 1));
        assert_eq!(time_interval(day(2020, 1, 1), day(2023, 1, 1), 4, true), (TimeInterval::Year, 1));
        assert_eq!(time_interval(day(1900, 1, 1), day(2020, 1, 1), 10, true), (TimeInterval::Year, 10));
        assert_eq!(time_interval(day(2024, 1, 1), day(2024, 3, 1), 8, true), (TimeInterval::Week, 1));
        assert_eq!(time_interval(day(2024, 1, 1), day(2024, 1, 8), 10, true), (TimeInterval::Day, 1));
        assert_eq!(time_interval(0.0, 6.0 * MS_HOUR, 6, false), (TimeInterval::Hour, 1));
        assert_eq!(time_interval(0.0, 1.0, 10, true), (TimeInterval::Day, 1));
    }

    #[test]
    fn floors_and_ceils() {
        let x = day(2024, 5, 17) + 3.0 * MS_HOUR;
        assert_eq!(floor_time(TimeInterval::Year, 1, x), day(2024, 1, 1));
        assert_eq!(ceil_time(TimeInterval::Year, 1, x), day(2025, 1, 1));
        assert_eq!(floor_time(TimeInterval::Year, 10, x), day(2020, 1, 1));
        assert_eq!(floor_time(TimeInterval::Quarter, 1, x), day(2024, 4, 1));
        assert_eq!(ceil_time(TimeInterval::Month, 1, x), day(2024, 6, 1));
        assert_eq!(floor_time(TimeInterval::Week, 1, x), day(2024, 5, 13)); // Monday
        assert_eq!(floor_time(TimeInterval::Hour, 6, x), day(2024, 5, 17));
        assert_eq!(ceil_time(TimeInterval::Month, 1, day(2024, 6, 1)), day(2024, 6, 1));
        assert_eq!(floor_time(TimeInterval::Year, 1, day(1969, 6, 1)), day(1969, 1, 1));
    }

    #[test]
    fn boundaries() {
        assert_eq!(boundary_of(day(2024, 1, 1)), TimeInterval::Year);
        assert_eq!(boundary_of(day(2024, 4, 1)), TimeInterval::Quarter);
        assert_eq!(boundary_of(day(2024, 2, 1)), TimeInterval::Month);
        assert_eq!(boundary_of(day(2024, 2, 3)), TimeInterval::Day);
        assert_eq!(boundary_of(day(2024, 2, 3) + MS_HOUR), TimeInterval::Hour);
    }

    /// Weekdays from `from` for `n` sessions, skipping `holidays`.
    fn sessions(from: (i32, u32, u32), n: usize, holidays: &[(i32, u32, u32)]) -> Vec<i32> {
        let skip: Vec<i32> = holidays.iter().map(|&(y, m, d)| date::date_from_ymd(y, m, d).unwrap()).collect();
        let mut d = date::date_from_ymd(from.0, from.1, from.2).unwrap();
        let mut out = Vec::new();
        while out.len() < n {
            // 1970-01-01 was a Thursday: day 0 → weekday 3 (Monday = 0).
            let weekday = (d + 3).rem_euclid(7);
            if weekday < 5 && !skip.contains(&d) {
                out.push(d);
            }
            d += 1;
        }
        out
    }

    fn ymd(d: i32) -> (i32, u32, u32) {
        date::ymd_from_date(d)
    }

    #[test]
    fn trading_days_tick_at_the_first_session_of_each_month() {
        // A year of sessions from Tuesday 2 January 2024 (New Year's Day is a holiday), with 1 July
        // missing too: ten ticks' room over 252 slots is a label a month.
        let days = sessions((2024, 1, 2), 262, &[(2024, 7, 1), (2024, 12, 25), (2025, 1, 1)]);
        let ticks = slot_time_ticks(&days, 10);
        let dates: Vec<(i32, u32, u32)> = ticks.iter().map(|(i, _)| ymd(days[*i])).collect();
        assert_eq!(dates[0], (2024, 2, 1), "the 2nd isn't a month's start: no tick at the first slot");
        assert_eq!(dates[5], (2024, 7, 2), "July keeps its label on its first session");
        assert_eq!(*dates.last().unwrap(), (2025, 1, 2));
        assert!(ticks.iter().all(|(_, h)| matches!(h, LabelHint::Time { interval: TimeInterval::Month, step: 1, .. })));
        let boundary = |k: usize| match ticks[k].1 {
            LabelHint::Time { boundary, .. } => boundary,
            _ => unreachable!(),
        };
        assert_eq!(boundary(0), TimeInterval::Month);
        assert_eq!(boundary(2), TimeInterval::Quarter, "1 April");
        assert_eq!(boundary(5), TimeInterval::Quarter, "2 July starts the quarter");
        assert_eq!(boundary(ticks.len() - 1), TimeInterval::Year, "2 January 2025 starts the year");
    }

    #[test]
    fn slot_ticks_coarsen_with_more_slots_per_label() {
        let interval = |n: usize| match slot_time_ticks(&sessions((2024, 1, 1), n, &[]), 10).first().map(|t| t.1) {
            Some(LabelHint::Time { interval, step, .. }) => (interval, step),
            other => panic!("{other:?}"),
        };
        assert_eq!(interval(8), (TimeInterval::Day, 1));
        assert_eq!(interval(21), (TimeInterval::Week, 1));
        assert_eq!(interval(126), (TimeInterval::Month, 1));
        assert_eq!(interval(520), (TimeInterval::Quarter, 1));
        assert_eq!(interval(1300), (TimeInterval::Month, 6));
        assert_eq!(interval(2600), (TimeInterval::Year, 1));
        // A Monday that starts the week gets the first slot's tick.
        let week = slot_time_ticks(&sessions((2024, 1, 1), 21, &[]), 10);
        assert_eq!(week[0].0, 0);
        assert!(slot_time_ticks(&[], 10).is_empty());
    }

    #[test]
    fn log_tick_values() {
        let v: Vec<f64> = log_ticks(10.0, 1.0, 1000.0, 10).iter().map(Tick::num).collect();
        assert_eq!(v, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0]);
        let wide: Vec<f64> = log_ticks(10.0, 1.0, 1e12, 5).iter().map(Tick::num).collect();
        assert_eq!(wide, vec![1.0, 1e2, 1e4, 1e6, 1e8, 1e10, 1e12]);
        let neg: Vec<f64> = log_ticks(10.0, -100.0, -1.0, 10).iter().map(Tick::num).collect();
        assert_eq!(neg.first(), Some(&-100.0));
        assert_eq!(neg.last(), Some(&-1.0));
    }
}
