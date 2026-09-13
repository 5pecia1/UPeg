#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test fails loudly when workspace files cannot be inspected"
)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const RETIRED_IDENTIFIERS: &[&str] = &[
    "parse_schema_fields",
    "args_from_form_fields",
    "validate_args_against_input_schema",
    "__from_static_tool_schema",
    "InputSchema",
    "OwnedInputSchema",
    "FormField",
];

#[test]
fn 프로덕션_소스는_퇴역한_schema_폼_api를_사용하지_않는다() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-core has a workspace parent")
        .to_path_buf();
    let mut offenders = Vec::new();
    visit_rust_sources(&workspace, &mut |path| {
        if !is_production_source(path) {
            return;
        }
        let source = fs::read_to_string(path).expect("test should read source file");
        for identifier in RETIRED_IDENTIFIERS {
            if contains_identifier(&source, identifier) {
                offenders.push(format!("{} uses `{identifier}`", path.display()));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "retired JSON-schema form APIs must stay out of production paths:\n{}",
        offenders.join("\n")
    );
}

fn visit_rust_sources(dir: &Path, visitor: &mut impl FnMut(&Path)) {
    let entries = fs::read_dir(dir).expect("test should read workspace directories");
    for entry in entries {
        let path = entry.expect("test should read workspace entry").path();
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            visit_rust_sources(&path, visitor);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            visitor(&path);
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".sisyphus" | "target" | "tests" | "fixture-src" | "vendor"
            )
        })
}

fn is_production_source(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if file_name.contains("test") {
        return false;
    }
    path.components()
        .any(|component| component.as_os_str() == OsStr::new("src"))
}

fn contains_identifier(source: &str, identifier: &str) -> bool {
    source.match_indices(identifier).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + identifier.len()..].chars().next();
        before.is_none_or(|c| !is_identifier_char(c))
            && after.is_none_or(|c| !is_identifier_char(c))
    })
}

fn is_identifier_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}
