//! Bounded synchronous HTTP transport for host attachment.
//!
//! The host is authenticated and local, but its response remains an
//! untrusted wire value. This module owns framing and allocation limits;
//! callers only receive a parsed status and UTF-8 body.

use std::fmt::Write as _;
use std::io::{ErrorKind, Read, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::super::discovery::{
    DiscoveredHost, HEALTH_CONNECT_TIMEOUT_MS, HEALTH_READ_TIMEOUT_MS, parse_endpoint,
    socket_addr_str,
};

mod http_response;

use http_response::{
    HTTP_HEADER_BODY_SEPARATOR, MAX_ATTACH_HTTP_HEADER_BYTES, ResponseFraming, ResponseLimits,
    inspect_response_framing, parse_http_response, parse_response_head,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const RESPONSE_DEADLINE: Duration = Duration::from_secs(30);
const RESPONSE_READ_CHUNK_BYTES: usize = 8 * 1024;
const INITIAL_RESPONSE_CAPACITY_BYTES: usize = 2 * 1024;
const INITIAL_REQUEST_CAPACITY_BYTES: usize = 256;

/// How long one attach request may spend: connecting, then receiving
/// the whole response (the write share of the budget rides along with
/// the response half — the request bodies here are a few hundred
/// bytes).
///
/// Two budgets, because two kinds of route live behind this transport
/// and they have opposite failure meanings. A slow tool call is a tool
/// doing work; a slow `/healthz` is a host that is not answering. One
/// shared 30s number made the second wait a hundred times longer than
/// the liveness probe that sits right next to it in `discovery`.
#[derive(Clone, Copy)]
pub(super) struct RequestBudget {
    connect: Duration,
    response: Duration,
}

impl RequestBudget {
    /// A route that runs something: `POST /v1/tools/{id}`, `/mcp`.
    pub(super) const DISPATCH: Self = Self {
        connect: CONNECT_TIMEOUT,
        response: RESPONSE_DEADLINE,
    };

    /// A route that only reports: `/healthz`. Same numbers
    /// `discovery`'s status-line probe of the very same route uses, so
    /// "reachable" means one thing across this crate.
    pub(super) const HEALTH_PROBE: Self = Self {
        connect: Duration::from_millis(HEALTH_CONNECT_TIMEOUT_MS),
        response: Duration::from_millis(HEALTH_READ_TIMEOUT_MS),
    };
}

struct Request<'a> {
    method: &'a str,
    full_endpoint: &'a str,
    token: Option<&'a str>,
    extra_headers: &'a [(&'a str, &'a str)],
    body: &'a str,
    discovered: Option<&'a DiscoveredHost>,
}

#[cfg(test)]
pub(super) fn request_json(
    method: &str,
    full_endpoint: &str,
    token: Option<&str>,
    body: &str,
) -> std::io::Result<(u16, String)> {
    request(method, full_endpoint, token, &[], body)
}

#[cfg(test)]
pub(super) fn post_json(
    full_endpoint: &str,
    token: Option<&str>,
    body: &str,
) -> std::io::Result<(u16, String)> {
    request_json("POST", full_endpoint, token, body)
}

#[cfg(test)]
pub(super) fn request(
    method: &str,
    full_endpoint: &str,
    token: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: &str,
) -> std::io::Result<(u16, String)> {
    request_within(
        method,
        full_endpoint,
        token,
        extra_headers,
        body,
        RequestBudget::DISPATCH,
    )
}

/// [`request`] with an explicit [`RequestBudget`], for callers whose
/// route is a probe rather than something that runs.
pub(super) fn request_within(
    method: &str,
    full_endpoint: &str,
    token: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: &str,
    budget: RequestBudget,
) -> std::io::Result<(u16, String)> {
    execute(
        Request {
            method,
            full_endpoint,
            token,
            extra_headers,
            body,
            discovered: None,
        },
        ResponseLimits::for_untrusted_output()?,
        budget,
    )
}

pub(super) fn request_discovered(
    method: &str,
    full_endpoint: &str,
    host: &DiscoveredHost,
    extra_headers: &[(&str, &str)],
    body: &str,
) -> std::io::Result<(u16, String)> {
    execute(
        Request {
            method,
            full_endpoint,
            token: Some(&host.token),
            extra_headers,
            body,
            discovered: Some(host),
        },
        ResponseLimits::for_untrusted_output()?,
        RequestBudget::DISPATCH,
    )
}

fn execute(
    request: Request<'_>,
    limits: ResponseLimits,
    budget: RequestBudget,
) -> std::io::Result<(u16, String)> {
    let mut stream = connect_and_send(&request, budget)?;
    read_and_parse_response(&mut stream, limits, budget.response)
}

/// Connect, write the request, and hand back the live socket.
///
/// Shared by the buffered path above and the streaming one
/// ([`open_stream`]): the request wire shape must not fork, or a
/// streamed call would arrive at the host looking like a different
/// client than a plain one.
fn connect_and_send(request: &Request<'_>, budget: RequestBudget) -> std::io::Result<TcpStream> {
    let (host, port, path) = parse_endpoint(request.full_endpoint).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("attach: unparsable endpoint `{}`", request.full_endpoint),
        )
    })?;
    let request_path = if path.is_empty() { "/".into() } else { path };
    let authority = socket_addr_str(&host, port);
    let addr = authority
        .parse()
        .map_err(|error| std::io::Error::new(ErrorKind::InvalidInput, format!("addr: {error}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, budget.connect)?;
    stream.set_write_timeout(Some(budget.response))?;
    if let Some(discovered) = request.discovered {
        discovered.verify()?;
    }

    let mut wire =
        String::with_capacity(INITIAL_REQUEST_CAPACITY_BYTES.saturating_add(request.body.len()));
    let _ = write!(wire, "{} {request_path} HTTP/1.0\r\n", request.method);
    let _ = write!(wire, "Host: {authority}\r\n");
    let _ = write!(wire, "Content-Type: application/json\r\n");
    let _ = write!(wire, "Content-Length: {}\r\n", request.body.len());
    if let Some(token) = request.token {
        let _ = write!(wire, "Authorization: Bearer {token}\r\n");
    }
    for (name, value) in request.extra_headers {
        let _ = write!(wire, "{name}: {value}\r\n");
    }
    let _ = write!(wire, "Connection: close\r\n\r\n");
    stream.write_all(wire.as_bytes())?;
    stream.write_all(request.body.as_bytes())?;
    stream.flush()?;
    Ok(stream)
}

/// A response whose head has been read and whose body is still arriving.
///
/// The buffered transport reads to the end before answering, which is
/// the wrong shape for a route whose whole point is that the body
/// trickles in over minutes. This is the same request, stopped one step
/// earlier: status in hand, socket still open.
pub(super) struct OpenStream {
    /// Status line of the response.
    pub(super) status: u16,
    /// Body bytes that arrived in the same read as the head.
    pub(super) prefix: Vec<u8>,
    stream: TcpStream,
}

impl OpenStream {
    /// Read more body bytes, waiting at most `patience`.
    ///
    /// `Ok(0)` is end of body. A timeout is reported as
    /// [`ErrorKind::WouldBlock`] so the caller can decide what waiting
    /// longer means — for a tool that has printed nothing for a minute,
    /// it means "still running", not "dead".
    pub(super) fn read(&mut self, buffer: &mut [u8], patience: Duration) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(patience))?;
        match self.stream.read(buffer) {
            Ok(read) => Ok(read),
            Err(error) if error.kind() == ErrorKind::TimedOut => {
                Err(std::io::Error::from(ErrorKind::WouldBlock))
            }
            Err(error) => Err(error),
        }
    }

    /// Hang up. The host treats a dropped body as a cancellation, so
    /// this is how an attached surface stops a run it started.
    pub(super) fn abort(self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Open the streaming response after the same discovery recheck used by
/// buffered requests. The caller owns each body byte from this point on.
pub(super) fn open_stream_discovered(
    method: &str,
    full_endpoint: &str,
    host: &DiscoveredHost,
    extra_headers: &[(&str, &str)],
    body: &str,
    budget: RequestBudget,
) -> std::io::Result<OpenStream> {
    let request = Request {
        method,
        full_endpoint,
        token: Some(&host.token),
        extra_headers,
        body,
        discovered: Some(host),
    };
    let mut stream = connect_and_send(&request, budget)?;
    let (status, prefix) = read_response_head(&mut stream, budget.response)?;
    Ok(OpenStream {
        status,
        prefix,
        stream,
    })
}

/// Read until the header/body separator, then stop.
fn read_response_head(
    stream: &mut TcpStream,
    head_deadline: Duration,
) -> std::io::Result<(u16, Vec<u8>)> {
    let deadline = Instant::now()
        .checked_add(head_deadline)
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "attach head deadline"))?;
    let mut raw: Vec<u8> = Vec::with_capacity(INITIAL_RESPONSE_CAPACITY_BYTES);
    let mut chunk = [0_u8; RESPONSE_READ_CHUNK_BYTES];
    loop {
        if let Some(split) = find_head_separator(&raw) {
            let body_start = split.saturating_add(HTTP_HEADER_BODY_SEPARATOR.len());
            let (status, _) = parse_response_head(&raw[..split])?;
            return Ok((status, raw[body_start..].to_vec()));
        }
        if raw.len() > MAX_ATTACH_HTTP_HEADER_BYTES {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "attach response headers exceed the configured limit",
            ));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(response_deadline_error)?;
        stream.set_read_timeout(Some(remaining))?;
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Err(response_deadline_error());
            }
            Err(error) => return Err(error),
        };
        if read == 0 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "attach response ended before its head",
            ));
        }
        raw.try_reserve(read)
            .map_err(|error| std::io::Error::other(format!("attach head allocation: {error}")))?;
        raw.extend_from_slice(&chunk[..read]);
    }
}

fn find_head_separator(raw: &[u8]) -> Option<usize> {
    raw.windows(HTTP_HEADER_BODY_SEPARATOR.len())
        .position(|window| window == HTTP_HEADER_BODY_SEPARATOR)
}

trait ResponseDeadlineReader: Read {
    fn set_remaining_response_timeout(&self, timeout: Duration) -> std::io::Result<()>;
}

impl ResponseDeadlineReader for TcpStream {
    fn set_remaining_response_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }
}

#[cfg(test)]
impl<T> ResponseDeadlineReader for std::io::Cursor<T>
where
    Self: Read,
{
    fn set_remaining_response_timeout(&self, _timeout: Duration) -> std::io::Result<()> {
        Ok(())
    }
}

fn read_and_parse_response(
    reader: &mut impl ResponseDeadlineReader,
    limits: ResponseLimits,
    response_deadline: Duration,
) -> std::io::Result<(u16, String)> {
    let max_response_bytes = limits.response_bytes()?;
    let probe_bytes = max_response_bytes.checked_add(1).ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidInput, "attach response probe overflow")
    })?;
    let mut raw = Vec::with_capacity(INITIAL_RESPONSE_CAPACITY_BYTES.min(probe_bytes));
    let mut chunk = [0_u8; RESPONSE_READ_CHUNK_BYTES];
    let deadline = Instant::now()
        .checked_add(response_deadline)
        .ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "attach response deadline overflow")
        })?;
    let mut framing = None;

    while raw.len() < probe_bytes {
        let remaining_timeout = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(response_deadline_error)?;
        reader.set_remaining_response_timeout(remaining_timeout)?;

        let remaining = probe_bytes - raw.len();
        let read_capacity = remaining.min(chunk.len());
        let read_bytes = match reader.read(&mut chunk[..read_capacity]) {
            Ok(read_bytes) => read_bytes,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Err(response_deadline_error());
            }
            Err(error) => return Err(error),
        };
        if Instant::now() >= deadline {
            return Err(response_deadline_error());
        }
        if read_bytes == 0 {
            break;
        }
        raw.try_reserve(read_bytes).map_err(|error| {
            std::io::Error::other(format!("attach response allocation: {error}"))
        })?;
        raw.extend_from_slice(&chunk[..read_bytes]);
        if raw.len() > max_response_bytes {
            return Err(framed_wire_limit_error(max_response_bytes));
        }

        if framing.is_none() {
            framing = inspect_response_framing(&raw, limits)?;
        }
        if let Some(ResponseFraming::ContentLength { response_bytes }) = framing {
            if raw.len() > response_bytes {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "attach response body exceeds Content-Length",
                ));
            }
            if raw.len() == response_bytes {
                return parse_http_response(raw, limits);
            }
        }
    }

    parse_http_response(raw, limits)
}

fn framed_wire_limit_error(max_response_bytes: usize) -> std::io::Error {
    std::io::Error::new(
        ErrorKind::InvalidData,
        format!("attach response exceeds the {max_response_bytes}-byte framed wire limit"),
    )
}

fn response_deadline_error() -> std::io::Error {
    std::io::Error::new(
        ErrorKind::TimedOut,
        "attach response exceeded the absolute deadline",
    )
}

#[cfg(test)]
#[path = "http_transport_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "http_transport_deadline_tests.rs"]
mod deadline_tests;
