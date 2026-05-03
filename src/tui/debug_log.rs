//! Optional debug log file for troubleshooting.
//!
//! Activated by setting `BYTEWARDEN_DEBUG=1` in the environment. When
//! the variable is unset (the default), every entry point is a cheap
//! `is_err()` check that returns immediately — no overhead, no file
//! handle, no allocation.
//!
//! When active, every `App::push_cmd` line is appended to
//! `~/.bytewarden.log` (or `./.bytewarden.log` if `$HOME` is unset)
//! with a UTC timestamp. The file format is one line per entry:
//!
//! ```text
//! 2026-05-03T14:23:11Z  ✓  bw status                                 → Unlocked
//! 2026-05-03T14:23:12Z  ✓  bw list items --session ***               → 87 items loaded
//! 2026-05-03T14:25:40Z  ✕  bw sync --session ***                     → bw sync timed out after 30s
//! ```
//!
//! Same redaction rules as the in-app command log (session keys are
//! already replaced with `***` before reaching this module).
//!
//! ## Why a file instead of stderr / `RUST_LOG`
//!
//! The TUI owns stderr — anything written there scrambles the screen.
//! `tracing` / `env_logger` would be heavier dependencies for what is
//! essentially a one-line append on demand. A file the user `tail -f`s
//! from another terminal is the smallest tool that works.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Env var the user sets to turn the log file on. Any non-empty value
/// counts as "enabled" — `1`, `true`, `yes`, etc. all work.
const ENV_VAR: &str = "BYTEWARDEN_DEBUG";

/// Returns the absolute path of the debug log file, honoring `$HOME`
/// when set and falling back to the current working directory
/// otherwise. Pure — does not touch disk.
pub fn log_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".bytewarden.log")
}

/// Returns `true` when `BYTEWARDEN_DEBUG` is set to any non-empty
/// value. Cheap enough to call from `push_cmd` without caching.
pub fn is_enabled() -> bool {
    std::env::var(ENV_VAR)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Appends a single redacted command-log entry to the debug file when
/// enabled, otherwise no-op.
///
/// Best-effort: any I/O error is swallowed. The point of the file is
/// observability, not correctness — a failing append should never
/// break the TUI.
pub fn append(cmd: &str, ok: bool, detail: &str) {
    if !is_enabled() {
        return;
    }
    let icon = if ok { "✓" } else { "✕" };
    let line = format!("{}  {icon}  {cmd:<60}  → {detail}\n", iso_utc_now());
    write_line(&line);
}

/// Writes `line` (already terminated with `\n`) to the debug file
/// using `0o600` perms. Errors are dropped — the caller has nowhere
/// useful to surface them.
fn write_line(line: &str) {
    let path = log_path();
    let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
    else {
        return;
    };
    let _ = f.write_all(line.as_bytes());
}

/// Formats "now" as a compact `YYYY-MM-DDTHH:MM:SSZ` UTC stamp using
/// only the standard library — no `chrono`, no `time` dependency.
///
/// The conversion below is the well-known
/// [Howard Hinnant civil date algorithm](https://howardhinnant.github.io/date_algorithms.html)
/// adapted for unix-epoch seconds. It only uses integer arithmetic
/// and is correct for every date Linux/macOS will report.
fn iso_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = unix_to_civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Converts a unix-epoch second count to `(year, month, day, hour,
/// minute, second)`. UTC, proleptic Gregorian.
fn unix_to_civil(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    // Time of day.
    let secs_per_day: i64 = 86_400;
    let days = secs.div_euclid(secs_per_day);
    let tod = secs.rem_euclid(secs_per_day);
    let hh = (tod / 3600) as u32;
    let mm = ((tod % 3600) / 60) as u32;
    let ss = (tod % 60) as u32;

    // Date — Hinnant's "days_from_civil" inverted.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_civil_known_values() {
        // Three independently-verifiable epoch values.
        // 1970-01-01T00:00:00Z = 0 (the unix epoch itself).
        assert_eq!(unix_to_civil(0), (1970, 1, 1, 0, 0, 0));
        // 2000-01-01T00:00:00Z = 946684800.
        assert_eq!(unix_to_civil(946_684_800), (2000, 1, 1, 0, 0, 0));
        // 2038-01-19T03:14:07Z — the i32 overflow wall (Y2K38). We
        // pass it as i64 so the algorithm copes.
        assert_eq!(unix_to_civil(2_147_483_647), (2038, 1, 19, 3, 14, 7));
    }

    #[test]
    fn unix_to_civil_handles_leap_day() {
        // 2024-02-29T00:00:00Z — leap day. 2024 is divisible by 4 and
        // not by 100, so it is a leap year.
        let secs = 1_709_164_800;
        assert_eq!(unix_to_civil(secs), (2024, 2, 29, 0, 0, 0));
    }

    #[test]
    fn iso_format_has_expected_shape() {
        // We can't pin a specific timestamp without a clock fake, but
        // the format must have the exact `YYYY-MM-DDTHH:MM:SSZ` shape.
        let s = iso_utc_now();
        assert_eq!(s.len(), 20);
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(7), Some('-'));
        assert_eq!(s.chars().nth(10), Some('T'));
        assert_eq!(s.chars().nth(13), Some(':'));
        assert_eq!(s.chars().nth(16), Some(':'));
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn is_enabled_respects_env_var() {
        // We cannot mutate the global env safely from a parallel
        // test run, so just verify the helper falls through cleanly
        // for the most common case.
        let was_set = std::env::var(ENV_VAR).is_ok();
        if !was_set {
            assert!(!is_enabled());
        }
    }

    #[test]
    fn log_path_uses_home_when_set() {
        if let Ok(home) = std::env::var("HOME") {
            let p = log_path();
            assert!(p.starts_with(home));
            assert!(p.ends_with(".bytewarden.log"));
        }
    }
}
