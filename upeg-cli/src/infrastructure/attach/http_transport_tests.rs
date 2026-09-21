use std::io::{Cursor, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};

use super::*;
use upeg_core::MAX_UNTRUSTED_OUTPUT_WIRE_BYTES;

const TEST_BODY_LIMIT_BYTES: usize = 2;
const TEST_HEADER_LIMIT_BYTES: usize = 64;
const TEST_REQUEST_BODY_BYTES: usize = 2;
const TEST_REQUEST_READ_CHUNK_BYTES: usize = 512;

fn test_limits() -> ResponseLimits {
    ResponseLimits {
        body_bytes: TEST_BODY_LIMIT_BYTES,
        header_bytes: TEST_HEADER_LIMIT_BYTES,
    }
}

fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = format!("{status}\r\n{headers}\r\n").into_bytes();
    raw.extend_from_slice(body);
    raw
}

fn read_test_request_to_end(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut chunk = [0_u8; TEST_REQUEST_READ_CHUNK_BYTES];
    loop {
        let read_bytes = stream.read(&mut chunk).unwrap();
        assert_ne!(read_bytes, 0, "test attach request ended before its body");
        request.extend_from_slice(&chunk[..read_bytes]);
        let body_start = request
            .windows(HTTP_HEADER_BODY_SEPARATOR.len())
            .position(|window| window == HTTP_HEADER_BODY_SEPARATOR)
            .map(|index| index + HTTP_HEADER_BODY_SEPARATOR.len());
        if body_start.is_some_and(|index| request.len() >= index + TEST_REQUEST_BODY_BYTES) {
            break;
        }
    }
}

#[test]
fn http_response_parses_status_and_body() {
    // Given
    let raw = response(
        "HTTP/1.0 200 OK",
        "Content-Type: application/json\r\n",
        b"{}",
    );

    // When
    let (status, body) = parse_http_response(raw, test_limits()).unwrap();

    // Then
    assert_eq!(status, 200);
    assert_eq!(body, "{}");
}

#[test]
fn http_response_allows_the_exact_body_limit() {
    // Given
    let raw = response("HTTP/1.0 200 OK", "Content-Length: 2\r\n", b"{}");

    // When
    let parsed = read_and_parse_response(&mut Cursor::new(raw), test_limits(), RESPONSE_DEADLINE);

    // Then
    assert_eq!(parsed.unwrap(), (200, "{}".to_string()));
}

#[test]
fn http_response_rejects_one_byte_over_body_limit_before_parsing() {
    // Given
    let head = b"HTTP/1.0 200 OK";
    let limits = ResponseLimits {
        body_bytes: TEST_BODY_LIMIT_BYTES,
        header_bytes: head.len(),
    };
    let raw = response("HTTP/1.0 200 OK", "", b"{}x");

    // When
    let error = read_and_parse_response(&mut Cursor::new(raw), limits, RESPONSE_DEADLINE)
        .expect_err("a response one byte over the limit must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    assert!(error.to_string().contains("framed wire limit"));
}

#[test]
fn attach_body_limit_uses_the_core_wire_constant() {
    // Given
    let expected = usize::try_from(MAX_UNTRUSTED_OUTPUT_WIRE_BYTES).unwrap();

    // When
    let limits = ResponseLimits::for_untrusted_output().unwrap();

    // Then
    assert_eq!(limits.body_bytes, expected);
}

#[test]
fn http_response_allows_the_exact_header_limit() {
    // Given
    let prefix = b"HTTP/1.0 204 No Content\r\nX-Fill: ";
    let filler_bytes = MAX_ATTACH_HTTP_HEADER_BYTES - prefix.len();
    let mut raw = Vec::from(prefix.as_slice());
    raw.extend(std::iter::repeat_n(b'a', filler_bytes));
    raw.extend_from_slice(HTTP_HEADER_BODY_SEPARATOR);

    // When
    let parsed = parse_http_response(raw, ResponseLimits::for_untrusted_output().unwrap());

    // Then
    assert_eq!(parsed.unwrap(), (204, String::new()));
}

#[test]
fn http_response_rejects_oversized_headers() {
    // Given
    let prefix = b"HTTP/1.0 204 No Content\r\nX-Fill: ";
    let filler_bytes = MAX_ATTACH_HTTP_HEADER_BYTES - prefix.len() + 1;
    let mut raw = Vec::from(prefix.as_slice());
    raw.extend(std::iter::repeat_n(b'a', filler_bytes));
    raw.extend_from_slice(HTTP_HEADER_BODY_SEPARATOR);

    // When
    let error = parse_http_response(raw, ResponseLimits::for_untrusted_output().unwrap())
        .expect_err("oversized attach HTTP headers must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_body_shorter_than_content_length() {
    // Given
    let raw = response("HTTP/1.0 200 OK", "Content-Length: 3\r\n", b"{}");
    let limits = ResponseLimits {
        body_bytes: 3,
        header_bytes: TEST_HEADER_LIMIT_BYTES,
    };

    // When
    let error = parse_http_response(raw, limits)
        .expect_err("a truncated Content-Length body must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
}

#[test]
fn http_response_rejects_body_longer_than_content_length() {
    // Given
    let raw = response("HTTP/1.0 200 OK", "Content-Length: 1\r\n", b"{}");

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("extra bytes after Content-Length must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_duplicate_content_length() {
    // Given
    let raw = response(
        "HTTP/1.0 200 OK",
        "Content-Length: 2\r\nContent-Length: 3\r\n",
        b"{}",
    );

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("duplicate Content-Length must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_whitespace_folded_duplicate_content_length() {
    // Given
    let raw = response(
        "HTTP/1.0 200 OK",
        "Content-Length: 2\r\n Content-Length: 3\r\n",
        b"{}",
    );

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("whitespace-folded duplicate Content-Length must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_malformed_content_length() {
    // Given
    let raw = response("HTTP/1.0 200 OK", "Content-Length: nope\r\n", b"{}");

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("malformed Content-Length must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_plus_prefixed_content_length() {
    // Given
    let raw = response("HTTP/1.0 200 OK", "Content-Length: +2\r\n", b"{}");

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("Content-Length must allow ASCII digits only");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_transfer_encoding_with_space_before_colon() {
    // Given
    let raw = response("HTTP/1.1 200 OK", "Transfer-Encoding : chunked\r\n", b"{}");

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("Transfer-Encoding hidden behind post-field-name whitespace must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_chunked_transfer() {
    // Given
    let raw = response(
        "HTTP/1.1 200 OK",
        "Transfer-Encoding: chunked\r\n",
        b"2\r\n{}\r\n0\r\n\r\n",
    );

    // When
    let error = parse_http_response(raw, test_limits())
        .expect_err("a chunked attach response must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_bare_lf_headers() {
    // Given
    let raw = b"HTTP/1.0 200 OK\nContent-Length: 2\r\n\r\n{}".to_vec();

    // When
    let error =
        parse_http_response(raw, test_limits()).expect_err("bare LF headers must be rejected");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_garbage() {
    // Given
    let malformed_head = b"definitely not http".to_vec();

    // When
    let error = parse_http_response(malformed_head, test_limits());

    // Then
    assert_eq!(error.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[test]
fn http_response_rejects_non_utf8_body() {
    // Given
    let malformed_body = response("HTTP/1.0 200 OK", "Content-Length: 1\r\n", &[0xff]);

    // When
    let error = parse_http_response(malformed_body, test_limits());

    // Then
    assert_eq!(error.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[test]
fn real_tcp_attach_returns_oversized_response_as_controlled_error() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_test_request_to_end(&mut stream);
        stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n{}x").unwrap();
    });
    let endpoint = format!("http://{address}/attach-test");
    let limits = ResponseLimits {
        body_bytes: TEST_BODY_LIMIT_BYTES,
        header_bytes: b"HTTP/1.0 200 OK".len(),
    };

    // When
    let error = execute(
        Request {
            method: "POST",
            full_endpoint: &endpoint,
            token: None,
            extra_headers: &[],
            body: "{}",
        },
        limits,
        RequestBudget::DISPATCH,
    )
    .expect_err("real TCP attach must also reject an oversized response");
    server.join().unwrap();

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    assert!(error.to_string().contains("framed wire limit"));
}

#[test]
fn real_tcp_attach_returns_a_well_formed_json_response() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_test_request_to_end(&mut stream);
        stream
            .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
    });
    let endpoint = format!("http://{address}/attach-test");

    // When
    let response = post_json(&endpoint, None, "{}").unwrap();
    server.join().unwrap();

    // Then
    assert_eq!(response, (200, "{}".to_string()));
}
