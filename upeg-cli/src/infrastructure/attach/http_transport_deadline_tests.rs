use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use super::*;

const TEST_REQUEST_BODY_BYTES: usize = 2;
const TEST_REQUEST_READ_CHUNK_BYTES: usize = 512;
const TEST_DRIP_BODY_BYTES: usize = 20;
const TEST_RESPONSE_DEADLINE: Duration = Duration::from_millis(80);
/// A dispatch budget with the response half shrunk to
/// [`TEST_RESPONSE_DEADLINE`] — the connect half is irrelevant to these
/// tests (the listener is already bound on loopback).
fn 테스트_예산() -> RequestBudget {
    RequestBudget {
        response: TEST_RESPONSE_DEADLINE,
        ..RequestBudget::DISPATCH
    }
}
const TEST_DRIP_INTERVAL: Duration = Duration::from_millis(20);
const TEST_COMPLETION_BOUND: Duration = Duration::from_millis(500);
const TEST_SERVER_HOLD_OPEN: Duration = Duration::from_millis(200);

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

fn 테스트_요청(endpoint: &str) -> Request<'_> {
    Request {
        method: "POST",
        full_endpoint: endpoint,
        token: None,
        extra_headers: &[],
        body: "{}",
    }
}

#[test]
fn slow_drip_응답은_절대_deadline에서_시간초과한다() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = Arc::clone(&stop);
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        테스트_요청을_끝까지_읽는다(&mut stream);
        stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n").unwrap();
        for _ in 0..TEST_DRIP_BODY_BYTES {
            if server_stop.load(Ordering::Relaxed) || stream.write_all(b"x").is_err() {
                break;
            }
            std::thread::sleep(TEST_DRIP_INTERVAL);
        }
    });
    let endpoint = format!("http://{address}/slow-drip");

    // When
    let started = Instant::now();
    let result = execute(
        테스트_요청(&endpoint),
        ResponseLimits::for_untrusted_output().unwrap(),
        테스트_예산(),
    );
    let elapsed = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();

    // Then
    let error = result.expect_err("slow drip 응답은 절대 deadline에서 실패해야 한다");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(elapsed < TEST_COMPLETION_BOUND, "elapsed={elapsed:?}");
}

#[test]
fn content_length_본문이_완성되면_eof를_기다리지_않는다() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release_tx, release_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        테스트_요청을_끝까지_읽는다(&mut stream);
        stream
            .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
        let _ = release_rx.recv_timeout(TEST_SERVER_HOLD_OPEN);
    });
    let endpoint = format!("http://{address}/content-length");

    // When
    let started = Instant::now();
    let result = execute(
        테스트_요청(&endpoint),
        ResponseLimits::for_untrusted_output().unwrap(),
        테스트_예산(),
    );
    let elapsed = started.elapsed();
    let _ = release_tx.send(());
    server.join().unwrap();

    // Then
    assert_eq!(result.unwrap(), (200, "{}".to_string()));
    assert!(elapsed < TEST_COMPLETION_BOUND, "elapsed={elapsed:?}");
}
