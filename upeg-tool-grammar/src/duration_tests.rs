use std::time::Duration;

use crate::{format_duration, parse_duration};

#[test]
fn duration_aliases_keep_the_previous_fixed_unit_values() {
    let units: &[(&[&str], Duration)] = &[
        (&["ns", "nsec", "nanos"], Duration::from_nanos(1)),
        (&["us", "usec", "µs"], Duration::from_micros(1)),
        (&["ms", "msec", "millis"], Duration::from_millis(1)),
        (
            &["s", "sec", "secs", "second", "seconds"],
            Duration::from_secs(1),
        ),
        (
            &["m", "min", "mins", "minute", "minutes"],
            Duration::from_secs(60),
        ),
        (
            &["h", "hr", "hrs", "hour", "hours"],
            Duration::from_secs(3_600),
        ),
        (&["d", "day", "days"], Duration::from_secs(86_400)),
        (
            &["w", "wk", "wks", "week", "weeks"],
            Duration::from_secs(604_800),
        ),
        (&["M", "month", "months"], Duration::from_secs(2_630_016)),
        (
            &["y", "yr", "yrs", "year", "years"],
            Duration::from_secs(31_557_600),
        ),
    ];
    for (aliases, expected) in units {
        for alias in *aliases {
            assert_eq!(
                parse_duration(&format!("1{alias}")),
                Ok(*expected),
                "{alias}"
            );
        }
    }
}

#[test]
fn duration_compounds_and_whitespace_match_recorded_pre_migration_outputs() {
    for (text, expected) in [
        ("0", Duration::ZERO),
        ("0s", Duration::ZERO),
        ("1m30s", Duration::from_secs(90)),
        ("2s500ms", Duration::from_millis(2_500)),
        ("1h 30m", Duration::from_secs(5_400)),
        ("1 h", Duration::from_secs(3_600)),
        ("1 2s", Duration::from_secs(12)),
        ("1\t2s", Duration::from_secs(12)),
        ("1\u{2003}h\u{00a0}30m", Duration::from_secs(5_400)),
        ("1s1s", Duration::from_secs(2)),
        ("1.5s", Duration::from_millis(1_500)),
        ("0.000000001s", Duration::from_nanos(1)),
        ("0.001us", Duration::from_nanos(1)),
        ("1.25ms", Duration::from_micros(1_250)),
        ("0.0001m", Duration::from_millis(6)),
        ("0.1h", Duration::from_secs(360)),
        ("0.5month", Duration::from_secs(1_315_008)),
        ("0.5year", Duration::from_secs(15_778_800)),
        ("18446744073709551615s", Duration::from_secs(u64::MAX)),
        ("18446744073709551615ns", Duration::from_nanos(u64::MAX)),
        ("18446744073709551615s999999999ns", Duration::MAX),
    ] {
        assert_eq!(parse_duration(text), Ok(expected), "{text:?}");
    }
}

#[test]
fn duration_rejects_unsupported_syntax_precision_and_overflow_without_saturation() {
    for text in [
        "",
        "   ",
        "s",
        "soon",
        "1",
        "1s2",
        "+1s",
        "-1s",
        ".5s",
        "1.s",
        "1e3s",
        "inf",
        "infinity",
        "1μs",
        "1S",
        "1fortnight",
        "0.5ns",
        "0.0ns",
        "1.0000000001s",
        "0.0001h",
        "0.0001d",
        "0.00000000000000000001s",
        "18446744073709551616s",
        "18446744073709551615us",
        "18446744073709551615s1s",
        "18446744073709551615s500ms500ms",
    ] {
        assert!(parse_duration(text).is_err(), "{text:?}");
    }
}

#[test]
fn duration_errors_retain_actionable_number_unit_and_byte_offset_details() {
    for (text, expected) in [
        ("", "value was empty"),
        ("abc", "expected number at 0"),
        ("1!s", "invalid character at 1"),
        ("1.s", "invalid character at 1"),
        ("1. s", "invalid character at 3"),
        ("1. !s", "invalid character at 3"),
        ("1s2", "time unit needed, for example 2sec or 2ms"),
    ] {
        assert_eq!(parse_duration(text).unwrap_err().to_string(), expected);
    }
    assert!(
        parse_duration("1fortnight")
            .unwrap_err()
            .to_string()
            .contains("fortnight")
    );
}

#[test]
fn duration_formatting_preserves_compound_tokens_and_round_trips() {
    for (duration, expected) in [
        (Duration::ZERO, "0s"),
        (Duration::from_secs(1), "1s"),
        (Duration::from_millis(1_500), "1s 500ms"),
        (Duration::from_nanos(1_001_001), "1ms 1us 1ns"),
        (Duration::from_secs(604_800), "7days"),
        (Duration::from_secs(2_630_016), "1month"),
        (Duration::from_secs(31_557_600), "1year"),
        (
            Duration::from_secs(68_649_155),
            "2years 2months 3days 4h 5m 23s",
        ),
    ] {
        assert_eq!(format_duration(duration).to_string(), expected);
        assert_eq!(parse_duration(expected), Ok(duration));
    }
    assert_eq!(
        parse_duration(&format_duration(Duration::MAX).to_string()),
        Ok(Duration::MAX)
    );
}
