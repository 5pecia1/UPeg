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
fn test_budget() -> RequestBudget {
    RequestBudget {
        response: TEST_RESPONSE_DEADLINE,
        ..RequestBudget::DISPATCH
    }
}
const TEST_DRIP_INTERVAL: Duration = Duration::from_millis(20);
const TEST_COMPLETION_BOUND: Duration = Duration::from_millis(500);
const TEST_SERVER_HOLD_OPEN: Duration = Duration::from_millis(200);

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

fn test_request(endpoint: &str) -> Request<'_> {
    Request {
        method: "POST",
        full_endpoint: endpoint,
        token: None,
        extra_headers: &[],
        body: "{}",
        discovered: None,
    }
}

#[test]
fn slow_drip_response_times_out_at_the_absolute_deadline() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = Arc::clone(&stop);
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_test_request_to_end(&mut stream);
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
        test_request(&endpoint),
        ResponseLimits::for_untrusted_output().unwrap(),
        test_budget(),
    );
    let elapsed = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();

    // Then
    let error = result.expect_err("a slow drip response must fail at the absolute deadline");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(elapsed < TEST_COMPLETION_BOUND, "elapsed={elapsed:?}");
}

#[test]
fn completed_content_length_body_does_not_wait_for_eof() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release_tx, release_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_test_request_to_end(&mut stream);
        stream
            .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
        let _ = release_rx.recv_timeout(TEST_SERVER_HOLD_OPEN);
    });
    let endpoint = format!("http://{address}/content-length");

    // When
    let started = Instant::now();
    let result = execute(
        test_request(&endpoint),
        ResponseLimits::for_untrusted_output().unwrap(),
        test_budget(),
    );
    let elapsed = started.elapsed();
    let _ = release_tx.send(());
    server.join().unwrap();

    // Then
    assert_eq!(result.unwrap(), (200, "{}".to_string()));
    assert!(elapsed < TEST_COMPLETION_BOUND, "elapsed={elapsed:?}");
}
