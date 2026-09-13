//! Manifest example fixtures for documentation generation.
//!
//! All TOML examples use `include_str!` to reference committed files under
//! `examples/tools/`. Bash command snippets remain inline because they are
//! commands, not duplicated TOML manifest bodies.

use crate::manifest_docs::sections::ManifestExample;

pub(crate) const QUICK_START_EXAMPLES: &[ManifestExample] = &[
    ManifestExample {
        title: "~/.upeg/toolkits/demo.toml",
        language: "toml",
        body: include_str!("../../examples/tools/echo-bracketed.toml"),
    },
    ManifestExample {
        title: "Validate and call it",
        language: "bash",
        body: "upeg tool validate ~/.upeg/toolkits/demo.toml\nupeg call demo.echo_bracketed -a tag=hi",
    },
];

pub(crate) const CHAIN_EXAMPLES: &[ManifestExample] = &[ManifestExample {
    title: "Toolkit with a chain tool",
    language: "toml",
    body: include_str!("../../examples/tools/chain-md5-then-uppercase.toml"),
}];

pub(crate) const INPUT_EXAMPLES: &[ManifestExample] = &[ManifestExample {
    title: "Llm tool with inputs",
    language: "toml",
    body: include_str!("../../examples/tools/llm-echo.toml"),
}];

pub(crate) const INVOKER_EXAMPLES: &[ManifestExample] = &[ManifestExample {
    title: "External execution knobs (cwd, env, timeout_ms, input defaults)",
    language: "toml",
    body: include_str!("../../examples/tools/dev-external-demo.toml"),
}];

pub(crate) const CREDENTIAL_EXAMPLES: &[ManifestExample] = &[ManifestExample {
    title: "Environment credential reference",
    language: "toml",
    body: include_str!("../../examples/tools/http-env-credential.toml"),
}];

pub(crate) const VALIDATION_EXAMPLES: &[ManifestExample] = &[
    ManifestExample {
        title: "Validate one Toolkit TOML file",
        language: "bash",
        body: "upeg tool validate ~/.upeg/toolkits/demo.toml",
    },
    ManifestExample {
        title: "Validate every Toolkit TOML file in a directory",
        language: "bash",
        body: "upeg toolkit validate               # defaults to ~/.upeg/toolkits (or $UPEG_TOOLKITS_DIR)\nupeg toolkit validate ~/.upeg/toolkits",
    },
    ManifestExample {
        title: "Regenerate committed schema and guide artifacts",
        language: "bash",
        body: "just toolkit-schema",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 빠른_시작_예제_파일이_존재한다() {
        let body = QUICK_START_EXAMPLES[0].body;
        assert!(body.starts_with("# Minimal External invoker example"));
        assert!(body.contains("invoker = \"External\""));
    }

    #[test]
    fn 체인_예제_파일이_존재한다() {
        let body = CHAIN_EXAMPLES[0].body;
        assert!(body.starts_with("# Chain Tool composing two built-ins"));
        assert!(body.contains("invoker = \"Chain\""));
        assert!(body.contains("steps"));
    }

    #[test]
    fn 입력_예제_파일이_존재한다() {
        let body = INPUT_EXAMPLES[0].body;
        assert!(body.contains("invoker = \"Llm\""));
        assert!(body.contains("inputs"));
    }

    #[test]
    fn 호출자_예제는_외부_실행_설정을_보여준다() {
        let body = INVOKER_EXAMPLES[0].body;
        assert!(body.contains("cwd = "));
        assert!(body.contains("timeout_ms = "));
        assert!(body.contains("[[tools.env]]"));
        assert!(body.contains("default = "));
    }

    #[test]
    fn 자격증명_예제_파일이_존재한다() {
        let body = CREDENTIAL_EXAMPLES[0].body;
        assert!(body.starts_with("# Http invoker example"));
        assert!(body.contains("store = \"env\""));
        assert!(!body.contains("secret_value"));
        assert!(!body.contains("literal_secret"));
        assert!(!body.contains("value = "));
    }

    #[test]
    fn 검증_예제는_배시_조각이다() {
        for example in VALIDATION_EXAMPLES {
            assert_eq!(example.language, "bash");
        }
    }

    #[test]
    fn 모든_toml_예제는_깔끔하게_파싱된다() {
        for (name, examples) in [
            ("quick_start", QUICK_START_EXAMPLES),
            ("chain", CHAIN_EXAMPLES),
            ("inputs", INPUT_EXAMPLES),
            ("invoker", INVOKER_EXAMPLES),
            ("credential", CREDENTIAL_EXAMPLES),
        ] {
            for example in examples {
                if example.language == "toml" {
                    crate::parse_toolkit_full(example.body).unwrap_or_else(|e| {
                        panic!("{name} example '{}' failed to parse: {e}", example.title);
                    });
                }
            }
        }
    }
}
