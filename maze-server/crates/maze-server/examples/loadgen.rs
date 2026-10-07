//! Minimal dependency-free HTTP load generator for the maze server.
//!
//! Each worker holds one keep-alive connection and measures end-to-end
//! latency (write request -> full response read) per request.
//!
//! Usage:
//!   cargo run --release -p maze-server --example loadgen -- <addr> <path> <cookie|-> [workers] [secs]
//!
//! The cookie value is sent raw as `Cookie: path=<value>`; pass "-" for none.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let addr = a.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8091".into());
    let path = a.get(2).cloned().unwrap_or_else(|| "/move".into());
    let cookie = a.get(3).cloned().unwrap_or_else(|| "-".into());
    let workers: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(4);
    let secs: u64 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);

    let mut req = format!("GET {path} HTTP/1.1\r\nHost: bench\r\n");
    if cookie != "-" {
        req.push_str(&format!("Cookie: path={cookie}\r\n"));
    }
    req.push_str("\r\n");
    let req = req.into_bytes();

    println!("loadgen: {workers} workers x {secs}s -> http://{addr}{path} (cookie {cookie})");
    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let req = req.clone();
            let addr = addr.clone();
            thread::spawn(move || worker(addr, req, Duration::from_secs(secs)))
        })
        .collect();
    let mut lats: Vec<u64> = Vec::new();
    let mut total = 0u64;
    for h in handles {
        let (n, lat) = h.join().unwrap();
        total += n;
        lats.extend(lat);
    }
    if lats.is_empty() {
        println!("no successful requests");
        return;
    }
    lats.sort_unstable();
    let pick = |q: f64| {
        lats[((lats.len() as f64 * q) as usize).min(lats.len() - 1)]
    };
    let us = |ns: u64| ns as f64 / 1000.0;
    println!(
        "requests={total} rps={:.0}",
        total as f64 / secs as f64
    );
    println!(
        "latency us: p50={:.1} p90={:.1} p99={:.1} p999={:.1} max={:.1}",
        us(pick(0.50)),
        us(pick(0.90)),
        us(pick(0.99)),
        us(pick(0.999)),
        us(*lats.last().unwrap())
    );
}

fn worker(addr: String, req: Vec<u8>, dur: Duration) -> (u64, Vec<u64>) {
    let deadline = Instant::now() + dur;
    let mut lat = Vec::new();
    let mut n = 0u64;
    let mut stream = match connect(&addr, deadline) {
        Some(s) => s,
        None => return (0, lat),
    };
    let mut carry: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 16384];
    while Instant::now() < deadline {
        let t0 = Instant::now();
        if stream.write_all(&req).is_err() {
            if let Some(s) = connect(&addr, deadline) {
                stream = s;
                continue;
            }
            break;
        }
        match read_response(&mut stream, &mut carry, &mut tmp) {
            Ok(()) => {
                n += 1;
                lat.push(t0.elapsed().as_nanos() as u64);
            }
            Err(_) => match connect(&addr, deadline) {
                Some(s) => {
                    stream = s;
                    carry.clear();
                }
                None => break,
            },
        }
    }
    (n, lat)
}

/// Connects with retries until `deadline`; `None` means the server never came up.
fn connect(addr: &str, deadline: Instant) -> Option<TcpStream> {
    loop {
        if let Ok(s) = TcpStream::connect(addr) {
            let _ = s.set_nodelay(true);
            return Some(s);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Reads one full response (headers + Content-Length body) from the stream,
/// leaving any pipelined leftover bytes in `carry` for the next response.
fn read_response(stream: &mut TcpStream, carry: &mut Vec<u8>, tmp: &mut [u8]) -> std::io::Result<()> {
    let head_end = loop {
        if let Some(p) = carry.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        let r = stream.read(tmp)?;
        if r == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"));
        }
        carry.extend_from_slice(&tmp[..r]);
    };
    let cl = content_length(&carry[..head_end])
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no content-length"))?;
    let total = head_end + 4 + cl;
    while carry.len() < total {
        let r = stream.read(tmp)?;
        if r == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof in body"));
        }
        carry.extend_from_slice(&tmp[..r]);
    }
    carry.drain(..total);
    Ok(())
}

fn content_length(head: &[u8]) -> Option<usize> {
    let i = head
        .windows(15)
        .position(|w| w.eq_ignore_ascii_case(b"content-length:"))?;
    let rest = &head[i + 15..];
    let end = rest.iter().position(|&b| b == b'\r')?;
    std::str::from_utf8(&rest[..end]).ok()?.trim().parse().ok()
}