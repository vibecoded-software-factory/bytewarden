//! Pure UTC date/time formatting — integer-only, no dependencies.
//!
//! Lives in `domain` (not the TUI) so both the debug log and the
//! adapters can format timestamps without either depending on the other
//! or on a `chrono`-style crate.

/// Converts a unix-epoch second count to `(year, month, day, hour,
/// minute, second)` in UTC, proleptic Gregorian.
///
/// Uses Howard Hinnant's `civil_from_days` inverse — integer arithmetic
/// only, correct for every date Linux/macOS will report.
pub fn unix_to_civil(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let secs_per_day: i64 = 86_400;
    let days = secs.div_euclid(secs_per_day);
    let tod = secs.rem_euclid(secs_per_day);
    let hh = (tod / 3600) as u32;
    let mm = ((tod % 3600) / 60) as u32;
    let ss = (tod % 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32; // 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, hh, mm, ss)
}

/// Formats a unix-epoch second count as an ISO-8601 UTC timestamp
/// (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn unix_to_iso_utc(secs: i64) -> String {
    let (y, m, d, hh, mm, ss) = unix_to_civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_civil_known_values() {
        // 2001-09-09T01:46:40Z is unix 1_000_000_000.
        assert_eq!(unix_to_civil(1_000_000_000), (2001, 9, 9, 1, 46, 40));
        // The epoch itself.
        assert_eq!(unix_to_civil(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn unix_to_civil_handles_leap_day() {
        // 2020-02-29T12:00:00Z is unix 1_582_977_600.
        assert_eq!(unix_to_civil(1_582_977_600), (2020, 2, 29, 12, 0, 0));
    }

    #[test]
    fn iso_format_is_zero_padded_and_z_suffixed() {
        assert_eq!(unix_to_iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_iso_utc(1_000_000_000), "2001-09-09T01:46:40Z");
    }
}
