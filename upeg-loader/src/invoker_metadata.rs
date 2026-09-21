//! Metadata for runtime invokers.
//!
//! Shared source of truth for invoker requirements used by both parser
//! validation (`parse::validate_required_invoker_fields`) and doc
//! generation (`manifest_docs`).

/// Required-field definition for one runtime invoker.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeInvokerSpec {
    /// Canonical invoker name as written in TOML.
    pub(crate) name: &'static str,
    /// One-line purpose description.
    pub(crate) purpose: &'static str,
    /// Fields that MUST be present and non-empty for this invoker.
    pub(crate) required_fields: &'static [&'static str],
    /// Fields that MAY be present for this invoker.
    pub(crate) optional_fields: &'static [&'static str],
}

/// All supported runtime invokers, in registration order.
pub(crate) const RUNTIME_INVOKERS: &[RuntimeInvokerSpec] = &[
    RuntimeInvokerSpec {
        name: "External",
        purpose: "spawn a local process",
        required_fields: &["command"],
        optional_fields: &[
            "args_template",
            "cwd",
            "env",
            "timeout_ms",
            "color",
            "credentials",
        ],
    },
    RuntimeInvokerSpec {
        name: "Http",
        purpose: "call an HTTP endpoint",
        required_fields: &["url"],
        optional_fields: &["method", "headers", "body", "credential", "credentials"],
    },
    RuntimeInvokerSpec {
        name: "Embed",
        purpose: "open a GUI sidecar runtime adapter in a WebView or iframe",
        required_fields: &[],
        optional_fields: &[
            "embed_url",
            "controlled_embed.browser",
            "controlled_embed.bindings",
        ],
    },
    RuntimeInvokerSpec {
        name: "Chain",
        purpose: "compose other tools with ordered steps and connections",
        required_fields: &["steps"],
        optional_fields: &["connections", "output"],
    },
    RuntimeInvokerSpec {
        name: "Llm",
        purpose: "render a prompt into an LLM/provider adapter",
        required_fields: &["prompt"],
        optional_fields: &["provider", "model", "credential", "credentials"],
    },
    RuntimeInvokerSpec {
        name: "Wasm",
        purpose: "load a WASM module adapter",
        required_fields: &["wasm_path"],
        optional_fields: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_invoker_list_has_six_entries() {
        assert_eq!(RUNTIME_INVOKERS.len(), 6);
    }

    #[test]
    fn runtime_invoker_names_match_supported_set() {
        let names: Vec<_> = RUNTIME_INVOKERS.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            &["External", "Http", "Embed", "Chain", "Llm", "Wasm"]
        );
    }

    #[test]
    fn external_invoker_requires_command() {
        let spec = RUNTIME_INVOKERS
            .iter()
            .find(|s| s.name == "External")
            .unwrap();
        assert_eq!(spec.required_fields, &["command"]);
        assert!(spec.optional_fields.contains(&"cwd"));
        assert!(spec.optional_fields.contains(&"env"));
        assert!(spec.optional_fields.contains(&"timeout_ms"));
        assert!(spec.optional_fields.contains(&"color"));
    }

    #[test]
    fn http_invoker_requires_url() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Http").unwrap();
        assert_eq!(spec.required_fields, &["url"]);
    }

    #[test]
    fn embed_invoker_requires_nothing() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Embed").unwrap();
        assert!(spec.required_fields.is_empty());
    }

    #[test]
    fn chain_invoker_requires_steps() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Chain").unwrap();
        assert_eq!(spec.required_fields, &["steps"]);
    }

    #[test]
    fn llm_invoker_requires_prompt() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Llm").unwrap();
        assert_eq!(spec.required_fields, &["prompt"]);
    }

    #[test]
    fn wasm_invoker_requires_wasm_path() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Wasm").unwrap();
        assert_eq!(spec.required_fields, &["wasm_path"]);
    }
}
