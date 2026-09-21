//! Error-family-owned troubleshooting metadata.
//!
//! These entries live next to `LoadError` so documentation stays coupled to the
//! actual validation surface. The docs renderer (Task 6) will read
//! `TOOLKIT_TROUBLESHOOTING` instead of defining its own rows.

/// Classification of a `LoadError` family that a troubleshooting entry targets.
///
/// Each variant maps to one or more `LoadError` enum discriminants; the test
/// `troubleshooting_uses_real_load_error_families` proves coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadErrorFamily {
    /// `LoadError::RetiredField`
    RetiredField,
    /// `LoadError::MissingPegboardUnits` | `LoadError::EmptyPegboardUnits` | `LoadError::UnknownPegboardUnits`
    PegboardUnits,
    /// `LoadError::MissingInvoker` | `LoadError::EmptyInvoker`
    InvokerMissing,
    /// `LoadError::MissingInvokerField` | `LoadError::ExternalRequiresCommand` | `LoadError::UnsupportedRuntimeInvoker` | `LoadError::InvokerFieldConflict` | `LoadError::MissingDispatcher` | `LoadError::ZeroExternalTimeout` | `LoadError::EmptyEnvName` | `LoadError::UnknownExternalColor` | `LoadError::PtyUnsupportedOnHost`
    InvokerField,
    /// `LoadError::SecretField` | `LoadError::EmptyCredentialField` | `LoadError::UnknownCredentialStore`
    CredentialSecret,
    /// `LoadError::UnknownSurface` | `LoadError::UnknownInvoker` | `LoadError::EmptyInSurfaces`
    UnknownSurfaceOrInvoker,
    /// `LoadError::ChainConnectionCycle`
    ChainCycle,
    /// `LoadError::InvalidInputSpec` | `LoadError::UnknownInputType` | `LoadError::UnexpectedInputOptions` | `LoadError::UnexpectedInputFilePolicy` | `LoadError::UnsupportedInputDefault` | `LoadError::InvalidInputDefault`
    InvalidInputs,
    /// `LoadError::InvalidOutputSpec` | `LoadError::UnknownOutputType` | output URL/options shape errors
    InvalidOutputs,
}

impl LoadErrorFamily {
    /// Discriminant names present in `error.rs` for this family.
    #[allow(
        dead_code,
        reason = "used by test-only coverage checks for docs metadata"
    )]
    pub(crate) fn variant_names(&self) -> &'static [&'static str] {
        match self {
            Self::RetiredField => &["RetiredField"],
            Self::PegboardUnits => &[
                "MissingPegboardUnits",
                "EmptyPegboardUnits",
                "UnknownPegboardUnits",
            ],
            Self::InvokerMissing => &["MissingInvoker", "EmptyInvoker"],
            Self::InvokerField => &[
                "MissingInvokerField",
                "ExternalRequiresCommand",
                "UnsupportedRuntimeInvoker",
                "InvokerFieldConflict",
                "MissingDispatcher",
                "ZeroExternalTimeout",
                "EmptyEnvName",
                "UnknownExternalColor",
                "PtyUnsupportedOnHost",
            ],
            Self::CredentialSecret => &[
                "SecretField",
                "EmptyCredentialField",
                "UnknownCredentialStore",
            ],
            Self::UnknownSurfaceOrInvoker => {
                &["UnknownSurface", "UnknownInvoker", "EmptyInSurfaces"]
            }
            Self::ChainCycle => &["ChainConnectionCycle"],
            Self::InvalidInputs => &[
                "InvalidInputSpec",
                "UnknownInputType",
                "UnexpectedInputOptions",
                "UnexpectedInputFilePolicy",
                "UnsupportedInputDefault",
                "InvalidInputDefault",
            ],
            Self::InvalidOutputs => &[
                "InvalidOutputSpec",
                "UnknownOutputType",
                "UnexpectedOutputOptions",
                "UnexpectedOutputUrl",
                "MissingEmbeddedViewUrl",
                "EmptyEmbeddedViewUrl",
                "NonCanonicalEmbeddedViewUrl",
            ],
        }
    }
}

/// One troubleshooting row: the problem a user sees and the fix to apply.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TroubleshootingEntry {
    /// Short description of the validation problem as it appears to the user.
    pub(crate) problem: &'static str,
    /// Actionable fix the user should apply.
    pub(crate) fix: &'static str,
    /// Which `LoadError` family (variants in `error.rs`) this row addresses.
    #[allow(
        dead_code,
        reason = "used by test-only coverage checks for docs metadata"
    )]
    pub(crate) related_error: LoadErrorFamily,
}

/// Canonical troubleshooting rows for Toolkit TOML validation.
///
/// Every row maps to at least one real `LoadError` variant; the test
/// `troubleshooting_rows_cover_real_error_families` proves this invariant.
pub(crate) const TOOLKIT_TROUBLESHOOTING: &[TroubleshootingEntry] = &[
    TroubleshootingEntry {
        problem: "retired `category` or `cat` field",
        fix: "Replace the field with `tags` on the Toolkit or tool entry.",
        related_error: LoadErrorFamily::RetiredField,
    },
    TroubleshootingEntry {
        problem: "`pegboard_units` is required",
        fix: "Set `pegboard_units = \"U1\"`, `\"U2\"`, or `\"U2T\"` on each runtime tool.",
        related_error: LoadErrorFamily::PegboardUnits,
    },
    TroubleshootingEntry {
        problem: "`invoker` is required",
        fix: "Set an invoker explicitly, or add `steps` to infer `Chain`.",
        related_error: LoadErrorFamily::InvokerMissing,
    },
    TroubleshootingEntry {
        problem: "missing invoker-specific field",
        fix: "Add the required field for the selected invoker, such as `command` for `External`, `url` for `Http`, `prompt` for `Llm`, or `wasm_path` for `Wasm`. `cwd`, `env`, and `timeout_ms` belong to `External` only, `timeout_ms` must be greater than zero, and every `env` entry needs a non-empty `name`.",
        related_error: LoadErrorFamily::InvokerField,
    },
    TroubleshootingEntry {
        problem: "inline secret field",
        fix: "Remove `credentials[].value`, `credentials[].secret_value`, or `credentials[].literal_secret`; store the secret in env or keychain and keep only the reference in TOML.",
        related_error: LoadErrorFamily::CredentialSecret,
    },
    TroubleshootingEntry {
        problem: "unknown surface or invoker",
        fix: "Use supported surfaces `cli`, `tui`, `desktop`, `pwa`, `ext`, `mcp`, or `http`, and supported runtime invokers `External`, `Http`, `Embed`, `Chain`, `Llm`, or `Wasm`.",
        related_error: LoadErrorFamily::UnknownSurfaceOrInvoker,
    },
    TroubleshootingEntry {
        problem: "chain cycle",
        fix: "Keep `connections` acyclic. Model loops with an explicit loop-capable tool instead of cyclic Chain edges.",
        related_error: LoadErrorFamily::ChainCycle,
    },
    TroubleshootingEntry {
        problem: "invalid `inputs`",
        fix: "Declare each input with a canonical `name`, closed `type`, and non-empty `options` choices when using `options` or `multi_options`. A `default` must match the input type: a number for `number` / `integer`, a string for the text-shaped types.",
        related_error: LoadErrorFamily::InvalidInputs,
    },
    TroubleshootingEntry {
        problem: "invalid `outputs`",
        fix: "Declare each output with a canonical `name`, closed `type`, non-empty choices for `options` / `multi_options`, and a canonical `url` for `embedded_view`.",
        related_error: LoadErrorFamily::InvalidOutputs,
    },
];

#[cfg(test)]
mod tests {
    use super::{LoadErrorFamily, TOOLKIT_TROUBLESHOOTING, TroubleshootingEntry};

    /// Every troubleshooting row must reference at least one real LoadError variant.
    ///
    /// The expected discriminant set is derived from `error.rs`. If a variant is
    /// renamed or removed, this test will catch it.
    #[test]
    fn troubleshooting_rows_cover_real_error_families() {
        let known_variants: &[&str] = &[
            "RetiredField",
            "MissingPegboardUnits",
            "EmptyPegboardUnits",
            "UnknownPegboardUnits",
            "MissingInvoker",
            "EmptyInvoker",
            "MissingInvokerField",
            "ExternalRequiresCommand",
            "UnsupportedRuntimeInvoker",
            "InvokerFieldConflict",
            "MissingDispatcher",
            "ZeroExternalTimeout",
            "EmptyEnvName",
            "UnknownExternalColor",
            "PtyUnsupportedOnHost",
            "SecretField",
            "EmptyCredentialField",
            "UnknownCredentialStore",
            "UnknownSurface",
            "UnknownInvoker",
            "EmptyInSurfaces",
            "ChainConnectionCycle",
            "InvalidInputSpec",
            "UnknownInputType",
            "UnexpectedInputOptions",
            "UnexpectedInputFilePolicy",
            "UnsupportedInputDefault",
            "InvalidInputDefault",
            "InvalidOutputSpec",
            "UnknownOutputType",
            "UnexpectedOutputOptions",
            "UnexpectedOutputUrl",
            "MissingEmbeddedViewUrl",
            "EmptyEmbeddedViewUrl",
            "NonCanonicalEmbeddedViewUrl",
        ];

        for entry in TOOLKIT_TROUBLESHOOTING {
            let names = entry.related_error.variant_names();
            assert!(
                !names.is_empty(),
                "entry problem={:?} has no variant names",
                entry.problem
            );
            for name in names {
                assert!(
                    known_variants.contains(name),
                    "entry problem={:?} references unknown LoadError variant {:?} — \
                     if the variant was renamed, fix the LoadErrorFamily::variant_names match arm",
                    entry.problem,
                    name
                );
            }
        }
    }

    /// All required troubleshooting topics are present.
    #[test]
    fn troubleshooting_covers_required_topics() {
        let families: Vec<LoadErrorFamily> = TOOLKIT_TROUBLESHOOTING
            .iter()
            .map(|e| e.related_error)
            .collect();

        assert!(
            families.contains(&LoadErrorFamily::RetiredField),
            "must cover RetiredField"
        );
        assert!(
            families.contains(&LoadErrorFamily::PegboardUnits),
            "must cover PegboardUnits"
        );
        assert!(
            families.contains(&LoadErrorFamily::InvokerMissing),
            "must cover InvokerMissing"
        );
        assert!(
            families.contains(&LoadErrorFamily::InvokerField),
            "must cover InvokerField"
        );
        assert!(
            families.contains(&LoadErrorFamily::CredentialSecret),
            "must cover CredentialSecret"
        );
        assert!(
            families.contains(&LoadErrorFamily::UnknownSurfaceOrInvoker),
            "must cover UnknownSurfaceOrInvoker"
        );
        assert!(
            families.contains(&LoadErrorFamily::ChainCycle),
            "must cover ChainCycle"
        );
        assert!(
            families.contains(&LoadErrorFamily::InvalidInputs),
            "must cover InvalidInputs"
        );
        assert!(
            families.contains(&LoadErrorFamily::InvalidOutputs),
            "must cover InvalidOutputs"
        );
    }

    /// Troubleshooting entries do not mention invokers or surfaces absent from LoadError metadata.
    #[test]
    fn troubleshooting_uses_supported_invokers_and_surfaces() {
        let known_invokers = [
            "Function", "External", "Http", "Embed", "Chain", "Llm", "Wasm",
        ];
        let known_surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"];

        for entry in TOOLKIT_TROUBLESHOOTING {
            let combined = format!("{} {}", entry.problem, entry.fix);
            for keyword in combined.split_whitespace() {
                let cleaned = keyword.trim_matches(|c: char| !c.is_alphanumeric());
                if matches!(
                    cleaned,
                    "Function" | "External" | "Http" | "Embed" | "Chain" | "Llm" | "Wasm"
                ) {
                    assert!(
                        known_invokers.contains(&cleaned),
                        "troubleshooting entry mentions unknown invoker {cleaned:?}"
                    );
                }
                if matches!(
                    cleaned,
                    "cli" | "tui" | "desktop" | "pwa" | "ext" | "mcp" | "http"
                ) {
                    assert!(
                        known_surfaces.contains(&cleaned),
                        "troubleshooting entry mentions unknown surface {cleaned:?}"
                    );
                }
            }
        }
    }

    /// Entry type is non-exhaustive for compile time stability.
    #[test]
    fn troubleshooting_entry_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<TroubleshootingEntry>();
        assert_copy::<LoadErrorFamily>();
    }
}
