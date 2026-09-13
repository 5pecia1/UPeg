use serde_json::Value;

pub(crate) const MAX_LINE_BYTES: usize = 1_000_000;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LineReadOutcome {
    Eof,
    Line { bytes: usize },
    CapHit,
}

pub(crate) fn extract_text_content(result: &Value) -> String {
    let mut text = String::new();
    if let Some(arr) = result.get("content").and_then(Value::as_array) {
        for part in arr {
            if part.get("type").and_then(Value::as_str) == Some("text")
                && let Some(t) = part.get("text").and_then(Value::as_str)
            {
                if t.is_empty() {
                    continue;
                }
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
    }
    text
}
