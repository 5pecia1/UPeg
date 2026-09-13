//! Tests for the pure `EnvironmentBackup` model.
//!
//! These cover the contract that survived the port from the retired
//! Dioxus surface: schema version constant, exact-schema serde guard,
//! JSON round-trip stability, and the version-mismatch validation
//! helper. Surface-side (UI / DOM) backup wiring is deliberately out
//! of scope here — it lives in `upeg-frb/src/api/backup.rs` instead.

use std::collections::BTreeMap;

use upeg_core::prefs::{Accent, Locale, Theme, Tweaks};
use upeg_core::{ArgsPreset, ColSpan, PinSpan, Placement, RowSpan};

use crate::features::backup::{BACKUP_VERSION, EnvironmentBackup};
use crate::features::boards::BoardData;

const BACKUP_BEFORE_GUIDANCE_VERSION: u32 = 3;

fn fixture_backup() -> EnvironmentBackup {
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0),
            Placement::new("id.uuid_v7", 2, 0),
        ],
    );
    let mut memos = BTreeMap::new();
    memos.insert("scratch".into(), "note".into());
    EnvironmentBackup {
        version: BACKUP_VERSION,
        tweaks: Tweaks {
            theme: Theme::Dark,
            accent: Accent::Cyan,
            show_holes: false,
            local_http_host: false,
            locale: Locale::En,
        },
        layouts,
        memos,
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance {
                description: "개발 작업".into(),
                instructions: "# 순서\n\n    cargo test\n".into(),
            },
        }],
    }
}

#[test]
fn 이전_백업은_보드_안내_유실을_막기_위해_거부한다() {
    let mut previous = fixture_backup();
    previous.version = BACKUP_BEFORE_GUIDANCE_VERSION;
    previous
        .validate_version()
        .expect_err("이전 백업 형식 거부");
}

#[test]
fn from_json은_올바른_json을_파싱한다() {
    let json = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[]}"#;
    let parsed = EnvironmentBackup::from_json(json).expect("parse");
    assert_eq!(parsed.version, 4);
    assert!(parsed.layouts.is_empty());
    assert!(parsed.boards.is_empty());
}

#[test]
fn from_json은_unknown_field를_거부한다() {
    let bad = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[],"future":1}"#;
    let err = EnvironmentBackup::from_json(bad).expect_err("unknown fields rejected");
    assert!(
        err.contains("future") || err.contains("unknown field"),
        "exact-schema error should mention the unknown field; got {err}"
    );
}

#[test]
fn validate_version은_불일치시_에러를_반환한다() {
    let mut backup = fixture_backup();
    backup.version = 999;
    let err = backup.validate_version().expect_err("mismatch rejected");
    assert!(
        err.contains("999"),
        "mismatch error must mention the input version; got {err}"
    );
}

#[test]
fn json_왕복은_모든_필드를_보존한다() {
    let original = fixture_backup();
    let json = original.to_json_pretty();
    let back = EnvironmentBackup::from_json(&json).expect("parse");
    assert_eq!(back, original);
}

#[test]
fn validate_version은_현재_버전을_허용한다() {
    let backup = fixture_backup();
    assert!(backup.validate_version().is_ok());
}

#[test]
fn span과_args_preset이_실린_placement도_backup_json으로_왕복한다() {
    let mut backup = fixture_backup();
    backup.layouts.insert(
        "styled".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0)
                .with_span(Some(PinSpan::new(
                    ColSpan::new(3).expect("테스트 cols"),
                    RowSpan::new(2).expect("테스트 rows"),
                )))
                .with_args_preset(Some(
                    ArgsPreset::parse(r#"{"hex":"ff"}"#).expect("테스트 preset"),
                )),
        ],
    );

    let json = backup.to_json_pretty();
    let back = EnvironmentBackup::from_json(&json).expect("parse");
    assert_eq!(
        back, backup,
        "span/args_preset 필드가 손실 없이 왕복해야 한다"
    );
    // deny_unknown_fields 정책은 그대로: 낯선 필드는 여전히 거부된다.
    let with_unknown = json.trim_end().trim_end_matches('}').to_string() + r#","future":1}"#;
    assert!(EnvironmentBackup::from_json(&with_unknown).is_err());
}
