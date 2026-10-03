//! UTC timestamps as Unix nanoseconds, formatted and parsed as RFC 3339 (`Z` offset only).
//!
//! Dataset records carry one `created_at` per run (a single clock reading), so the format only
//! needs to round-trip what it writes; no time-zone handling is needed.

/// Nanoseconds per second.
const NANOS: i64 = 1_000_000_000;

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's `days_from_civil`).
const fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day count since 1970-01-01 (H. Hinnant's `civil_from_days`).
const fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

/// `YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ`.
pub fn format_rfc3339(unix_nanos: i64) -> String {
    let secs = unix_nanos.div_euclid(NANOS);
    let nanos = unix_nanos.rem_euclid(NANOS);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let tod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{nanos:09}Z",
        tod / 3600,
        tod / 60 % 60,
        tod % 60
    )
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fraction]Z` (up to 9 fraction digits).
///
/// # Errors
///
/// Anything else, including other UTC offsets.
pub fn parse_rfc3339(s: &str) -> Result<i64, String> {
    let err = || format!("invalid timestamp {s:?} (expected YYYY-MM-DDTHH:MM:SS[.fff]Z)");
    let body = s.strip_suffix(['Z', 'z']).ok_or_else(err)?;
    let (date, time) = body.split_once(['T', 't']).ok_or_else(err)?;
    let num = |t: &str, len: usize| -> Result<i64, String> {
        if t.len() == len && t.bytes().all(|b| b.is_ascii_digit()) {
            t.parse().map_err(|_| err())
        } else {
            Err(err())
        }
    };
    let mut date_parts = date.split('-');
    let (Some(y), Some(mo), Some(d), None) = (
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
    ) else {
        return Err(err());
    };
    let (y, mo, d) = (num(y, 4)?, num(mo, 2)?, num(d, 2)?);
    let (hms, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut time_parts = hms.split(':');
    let (Some(h), Some(mi), Some(se), None) = (
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
    ) else {
        return Err(err());
    };
    let (h, mi, se) = (num(h, 2)?, num(mi, 2)?, num(se, 2)?);
    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max_day = if mo == 2 && leap {
        29
    } else {
        usize::try_from(mo - 1)
            .ok()
            .and_then(|i| month_days.get(i).copied())
            .unwrap_or(0)
    };
    if !(1..=max_day).contains(&d) || h > 23 || mi > 59 || se > 59 {
        return Err(err());
    }
    if fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err(err());
    }
    let frac_nanos = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<9}")
            .parse::<i64>()
            .map_err(|_| err())?
    };
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se;
    secs.checked_mul(NANOS)
        .and_then(|n| n.checked_add(frac_nanos))
        .ok_or_else(err)
}

/// The current time as Unix nanoseconds. The only clock reading of a dataset run (D1: core never
/// reads the clock; the CLI does, once).
pub fn now_unix_nanos() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00.000000000Z");
        // 2026-10-03T12:34:56.789Z, checked with `date -u -d @1791030896`.
        let t = 1_791_030_896_789_000_000;
        assert_eq!(format_rfc3339(t), "2026-10-03T12:34:56.789000000Z");
        assert_eq!(parse_rfc3339("2026-10-03T12:34:56.789Z"), Ok(t));
        assert_eq!(parse_rfc3339("2026-10-03T12:34:56Z"), Ok(t - 789_000_000));
        assert_eq!(
            parse_rfc3339("2000-02-29T00:00:00Z"),
            Ok(951_782_400 * NANOS)
        );
        assert_eq!(format_rfc3339(-1), "1969-12-31T23:59:59.999999999Z");
        for t in [
            0,
            1,
            -1,
            951_782_400 * NANOS + 7,
            4_102_444_800 * NANOS - 1,
            t,
        ] {
            assert_eq!(parse_rfc3339(&format_rfc3339(t)), Ok(t));
        }
        for bad in [
            "2026-10-03 12:34:56Z",
            "2026-10-03T12:34:56+01:00",
            "2026-13-03T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-10-03T24:00:00Z",
            "2026-10-03T12:34:56.1234567890Z",
            "26-10-03T12:34:56Z",
        ] {
            assert!(parse_rfc3339(bad).is_err(), "{bad}");
        }
    }
}
