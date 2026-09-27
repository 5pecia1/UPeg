//! Shared, privacy-bounded failure-report vocabulary.
//!
//! A diagnostic is deliberately separate from the metadata-only execution
//! log. It is written only for failures and may contain the capped process
//! streams that explain an External command failure. Arguments, environment
//! values and successful outputs never belong here.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

/// Maximum bytes retained for either normal process stream.
pub const DIAGNOSTIC_STREAM_MAX_BYTES: usize = 16 * 1024;
/// Maximum bytes retained for the structured debug context.
pub const DIAGNOSTIC_CONTEXT_MAX_BYTES: usize = 32 * 1024;

/// One failure ready to be persisted by a native diagnostic store.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagnosticDraft {
    pub run_id: String,
    pub occurred_at_ms: u64,
    pub app_version: String,
    pub os: String,
    pub tool_id: Option<String>,
    /// The execution surface or loader that observed the failure.
    pub source: String,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub error_code: String,
    pub error_message: String,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    /// Redacted structured details retained for an explicit debug export.
    pub debug_context: Option<Value>,
}

impl DiagnosticDraft {
    /// Return a storage-safe copy: bounded and with common credential forms
    /// replaced before they cross the persistence boundary.
    #[must_use]
    pub fn redacted(mut self) -> Self {
        self.error_message =
            redact_diagnostic_text(&self.error_message, DIAGNOSTIC_STREAM_MAX_BYTES);
        self.stdout = redact_diagnostic_text(&self.stdout, DIAGNOSTIC_STREAM_MAX_BYTES);
        self.stderr = redact_diagnostic_text(&self.stderr, DIAGNOSTIC_STREAM_MAX_BYTES);
        self.debug_context = self.debug_context.map(redact_diagnostic_value);
        self
    }
}

/// Remove common token assignments and impose a UTF-8-safe byte cap.
#[must_use]
pub fn redact_diagnostic_text(value: &str, max_bytes: usize) -> String {
    let redacted = redact_plain_diagnostic_text(value);
    // Compiler/tool output often contains one JSON error object per line.
    // When the whole string is JSON, redact by key too so quoted forms such
    // as `{"token":"value"}` do not evade assignment-style matching.
    let redacted = serde_json::from_str::<Value>(&redacted)
        .ok()
        .and_then(|json| serde_json::to_string(&redact_diagnostic_value(json)).ok())
        .unwrap_or(redacted);
    truncate_utf8(&redacted, max_bytes)
}

fn redact_plain_diagnostic_text(value: &str) -> String {
    static ASSIGNMENT: OnceLock<Option<Regex>> = OnceLock::new();
    static AUTHORIZATION: OnceLock<Option<Regex>> = OnceLock::new();
    static QUOTED_JSON_ASSIGNMENT: OnceLock<Option<Regex>> = OnceLock::new();
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(r"(?i)\b(api[_-]?key|token|secret|password|passwd|access[_-]?token|refresh[_-]?token)\s*([:=])\s*[^\s,;]+")
            .ok()
    });
    let authorization = AUTHORIZATION
        .get_or_init(|| Regex::new(r"(?i)\bauthorization\s*:\s*(?:bearer\s+)?[^\s,;]+").ok());
    let quoted_json_assignment = QUOTED_JSON_ASSIGNMENT.get_or_init(|| {
        Regex::new(r#"(?i)(["']?(api[_-]?key|token|secret|password|passwd|access[_-]?token|refresh[_-]?token)["']?\s*:\s*["']?)[^"'\s,;}]+"#)
            .ok()
    });
    let (Some(assignment), Some(authorization), Some(quoted_json_assignment)) =
        (assignment, authorization, quoted_json_assignment)
    else {
        return "[REDACTED: diagnostic redaction unavailable]".to_string();
    };
    let redacted = assignment.replace_all(value, "$1$2[REDACTED]");
    let redacted = authorization.replace_all(&redacted, "authorization: [REDACTED]");
    quoted_json_assignment
        .replace_all(&redacted, "$1[REDACTED]")
        .into_owned()
}

fn redact_diagnostic_value(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    let normalized = key
                        .chars()
                        .filter(char::is_ascii_alphanumeric)
                        .flat_map(char::to_lowercase)
                        .collect::<String>();
                    let sensitive = [
                        "token",
                        "secret",
                        "password",
                        "passwd",
                        "authorization",
                        "apikey",
                        "accesstoken",
                        "refreshtoken",
                    ]
                    .iter()
                    .any(|needle| normalized.contains(needle));
                    let value = if sensitive {
                        Value::String("[REDACTED]".to_string())
                    } else {
                        redact_diagnostic_value(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_diagnostic_value).collect())
        }
        Value::String(value) => Value::String(truncate_utf8(
            &redact_plain_diagnostic_text(&value),
            DIAGNOSTIC_CONTEXT_MAX_BYTES,
        )),
        other => other,
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_common_credential_forms_and_keeps_utf8_valid() {
        let text = "TOKEN=abc authorization: Bearer xyz password: hello 한글";
        let redacted = redact_diagnostic_text(text, 1024);
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("xyz"));
        assert!(!redacted.contains("hello"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn redaction_covers_quoted_json_keys_and_common_api_key_spelling() {
        let text = r#"{"token":"value","apiKey":"api-value","authorization":"Bearer auth-value"}"#;
        let redacted = redact_diagnostic_text(text, 1024);
        assert!(!redacted.contains("value"));
        assert!(!redacted.contains("api-value"));
        assert!(!redacted.contains("auth-value"));
        let json: Value = serde_json::from_str(&redacted).expect("redacted JSON stays valid");
        assert_eq!(json["token"], "[REDACTED]");
        assert_eq!(json["apiKey"], "[REDACTED]");
        assert_eq!(json["authorization"], "[REDACTED]");
    }
}
