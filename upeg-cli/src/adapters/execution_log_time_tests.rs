//! Tests for [`super::epoch_ms_to_rfc3339_local`] and its
//! `civil_from_days` day-math core. Fixture epoch-ms values were computed
//! independently via Python's `datetime` (`timestamp() * 1000`), not by
//! re-deriving them from this module's own algorithm.

use super::{epoch_ms_to_rfc3339_local, local_utc_offset_seconds_at};

const KST_OFFSET_SECONDS: i32 = 9 * 3_600;
const NEGATIVE_FIVE_OFFSET_SECONDS: i32 = -5 * 3_600;
/// 2026-08-17T09:05:03Z — 오프셋 접미사 케이스들이 공유하는 기준 시각.
const RENDERED_INSTANT_MS: i64 = 1_786_957_503_000;

#[test]
fn epoch_0은_1970년_1월_1일_자정_utc로_렌더링된다() {
    assert_eq!(epoch_ms_to_rfc3339_local(0, 0), "1970-01-01T00:00:00Z");
}

#[test]
fn epoch_이전_음수_ms도_1969년_마지막_초로_렌더링된다() {
    // 1969-12-31T23:59:59Z
    assert_eq!(epoch_ms_to_rfc3339_local(-1_000, 0), "1969-12-31T23:59:59Z");
}

#[test]
fn _400으로_나누어지는_윤년_2000년_2월_29일이_올바르게_렌더링된다() {
    // 2000-02-29T12:30:45Z
    assert_eq!(
        epoch_ms_to_rfc3339_local(951_827_445_000, 0),
        "2000-02-29T12:30:45Z"
    );
}

#[test]
fn _100으로만_나누어지는_1900년은_윤년이_아니라서_2월_28일이_마지막날이다() {
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
fn 윤년_2024년_2월_29일_자정_직전이_올바르게_렌더링된다() {
    // 2024-02-29T23:59:59Z
    assert_eq!(
        epoch_ms_to_rfc3339_local(1_709_251_199_000, 0),
        "2024-02-29T23:59:59Z"
    );
}

#[test]
fn 오프셋이_0이면_z_접미사를_사용한다() {
    let rendered = epoch_ms_to_rfc3339_local(RENDERED_INSTANT_MS, 0);
    assert!(rendered.ends_with('Z'), "got: {rendered}");
    assert_eq!(rendered, "2026-08-17T09:05:03Z");
}

#[test]
fn 양수_utc_오프셋을_더하면_해당_지역_시간과_플러스_오프셋_문자열이_렌더링된다() {
    // 2026-08-17T09:05:03Z + 9h (KST) = 2026-08-17T18:05:03+09:00
    assert_eq!(
        epoch_ms_to_rfc3339_local(RENDERED_INSTANT_MS, KST_OFFSET_SECONDS),
        "2026-08-17T18:05:03+09:00"
    );
}

#[test]
fn 음수_utc_오프셋은_날짜_경계를_넘어_렌더링된다() {
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
fn 실제_로컬_오프셋_경로는_z_또는_숫자_오프셋을_가진_rfc3339를_렌더링한다() {
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
