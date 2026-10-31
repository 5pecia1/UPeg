//! `time` toolkit — Unix epoch and ISO 8601 UTC timestamps.

use chrono::{DateTime, Datelike, SecondsFormat, Timelike, Utc};
use serde::Serialize;
use upeg_core::tool;

/// Howard Hinnant's `civil_from_days` algorithm: convert days-since-
/// epoch (1970-01-01) to a (year, month, day) tuple. Handles all
/// proleptic Gregorian dates.
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
/// Pre-1970 system clocks return `Err` (matches `epoch_now`'s contract).
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

#[derive(Clone, Copy)]
enum EpochUnit {
    Seconds,
    Milliseconds,
}

fn epoch_unit(unit: &str) -> Result<EpochUnit, &'static str> {
    match if unit.is_empty() { "seconds" } else { unit } {
        "seconds" => Ok(EpochUnit::Seconds),
        "milliseconds" => Ok(EpochUnit::Milliseconds),
        _ => Err("unit must be seconds or milliseconds"),
    }
}

fn parse_datetime(input: &str) -> Result<DateTime<Utc>, &'static str> {
    let value = DateTime::parse_from_rfc3339(input).map_err(|_| "invalid RFC 3339 timestamp")?;
    if value.nanosecond() >= 1_000_000_000 {
        return Err("leap seconds are not supported");
    }
    let utc = value.with_timezone(&Utc);
    if !(0..=9999).contains(&utc.year()) {
        return Err("timestamp is outside RFC 3339 year range");
    }
    Ok(utc)
}

#[tool(
    id = "time.from_epoch", display_label = "Epoch to date", toolkit = "time",
    description = "Convert a signed Unix timestamp to UTC RFC 3339.",
    inputs = [required input: String = "Integer timestamp", optional unit: Options(["seconds", "milliseconds"]) = "Unit (default: seconds)"],
    pin = Inline, pegboard_units = U1, invoker = Function,
)]
pub fn from_epoch(input: &str, unit: &str) -> Result<String, &'static str> {
    let value: i64 = input
        .parse()
        .map_err(|_| "timestamp must be a signed integer")?;
    let (secs, nanos) = match epoch_unit(unit)? {
        EpochUnit::Seconds => (value, 0),
        EpochUnit::Milliseconds => (
            value.div_euclid(1000),
            value.rem_euclid(1000) as u32 * 1_000_000,
        ),
    };
    let date = DateTime::from_timestamp(secs, nanos).ok_or("timestamp out of range")?;
    if !(0..=9999).contains(&date.year()) {
        return Err("timestamp is outside RFC 3339 year range");
    }
    Ok(date.to_rfc3339_opts(SecondsFormat::AutoSi, true))
}

#[tool(
    id = "time.to_epoch", display_label = "Date to epoch", toolkit = "time",
    description = "Convert RFC 3339 to a signed Unix timestamp; fractional units round down.",
    outputs = [result: Number = "Signed Unix timestamp"],
    inputs = [required input: String = "RFC 3339 timestamp with timezone", optional unit: Options(["seconds", "milliseconds"]) = "Unit (default: seconds)"],
    pin = Inline, pegboard_units = U1, invoker = Function,
)]
pub fn to_epoch(input: &str, unit: &str) -> Result<i64, &'static str> {
    let date = parse_datetime(input)?;
    Ok(match epoch_unit(unit)? {
        EpochUnit::Seconds => date.timestamp(),
        EpochUnit::Milliseconds => date.timestamp_millis(),
    })
}

fn cron_field_description(component: &croner::CronComponent, full_count: usize) -> String {
    let values = component.values();
    if values.len() == full_count {
        "any".to_owned()
    } else {
        values
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn cron_description(cron: &croner::Cron) -> String {
    let pattern = &cron.pattern;
    let minute = cron_field_description(&pattern.minutes, 60);
    let hour = cron_field_description(&pattern.hours, 24);
    let day = cron_field_description(&pattern.days, 31);
    let month = cron_field_description(&pattern.months, 12);
    let weekday = cron_field_description(&pattern.days_of_week, 7);
    let day_rule = if pattern.matches_dom_and_dow() {
        "AND"
    } else {
        "OR"
    };
    format!(
        "UTC minute: {minute}; hour: {hour}; day of month: {day}; month: {month}; day of week: {weekday} (Sunday = 0). Day of month {day_rule} day of week must match."
    )
}

#[derive(Serialize)]
pub struct CronInspection {
    description: String,
    timezone: &'static str,
    next: Vec<String>,
}

#[tool(
    id = "time.cron_inspect", display_label = "Inspect cron", toolkit = "time",
    description = "Explain a five-field Unix cron expression and show its next five UTC occurrences (years 1–5000; steps at most 65535).",
    outputs = [result: Json = "Description, UTC timezone, and next five occurrences"],
    inputs = [required expression: String = "Minute hour day-of-month month day-of-week", optional after: String = "RFC 3339 reference time (default: now)"],
    pin = Inline, pegboard_units = U1, invoker = Function,
)]
pub fn cron_inspect(expression: &str, after: &str) -> Result<CronInspection, &'static str> {
    let fields: Vec<_> = expression.split_whitespace().collect();
    if fields.len() != 5
        || fields.iter().any(|field| {
            !field
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "*,-/".contains(c))
        })
    {
        return Err("expected five standard Unix cron fields");
    }
    for field in &fields {
        for component in field.split(',') {
            if component.is_empty() {
                return Err("cron list items cannot be empty");
            }
            if let Some((_, step)) = component.split_once('/') {
                if step.is_empty() || !step.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err("cron step must be numeric");
                }
                if step
                    .parse::<u16>()
                    .ok()
                    .filter(|value| *value > 0)
                    .is_none()
                {
                    return Err("cron step must be between 1 and 65535");
                }
            }
        }
    }
    // Croner supports extensions; restrict alphabetic tokens to standard month and weekday names.
    for (index, field) in fields.iter().enumerate() {
        for word in field
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| s.chars().any(|c| c.is_ascii_alphabetic()))
        {
            let name = word.to_ascii_uppercase();
            let allowed = word.chars().all(|c| c.is_ascii_alphabetic())
                && match index {
                    3 => [
                        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT",
                        "NOV", "DEC",
                    ]
                    .contains(&name.as_str()),
                    4 => ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"].contains(&name.as_str()),
                    _ => false,
                };
            if !allowed {
                return Err("unsupported cron field name");
            }
        }
    }
    let cron = croner::parser::CronParser::builder()
        .dom_and_dow(fields[2].starts_with('*') || fields[4].starts_with('*'))
        .build()
        .parse(&fields.join(" "))
        .map_err(|_| "invalid cron expression")?;
    let mut cursor = if after.is_empty() {
        Utc::now()
    } else {
        parse_datetime(after)?
    };
    if !(1..=5000).contains(&cursor.year()) {
        return Err("cron reference year must be between 1 and 5000");
    }
    let mut next = Vec::with_capacity(5);
    for _ in 0..5 {
        cursor = cron
            .find_next_occurrence(&cursor, false)
            .map_err(|_| "no future cron occurrence")?;
        if !(1..=5000).contains(&cursor.year()) {
            return Err("cron occurrence out of range");
        }
        next.push(cursor.to_rfc3339_opts(SecondsFormat::Secs, true));
    }
    Ok(CronInspection {
        description: cron_description(&cron),
        timezone: "UTC",
        next,
    })
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

#[cfg(test)]
mod conversion_and_cron_tests;
