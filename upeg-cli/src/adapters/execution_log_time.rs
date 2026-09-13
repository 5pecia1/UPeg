//! Epoch-milliseconds → RFC 3339 timestamp rendering for the human
//! (non-JSON) `upeg log` output. Split out of `execution_log.rs` (SoC: the
//! store/dispatch glue in that file has nothing to do with calendar math).
//!
//! ## Local time vs. UTC
//!
//! [`epoch_ms_to_rfc3339_local`] is the pure renderer: it performs no I/O
//! and takes the UTC offset, in seconds, as a plain parameter, so its
//! calendar math is exhaustively unit-testable without touching the
//! system clock or timezone database. [`local_utc_offset_seconds_at`] is
//! the one impure function in this module — it asks `chrono::Local`
//! (which resolves offsets from the system timezone database, honouring
//! `TZ`) which offset from UTC was in effect **at a given instant**.
//! `upeg log`'s live call site chains the two once per row: resolve that
//! row's own offset, then render with it. Resolving per instant rather
//! than once per process is what keeps a log spanning a DST transition
//! honest — rows on either side render in the offset they actually ran
//! in, not in whichever one happens to be current today.

/// Milliseconds in one second.
const MS_PER_SECOND: i64 = 1_000;
/// Seconds in one (non-leap) day.
const SECONDS_PER_DAY: i64 = 86_400;
/// Seconds in one hour, for offset-suffix rendering.
const SECONDS_PER_HOUR: u32 = 3_600;
/// Seconds in one minute, for offset-suffix rendering.
const SECONDS_PER_MINUTE: u32 = 60;

/// Days from the Hinnant `civil_from_days` algorithm's era origin
/// (0000-03-01) to the Unix epoch (1970-01-01). See
/// <http://howardhinnant.github.io/date_algorithms.html>.
const DAYS_HINNANT_ERA_OFFSET: i64 = 719_468;
/// Days in one 400-year Gregorian cycle ("era").
const DAYS_PER_ERA: i64 = 146_097;
/// Years in one 400-year Gregorian cycle ("era").
const YEARS_PER_ERA: i64 = 400;

/// Offset used when `chrono::Local` cannot map an instant to a local
/// time at all (a timestamp outside the representable range). Rendering
/// falls back to UTC — a `Z`-suffixed timestamp — rather than dropping
/// the row from the log.
const FALLBACK_UTC_OFFSET_SECONDS: i32 = 0;

/// The local offset from UTC, in seconds east of UTC, that was in effect
/// **at** `ms` (Unix epoch milliseconds) — what `upeg log`'s human output
/// renders that row's timestamp in. Backed by `chrono::Local`, which
/// resolves offsets from the system timezone database (honouring `TZ`);
/// an instant it cannot map falls back to
/// [`FALLBACK_UTC_OFFSET_SECONDS`]. Mapping a UTC instant onto a local
/// one is never ambiguous, so the `.single()` here only ever loses
/// out-of-range timestamps, not the repeated hour of a DST fall-back.
/// Not a pure function: it reads process/system timezone state, so it
/// belongs at the live call site, not inside a test — see
/// [`epoch_ms_to_rfc3339_local`] for the pure renderer tests exercise.
pub(crate) fn local_utc_offset_seconds_at(ms: i64) -> i32 {
    use chrono::{Local, TimeZone};

    Local
        .timestamp_millis_opt(ms)
        .single()
        .map_or(FALLBACK_UTC_OFFSET_SECONDS, |at| {
            at.offset().local_minus_utc()
        })
}

/// Howard Hinnant's `civil_from_days`: given `z`, the number of days
/// since the Unix epoch (may be negative), return the proleptic-Gregorian
/// `(year, month, day)` it falls on. `month` is `1..=12`, `day` is
/// `1..=31`. Correct across the full `i64` range of `z`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + DAYS_HINNANT_ERA_OFFSET;
    // The original Hinnant algorithm ternary-adjusts `z` before a
    // truncating (round-to-zero) C++ `/` to emulate floor division.
    // Rust's `div_euclid` with a positive divisor already *is* floor
    // division, so no adjustment is needed here — this is correct for
    // the full `i64` range of `z`, positive or negative.
    let era = z.div_euclid(DAYS_PER_ERA);
    let day_of_era = z - era * DAYS_PER_ERA; // [0, 146096]
    let year_of_era = (day_of_era - day_of_era.div_euclid(1460) + day_of_era.div_euclid(36524)
        - day_of_era.div_euclid(146_096))
    .div_euclid(365); // [0, 399]
    let year = year_of_era + era * YEARS_PER_ERA;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era.div_euclid(4) - year_of_era.div_euclid(100)); // [0, 365]
    let month_prime = (5 * day_of_year + 2).div_euclid(153); // [0, 11]
    let day = day_of_year - (153 * month_prime + 2).div_euclid(5) + 1; // [1, 31]
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };
    (
        year,
        u32::try_from(month).unwrap_or(0),
        u32::try_from(day).unwrap_or(0),
    )
}

/// Render `ms` (Unix epoch milliseconds; negative values before 1970 are
/// supported) as an RFC 3339 timestamp shifted by `utc_offset_seconds`
/// (seconds to ADD to UTC to reach the rendered offset). `0` renders a
/// trailing `Z`; any other value renders a `+HH:MM`/`-HH:MM` suffix.
///
/// This is a pure function: it performs no I/O and probes no system
/// clock or timezone database. `upeg log`'s live call site sources
/// `utc_offset_seconds` from [`local_utc_offset_seconds_at`], per record;
/// tests pass a fixed value directly instead.
pub(crate) fn epoch_ms_to_rfc3339_local(ms: i64, utc_offset_seconds: i32) -> String {
    let shifted_ms = ms + i64::from(utc_offset_seconds) * MS_PER_SECOND;
    let total_seconds = shifted_ms.div_euclid(MS_PER_SECOND);
    let days = total_seconds.div_euclid(SECONDS_PER_DAY);
    let seconds_of_day = total_seconds.rem_euclid(SECONDS_PER_DAY);

    let (year, month, day) = civil_from_days(days);
    let seconds_of_day_u32 = u32::try_from(seconds_of_day).unwrap_or(0);
    let hour = seconds_of_day_u32.div_euclid(SECONDS_PER_HOUR);
    let minute = seconds_of_day_u32
        .rem_euclid(SECONDS_PER_HOUR)
        .div_euclid(SECONDS_PER_MINUTE);
    let second = seconds_of_day_u32.rem_euclid(SECONDS_PER_MINUTE);

    let offset_suffix = render_offset_suffix(utc_offset_seconds);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{offset_suffix}")
}

/// `Z` for offset `0`, else a signed `+HH:MM`/`-HH:MM` suffix.
fn render_offset_suffix(utc_offset_seconds: i32) -> String {
    if utc_offset_seconds == 0 {
        return "Z".to_string();
    }
    let sign = if utc_offset_seconds < 0 { '-' } else { '+' };
    let abs_offset = utc_offset_seconds.unsigned_abs();
    let offset_hours = abs_offset.div_euclid(SECONDS_PER_HOUR);
    let offset_minutes = abs_offset
        .rem_euclid(SECONDS_PER_HOUR)
        .div_euclid(SECONDS_PER_MINUTE);
    format!("{sign}{offset_hours:02}:{offset_minutes:02}")
}

#[cfg(test)]
#[path = "execution_log_time_tests.rs"]
mod tests;
