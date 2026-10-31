use super::*;

#[test]
fn epoch_conversion_handles_negative_values_and_offsets() {
    assert_eq!(from_epoch("-1", "").unwrap(), "1969-12-31T23:59:59Z");
    assert_eq!(
        from_epoch("-1", "milliseconds").unwrap(),
        "1969-12-31T23:59:59.999Z"
    );
    assert_eq!(to_epoch("1970-01-01T01:00:00+01:00", "").unwrap(), 0);
    assert_eq!(
        to_epoch("1969-12-31T23:59:59.9999Z", "milliseconds").unwrap(),
        -1
    );
    assert_eq!(to_epoch("1969-12-31T23:59:59.9999Z", "").unwrap(), -1);
    for invalid in [
        "2020-01-01T00:00:00",
        "2016-12-31T23:59:60Z",
        "2020-02-30T00:00:00Z",
    ] {
        assert!(to_epoch(invalid, "").is_err());
    }
    assert!(from_epoch("99999999999999999999", "").is_err());
}

#[test]
fn cron_uses_unix_days_and_strictly_later_times() {
    let result = cron_inspect("0 0 1 * MON", "2024-01-01T00:00:00Z").unwrap();
    assert_eq!(result.next[0], "2024-01-08T00:00:00Z");
    assert_eq!(result.next.len(), 5);
    assert_eq!(
        cron_inspect("0 0 * * 7", "2024-01-01T00:00:00Z")
            .unwrap()
            .next[0],
        "2024-01-07T00:00:00Z"
    );
    for invalid in ["@daily", "0 0 * * * *", "0 0 L * *", "0 0 31 2 *"] {
        assert!(cron_inspect(invalid, "2024-01-01T00:00:00Z").is_err());
    }
}

#[test]
fn cron_supports_names_steps_and_fractional_reference() {
    let result = cron_inspect("  */15  9-10 * JAN MON-FRI  ", "2024-01-01T09:00:00.001Z").unwrap();
    assert_eq!(result.next[0], "2024-01-01T09:15:00Z");
    assert_eq!(result.timezone, "UTC");
    assert!(!result.description.is_empty());
}

#[test]
fn timestamps_reject_invalid_units_and_out_of_range_years() {
    assert!(from_epoch("253402300800", "seconds").is_err());
    assert!(from_epoch("0", "minutes").is_err());
    assert!(to_epoch("9999-12-31T23:59:59-01:00", "seconds").is_err());
    assert_eq!(from_epoch("0", "seconds").unwrap(), "1970-01-01T00:00:00Z");
}

#[test]
fn cron_rejects_names_concatenated_with_digits() {
    for expression in [
        "0 0 * 1JAN *",
        "0 0 * JAN1 *",
        "0 0 * * 1MON",
        "0 0 * * MON1",
    ] {
        assert!(
            cron_inspect(expression, "2024-01-01T00:00:00Z").is_err(),
            "{expression}"
        );
    }
}

#[test]
fn cron_intersects_days_when_either_field_starts_with_star() {
    let result = cron_inspect("0 0 */2 * MON", "2024-01-01T00:00:00Z").unwrap();
    assert_eq!(
        result.next,
        [
            "2024-01-15T00:00:00Z",
            "2024-01-29T00:00:00Z",
            "2024-02-05T00:00:00Z",
            "2024-02-19T00:00:00Z",
            "2024-03-11T00:00:00Z"
        ]
    );
    assert!(
        result.description.contains("Day of month AND day of week"),
        "{}",
        result.description
    );
}

#[test]
fn cron_reports_unsupported_reference_year() {
    for after in ["0000-01-01T00:00:00Z", "5001-01-01T00:00:00Z"] {
        assert_eq!(
            cron_inspect("* * * * *", after).err(),
            Some("cron reference year must be between 1 and 5000")
        );
    }
}

#[test]
fn cron_requires_numeric_step_after_named_ranges() {
    for expression in ["0 0 * JAN-MAR/FEB *", "0 0 * * MON-FRI/TUE"] {
        assert!(
            cron_inspect(expression, "2024-01-01T00:00:00Z").is_err(),
            "{expression}"
        );
    }
    assert_eq!(
        cron_inspect("0 0 * JAN-MAR/2 *", "2024-01-31T00:00:00Z")
            .unwrap()
            .next[0],
        "2024-03-01T00:00:00Z"
    );
    assert_eq!(
        cron_inspect("0 0 * * MON-FRI/2", "2024-01-01T00:00:00Z")
            .unwrap()
            .next[0],
        "2024-01-03T00:00:00Z"
    );
}

#[test]
fn cron_rejects_empty_list_items_and_limits_step_size() {
    for expression in ["1,,2 * * * *", ",1 * * * *", "1, * * * *"] {
        assert!(
            cron_inspect(expression, "2024-01-01T00:00:00Z").is_err(),
            "{expression}"
        );
    }
    assert_eq!(
        cron_inspect("*/65536 * * * *", "2024-01-01T00:00:00Z").err(),
        Some("cron step must be between 1 and 65535")
    );
    for expression in ["*/60 * * * *", "1-59/100 * * * *", "*/65535 * * * *"] {
        assert!(
            cron_inspect(expression, "2024-01-01T00:00:00Z").is_ok(),
            "{expression}"
        );
    }
}

#[test]
fn cron_description_lists_the_actual_selected_values() {
    let result = cron_inspect("0 0 * JAN-MAR/2 *", "2024-01-31T00:00:00Z").unwrap();
    assert!(
        result.description.contains("month: 1, 3"),
        "{}",
        result.description
    );
    assert!(!result.description.contains("every 2 months"));
    let weekly = cron_inspect("0 0 * * MON", "2024-01-01T00:00:00Z").unwrap();
    assert!(
        weekly.description.contains("day of week: 1"),
        "{}",
        weekly.description
    );
    assert!(weekly.description.contains("AND"), "{}", weekly.description);
}
