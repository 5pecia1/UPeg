use std::io::ErrorKind;

use super::{HTTP_HEADER_BODY_SEPARATOR, ResponseLimits, parse_response_head};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum ResponseFraming {
    ContentLength { response_bytes: usize },
    Eof,
}

pub(in super::super) fn inspect_response_framing(
    raw: &[u8],
    limits: ResponseLimits,
) -> std::io::Result<Option<ResponseFraming>> {
    let Some(split) = raw
        .windows(HTTP_HEADER_BODY_SEPARATOR.len())
        .position(|window| window == HTTP_HEADER_BODY_SEPARATOR)
    else {
        reject_oversized_header_prefix(raw.len(), limits)?;
        return Ok(None);
    };
    if split > limits.header_bytes {
        return Err(oversized_header_error());
    }

    let body_start = split
        .checked_add(HTTP_HEADER_BODY_SEPARATOR.len())
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "invalid response framing"))?;
    let (_, content_length) = parse_response_head(&raw[..split])?;
    match content_length {
        Some(body_bytes) => inspect_content_length(raw.len(), body_start, body_bytes, limits),
        None => inspect_eof_framing(raw.len(), body_start, limits),
    }
}

fn reject_oversized_header_prefix(
    received_bytes: usize,
    limits: ResponseLimits,
) -> std::io::Result<()> {
    let max_prefix_bytes = limits
        .header_bytes
        .checked_add(HTTP_HEADER_BODY_SEPARATOR.len() - 1)
        .ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "attach header limit overflow")
        })?;
    if received_bytes > max_prefix_bytes {
        return Err(oversized_header_error());
    }
    Ok(())
}

fn inspect_content_length(
    received_bytes: usize,
    body_start: usize,
    body_bytes: usize,
    limits: ResponseLimits,
) -> std::io::Result<Option<ResponseFraming>> {
    if body_bytes > limits.body_bytes {
        return Err(oversized_body_error(limits.body_bytes));
    }
    let response_bytes = body_start.checked_add(body_bytes).ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidData, "invalid Content-Length framing")
    })?;
    if received_bytes > response_bytes {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "attach response body exceeds Content-Length",
        ));
    }
    Ok(Some(ResponseFraming::ContentLength { response_bytes }))
}

fn inspect_eof_framing(
    received_bytes: usize,
    body_start: usize,
    limits: ResponseLimits,
) -> std::io::Result<Option<ResponseFraming>> {
    if received_bytes - body_start > limits.body_bytes {
        return Err(oversized_body_error(limits.body_bytes));
    }
    Ok(Some(ResponseFraming::Eof))
}

fn oversized_header_error() -> std::io::Error {
    std::io::Error::new(
        ErrorKind::InvalidData,
        "attach response headers exceed the configured limit",
    )
}

fn oversized_body_error(limit: usize) -> std::io::Error {
    std::io::Error::new(
        ErrorKind::InvalidData,
        format!("attach response body exceeds the {limit}-byte wire limit"),
    )
}
