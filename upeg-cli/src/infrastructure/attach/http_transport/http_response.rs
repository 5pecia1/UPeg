//! Strict HTTP/1.x response framing for the attach transport.

use std::io::ErrorKind;

use upeg_core::MAX_UNTRUSTED_OUTPUT_WIRE_BYTES;

mod framing;

pub(super) use framing::{ResponseFraming, inspect_response_framing};

pub(super) const MAX_ATTACH_HTTP_HEADER_BYTES: usize = 16 * 1024;
pub(super) const HTTP_HEADER_BODY_SEPARATOR: &[u8; 4] = b"\r\n\r\n";

#[derive(Clone, Copy)]
pub(super) struct ResponseLimits {
    pub(super) body_bytes: usize,
    pub(super) header_bytes: usize,
}

impl ResponseLimits {
    pub(super) fn for_untrusted_output() -> std::io::Result<Self> {
        let body_bytes = usize::try_from(MAX_UNTRUSTED_OUTPUT_WIRE_BYTES).map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "attach response limit does not fit this platform",
            )
        })?;
        Ok(Self {
            body_bytes,
            header_bytes: MAX_ATTACH_HTTP_HEADER_BYTES,
        })
    }

    pub(super) fn response_bytes(self) -> std::io::Result<usize> {
        self.body_bytes
            .checked_add(self.header_bytes)
            .and_then(|bytes| bytes.checked_add(HTTP_HEADER_BODY_SEPARATOR.len()))
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "attach response limit overflow")
            })
    }
}

pub(super) fn parse_http_response(
    mut raw: Vec<u8>,
    limits: ResponseLimits,
) -> std::io::Result<(u16, String)> {
    let split = raw
        .windows(HTTP_HEADER_BODY_SEPARATOR.len())
        .position(|window| window == HTTP_HEADER_BODY_SEPARATOR)
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "no header/body separator"))?;
    if split > limits.header_bytes {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "attach response headers exceed the configured limit",
        ));
    }
    let body_start = split
        .checked_add(HTTP_HEADER_BODY_SEPARATOR.len())
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "invalid response framing"))?;
    let body_bytes = raw.len() - body_start;
    let (status, content_length) = parse_response_head(&raw[..split])?;
    validate_body_length(body_bytes, content_length, limits.body_bytes)?;

    raw.copy_within(body_start.., 0);
    raw.truncate(body_bytes);
    let body = String::from_utf8(raw).map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("attach response body utf8: {error}"),
        )
    })?;
    Ok((status, body))
}

pub(super) fn parse_response_head(head: &[u8]) -> std::io::Result<(u16, Option<usize>)> {
    let head = std::str::from_utf8(head)
        .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))?;
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "empty response head"))?;
    reject_bare_line_break(status_line)?;
    let mut status_parts = status_line.split_whitespace();
    match status_parts.next() {
        Some("HTTP/1.0" | "HTTP/1.1") => {}
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "unsupported HTTP response version",
            ));
        }
    }
    let status = status_parts
        .next()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "missing status"))?
        .parse()
        .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, format!("status: {error}")))?;
    let mut content_length = None;
    for line in lines {
        reject_bare_line_break(line)?;
        let (name, value) = line.split_once(':').ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidData, "malformed response header")
        })?;
        if !is_http_field_name(name) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "invalid response header field-name",
            ));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "attach does not support Transfer-Encoding responses",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "duplicate Content-Length response header",
                ));
            }
            content_length = Some(parse_content_length(value)?);
        }
    }
    Ok((status, content_length))
}

fn parse_content_length(value: &str) -> std::io::Result<usize> {
    let digits = value.trim_matches(|character| matches!(character, ' ' | '\t'));
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "invalid Content-Length",
        ));
    }
    digits.parse().map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("invalid Content-Length: {error}"),
        )
    })
}

fn reject_bare_line_break(line: &str) -> std::io::Result<()> {
    if line
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "attach response headers require CRLF framing",
        ));
    }
    Ok(())
}

fn is_http_field_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
                    | b'0'..=b'9'
                    | b'A'..=b'Z'
                    | b'a'..=b'z'
            )
        })
}

fn validate_body_length(
    actual: usize,
    declared: Option<usize>,
    limit: usize,
) -> std::io::Result<()> {
    if actual > limit || declared.is_some_and(|bytes| bytes > limit) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("attach response body exceeds the {limit}-byte wire limit"),
        ));
    }
    match declared {
        Some(expected) if actual < expected => Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            format!("attach response body is truncated: expected {expected}, received {actual}"),
        )),
        Some(expected) if actual > expected => Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attach response body exceeds Content-Length: expected {expected}, received {actual}"
            ),
        )),
        Some(_) | None => Ok(()),
    }
}
