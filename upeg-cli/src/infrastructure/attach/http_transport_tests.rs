use std::io::{Cursor, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};

use super::*;
use upeg_core::MAX_UNTRUSTED_OUTPUT_WIRE_BYTES;

const TEST_BODY_LIMIT_BYTES: usize = 2;
const TEST_HEADER_LIMIT_BYTES: usize = 64;
const TEST_REQUEST_BODY_BYTES: usize = 2;
const TEST_REQUEST_READ_CHUNK_BYTES: usize = 512;

fn 테스트_제한() -> ResponseLimits {
    ResponseLimits {
        body_bytes: TEST_BODY_LIMIT_BYTES,
        header_bytes: TEST_HEADER_LIMIT_BYTES,
    }
}

fn 응답(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = format!("{status}\r\n{headers}\r\n").into_bytes();
    raw.extend_from_slice(body);
    raw
}

fn 테스트_요청을_끝까지_읽는다(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut chunk = [0_u8; TEST_REQUEST_READ_CHUNK_BYTES];
    loop {
        let read_bytes = stream.read(&mut chunk).unwrap();
        assert_ne!(read_bytes, 0, "테스트 attach 요청이 본문 전에 끝났다");
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
fn http_응답은_상태와_본문을_파싱한다() {
    // Given
    let raw = 응답(
        "HTTP/1.0 200 OK",
        "Content-Type: application/json\r\n",
        b"{}",
    );

    // When
    let (status, body) = parse_http_response(raw, 테스트_제한()).unwrap();

    // Then
    assert_eq!(status, 200);
    assert_eq!(body, "{}");
}

#[test]
fn http_응답은_정확한_본문_상한을_허용한다() {
    // Given
    let raw = 응답("HTTP/1.0 200 OK", "Content-Length: 2\r\n", b"{}");

    // When
    let parsed = read_and_parse_response(&mut Cursor::new(raw), 테스트_제한(), RESPONSE_DEADLINE);

    // Then
    assert_eq!(parsed.unwrap(), (200, "{}".to_string()));
}

#[test]
fn http_응답은_본문_상한보다_한_byte_크면_파싱전에_거부한다() {
    // Given
    let head = b"HTTP/1.0 200 OK";
    let limits = ResponseLimits {
        body_bytes: TEST_BODY_LIMIT_BYTES,
        header_bytes: head.len(),
    };
    let raw = 응답("HTTP/1.0 200 OK", "", b"{}x");

    // When
    let error = read_and_parse_response(&mut Cursor::new(raw), limits, RESPONSE_DEADLINE)
        .expect_err("상한보다 한 byte 큰 응답은 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    assert!(error.to_string().contains("framed wire limit"));
}

#[test]
fn attach_본문_상한은_core_wire_상수를_사용한다() {
    // Given
    let expected = usize::try_from(MAX_UNTRUSTED_OUTPUT_WIRE_BYTES).unwrap();

    // When
    let limits = ResponseLimits::for_untrusted_output().unwrap();

    // Then
    assert_eq!(limits.body_bytes, expected);
}

#[test]
fn http_응답은_정확한_헤더_상한을_허용한다() {
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
fn http_응답은_과대한_헤더를_거부한다() {
    // Given
    let prefix = b"HTTP/1.0 204 No Content\r\nX-Fill: ";
    let filler_bytes = MAX_ATTACH_HTTP_HEADER_BYTES - prefix.len() + 1;
    let mut raw = Vec::from(prefix.as_slice());
    raw.extend(std::iter::repeat_n(b'a', filler_bytes));
    raw.extend_from_slice(HTTP_HEADER_BODY_SEPARATOR);

    // When
    let error = parse_http_response(raw, ResponseLimits::for_untrusted_output().unwrap())
        .expect_err("과대한 attach HTTP 헤더는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_content_length보다_짧은_본문을_거부한다() {
    // Given
    let raw = 응답("HTTP/1.0 200 OK", "Content-Length: 3\r\n", b"{}");
    let limits = ResponseLimits {
        body_bytes: 3,
        header_bytes: TEST_HEADER_LIMIT_BYTES,
    };

    // When
    let error =
        parse_http_response(raw, limits).expect_err("잘린 Content-Length 본문은 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
}

#[test]
fn http_응답은_content_length보다_긴_본문을_거부한다() {
    // Given
    let raw = 응답("HTTP/1.0 200 OK", "Content-Length: 1\r\n", b"{}");

    // When
    let error = parse_http_response(raw, 테스트_제한())
        .expect_err("Content-Length 뒤의 추가 byte는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_중복된_content_length를_거부한다() {
    // Given
    let raw = 응답(
        "HTTP/1.0 200 OK",
        "Content-Length: 2\r\nContent-Length: 3\r\n",
        b"{}",
    );

    // When
    let error =
        parse_http_response(raw, 테스트_제한()).expect_err("중복 Content-Length는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_공백으로_접힌_중복_content_length를_거부한다() {
    // Given
    let raw = 응답(
        "HTTP/1.0 200 OK",
        "Content-Length: 2\r\n Content-Length: 3\r\n",
        b"{}",
    );

    // When
    let error = parse_http_response(raw, 테스트_제한())
        .expect_err("공백으로 접힌 중복 Content-Length는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_비정상_content_length를_거부한다() {
    // Given
    let raw = 응답("HTTP/1.0 200 OK", "Content-Length: nope\r\n", b"{}");

    // When
    let error =
        parse_http_response(raw, 테스트_제한()).expect_err("비정상 Content-Length는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_plus_접두사의_content_length를_거부한다() {
    // Given
    let raw = 응답("HTTP/1.0 200 OK", "Content-Length: +2\r\n", b"{}");

    // When
    let error = parse_http_response(raw, 테스트_제한())
        .expect_err("Content-Length는 ASCII 숫자만 허용해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_colon_앞_공백이_있는_transfer_encoding을_거부한다() {
    // Given
    let raw = 응답("HTTP/1.1 200 OK", "Transfer-Encoding : chunked\r\n", b"{}");

    // When
    let error = parse_http_response(raw, 테스트_제한())
        .expect_err("field-name 뒤 공백으로 숨긴 Transfer-Encoding은 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_chunked_전송을_거부한다() {
    // Given
    let raw = 응답(
        "HTTP/1.1 200 OK",
        "Transfer-Encoding: chunked\r\n",
        b"2\r\n{}\r\n0\r\n\r\n",
    );

    // When
    let error =
        parse_http_response(raw, 테스트_제한()).expect_err("chunked attach 응답은 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_bare_lf_헤더를_거부한다() {
    // Given
    let raw = b"HTTP/1.0 200 OK\nContent-Length: 2\r\n\r\n{}".to_vec();

    // When
    let error = parse_http_response(raw, 테스트_제한()).expect_err("bare LF 헤더는 거부해야 한다");

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_쓰레기값을_거부한다() {
    // Given
    let malformed_head = b"definitely not http".to_vec();

    // When
    let error = parse_http_response(malformed_head, 테스트_제한());

    // Then
    assert_eq!(error.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[test]
fn http_응답은_utf8이_아닌_본문을_거부한다() {
    // Given
    let malformed_body = 응답("HTTP/1.0 200 OK", "Content-Length: 1\r\n", &[0xff]);

    // When
    let error = parse_http_response(malformed_body, 테스트_제한());

    // Then
    assert_eq!(error.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[test]
fn 실제_tcp_attach는_과대한_응답을_제어된_오류로_반환한다() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        테스트_요청을_끝까지_읽는다(&mut stream);
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
    .expect_err("실제 TCP attach도 과대 응답을 거부해야 한다");
    server.join().unwrap();

    // Then
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    assert!(error.to_string().contains("framed wire limit"));
}

#[test]
fn 실제_tcp_attach는_정상_json_응답을_반환한다() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        테스트_요청을_끝까지_읽는다(&mut stream);
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
