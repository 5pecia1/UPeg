//! `time` toolkit — Unix epoch and ISO 8601 UTC timestamps.

use upeg_core::tool;

/// Howard Hinnant's `civil_from_days` algorithm: convert days-since-
/// epoch (1970-01-01) to a (year, month, day) tuple. Handles all
/// proleptic Gregorian dates without needing chrono / time deps.
///
/// See <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>
pub(crate) const fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y_unadj = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y_unadj + 1 } else { y_unadj };
    (y, m, d)
}

/// Format an unsigned epoch-second value as ISO 8601 UTC. Pulled out
/// so tests can pin specific epoch values without depending on the
/// real wall clock.
pub(crate) fn format_iso_utc(secs: u64) -> String {
    let day = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(day);
    let rem = secs % 86_400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// `time.iso_now` — current UTC time as ISO 8601
/// (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Hand-rolled from `SystemTime` + `civil_from_days` so we don't pull
/// in `chrono` or `time` for one tool. Pre-1970 system clocks return
/// `Err` (matches `epoch_now`'s contract).
#[tool(
    id = "time.iso_now",
    display_label = "ISO timestamp",
    toolkit = "time",
    description = "Current UTC time as ISO 8601 (YYYY-MM-DDTHH:MM:SSZ).",
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn iso_now() -> Result<String, &'static str> {
    let secs = epoch_now()?;
    Ok(format_iso_utc(secs))
}

/// `time.epoch_now` — current UTC time as Unix epoch seconds.
///
/// One-shot counterpart to the live `time.epoch` ticker — both speak the
/// same "epoch" vocabulary (W5 consistency pass unified the former
/// `time.unix_now` id/label onto `epoch`). Uses `SystemTime::now()`
/// against `UNIX_EPOCH`. Pre-1970 system clocks — possible only with
/// deliberate misconfiguration — return `Err`.
#[tool(
    id = "time.epoch_now",
    display_label = "Epoch timestamp",
    toolkit = "time",
    description = "Current UTC time as Unix epoch seconds.",
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn epoch_now() -> Result<u64, &'static str> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "system clock is before 1970-01-01 UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── epoch_now ─────────────────────────────────────

    #[test]
    fn epoch_now_is_within_a_plausible_time_window() {
        let t = epoch_now().expect("system clock should be post-1970");
        assert!(t > 1_577_836_800, "expected post-2020 timestamp, got {t}");
    }

    #[test]
    fn epoch_now_two_calls_are_monotonic_or_equal() {
        let a = epoch_now().unwrap();
        let b = epoch_now().unwrap();
        assert!(b >= a, "second call must not be earlier ({a} → {b})");
    }

    // ─── iso_now / format_iso_utc ─────────────

    #[test]
    fn format_iso_utc_maps_known_epoch_values_to_fixed_strings() {
        assert_eq!(format_iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso_utc(1_577_836_800), "2020-01-01T00:00:00Z");
        assert_eq!(format_iso_utc(1_577_836_799), "2019-12-31T23:59:59Z");
        assert_eq!(format_iso_utc(1_582_934_400), "2020-02-29T00:00:00Z");
        assert_eq!(format_iso_utc(946_684_801), "2000-01-01T00:00:01Z");
    }

    #[test]
    fn iso_now_matches_the_expected_format_shape() {
        let s = iso_now().expect("iso_now");
        assert_eq!(s.len(), 20, "got: {s}");
        assert!(s.ends_with('Z'));
        let (date, time) = s.split_once('T').unwrap();
        let date_parts: Vec<&str> = date.split('-').collect();
        assert_eq!(date_parts.len(), 3);
        assert_eq!(date_parts[0].len(), 4);
        let year: u32 = date_parts[0].parse().unwrap();
        assert!(
            year >= 2025,
            "post-2025 timestamp expected, got year={year}"
        );
        let time_no_z = time.trim_end_matches('Z');
        let time_parts: Vec<&str> = time_no_z.split(':').collect();
        assert_eq!(time_parts.len(), 3);
    }
}
