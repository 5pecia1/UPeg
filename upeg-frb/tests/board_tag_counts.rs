//! Compare the batched Board tag read with the existing APIs on saved state.

#![cfg(unix)]
#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "single-process integration assertions"
)]

use upeg_core::{PinId, Placement};
use upeg_frb::api::{pegboard, tools};

fn legacy_counts(board_key: Option<String>) -> Vec<(String, u32)> {
    pegboard::tag_options_for_board(board_key.clone())
        .into_iter()
        .map(|tag| {
            let count = pegboard::count_pinned_for_tag(board_key.clone(), tag.clone());
            (tag, count)
        })
        .collect()
}

#[test]
#[allow(
    unsafe_code,
    reason = "this integration binary has one test and owns its process environment"
)]
fn batched_counts_match_legacy_reads_on_saved_boards_and_union() {
    let scratch = tempfile::tempdir().expect("scratch UPEG_HOME");
    let home = scratch
        .path()
        .canonicalize()
        .expect("canonical scratch home");
    // This binary has one test, so no other test thread can read either
    // process-wide variable while these two individual writes run.
    unsafe { std::env::set_var(upeg_core::paths::env::UPEG_HOME, &home) };
    unsafe { std::env::set_var("UPEG_PROJECT_MANIFEST_PATH", "off") };
    // Register the same compiled-in metadata that the desktop uses, then
    // persist a fixture with one tool pinned on both boards.
    assert!(!tools::list_tools(None).is_empty());
    let mut state = upeg_sources::pegboard::load_state();
    // Missing layouts are reseeded with default pins, so save explicit empty
    // layouts for every built-in Board outside this fixture's two Boards.
    for layout in state.layouts.values_mut() {
        layout.clear();
    }
    state.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("time.epoch", 0, 0),
            Placement::new("num.hex_to_decimal", 1, 0),
            Placement::new("time.epoch", 2, 0)
                .with_pin_id(PinId::parse("epoch-copy").expect("valid fixture pin id")),
        ],
    );
    state.layouts.insert(
        "personal".into(),
        vec![
            Placement::new("time.epoch", 0, 0),
            Placement::new("text.lowercase", 1, 0),
        ],
    );
    upeg_sources::pegboard::save_state(&state).expect("save board fixture");

    for (key, expected_all, expected_time) in
        [(Some("dev"), 3, 2), (Some("personal"), 2, 1), (None, 3, 1)]
    {
        let key = key.map(str::to_string);
        let batched: Vec<_> = pegboard::board_tag_counts(key.clone())
            .into_iter()
            .map(|entry| (entry.tag, entry.count))
            .collect();
        assert_eq!(batched, legacy_counts(key.clone()), "board {key:?}");
        assert_eq!(
            pegboard::count_pinned_for_tag(key.clone(), "unknown-tag".into()),
            0
        );
        assert!(!batched.iter().any(|(tag, _)| tag == "unknown-tag"));
        assert_eq!(batched[0], ("all".into(), expected_all));
        assert_eq!(
            batched
                .iter()
                .find(|(tag, _)| tag == "time")
                .map(|(_, count)| *count),
            Some(expected_time),
        );
    }

    pegboard::pin_tool("dev".into(), "time.epoch_now".into()).expect("saved pin mutation");
    assert_eq!(pegboard::board_tag_counts(Some("dev".into()))[0].count, 4);
    assert_eq!(
        pegboard::board_tag_counts(Some("dev".into()))
            .into_iter()
            .map(|entry| (entry.tag, entry.count))
            .collect::<Vec<_>>(),
        legacy_counts(Some("dev".into()))
    );
}
