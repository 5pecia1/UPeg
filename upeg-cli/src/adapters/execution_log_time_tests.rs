//! Tests for [`super::epoch_ms_to_rfc3339_local`] and its
//! `civil_from_days` day-math core. Fixture epoch-ms values were computed
//! independently via Python's `datetime` (`timestamp() * 1000`), not by
//! re-deriving them from this module's own algorithm.

use super::{epoch_ms_to_rfc3339_local, local_utc_offset_seconds_at};

const KST_OFFSET_SECONDS: i32 = 9 * 3_600;
const NEGATIVE_FIVE_OFFSET_SECONDS: i32 = -5 * 3_600;
/// 2026-08-17T09:05:03Z — the reference instant shared by the offset-suffix cases.
const RENDERED_INSTANT_MS: i64 = 1_786_957_503_000;

#[test]
fn epoch_zero_renders_as_1970_01_01_midnight_utc() {
    assert_eq!(epoch_ms_to_rfc3339_local(0, 0), "1970-01-01T00:00:00Z");
}

#[test]
fn negative_ms_before_epoch_renders_as_last_second_of_1969() {
    // 1969-12-31T23:59:59Z
    assert_eq!(epoch_ms_to_rfc3339_local(-1_000, 0), "1969-12-31T23:59:59Z");
}

#[test]
fn year_2000_divisible_by_400_renders_feb_29_correctly() {
    // 2000-02-29T12:30:45Z
    assert_eq!(
        epoch_ms_to_rfc3339_local(951_827_445_000, 0),
        "2000-02-29T12:30:45Z"
    );
}

#[test]
fn year_1900_divisible_by_100_only_is_not_leap_so_feb_ends_on_28() {
    // 1900-02-28T00:00:00Z
    assert_eq!(
        epoch_ms_to_rfc3339_local(-2_203_977_600_000, 0),
        "1900-02-28T00:00:00Z"
    );
    // Exactly one day later must be 1900-03-01, never 1900-02-29.
    assert_eq!(
        epoch_ms_to_rfc3339_local(-2_203_977_600_000 + 86_400_000, 0),
        "1900-03-01T00:00:00Z"
    );
}

#[test]
fn leap_year_2024_feb_29_just_before_midnight_renders_correctly() {
    // 2024-02-29T23:59:59Z
    assert_eq!(
        epoch_ms_to_rfc3339_local(1_709_251_199_000, 0),
        "2024-02-29T23:59:59Z"
    );
}

#[test]
fn zero_offset_uses_z_suffix() {
    let rendered = epoch_ms_to_rfc3339_local(RENDERED_INSTANT_MS, 0);
    assert!(rendered.ends_with('Z'), "got: {rendered}");
    assert_eq!(rendered, "2026-08-17T09:05:03Z");
}

#[test]
fn positive_utc_offset_renders_local_time_with_plus_suffix() {
    // 2026-08-17T09:05:03Z + 9h (KST) = 2026-08-17T18:05:03+09:00
    assert_eq!(
        epoch_ms_to_rfc3339_local(RENDERED_INSTANT_MS, KST_OFFSET_SECONDS),
        "2026-08-17T18:05:03+09:00"
    );
}

#[test]
fn negative_utc_offset_renders_across_date_boundary() {
    // epoch 0 (1970-01-01T00:00:00Z) minus 5h = 1969-12-31T19:00:00-05:00
    assert_eq!(
        epoch_ms_to_rfc3339_local(0, NEGATIVE_FIVE_OFFSET_SECONDS),
        "1969-12-31T19:00:00-05:00"
    );
}

/// Exercises the live (non-pure) path: `local_utc_offset_seconds_at`
/// resolves the process's actual timezone via `chrono::Local`, so —
/// unlike every other test in this file — the expected offset isn't a
/// fixture; it's whatever the test-running environment's `TZ` happens to
/// be. This doesn't force `TZ` (mutating it is process-global and
/// edition-2024 makes `set_var` `unsafe`); instead it accepts either
/// shape the pure renderer can produce and checks the one the live
/// offset actually selects: a trailing `Z` when the offset is `0`
/// (`TZ=UTC`, this repo's devcontainer default), or a `+HH:MM`/`-HH:MM`
/// suffix otherwise — plus, either way, a well-formed
/// `YYYY-MM-DDTHH:MM:SS` prefix.
#[test]
fn live_local_offset_path_renders_rfc3339_with_z_or_numeric_offset() {
    let offset = local_utc_offset_seconds_at(RENDERED_INSTANT_MS);
    let rendered = epoch_ms_to_rfc3339_local(RENDERED_INSTANT_MS, offset);

    let prefix = &rendered[..19];
    let prefix_bytes = prefix.as_bytes();
    assert_eq!(prefix.len(), 19, "got: {rendered}");
    assert_eq!(prefix_bytes[4], b'-', "got: {rendered}");
    assert_eq!(prefix_bytes[7], b'-', "got: {rendered}");
    assert_eq!(prefix_bytes[10], b'T', "got: {rendered}");
    assert_eq!(prefix_bytes[13], b':', "got: {rendered}");
    assert_eq!(prefix_bytes[16], b':', "got: {rendered}");

    let suffix = &rendered[19..];
    if offset == 0 {
        assert_eq!(suffix, "Z", "got: {rendered}");
    } else {
        assert_eq!(suffix.len(), 6, "got: {rendered}");
        let suffix_bytes = suffix.as_bytes();
        assert!(
            suffix_bytes[0] == b'+' || suffix_bytes[0] == b'-',
            "got: {rendered}"
        );
        assert_eq!(suffix_bytes[3], b':', "got: {rendered}");
    }
}
