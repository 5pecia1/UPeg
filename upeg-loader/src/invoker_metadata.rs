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
    fn runtime_호출자_목록은_여섯_항목을_포함한다() {
        assert_eq!(RUNTIME_INVOKERS.len(), 6);
    }

    #[test]
    fn runtime_호출자_이름은_지원_집합과_일치한다() {
        let names: Vec<_> = RUNTIME_INVOKERS.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            &["External", "Http", "Embed", "Chain", "Llm", "Wasm"]
        );
    }

    #[test]
    fn 외부_호출자는_명령을_요구한다() {
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
    fn http_호출자는_url을_요구한다() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Http").unwrap();
        assert_eq!(spec.required_fields, &["url"]);
    }

    #[test]
    fn embed_호출자는_아무것도_요구하지_않는다() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Embed").unwrap();
        assert!(spec.required_fields.is_empty());
    }

    #[test]
    fn 체인_호출자는_단계를_요구한다() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Chain").unwrap();
        assert_eq!(spec.required_fields, &["steps"]);
    }

    #[test]
    fn 엘엘엠_호출자는_프롬프트를_요구한다() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Llm").unwrap();
        assert_eq!(spec.required_fields, &["prompt"]);
    }

    #[test]
    fn wasm_호출자는_wasm_경로를_요구한다() {
        let spec = RUNTIME_INVOKERS.iter().find(|s| s.name == "Wasm").unwrap();
        assert_eq!(spec.required_fields, &["wasm_path"]);
    }
}
