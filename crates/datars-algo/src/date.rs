//! Proleptic Gregorian calendar arithmetic on days since 1970-01-01 (Howard Hinnant's algorithms).
//! Exact integer math, valid for every `i32` day.

/// Days since 1970-01-01 of the civil date `year-month-day` (month 1–12, day 1–31; out-of-range
/// months and days roll over arithmetically, e.g. month 13 is January of the next year).
pub fn days_from_civil(year: i32, month: u32, day: u32) -> i32 {
    let (y, mo) = {
        let m0 = month.max(1) as i64 - 1;
        (year as i64 + m0.div_euclid(12), m0.rem_euclid(12) + 1)
    };
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + day.max(1) as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// The civil date `(year, month 1–12, day 1–31)` of a day number.
pub fn civil_from_days(days: i32) -> (i32, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y as i32, m, d)
}

/// ISO weekday: 0 = Monday … 6 = Sunday. (1970-01-01 was a Thursday.)
pub fn weekday(days: i32) -> u32 {
    (days as i64 + 3).rem_euclid(7) as u32
}

/// Whether `year` is a Gregorian leap year.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Zero-based day of the year (0 = 1 January).
pub fn day_of_year(days: i32) -> u32 {
    let (y, _, _) = civil_from_days(days);
    (days as i64 - days_from_civil(y, 1, 1) as i64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(weekday(0), 3, "Thursday");
        assert_eq!(weekday(days_from_civil(2024, 1, 1)), 0, "Monday");
        assert_eq!(day_of_year(days_from_civil(2024, 12, 31)), 365);
        assert!(is_leap_year(2000) && !is_leap_year(1900) && is_leap_year(2024));
        assert_eq!(days_from_civil(2023, 13, 1), days_from_civil(2024, 1, 1));
    }

    #[test]
    fn round_trips() {
        for d in (-800_000..800_000).step_by(997) {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        let (y, m, d) = civil_from_days(i32::MIN);
        assert!((1..=12).contains(&m) && (1..=31).contains(&d) && y < 0);
        let _ = civil_from_days(i32::MAX);
    }
}
