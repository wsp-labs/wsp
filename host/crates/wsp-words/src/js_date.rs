// SPDX-License-Identifier: AGPL-3.0-only
//! JavaScript's `Date`: the range a time value holds (plus or minus 8.64e15 ms, which is year 275760), the Invalid Date
//! outside it, the extended year `toISOString` writes past 9999, and the truncation toward zero `new Date(ms)` does.
//! jiff holds years -9999 to 9999 only, so every sentence that dates a moment reads it here. See spec/js.md.

use jiff::tz::TimeZone;
use jiff::Timestamp;

/// The largest time value a `Date` holds, in milliseconds either side of the epoch.
pub const MAX_TIME_MS: f64 = 8.64e15;

/// Four hundred Gregorian years in milliseconds: a span after which the calendar and its weekdays repeat exactly.
pub const CYCLE_MS: i64 = 146_097 * 86_400_000;

/// `new Date(ms).getTime()`: the whole millisecond a `Date` made from `ms` holds, or none where it is Invalid Date (NaN,
/// or past 8.64e15 either way). A fraction is cut toward zero.
pub fn time_value(ms: f64) -> Option<i64> {
    (ms.abs() <= MAX_TIME_MS).then(|| ms.trunc() as i64)
}

/// The fields of a moment on a clock: the calendar year (negative before year 0), month 1 to 12, day, time of day and
/// the weekday, 0 for Sunday.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fields {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
    pub weekday: u32,
}

/// The fields of the time value `t` on a clock `offset_seconds` ahead of UTC.
pub fn fields(t: i64, offset_seconds: i64) -> Fields {
    let local = t + offset_seconds * 1000;
    let days = local.div_euclid(86_400_000);
    let of_day = local.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    Fields {
        year,
        month,
        day,
        hour: (of_day / 3_600_000) as u32,
        minute: (of_day / 60_000 % 60) as u32,
        second: (of_day / 1000 % 60) as u32,
        millisecond: (of_day % 1000) as u32,
        weekday: (days + 4).rem_euclid(7) as u32,
    }
}

/// The days since 1970-01-01 of a civil date, and the civil date of a day (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The year as `toISOString` writes it: four digits from 0 to 9999, else a sign and six.
pub fn iso_year(year: i64) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else {
        format!("{}{:06}", if year < 0 { '-' } else { '+' }, year.unsigned_abs())
    }
}

/// `new Date(ms).toISOString()`, or none where `new Date(ms)` is Invalid Date and the TS throws RangeError "Invalid time
/// value".
pub fn iso_string(ms: f64) -> Option<String> {
    let f = fields(time_value(ms)?, 0);
    Some(format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", iso_year(f.year), f.month, f.day, f.hour, f.minute, f.second, f.millisecond))
}

/// The year as `Intl.DateTimeFormat` writes it with `year: "numeric"`: the year of its era, so year 0 is 1 and year -1 is 2,
/// and no padding.
pub fn intl_year(year: i64) -> i64 {
    if year >= 1 {
        year
    } else {
        1 - year
    }
}

/// A time zone as `Intl` takes one by name: an IANA name in any case, or an offset (`+05:30`, `+0530`, `+05`).
pub enum DateZone {
    Named(TimeZone),
    Fixed(i64),
}

impl DateZone {
    /// The zone `name` names, or none where `Intl` throws RangeError "Invalid time zone specified".
    pub fn named(name: &str) -> Option<DateZone> {
        if let Some(sign) = match name.as_bytes().first() {
            Some(b'+') => Some(1),
            Some(b'-') => Some(-1),
            _ => None,
        } {
            let digits: String = name[1..].chars().filter(|c| *c != ':').collect();
            let shape_ok = match name.len() {
                3 | 5 => name[1..].bytes().all(|b| b.is_ascii_digit()),
                6 => name.as_bytes()[3] == b':' && digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()),
                _ => false,
            };
            if !shape_ok {
                return None;
            }
            let hours: i64 = digits[..2].parse().ok()?;
            let minutes: i64 = if digits.len() == 4 { digits[2..].parse().ok()? } else { 0 };
            return (hours < 24 && minutes < 60).then_some(DateZone::Fixed(sign * (hours * 3600 + minutes * 60)));
        }
        if let Ok(zone) = TimeZone::get(name) {
            return Some(DateZone::Named(zone));
        }
        let found = jiff::tz::db().available().find(|known| known.as_str().eq_ignore_ascii_case(name))?;
        TimeZone::get(found.as_str()).ok().map(DateZone::Named)
    }

    /// The zone this computer is set to.
    pub fn system() -> DateZone {
        DateZone::Named(TimeZone::system())
    }

    /// How far ahead of UTC this zone's clocks are at the time value `t`, in seconds. Past the years jiff holds, the same
    /// instant 400 years on or back, which has the calendar and the weekdays of this one, answers.
    pub fn offset_seconds(&self, t: i64) -> i64 {
        match self {
            DateZone::Fixed(seconds) => *seconds,
            DateZone::Named(zone) => {
                // Before the years jiff holds, a zone keeps the first offset it ever had, which is what the earliest moment answers.
                let inside = if t < -SAFE_MS { -SAFE_MS } else { within_jiff(t).0 };
                let at = Timestamp::from_millisecond(inside).expect("a moment inside jiff's years");
                i64::from(zone.to_offset(at).seconds())
            }
        }
    }
}

/// How far from the epoch a moment is for jiff to hold it with room for any zone's offset: about year 9892 and -5924.
const SAFE_MS: i64 = 250_000_000_000_000;

/// The time value `t` moved by whole 400-year cycles into the years 2000 to 2399, where jiff holds it, and how many
/// cycles it moved; `t` itself where jiff holds it already.
pub fn within_jiff(t: i64) -> (i64, i64) {
    if t.abs() <= SAFE_MS {
        return (t, 0);
    }
    let base = 946_684_800_000;
    let cycles = (t - base).div_euclid(CYCLE_MS);
    (t - cycles * CYCLE_MS, cycles)
}

/// `MakeDate(MakeDay(year, month, day), MakeTime(hour, minute, second, ms))` for a zero-based month: the time value of a
/// clock reading, with a day or a month past its end carrying into the next. None where it is past what an i64 holds.
pub fn fields_to_time(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64, ms: i64) -> Option<i64> {
    let year = year.checked_add(month.div_euclid(12))?;
    let days = days_from_civil(year, month.rem_euclid(12) as u32 + 1, 1)?.checked_add(day - 1)?;
    let time = ((hour * 60 + minute) * 60 + second) * 1000 + ms;
    days.checked_mul(86_400_000)?.checked_add(time)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i64> {
    let y = year.checked_sub(i64::from(month <= 2))?;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * i64::from((month + 9) % 12) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era.checked_mul(146_097)?.checked_add(doe - 719_468)
}

/// The time value of a clock reading in a zone, as `new Date(y, m, d, ...)` reads it: where the clock stood twice (the
/// hour a change back repeats) the earlier moment, and where it never stood (the hour a change forward skips) the moment
/// the clock before the change names.
pub fn local_to_utc(local: i64, zone: &DateZone) -> i64 {
    let before = zone.offset_seconds(local - 86_400_000);
    let after = zone.offset_seconds(local + 86_400_000);
    let (first, second) = (local - before * 1000, local - after * 1000);
    let holds = |utc: i64, offset: i64| zone.offset_seconds(utc) == offset;
    match (holds(first, before), holds(second, after)) {
        (true, true) => first.min(second),
        (true, false) => first,
        (false, true) => second,
        (false, false) => first,
    }
}
