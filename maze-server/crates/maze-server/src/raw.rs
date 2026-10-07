//! Hand-rolled HTTP/1.1 server, enabled with `--features raw`.
//!
//! One tokio task per connection with a minimal request parser and response
//! serializer. This keeps hyper out of the picture (no header parsing
//! machinery, no dispatcher) while still riding IOCP, which measured
//! dramatically faster than blocking `std::net` reads: a blocked thread has
//! to be scheduled back in for every request, whereas an overlapped read is
//! completed by the kernel and picked up by an already-running worker.
//!
//! The router is shared with the hyper backend through `route_ctx`, so
//! responses are byte-identical; only the parse/socket layers differ.
//!
//! Parser scope (documented limits, not oversights): HTTP/1.x only, no
//! chunked request bodies (rejected with 400), no TLS, no timeouts.
//! Pipelined bytes left after one request are handled on the next iteration.

use std::sync::RwLock;
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::router::{route_ctx, RequestCtx, Resp};

const MAX_HEAD: usize = 16 * 1024;
const MAX_TARGET: usize = 2048;
const MAX_COOKIE: usize = 4096;
const MAX_BODY: usize = 1024 * 1024;

/// Cached RFC 9110 `Date` header value (no field name), refreshed 2x/sec.
static DATE: RwLock<Vec<u8>> = RwLock::new(Vec::new());

fn start_date_thread() {
    // Seed synchronously so the first request already has a valid date.
    *DATE.write().unwrap() = httpdate::fmt_http_date(SystemTime::now()).into_bytes();
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::thread::Builder::new()
            .name("maze-date".to_string())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_millis(500));
                *DATE.write().unwrap() =
                    httpdate::fmt_http_date(SystemTime::now()).into_bytes();
            })
            .expect("spawn date thread");
    });
}

/// Accept loop: one task per connection. Task spawn cost is amortized to
/// nothing over keep-alive connections.
pub async fn serve(listener: TcpListener) {
    start_date_thread();
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        tokio::spawn(handle_conn(stream));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Get,
    Head,
    Other,
}

/// A parsed request with owned copies of the small byte strings the router
/// needs (target, cookie value). Copying ~a few hundred bytes per request is
/// far cheaper than fighting borrowck over the read buffer.
struct Req {
    method: Method,
    path: [u8; MAX_TARGET],
    path_len: usize,
    cookie: [u8; MAX_COOKIE],
    cookie_len: Option<usize>,
    content_length: usize,
    /// HTTP/1.0 defaults to close unless the client asks for keep-alive.
    close: bool,
}

impl Req {
    fn new() -> Self {
        Req {
            method: Method::Other,
            path: [0u8; MAX_TARGET],
            path_len: 0,
            cookie: [0u8; MAX_COOKIE],
            cookie_len: None,
            content_length: 0,
            close: false,
        }
    }

    fn method_bytes(&self) -> &'static [u8] {
        match self.method {
            Method::Get => b"GET",
            // HEAD is routed as non-GET (405) exactly like the hyper backend;
            // the serializer only uses it to omit the body.
            Method::Head | Method::Other => b"POST",
        }
    }

    fn path_slice(&self) -> &[u8] {
        &self.path[..self.path_len]
    }

    fn cookie_slice(&self) -> Option<&[u8]> {
        self.cookie_len.map(|n| &self.cookie[..n])
    }
}

// Req is intentionally a stack-resident value (~12 KB) to keep the request
// path allocation-light; boxing it would trade cache locality for a heap
// allocation on every request.
#[allow(clippy::large_enum_variant)]
enum Parsed {
    /// One request plus the number of bytes consumed from the buffer.
    Ok(Req, usize),
    /// Need more bytes to complete the head.
    Incomplete,
    /// Malformed beyond repair: answer and close.
    Fatal(u16, &'static [u8]),
}

fn parse_request(buf: &[u8]) -> Parsed {
    let head_end = match buf.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(p) => p,
        None => {
            return if buf.len() > MAX_HEAD {
                Parsed::Fatal(431, b"headers too large")
            } else {
                Parsed::Incomplete
            };
        }
    };
    let head = &buf[..head_end];
    let mut lines = head.split(|&b| b == b'\n');

    // Request line: METHOD SP TARGET SP VERSION
    let request_line = strip_cr(lines.next().unwrap_or(&[]));
    let mut fields = request_line.splitn(3, |&b| b == b' ');
    let method_raw = fields.next().unwrap_or(&[]);
    let target = fields.next().unwrap_or(&[]);
    let version = fields.next().unwrap_or(&[]);
    if method_raw.is_empty() || target.is_empty() || !version.starts_with(b"HTTP/1.") {
        return Parsed::Fatal(400, b"bad request line");
    }
    if target.len() > MAX_TARGET {
        return Parsed::Fatal(414, b"uri too long");
    }
    let mut req = Req::new();
    req.method = match method_raw {
        b"GET" => Method::Get,
        b"HEAD" => Method::Head,
        _ => Method::Other,
    };
    req.close = version == b"HTTP/1.0";
    match target.iter().position(|&b| b == b'?') {
        Some(q) => {
            // Routes match on the path alone; any query bytes are ignored
            // (the maze protocol carries all state in the `path` cookie).
            req.path[..q].copy_from_slice(&target[..q]);
            req.path_len = q;
        }
        None => {
            req.path[..target.len()].copy_from_slice(target);
            req.path_len = target.len();
        }
    }

    for line in lines {
        let line = strip_cr(line);
        if line.is_empty() {
            continue;
        }
        let colon = match line.iter().position(|&b| b == b':') {
            Some(c) => c,
            None => return Parsed::Fatal(400, b"bad header line"),
        };
        let name = line[..colon].trim_ascii();
        let value = line[colon + 1..].trim_ascii();
        if name.eq_ignore_ascii_case(b"cookie") {
            if req.cookie_len.is_none() {
                if let Some(v) = crate::router::cookie_value_in(value) {
                    let n = v.len().min(MAX_COOKIE);
                    req.cookie[..n].copy_from_slice(&v[..n]);
                    req.cookie_len = Some(n);
                }
            }
        } else if name.eq_ignore_ascii_case(b"connection") {
            let v = value.to_ascii_lowercase();
            if v.windows(5).any(|w| w == b"close") {
                req.close = true;
            } else if v.windows(9).any(|w| w == b"keep-alive") {
                req.close = false;
            }
        } else if name.eq_ignore_ascii_case(b"transfer-encoding") {
            // Not supported on requests; rejecting keeps the stream framing
            // unambiguous.
            return Parsed::Fatal(400, b"transfer-encoding not supported");
        } else if name.eq_ignore_ascii_case(b"content-length") {
            match std::str::from_utf8(value)
                .ok()
                .and_then(|s| s.trim().parse::<usize>().ok())
            {
                Some(v) if v <= MAX_BODY => req.content_length = req.content_length.max(v),
                Some(_) => return Parsed::Fatal(413, b"request body too large"),
                None => return Parsed::Fatal(400, b"bad content-length"),
            }
        }
    }
    Parsed::Ok(req, head_end + 4)
}

async fn handle_conn(mut s: TcpStream) {
    let mut carry: Vec<u8> = Vec::with_capacity(8192);
    let mut out: Vec<u8> = Vec::with_capacity(4096);
    let mut tmp = [0u8; 16384];
    loop {
        match parse_request(&carry) {
            Parsed::Incomplete => match s.read(&mut tmp).await {
                Ok(0) => return, // client closed between requests
                Ok(n) => {
                    carry.extend_from_slice(&tmp[..n]);
                    if carry.len() > MAX_HEAD {
                        let _ = write_simple(&mut s, 431, b"headers too large").await;
                        return;
                    }
                }
                Err(_) => return,
            },
            Parsed::Fatal(code, body) => {
                let _ = write_simple(&mut s, code, body).await;
                return;
            }
            Parsed::Ok(req, consumed) => {
                carry.drain(..consumed);
                // Discard any request body so the keep-alive stream stays in
                // sync for the next request.
                if req.content_length > 0
                    && !consume_body(&mut s, &mut carry, req.content_length, &mut tmp).await
                {
                    return;
                }
                let ctx = RequestCtx {
                    method: req.method_bytes(),
                    path: req.path_slice(),
                    cookie: req.cookie_slice(),
                };
                let resp = route_ctx(&ctx);
                let close = req.close;
                let head_only = req.method == Method::Head;
                match write_response(&mut s, resp, head_only, close, &mut out).await {
                    Ok(()) if !close => {}
                    _ => return,
                }
            }
        }
    }
}

async fn consume_body(
    s: &mut TcpStream,
    carry: &mut Vec<u8>,
    mut need: usize,
    tmp: &mut [u8],
) -> bool {
    while need > 0 {
        if !carry.is_empty() {
            let take = need.min(carry.len());
            carry.drain(..take);
            need -= take;
        } else {
            match s.read(tmp).await {
                Ok(0) => return false,
                Ok(r) => need -= r,
                Err(_) => return false,
            }
        }
    }
    true
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        431 => "Request Header Fields Too Large",
        _ => "OK",
    }
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    push_usize(out, v as usize)
}

/// Writes `v` in decimal. Takes a `usize` on purpose: content lengths must
/// not truncate (the embedded assets are ~0.5 MB each).
fn push_usize(out: &mut Vec<u8>, v: usize) {
    if v == 0 {
        out.push(b'0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut i = 0usize;
    let mut v = v;
    while v > 0 {
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        out.push(digits[i]);
    }
}

/// Serializes the router's response into the connection's reused `out`
/// buffer and writes it in one syscall. The body bytes come straight out of
/// the `Full<Bytes>` body without polling.
async fn write_response(
    s: &mut TcpStream,
    resp: Resp,
    head_only: bool,
    close: bool,
    out: &mut Vec<u8>,
) -> std::io::Result<()> {
    let (parts, body) = resp.into_parts();
    // Our bodies are never polled before serialization, so this is always
    // Some; default (empty) keeps the code total even if that ever changes.
    let body = body.into_inner().unwrap_or_default();
    let code = u16::from(parts.status);
    out.clear();
    if out.capacity() < 256 + body.len() {
        out.reserve(256 + body.len());
    }
    out.extend_from_slice(b"HTTP/1.1 ");
    push_u16(out, code);
    out.push(b' ');
    out.extend_from_slice(reason(code).as_bytes());
    for (name, value) in parts.headers.iter() {
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(name.as_str().as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(value.as_bytes());
    }
    out.extend_from_slice(b"\r\ncontent-length: ");
    push_usize(out, body.len());
    out.extend_from_slice(b"\r\ndate: ");
    out.extend_from_slice(&DATE.read().unwrap());
    if close {
        out.extend_from_slice(b"\r\nconnection: close");
    }
    out.extend_from_slice(b"\r\n\r\n");
    if !head_only {
        out.extend_from_slice(&body);
    }
    s.write_all(&out).await
}

/// Minimal error response for parser-level failures.
async fn write_simple(s: &mut TcpStream, code: u16, body: &[u8]) -> std::io::Result<()> {
    let mut out: Vec<u8> = Vec::with_capacity(160 + body.len());
    out.extend_from_slice(b"HTTP/1.1 ");
    push_u16(&mut out, code);
    out.push(b' ');
    out.extend_from_slice(reason(code).as_bytes());
    out.extend_from_slice(b"\r\ncontent-type: text/plain\r\ncontent-length: ");
    push_usize(&mut out, body.len());
    out.extend_from_slice(b"\r\ndate: ");
    out.extend_from_slice(&DATE.read().unwrap());
    out.extend_from_slice(b"\r\nconnection: close\r\n\r\n");
    out.extend_from_slice(body);
    s.write_all(&out).await
}

fn strip_cr(line: &[u8]) -> &[u8] {
    match line.split_last() {
        Some((&b'\r', rest)) => rest,
        _ => line,
    }
}