use std::time::Duration;

/// Shared connect/read/write deadline for both transports. Explicit so the
/// plain-HTTP TCP client and the TLS client agree on one timeout.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// Redirect ceiling for the TLS client. The plain-HTTP TCP client does not
/// follow redirects at all; the TLS client follows a bounded few so real
/// HTTPS APIs that 30x between hosts resolve without unbounded chasing.
const HTTPS_MAX_REDIRECTS: u32 = 5;

const MOCK_ECHO_PREFIX: &str = "mock://echo";
const HTTP_SCHEME_PREFIX: &str = "http://";
const HTTPS_SCHEME_PREFIX: &str = "https://";

/// Transport [`run_http_request`] routes a URL to. `mock://echo` is an
/// in-process echo used by tests; `https://` goes through the TLS client
/// (`ureq` + rustls); everything else uses the hand-rolled plain-HTTP client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestScheme {
    MockEcho,
    Http,
    Https,
}

fn classify_scheme(url: &str) -> RequestScheme {
    if url.starts_with(MOCK_ECHO_PREFIX) {
        RequestScheme::MockEcho
    } else if url.starts_with(HTTPS_SCHEME_PREFIX) {
        RequestScheme::Https
    } else {
        RequestScheme::Http
    }
}

pub(super) fn run_http_request(
    method: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<String, String> {
    validate_request_parts(method, url, &headers)?;
    match classify_scheme(url) {
        RequestScheme::MockEcho => {
            let rest = url.strip_prefix(MOCK_ECHO_PREFIX).unwrap_or_default();
            Ok(body.unwrap_or_else(|| rest.trim_start_matches('/').to_string()))
        }
        RequestScheme::Https => run_https_request(method, url, headers, body),
        RequestScheme::Http => run_plain_http_request(method, url, headers, body),
    }
}

/// Shared, lazily-built TLS-capable agent (rustls backend). Explicit timeout
/// and redirect budget; status codes are surfaced as responses (not errors)
/// so this module keeps its own 2xx check and error snippet for parity with
/// the plain-HTTP path.
fn https_agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(HTTP_TIMEOUT))
                .max_redirects(HTTPS_MAX_REDIRECTS)
                .http_status_as_error(false)
                .build(),
        )
    })
}

fn run_https_request(
    method: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<String, String> {
    let http_method = ::http::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("invalid HTTP method `{method}`"))?;
    let mut builder = ::http::Request::builder().method(http_method).uri(url);
    for (name, value) in &headers {
        if !name.is_empty() {
            builder = builder.header(name.as_str(), value.as_str());
        }
    }
    let request = builder
        .body(body.unwrap_or_default())
        .map_err(|error| format!("https build request: {error}"))?;
    let mut response = https_agent()
        .run(request)
        .map_err(|error| format!("https request failed: {error}"))?;
    let status = response.status().as_u16();
    let body_text = response
        .body_mut()
        .with_config()
        .limit(MAX_HTTP_RESPONSE_BYTES as u64)
        .read_to_string()
        .map_err(|error| format!("https read: {error}"))?;
    if (200..300).contains(&status) {
        Ok(body_text)
    } else {
        let snippet = response_snippet(&body_text);
        Err(format!(
            "https request failed: HTTP {status}{}",
            if snippet.is_empty() {
                String::new()
            } else {
                format!(" — body: {snippet}")
            }
        ))
    }
}

fn run_plain_http_request(
    method: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::io::{Read, Write};
    let parsed = parse_http_url(url)?;
    reject_crlf("HTTP host", &parsed.authority)?;
    reject_crlf("HTTP request target", &parsed.path)?;
    let body = body.unwrap_or_default();
    let mut stream = connect_with_timeout(&parsed.host, parsed.port)?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| format!("http set read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| format!("http set write timeout: {e}"))?;
    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        parsed.path, parsed.authority
    );
    for (name, value) in headers {
        if !name.is_empty() {
            let _ = write!(request, "{name}: {value}\r\n");
        }
    }
    if !body.is_empty() {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    request.push_str(&body);
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("http write: {e}"))?;
    let mut response = String::new();
    let mut limited = stream.take(MAX_HTTP_RESPONSE_BYTES as u64 + 1);
    limited
        .read_to_string(&mut response)
        .map_err(|e| format!("http read: {e}"))?;
    if response.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(format!(
            "http response exceeded {MAX_HTTP_RESPONSE_BYTES} bytes"
        ));
    }
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "http response missing header/body separator".to_string())?;
    let status_ok = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .is_some_and(|code| (200..300).contains(&code));
    if status_ok {
        Ok(body.to_string())
    } else {
        let snippet = response_snippet(body);
        Err(format!(
            "http request failed: {}{}",
            head.lines().next().unwrap_or(head),
            if snippet.is_empty() {
                String::new()
            } else {
                format!(" — body: {snippet}")
            }
        ))
    }
}

const MAX_HTTP_RESPONSE_BYTES: usize = 1024 * 1024;

fn validate_request_parts(
    method: &str,
    url: &str,
    headers: &[(String, String)],
) -> Result<(), String> {
    reject_crlf("HTTP method", method)?;
    method
        .parse::<::http::Method>()
        .map_err(|_| format!("invalid HTTP method `{method}`"))?;
    reject_crlf("HTTP URL", url)?;

    for (name, value) in headers {
        if name.is_empty() {
            continue;
        }
        reject_crlf("HTTP header name", name)?;
        name.parse::<::http::header::HeaderName>()
            .map_err(|_| format!("invalid HTTP header name `{name}`"))?;
        reject_crlf("HTTP header value", value)?;
        value
            .parse::<::http::HeaderValue>()
            .map_err(|_| format!("invalid HTTP header value for `{name}`"))?;
    }
    Ok(())
}

fn reject_crlf(field: &str, value: &str) -> Result<(), String> {
    if value.bytes().any(|b| matches!(b, b'\r' | b'\n')) {
        return Err(format!("{field} must not contain CR/LF"));
    }
    Ok(())
}

#[derive(Debug)]
struct ParsedHttpUrl {
    host: String,
    port: u16,
    path: String,
    authority: String,
}

fn parse_http_url(url: &str) -> Result<ParsedHttpUrl, String> {
    // https:// is routed to the TLS client in `run_http_request` before this
    // parser is reached, so this hand-rolled parser only handles plain http://.
    let rest = url.strip_prefix(HTTP_SCHEME_PREFIX).ok_or_else(|| {
        "only http:// URLs are handled by the plain-HTTP client (https:// uses the TLS client)"
            .to_string()
    })?;
    let split_at = match (rest.find('/'), rest.find('?')) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    };
    let (host_port, target) = split_at.map_or((rest, ""), |idx| rest.split_at(idx));
    let authority = host_port.to_string();
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|_| format!("invalid HTTP port `{port}`"))?,
        ),
        None => (host_port.to_string(), 80),
    };
    if host.trim().is_empty() {
        return Err("http URL host is empty".into());
    }
    if target.contains('#') {
        return Err("http URL fragments are client-side only and are not sent in requests".into());
    }
    let path = if target.is_empty() {
        "/".to_string()
    } else if target.starts_with('?') {
        format!("/{target}")
    } else {
        target.to_string()
    };
    Ok(ParsedHttpUrl {
        host,
        port,
        path,
        authority,
    })
}

fn connect_with_timeout(host: &str, port: u16) -> Result<std::net::TcpStream, String> {
    use std::net::{TcpStream, ToSocketAddrs};
    let timeout = HTTP_TIMEOUT;
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("http resolve `{host}`:{port}: {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("http resolve `{host}`:{port}: no socket addresses"));
    }
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = Some(err),
        }
    }
    Err(format!(
        "http connect `{host}`:{port}: {}",
        last_err.map_or_else(|| "no address attempted".to_string(), |err| err.to_string())
    ))
}

fn response_snippet(body: &str) -> String {
    let trimmed = body.trim();
    let mut snippet: String = trimmed.chars().take(200).collect();
    if trimmed.chars().count() > 200 {
        snippet.push('…');
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_url_parse_preserves_query_only_target() {
        let parsed = parse_http_url("http://example.test?x=1").expect("parse");
        assert_eq!(parsed.host, "example.test");
        assert_eq!(parsed.port, 80);
        assert_eq!(parsed.path, "/?x=1");
    }

    #[test]
    fn http_request_includes_non_default_port_in_host_header() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead as _, Write as _};

            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
            let mut request = String::new();
            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).expect("read request line");
                if bytes == 0 || line == "\r\n" {
                    break;
                }
                request.push_str(&line);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .expect("write response");
            request
        });

        let out = run_http_request("GET", &format!("http://{addr}/bridge"), vec![], None)
            .expect("request succeeds");
        assert_eq!(out, "ok");
        let request = handle.join().expect("server thread");
        assert!(
            request.contains(&format!("Host: {addr}\r\n")),
            "request must include full host authority, got:\n{request}"
        );
    }

    #[test]
    fn classify_scheme_selects_transport_per_scheme() {
        // E8: https:// used to be pinned as "not supported" and rejected
        // before any transport ran. It is now accepted and routed to the
        // TLS client; plain http:// keeps the hand-rolled TCP client and
        // mock://echo stays the in-process echo. This replaces the
        // https-rejection pinned test with the stronger positive contract
        // that each scheme selects the right transport (no network needed).
        assert_eq!(
            classify_scheme("https://example.test/api"),
            RequestScheme::Https
        );
        assert_eq!(
            classify_scheme("http://example.test/api"),
            RequestScheme::Http
        );
        assert_eq!(classify_scheme("mock://echo/foo"), RequestScheme::MockEcho);
    }

    #[test]
    fn http_url_parse_rejects_fragments() {
        let err = parse_http_url("http://example.test/path#frag").expect_err("fragment rejected");
        assert!(err.contains("fragments"), "{err}");
        assert!(err.contains("not sent in requests"), "{err}");
    }

    #[test]
    fn http_request_rejects_newline_in_method() {
        let err = run_http_request("GET\r\nPOST", "mock://echo", vec![], None)
            .expect_err("method CRLF rejected");
        assert!(err.contains("HTTP method"), "{err}");
        assert!(err.contains("CR/LF"), "{err}");
    }

    #[test]
    fn http_request_rejects_newline_in_url() {
        let err = run_http_request(
            "GET",
            "http://example.test/path\r\nInjected: yes",
            vec![],
            None,
        )
        .expect_err("URL CRLF rejected");
        assert!(err.contains("HTTP URL"), "{err}");
        assert!(err.contains("CR/LF"), "{err}");
    }

    #[test]
    fn http_request_rejects_newline_in_header_name() {
        let err = run_http_request(
            "GET",
            "mock://echo",
            vec![("X-Test\r\nInjected".into(), "ok".into())],
            None,
        )
        .expect_err("header name CRLF rejected");
        assert!(err.contains("HTTP header name"), "{err}");
        assert!(err.contains("CR/LF"), "{err}");
    }

    #[test]
    fn http_request_rejects_newline_in_header_value() {
        let err = run_http_request(
            "GET",
            "mock://echo",
            vec![("X-Test".into(), "ok\r\nInjected: yes".into())],
            None,
        )
        .expect_err("header value CRLF rejected");
        assert!(err.contains("HTTP header value"), "{err}");
        assert!(err.contains("CR/LF"), "{err}");
    }

    #[test]
    fn response_snippet_truncates_long_body() {
        let long = "x".repeat(220);
        let snippet = response_snippet(&long);
        assert_eq!(snippet.chars().count(), 201);
        assert!(snippet.ends_with('…'));
    }
}
