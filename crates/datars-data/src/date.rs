//! Calendar dates as days since 1970-01-01 (proleptic Gregorian, no time zone): the representation
//! of [`crate::Column::Date`]. Pure integer arithmetic (Howard Hinnant's civil-date algorithms), so
//! every target agrees.

/// Days since 1970-01-01 for a valid calendar date, or `None` (month 1–12, day within the month).
pub fn date_from_ymd(year: i32, month: u32, day: u32) -> Option<i32> {
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year as i64, month as i64, day as i64) as i32)
}

/// Days since the epoch for any (year, month, day), normalising overflowing months/days the way a
/// calendar would (month 13 = January next year). For callers that step through calendars.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    // Normalise the month into 1..=12 first.
    let y = y + (m - 1).div_euclid(12);
    let m = (m - 1).rem_euclid(12) + 1;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month 1–12, day 1–31) of a day number.
pub fn ymd_from_date(days: i32) -> (i32, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }) as i32, m as u32, d as u32)
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// ISO weekday: 0 = Monday … 6 = Sunday. (1970-01-01 was a Thursday.)
pub fn weekday(days: i32) -> u32 {
    (days as i64 + 3).rem_euclid(7) as u32
}

/// Day of the year, 1-based (1 = January 1st).
pub fn day_of_year(days: i32) -> u32 {
    let (y, _, _) = ymd_from_date(days);
    (days as i64 - days_from_civil(y as i64, 1, 1) + 1) as u32
}

/// Quarter 1–4.
pub fn quarter(days: i32) -> u32 {
    let (_, m, _) = ymd_from_date(days);
    (m - 1) / 3 + 1
}

/// ISO 8601 week: (ISO year, week 1–53). Weeks start on Monday; week 1 holds the year's first
/// Thursday, so early January can belong to the previous ISO year and late December to the next.
pub fn iso_week(days: i32) -> (i32, u32) {
    // The Thursday of this date's week decides the ISO year.
    let thursday = days as i64 - weekday(days) as i64 + 3;
    let (y, _, _) = ymd_from_date(thursday as i32);
    let week = (thursday - days_from_civil(y as i64, 1, 1)) / 7 + 1;
    (y, week as u32)
}

/// The Monday starting the ISO week that contains `days`.
pub fn start_of_week(days: i32) -> i32 {
    days - weekday(days) as i32
}

/// `YYYY-MM-DD` (years outside 0–9999 get a sign / more digits).
pub fn format_date(days: i32) -> String {
    let (y, m, d) = ymd_from_date(days);
    if (0..=9999).contains(&y) {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:+05}-{m:02}-{d:02}")
    }
}

/// Parse an ISO calendar date: `YYYY-MM-DD` or `YYYY-MM` (the 1st of the month). Strict: exactly
/// four year digits, two month digits, two day digits, nothing else. Plain years are *not*
/// accepted here (they're usually numbers); see [`parse_date_with`].
pub fn parse_date(s: &str) -> Option<i32> {
    parse_date_with(s, false)
}

/// [`parse_date`], optionally accepting a bare four-digit year `YYYY` (January 1st).
pub fn parse_date_with(s: &str, allow_year: bool) -> Option<i32> {
    let b = s.trim().as_bytes();
    let digits = |r: &[u8]| -> Option<u32> {
        if r.is_empty() || !r.iter().all(u8::is_ascii_digit) {
            return None;
        }
        r.iter().try_fold(0u32, |acc, &c| acc.checked_mul(10)?.checked_add((c - b'0') as u32))
    };
    match b.len() {
        4 if allow_year => date_from_ymd(digits(b)? as i32, 1, 1),
        7 if b[4] == b'-' => date_from_ymd(digits(&b[0..4])? as i32, digits(&b[5..7])?, 1),
        10 if b[4] == b'-' && b[7] == b'-' => date_from_ymd(digits(&b[0..4])? as i32, digits(&b[5..7])?, digits(&b[8..10])?),
        _ => None,
    }
}

/// Lenient parse for columns the author declared as dates: everything [`parse_date_with`] accepts
/// (years included), `/` as a separator, and an ISO time suffix (`T…` or ` hh:mm…`), which is
/// dropped.
pub fn parse_date_lenient(s: &str) -> Option<i32> {
    let s = s.trim();
    let head = match s.find(['T', ' ']) {
        Some(i) if i >= 7 => &s[..i],
        _ => s,
    };
    let head = head.replace('/', "-");
    parse_date_with(&head, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_day_for_centuries() {
        let start = date_from_ymd(1600, 1, 1).unwrap();
        let end = date_from_ymd(2400, 12, 31).unwrap();
        let mut prev = ymd_from_date(start - 1);
        for d in start..=end {
            let (y, m, dd) = ymd_from_date(d);
            assert_eq!(date_from_ymd(y, m, dd), Some(d));
            assert!((y, m, dd) > prev);
            prev = (y, m, dd);
        }
    }

    #[test]
    fn known_dates() {
        assert_eq!(date_from_ymd(1970, 1, 1), Some(0));
        assert_eq!(date_from_ymd(2000, 3, 1), Some(11_017));
        assert_eq!(ymd_from_date(-1), (1969, 12, 31));
        assert_eq!(date_from_ymd(2023, 2, 29), None);
        assert_eq!(date_from_ymd(2024, 2, 29).map(ymd_from_date), Some((2024, 2, 29)));
        assert_eq!(date_from_ymd(2023, 13, 1), None);
        assert_eq!(weekday(0), 3); // Thursday
        assert_eq!(weekday(date_from_ymd(2024, 1, 1).unwrap()), 0); // Monday
        assert_eq!(day_of_year(date_from_ymd(2024, 12, 31).unwrap()), 366);
        assert_eq!(quarter(date_from_ymd(2024, 7, 1).unwrap()), 3);
        assert_eq!(format_date(date_from_ymd(2024, 7, 1).unwrap()), "2024-07-01");
        assert_eq!(days_from_civil(2023, 13, 1), date_from_ymd(2024, 1, 1).unwrap() as i64);
    }

    #[test]
    fn iso_weeks_across_year_boundaries() {
        let w = |y, m, d| iso_week(date_from_ymd(y, m, d).unwrap());
        assert_eq!(w(2021, 1, 1), (2020, 53)); // Friday: belongs to 2020's last week
        assert_eq!(w(2021, 1, 4), (2021, 1));
        assert_eq!(w(2019, 12, 30), (2020, 1)); // Monday of 2020-W01
        assert_eq!(w(2024, 12, 30), (2025, 1));
        assert_eq!(w(2026, 9, 26), (2026, 39));
    }

    #[test]
    fn parsing() {
        assert_eq!(parse_date("2024-02-29"), date_from_ymd(2024, 2, 29));
        assert_eq!(parse_date("2024-02"), date_from_ymd(2024, 2, 1));
        assert_eq!(parse_date("2024"), None);
        assert_eq!(parse_date_with("2024", true), date_from_ymd(2024, 1, 1));
        assert_eq!(parse_date("2024-2-1"), None);
        assert_eq!(parse_date("2023-02-29"), None);
        assert_eq!(parse_date("2024-02-29T10:00"), None);
        assert_eq!(parse_date_lenient("2024-02-29T10:00:00Z"), date_from_ymd(2024, 2, 29));
        assert_eq!(parse_date_lenient("2024/02/29"), date_from_ymd(2024, 2, 29));
        assert_eq!(parse_date_lenient("1999"), date_from_ymd(1999, 1, 1));
    }
}
