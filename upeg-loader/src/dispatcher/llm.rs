//! `ProviderKind` and the OpenAI-compatible chat-completions provider for
//! the `Llm` invoker.
//!
//! `ProviderKind` closes the open-ended `provider` TOML string into three
//! variants a Toolkit author can declare: `Echo` (deterministic local
//! pattern — no network), `ToolDelegate` (forward the rendered prompt into
//! another registered Tool), and `OpenAiCompatible` (POST the rendered
//! prompt to an OpenAI-compatible `/chat/completions` endpoint). This
//! replaces the old `provider.as_str()` string match in `dispatcher.rs`
//! with a single parse step and one closed error type.
//!
//! HTTP mechanics for the `OpenAiCompatible` variant reuse
//! [`super::http::run_http_request`], which the `Http` invoker also uses.
//! That client now selects its transport by scheme: plain `http://` goes
//! through the hand-rolled TCP client, while `https://` goes through a
//! TLS-capable client (`ureq` + rustls). [`OPENAI_DEFAULT_BASE_URL`] is
//! `https://`, so out of the box a request against the real OpenAI API is
//! reached over TLS. Point `base_url` at any OpenAI-compatible endpoint;
//! both `http://` and `https://` are supported.

use serde_json::{Value, json};

/// Default OpenAI-compatible base URL used when a Tool omits `base_url`.
pub(super) const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

const ECHO_PROVIDER: &str = "echo";
const OPENAI_PROVIDER: &str = "openai";
const TOOL_DELEGATE_PREFIX: &str = "tool:";
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

/// Closed set of LLM provider adapters a `Llm`-invoker Tool's `provider`
/// TOML string resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderKind {
    /// `""` or `"echo"` — deterministic local pattern; no network call.
    Echo,
    /// `"tool:<id>"` — delegate the rendered prompt into another Tool.
    ToolDelegate(DelegateToolId),
    /// `"openai"` — OpenAI-compatible chat completions HTTP provider.
    OpenAiCompatible,
}

/// Non-empty, trimmed Tool id extracted from `provider = "tool:<id>"`.
///
/// A small owned newtype rather than `upeg_core::ToolId<'a>`: dispatcher
/// closures are `'static` (they must outlive the `ToolToml` they were built
/// from), so the delegate id needs an owned `String`, not a borrow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DelegateToolId(String);

impl DelegateToolId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

/// `provider` string parse failures. Converted to a plain `String` at the
/// dispatcher boundary, matching the rest of `dispatcher`'s `Result<_, String>`
/// convention.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum ProviderKindParseError {
    #[error("llm provider `tool:` requires a non-empty delegate Tool id")]
    EmptyDelegateToolId,
    #[error(
        "llm provider `{0}` is not configured; use `provider = \"echo\"` for local patterns, \
         `provider = \"tool:<id>\"` to delegate, or `provider = \"openai\"` for the \
         OpenAI-compatible HTTP provider"
    )]
    Unknown(String),
}

impl std::str::FromStr for ProviderKind {
    type Err = ProviderKindParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == ECHO_PROVIDER {
            return Ok(Self::Echo);
        }
        if trimmed == OPENAI_PROVIDER {
            return Ok(Self::OpenAiCompatible);
        }
        if let Some(id) = trimmed.strip_prefix(TOOL_DELEGATE_PREFIX) {
            let id = id.trim();
            if id.is_empty() {
                return Err(ProviderKindParseError::EmptyDelegateToolId);
            }
            return Ok(Self::ToolDelegate(DelegateToolId(id.to_string())));
        }
        Err(ProviderKindParseError::Unknown(trimmed.to_string()))
    }
}

/// Typed OpenAI-compatible provider failure modes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum OpenAiProviderError {
    #[error("openai provider requires `model = \"<name>\"`")]
    MissingModel,
    #[error(
        "openai provider requires `credential = \"<name>\"` resolving the API key \
         (declare it in `credentials` or rely on the default `UPEG_CREDENTIAL_<NAME>` env fallback)"
    )]
    MissingCredential,
    #[error("openai request failed: {0}")]
    Http(String),
    #[error("openai response is not valid JSON: {0}")]
    MalformedResponse(String),
    #[error("openai response missing `choices[0].message.content`")]
    MissingContent,
}

/// Inputs needed to run one OpenAI-compatible chat completion request.
pub(super) struct OpenAiRequest<'a> {
    pub(super) base_url: &'a str,
    pub(super) model: &'a str,
    pub(super) api_key: &'a str,
    pub(super) prompt: &'a str,
}

/// POST `{base_url}/chat/completions` with `model` + the rendered prompt as
/// a single user message, then extract `choices[0].message.content`.
pub(super) fn run_openai_chat_completion(
    request: OpenAiRequest<'_>,
) -> Result<String, OpenAiProviderError> {
    if request.model.trim().is_empty() {
        return Err(OpenAiProviderError::MissingModel);
    }
    if request.api_key.trim().is_empty() {
        return Err(OpenAiProviderError::MissingCredential);
    }
    let base = request.base_url.trim().trim_end_matches('/');
    let url = format!("{base}{CHAT_COMPLETIONS_PATH}");
    let body = json!({
        "model": request.model,
        "messages": [{ "role": "user", "content": request.prompt }],
    })
    .to_string();
    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        (
            "Authorization".to_string(),
            format!("Bearer {}", request.api_key),
        ),
    ];
    let response_text = super::http::run_http_request("POST", &url, headers, Some(body))
        .map_err(OpenAiProviderError::Http)?;
    extract_message_content(&response_text)
}

fn extract_message_content(response_text: &str) -> Result<String, OpenAiProviderError> {
    let value: Value = serde_json::from_str(response_text)
        .map_err(|error| OpenAiProviderError::MalformedResponse(error.to_string()))?;
    value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(OpenAiProviderError::MissingContent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_provider_문자열은_provider_kind로_파싱된다() {
        assert_eq!("".parse::<ProviderKind>().unwrap(), ProviderKind::Echo);
        assert_eq!("echo".parse::<ProviderKind>().unwrap(), ProviderKind::Echo);
        assert_eq!(
            "  echo  ".parse::<ProviderKind>().unwrap(),
            ProviderKind::Echo
        );
        assert_eq!(
            "openai".parse::<ProviderKind>().unwrap(),
            ProviderKind::OpenAiCompatible
        );
        assert_eq!(
            "tool:ai.summarize".parse::<ProviderKind>().unwrap(),
            ProviderKind::ToolDelegate(DelegateToolId("ai.summarize".to_string()))
        );
        assert_eq!(
            "tool: ai.summarize ".parse::<ProviderKind>().unwrap(),
            ProviderKind::ToolDelegate(DelegateToolId("ai.summarize".to_string()))
        );
    }

    #[test]
    fn 알_수_없는_provider는_에러를_반환한다() {
        let err = "azure".parse::<ProviderKind>().unwrap_err();
        assert!(err.to_string().contains("llm provider `azure`"));
        assert!(err.to_string().contains("provider = \"openai\""));
    }

    #[test]
    fn tool_위임_provider의_빈_아이디는_에러를_반환한다() {
        let err = "tool:  ".parse::<ProviderKind>().unwrap_err();
        assert_eq!(err, ProviderKindParseError::EmptyDelegateToolId);
    }

    #[test]
    fn openai_provider는_credential을_해석해_요청을_보낸다() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead as _, Read as _, Write as _};

            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
            let mut header_text = String::new();
            let mut content_length = 0_usize;
            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).expect("read request line");
                if bytes == 0 || line == "\r\n" {
                    break;
                }
                if let Some(rest) = line
                    .to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(str::to_string)
                {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
                header_text.push_str(&line);
            }
            let mut body = vec![0_u8; content_length];
            reader.read_exact(&mut body).expect("read request body");
            let body = String::from_utf8(body).expect("utf8 body");

            let response_body =
                r#"{"choices":[{"message":{"role":"assistant","content":"pong"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            (header_text, body)
        });

        let out = run_openai_chat_completion(OpenAiRequest {
            base_url: &format!("http://{addr}"),
            model: "gpt-test",
            api_key: "sk-test-secret",
            prompt: "ping",
        })
        .expect("openai request succeeds against mock server");
        assert_eq!(out, "pong");

        let (headers, body) = handle.join().expect("mock server thread");
        assert!(
            headers.contains("Authorization: Bearer sk-test-secret"),
            "credential must be resolved into the Authorization header, got:\n{headers}"
        );
        assert!(body.contains("\"model\":\"gpt-test\""), "got body: {body}");
        assert!(body.contains("\"content\":\"ping\""), "got body: {body}");
    }

    #[test]
    fn openai_provider는_모델이_없으면_에러를_반환한다() {
        let err = run_openai_chat_completion(OpenAiRequest {
            base_url: OPENAI_DEFAULT_BASE_URL,
            model: "",
            api_key: "sk-test",
            prompt: "ping",
        })
        .expect_err("missing model rejected");
        assert_eq!(err, OpenAiProviderError::MissingModel);
    }

    #[test]
    fn openai_provider는_credential이_없으면_에러를_반환한다() {
        let err = run_openai_chat_completion(OpenAiRequest {
            base_url: OPENAI_DEFAULT_BASE_URL,
            model: "gpt-test",
            api_key: "",
            prompt: "ping",
        })
        .expect_err("missing credential rejected");
        assert_eq!(err, OpenAiProviderError::MissingCredential);
    }

    #[test]
    fn openai_provider는_잘못된_형식의_응답에_에러를_반환한다() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead as _, Write as _};

            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).expect("read request line");
                if bytes == 0 || line == "\r\n" {
                    break;
                }
            }
            // 200 OK but the body is not valid chat-completions JSON.
            let response_body = "not json";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let err = run_openai_chat_completion(OpenAiRequest {
            base_url: &format!("http://{addr}"),
            model: "gpt-test",
            api_key: "sk-test",
            prompt: "ping",
        })
        .expect_err("malformed response rejected");
        assert!(matches!(err, OpenAiProviderError::MalformedResponse(_)));
        handle.join().expect("mock server thread");
    }

    #[test]
    fn openai_provider는_choices가_없는_응답에_에러를_반환한다() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead as _, Write as _};

            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).expect("read request line");
                if bytes == 0 || line == "\r\n" {
                    break;
                }
            }
            let response_body = r#"{"choices":[]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let err = run_openai_chat_completion(OpenAiRequest {
            base_url: &format!("http://{addr}"),
            model: "gpt-test",
            api_key: "sk-test",
            prompt: "ping",
        })
        .expect_err("empty choices rejected");
        assert_eq!(err, OpenAiProviderError::MissingContent);
        handle.join().expect("mock server thread");
    }
}
