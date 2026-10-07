//! Wire-level integration tests: a real HTTP server on an ephemeral port and
//! a raw-socket client. This validates the actual protocol bytes against the
//! recreated Daedalus site: status codes, exact `Content-Length` values
//! (1730 / 1744 / 1774), the fresh cookie on `/`, redirects, Flask-shaped
//! error bodies, and the static assets the pages reference.

use maze_core::base64;
use maze_core::maze::{cell_at, H, MAX_COOKIE_LEN, START, W};
use maze_server::bodies;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The known-good shortest solution to the bundled maze (1457 lowercase
/// moves, surveyed from the site), written as (letter, run length) pairs.
fn solution() -> String {
    let mut s = String::new();
    for (c, n) in [
        ('r', 6),
        ('d', 2),
        ('l', 6),
        ('d', 6),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('u', 4),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 4),
        ('r', 2),
        ('u', 4),
        ('r', 6),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('l', 2),
        ('d', 4),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 8),
        ('d', 2),
        ('r', 6),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('r', 6),
        ('u', 2),
        ('r', 4),
        ('d', 4),
        ('l', 2),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 2),
        ('r', 4),
        ('u', 4),
        ('l', 2),
        ('u', 2),
        ('r', 4),
        ('u', 2),
        ('r', 8),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('l', 2),
        ('d', 6),
        ('r', 2),
        ('d', 4),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('l', 4),
        ('d', 4),
        ('l', 2),
        ('d', 4),
        ('l', 2),
        ('d', 2),
        ('l', 6),
        ('d', 6),
        ('r', 2),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('u', 12),
        ('r', 2),
        ('u', 2),
        ('l', 4),
        ('d', 12),
        ('l', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('r', 6),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('l', 4),
        ('d', 2),
        ('l', 4),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 6),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('d', 4),
        ('l', 4),
        ('u', 2),
        ('r', 2),
        ('u', 4),
        ('r', 2),
        ('u', 4),
        ('r', 2),
        ('d', 4),
        ('r', 6),
        ('u', 4),
        ('r', 4),
        ('u', 4),
        ('l', 4),
        ('d', 2),
        ('l', 2),
        ('u', 4),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 8),
        ('u', 2),
        ('l', 10),
        ('d', 2),
        ('l', 6),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('l', 6),
        ('d', 2),
        ('l', 2),
        ('u', 4),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('d', 6),
        ('r', 2),
        ('d', 2),
        ('l', 4),
        ('u', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('d', 4),
        ('l', 2),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 2),
        ('r', 4),
        ('u', 4),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('u', 2),
        ('r', 2),
        ('d', 4),
        ('l', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('l', 2),
        ('d', 8),
        ('l', 4),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('l', 6),
        ('d', 2),
        ('r', 4),
        ('d', 8),
        ('r', 6),
        ('u', 4),
        ('r', 4),
        ('d', 2),
        ('l', 2),
        ('d', 8),
        ('r', 2),
        ('d', 2),
        ('l', 2),
        ('d', 6),
        ('r', 2),
        ('u', 4),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('l', 4),
        ('u', 2),
        ('r', 4),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 8),
        ('u', 2),
        ('r', 2),
        ('u', 4),
        ('l', 4),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 8),
        ('r', 2),
        ('d', 2),
        ('r', 6),
        ('d', 2),
        ('r', 4),
        ('d', 4),
        ('r', 2),
        ('d', 4),
        ('r', 2),
        ('u', 4),
        ('r', 2),
        ('u', 2),
        ('l', 2),
        ('u', 4),
        ('r', 2),
        ('u', 2),
        ('l', 4),
        ('d', 2),
        ('l', 4),
        ('u', 4),
        ('l', 4),
        ('u', 4),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 4),
        ('d', 4),
        ('r', 6),
        ('d', 4),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('r', 6),
        ('d', 2),
        ('l', 2),
        ('d', 8),
        ('r', 2),
        ('d', 2),
        ('l', 4),
        ('u', 2),
        ('l', 2),
        ('d', 6),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('d', 6),
        ('l', 4),
        ('u', 4),
        ('l', 2),
        ('u', 4),
        ('r', 4),
        ('u', 4),
        ('l', 6),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('d', 4),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('d', 6),
        ('r', 2),
        ('d', 2),
        ('l', 4),
        ('d', 2),
        ('r', 2),
        ('d', 10),
        ('l', 8),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('l', 6),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('r', 2),
        ('d', 4),
        ('r', 2),
        ('u', 6),
        ('r', 4),
        ('d', 6),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('l', 2),
        ('d', 4),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('d', 4),
        ('l', 2),
        ('d', 2),
        ('r', 4),
        ('d', 2),
        ('r', 2),
        ('d', 6),
        ('l', 2),
        ('d', 2),
        ('r', 8),
        ('d', 2),
        ('l', 16),
        ('d', 2),
        ('l', 4),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('u', 8),
        ('l', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('l', 4),
        ('d', 2),
        ('r', 2),
        ('d', 2),
        ('l', 6),
        ('u', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('r', 6),
        ('u', 2),
        ('r', 2),
        ('d', 6),
        ('r', 2),
        ('d', 2),
        ('l', 4),
        ('u', 4),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('u', 2),
        ('l', 8),
        ('d', 4),
        ('l', 2),
        ('u', 4),
        ('l', 4),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('r', 2),
        ('d', 6),
        ('l', 8),
        ('u', 4),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('l', 4),
        ('d', 4),
        ('l', 2),
        ('d', 2),
        ('r', 4),
        ('d', 6),
        ('r', 6),
        ('u', 2),
        ('r', 4),
        ('d', 2),
        ('r', 4),
        ('u', 2),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 4),
        ('r', 4),
        ('d', 2),
        ('l', 2),
        ('d', 2),
        ('r', 4),
        ('u', 2),
        ('r', 2),
        ('d', 6),
        ('r', 14),
        ('u', 2),
        ('l', 8),
        ('u', 2),
        ('r', 6),
        ('u', 2),
        ('r', 2),
        ('d', 2),
        ('r', 2),
        ('d', 4),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('u', 4),
        ('l', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 2),
        ('u', 2),
        ('r', 12),
        ('d', 2),
        ('r', 2),
        ('d', 8),
        ('l', 4),
        ('d', 2),
        ('r', 5),
    ] {
        for _ in 0..n {
            s.push(c);
        }
    }
    s
}

fn b64(s: &str) -> String {
    let mut buf = [0u8; MAX_COOKIE_LEN];
    let n = base64::encode_into(s.as_bytes(), &mut buf).unwrap();
    String::from_utf8(buf[..n].to_vec()).unwrap()
}

/// BFS shortest path from the start to `target` (flat cell index), over the
/// public map API. Used to reach the minotaur cell in the wire tests.
fn path_to(target: usize) -> String {
    let mut prev = vec![usize::MAX; W * H];
    let mut dir = vec![0xffu8; W * H];
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(START);
    prev[START] = START;
    while let Some(pos) = queue.pop_front() {
        if pos == target {
            break;
        }
        let (x, y) = (pos % W, pos / W);
        for ((dx, dy), letter) in [
            ((0isize, -1isize), b'u'),
            ((0, 1), b'd'),
            ((-1, 0), b'l'),
            ((1, 0), b'r'),
        ] {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= W as isize || ny >= H as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if cell_at(nx, ny) == 1 {
                continue;
            }
            let n = ny * W + nx;
            if prev[n] == usize::MAX {
                prev[n] = pos;
                dir[n] = letter;
                queue.push_back(n);
            }
        }
    }
    assert_ne!(prev[target], usize::MAX, "target unreachable");
    let mut bytes = Vec::new();
    let mut pos = target;
    while pos != START {
        bytes.push(dir[pos]);
        pos = prev[pos];
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap()
}

struct Wire {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Wire {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
    fn content_length(&self) -> usize {
        self.header("content-length")
            .unwrap_or_else(|| panic!("no content-length; headers: {:?}", self.headers))
            .parse()
            .unwrap()
    }
}

/// Sends one GET (or other method) with `Connection: close` and reads the
/// whole response, which lets us read the body to EOF without framing logic.
async fn request(port: u16, method: &str, target: &str, cookie: Option<&str>) -> Wire {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut req = format!("{method} {target} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n");
    if let Some(c) = cookie {
        req.push_str(&format!("Cookie: path={c}\r\n"));
    }
    req.push_str("\r\n");
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    sock.read_to_end(&mut buf).await.unwrap();
    assert!(!buf.is_empty(), "empty response");
    let sep = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("no header/body separator");
    let head = String::from_utf8_lossy(&buf[..sep]).to_string();
    let body = buf[sep + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status line")
        .parse()
        .expect("status code");
    let mut headers = Vec::new();
    for line in lines {
        if let Some((n, v)) = line.split_once(':') {
            headers.push((n.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    Wire { status, headers, body }
}

async fn get(port: u16, target: &str, cookie: Option<&str>) -> Wire {
    request(port, "GET", target, cookie).await
}

async fn server() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(maze_server::serve(listener));
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn start_page_sets_fresh_cookie() {
    let port = server().await;
    let w = get(port, "/", None).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), w.body.len());
    // exactly the site's fresh cookie: session scope, no Max-Age
    assert_eq!(w.header("set-cookie"), Some("path=; Path=/"));
    let s = String::from_utf8(w.body.clone()).unwrap();
    assert!(s.contains("Tirésias"), "intro message");
    assert!(s.contains("send('u')"), "controls call the JS move helper");
    assert!(s.contains("Je me suis perdu :("), "lost button");
    assert!(s.contains("/static/img/tiresias.jpg"), "start image");
    assert_eq!(w.content_length(), 1843);
}

#[tokio::test(flavor = "multi_thread")]
async fn move_without_any_cookie_redirects() {
    let port = server().await;
    let w = get(port, "/move", None).await;
    assert_eq!(w.status, 302);
    assert_eq!(w.header("location"), Some("/"));
    // the site's Flask redirect body; only `GET /` hands out a fresh cookie
    assert_eq!(w.header("set-cookie"), None);
    assert_eq!(w.content_length(), bodies::redirect_body().len());
    assert!(String::from_utf8_lossy(&w.body).contains("Redirecting..."));
}

#[tokio::test(flavor = "multi_thread")]
async fn move_with_empty_cookie_sits_at_start() {
    // An empty cookie value is a legal empty path: the site answers with the
    // going-well page (the player stands on the start cell).
    let port = server().await;
    let w = get(port, "/move", Some("")).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), 1744);
    let s = String::from_utf8(w.body.clone()).unwrap();
    assert!(s.contains("Ça avance bien!"));
    assert!(w.header("set-cookie").is_none(), "cookie is client-owned");
}

#[tokio::test(flavor = "multi_thread")]
async fn move_chain_reads_cookie_only() {
    // The full path travels in the cookie; the URL carries no parameters and
    // the server never rewrites the cookie.
    let port = server().await;
    let w = get(port, "/move", Some(b64("rr").as_str())).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), 1744);
    assert_eq!(w.header("set-cookie"), None, "the cookie is client-owned");
    // a query string on the target is ignored, not treated as a param
    let w = get(port, "/move?d=d", Some(b64("rr").as_str())).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), 1744);
}

#[tokio::test(flavor = "multi_thread")]
async fn good_and_bad_move_content_lengths_differ() {
    let port = server().await;
    // Good move: one legal right from the start.
    let good = get(port, "/move", Some(b64("r").as_str())).await;
    assert_eq!(good.status, 200);
    assert_eq!(good.content_length(), good.body.len());
    assert_eq!(good.content_length(), bodies::WELL_SIZE);
    // Bad move: one legal up bonks the border wall above the start.
    let bad = get(port, "/move", Some(b64("u").as_str())).await;
    assert_eq!(bad.status, 200);
    assert_eq!(bad.content_length(), bad.body.len());
    assert_eq!(bad.content_length(), bodies::BONK_SIZE);
    assert_ne!(
        good.content_length(),
        bad.content_length(),
        "good and bad moves must not share a Content-Length"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn non_direction_bytes_bonk_like_the_site() {
    let port = server().await;
    // uppercase spellings are not directions: they play as walls
    for cookie in [b64("R").as_str(), b64("rX").as_str(), b64("rrrrX").as_str()] {
        let w = get(port, "/move", Some(cookie)).await;
        assert_eq!(w.status, 200, "cookie {cookie:?}");
        assert_eq!(w.content_length(), 1730, "cookie {cookie:?}");
        let s = String::from_utf8(w.body.clone()).unwrap();
        assert!(s.contains("BONK!"), "cookie {cookie:?}");
    }
    // a bonk earlier in the path wins over later content
    let w = get(port, "/move", Some(b64("uX").as_str())).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), 1730);
}

#[tokio::test(flavor = "multi_thread")]
async fn structurally_broken_base64_is_500() {
    let port = server().await;
    // missing padding: the site's decoder raises -> 500
    for cookie in ["cg", "cg=", "cg==x", "Y=g="] {
        let w = get(port, "/move", Some(cookie)).await;
        assert_eq!(w.status, 500, "cookie {cookie:?}");
        assert_eq!(w.content_length(), bodies::server_error_body().len());
        assert!(String::from_utf8_lossy(&w.body).contains("500 Internal Server Error"));
    }
    // but non-alphabet bytes are merely discarded: "***" -> empty path
    let w = get(port, "/move", Some("***")).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), 1744);
}

#[tokio::test(flavor = "multi_thread")]
async fn overlong_but_valid_cookie_bonks() {
    // A cookie that decodes to more than MAX_PATH_LEN moves can never
    // replay: trace() bonks such a path on the first step. The server must
    // answer with the bonk page, not a 500 (real clients send these once a
    // path outgrows the 4096-char cookie budget). Structurally valid base64
    // of 3073 'd' moves encodes to 4100 chars.
    let port = server().await;
    let mut buf = [0u8; 4200];
    let n = base64::encode_into(&[b'd'; 3073], &mut buf).unwrap();
    let cookie = String::from_utf8(buf[..n].to_vec()).unwrap();
    let w = get(port, "/move", Some(&cookie)).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), bodies::BONK_SIZE);
    assert!(String::from_utf8_lossy(&w.body).contains("BONK!"));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_reset_endpoint() {
    // the site has no /reset; restarting is `GET /` (the lost button)
    let port = server().await;
    let w = get(port, "/reset", None).await;
    assert_eq!(w.status, 404);
    assert_eq!(w.content_length(), bodies::not_found_body().len());
    assert!(String::from_utf8_lossy(&w.body).contains("404 Not Found"));
}

#[tokio::test(flavor = "multi_thread")]
async fn solution_solves_and_gets_flag() {
    let port = server().await;
    let w = get(port, "/move", Some(b64(&solution()).as_str())).await;
    assert_eq!(w.status, 200);
    let s = String::from_utf8(w.body.clone()).unwrap();
    assert!(s.contains(bodies::FLAG_MESSAGE));
    assert!(s.contains("/static/img/success.png"), "success image");
    assert!(!s.contains("Ça avance bien!"), "well message is replaced");
    let len = w.content_length();
    assert_ne!(len, 1730);
    assert_ne!(len, 1744);
    assert_ne!(len, 1774);
    assert_eq!(w.header("set-cookie"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn landing_on_the_minotaur_gets_the_minotaur_page() {
    // The surveyed maze has exactly one minotaur cell; a path ending there
    // is legal and gets the site's minotaur page (1774 bytes), not a bonk.
    let port = server().await;
    let w = get(port, "/move", Some(b64(&path_to(maze_core::maze::MINOTAUR)).as_str())).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.content_length(), bodies::MINOTAUR_SIZE);
    let s = String::from_utf8(w.body.clone()).unwrap();
    assert!(s.contains("le minotaure"), "minotaur message");
    assert!(s.contains("/static/img/nice.jpg"), "nice image");
    assert!(!s.contains("Ça avance bien!"), "well message is replaced");
    assert_eq!(w.header("set-cookie"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn static_assets_are_served() {
    let port = server().await;
    for (target, ctype) in [
        ("/static/css/index.css", "text/css; charset=utf-8"),
        ("/static/js/send.js", "text/javascript; charset=utf-8"),
        ("/static/img/tiresias.jpg", "image/jpeg"),
        ("/static/img/progress.png", "image/png"),
        ("/static/img/bonk.jpeg", "image/jpeg"),
        ("/static/img/success.png", "image/png"),
        ("/static/img/nice.jpg", "image/jpeg"),
    ] {
        let w = get(port, target, None).await;
        assert_eq!(w.status, 200, "target {target:?}");
        assert_eq!(w.header("content-type"), Some(ctype), "target {target:?}");
        assert_eq!(w.header("cache-control"), Some("no-cache"), "target {target:?}");
        assert_eq!(w.content_length(), w.body.len(), "target {target:?}");
        assert!(!w.body.is_empty(), "target {target:?}");
    }
    // unknown static paths are plain 404s
    let w = get(port, "/static/img/nope.png", None).await;
    assert_eq!(w.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn maze_endpoint_renders_svg() {
    let port = server().await;
    let w = get(port, "/maze", None).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.header("content-type"), Some("image/svg+xml"));
    assert_eq!(w.content_length(), w.body.len());
    assert_eq!(w.header("cache-control"), None, "not a static asset");
    assert_eq!(w.header("set-cookie"), None, "no session state involved");
    let s = String::from_utf8(w.body.clone()).unwrap();
    assert!(s.starts_with("<svg "), "SVG root element");
    assert!(s.contains("<polyline"), "A* path overlay is drawn");
    assert!(s.contains("A* optimal path"), "legend");
    // a query string changes nothing
    let w = get(port, "/maze?raw=1", None).await;
    assert_eq!(w.status, 200);
    assert_eq!(w.header("content-type"), Some("image/svg+xml"));
}

#[tokio::test(flavor = "multi_thread")]
async fn non_get_methods_are_405() {
    let port = server().await;
    for target in ["/", "/move", "/maze", "/static/css/index.css"] {
        let w = request(port, "POST", target, None).await;
        assert_eq!(w.status, 405, "target {target:?}");
        assert_eq!(w.header("allow"), Some("GET"));
        assert_eq!(w.content_length(), bodies::method_not_allowed_body().len());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_path_is_404() {
    let port = server().await;
    for target in ["/nope", "/move/extra", "/move/"] {
        let w = get(port, target, None).await;
        assert_eq!(w.status, 404, "target {target:?}");
        assert_eq!(w.content_length(), bodies::not_found_body().len());
    }
}
