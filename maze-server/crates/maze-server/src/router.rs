//! Synchronous request router.
//!
//! Per-request work: byte match on the path, linear scan for the `path`
//! cookie, base64 decode into a stack buffer, table replay, then selection
//! of a precomputed page body. There are no await points and no heap traffic
//! beyond the header map hyper requires.

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::http::request::Parts;
use hyper::http::{Request, Response, StatusCode};
use maze_core::base64::{self, Base64Error};
use maze_core::maze::{trace, Outcome, MAX_PATH_LEN};

use crate::{assets, bodies, maze_svg};

/// Every response in this server: fixed headers + an in-memory body.
pub type Resp = Response<Full<Bytes>>;

const HTML_CT: HeaderValue = HeaderValue::from_static("text/html; charset=utf-8");

/// The fresh cookie `GET /` sets: empty path, session scope, exactly like
/// the site (`path=; Path=/`, no Max-Age).
const SET_COOKIE_START: HeaderValue = HeaderValue::from_static("path=; Path=/");

/// Routes a request. GET routes: `/`, `/move`, `/maze` (the full-maze
/// visualization, a local addition to the site), and the six `/static/...`
/// assets the pages reference. Everything else 404s with the site's body,
/// non-GET on a GET route 405s, matching the site's Flask behavior
/// (there is no `/reset`: restarting is `GET /`, the "Je me suis perdu" link).
pub fn route(req: Request<Incoming>) -> Resp {
    let (parts, _body) = req.into_parts();
    route_parts(&parts)
}

/// Routing over already-dismantled request parts. Exposed separately so the
/// pure handler cost can be benchmarked and allocation-audited without
/// hyper's parse/socket layers.
pub fn route_parts(parts: &Parts) -> Resp {
    let ctx = RequestCtx {
        method: parts.method.as_str().as_bytes(),
        path: parts.uri.path().as_bytes(),
        cookie: find_cookie(&parts.headers),
    };
    route_ctx(&ctx)
}

/// The complete request view the router needs, independent of any HTTP
/// framework. Both backends (hyper and the raw `--features raw` server) build
/// this from their own parsers, so behavior is defined in exactly one place.
pub struct RequestCtx<'a> {
    /// Method bytes, e.g. `b"GET"`. Only `"GET"` routes; anything else 405s.
    pub method: &'a [u8],
    /// Path component without the query, e.g. `b"/move"`.
    pub path: &'a [u8],
    /// Value of the `path` cookie if a Cookie header carried one.
    pub cookie: Option<&'a [u8]>,
}

/// The routing decision, over a backend-neutral request view.
pub fn route_ctx(ctx: &RequestCtx) -> Resp {
    let get = ctx.method == b"GET";
    let known = matches!(
        ctx.path,
        b"/" | b"/move"
            | b"/maze"
            | b"/static/css/index.css"
            | b"/static/js/send.js"
            | b"/static/img/tiresias.jpg"
            | b"/static/img/progress.png"
            | b"/static/img/bonk.jpeg"
            | b"/static/img/success.png"
            | b"/static/img/nice.jpg"
    );
    if !get {
        return if known {
            method_not_allowed()
        } else {
            not_found()
        };
    }
    match ctx.path {
        b"/" => start_page(),
        b"/move" => move_page(ctx),
        b"/maze" => maze_page(),
        b"/static/css/index.css" => asset(&assets::INDEX_CSS),
        b"/static/js/send.js" => asset(&assets::SEND_JS),
        b"/static/img/tiresias.jpg" => asset(&assets::TIRESIAS_JPG),
        b"/static/img/progress.png" => asset(&assets::PROGRESS_PNG),
        b"/static/img/bonk.jpeg" => asset(&assets::BONK_JPEG),
        b"/static/img/success.png" => asset(&assets::SUCCESS_PNG),
        b"/static/img/nice.jpg" => asset(&assets::NICE_JPG),
        _ => not_found(),
    }
}

/// `GET /`: start page + fresh empty cookie, like the site.
fn start_page() -> Resp {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HTML_CT)
        .header(header::SET_COOKIE, SET_COOKIE_START)
        .body(Full::new(bodies::start_page()))
        .expect("static response parts are always valid")
}

/// `GET /move`: evaluate the path from the cookie and answer with the
/// outcome page.
///
/// The URL carries no parameters: the whole path travels in the `path`
/// cookie, base64-encoded, and the client manages that cookie itself. The
/// server only reads it and never rewrites it. Semantics match the site:
///
/// - no cookie header at all -> 302 to `/` (the site's session start)
/// - an empty cookie value is a legal empty path: the start cell, going well
/// - structurally broken base64 -> 500 (the site's decoder raises)
/// - a cookie decoding to more than [`MAX_PATH_LEN`] moves -> bonk page:
///   such a path can never replay (trace() bonks it on the first step), and
///   it arrives in practice once a path outgrows the 4096-char cookie budget
/// - a non-direction byte plays as a wall: bonk page, not an error
fn move_page(ctx: &RequestCtx) -> Resp {
    let Some(cookie_val) = ctx.cookie else {
        return redirect_to_start();
    };
    let mut raw = [0u8; MAX_PATH_LEN];
    let n = match base64::decode_into(cookie_val, &mut raw) {
        Ok(n) => n,
        Err(Base64Error::InvalidLength) => return server_error(),
        Err(Base64Error::BufferTooSmall { .. }) => return ok(bodies::bonk_page()),
    };
    let t = trace(&raw[..n]);
    let body = match t.outcome {
        Outcome::Bonk => bodies::bonk_page(),
        Outcome::GoingWell => bodies::well_page(),
        Outcome::Minotaur => bodies::minotaur_page(),
        Outcome::Solved => bodies::flag_page(),
    };
    ok(body)
}

/// `GET /maze`: the full maze as SVG, every wall drawn plus the A* optimal
/// path overlay. A local addition (the original site has no such endpoint):
/// everything it draws is derived from the same rodata `/move` replays
/// against, and the body is built once and cached like the page bodies.
fn maze_page() -> Resp {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )
        .body(Full::new(maze_svg::maze_svg()))
        .expect("static response parts are always valid")
}

/// Outcome page, no `Set-Cookie`: the cookie is entirely client-owned.
fn ok(body: Bytes) -> Resp {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HTML_CT)
        .body(Full::new(body))
        .expect("valid response parts")
}

/// Static asset: exact bytes, the site's content type, `Cache-Control`.
fn asset(a: &assets::Asset) -> Resp {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, a.content_type)
        .header(header::CACHE_CONTROL, assets::CACHE_CONTROL)
        .body(Full::new(a.body.clone()))
        .expect("static response parts are always valid")
}

/// 302 to `/` with the site's Flask redirect body. No `Set-Cookie`: only
/// `GET /` hands out a fresh cookie.
fn redirect_to_start() -> Resp {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "/")
        .header(header::CONTENT_TYPE, HTML_CT)
        .body(Full::new(Bytes::from_static(
            bodies::redirect_body().as_bytes(),
        )))
        .expect("static response parts are always valid")
}

fn server_error() -> Resp {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, HTML_CT)
        .body(Full::new(Bytes::from_static(
            bodies::server_error_body().as_bytes(),
        )))
        .expect("static response parts are always valid")
}

fn method_not_allowed() -> Resp {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, "GET")
        .header(header::CONTENT_TYPE, HTML_CT)
        .body(Full::new(Bytes::from_static(
            bodies::method_not_allowed_body().as_bytes(),
        )))
        .expect("static response parts are always valid")
}

fn not_found() -> Resp {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, HTML_CT)
        .body(Full::new(Bytes::from_static(
            bodies::not_found_body().as_bytes(),
        )))
        .expect("static response parts are always valid")
}

/// First value of the `path` cookie across all `Cookie` headers, byte-level.
fn find_cookie(headers: &HeaderMap) -> Option<&[u8]> {
    for val in headers.get_all(header::COOKIE) {
        if let Some(v) = cookie_value_in(val.as_bytes()) {
            return Some(v);
        }
    }
    None
}

/// Extracts the `path` cookie value from one `Cookie:` header value (the
/// bytes after `Cookie:`). Shared by the hyper backend and the raw backend so
/// cookie semantics live in one place.
pub fn cookie_value_in(header_value: &[u8]) -> Option<&[u8]> {
    for seg in header_value.split(|&b| b == b';') {
        let s = seg.trim_ascii();
        if s.starts_with(b"path=") {
            return Some(&s[5..]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie_header(v: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(header::COOKIE, HeaderValue::from_str(v).unwrap());
        m
    }

    #[test]
    fn cookie_scanning() {
        assert_eq!(find_cookie(&cookie_header("path=abc")), Some(&b"abc"[..]));
        assert_eq!(
            find_cookie(&cookie_header("x=1; path=ab c")),
            Some(&b"ab c"[..])
        );
        assert_eq!(
            find_cookie(&cookie_header("x=1; path=zz")),
            Some(&b"zz"[..])
        );
        assert_eq!(find_cookie(&cookie_header("xpath=1")), None);
        assert_eq!(find_cookie(&cookie_header("a=1; b=2")), None);
        assert_eq!(find_cookie(&HeaderMap::new()), None);
        // empty value is still a found cookie (the caller decides semantics)
        assert_eq!(find_cookie(&cookie_header("path=")), Some(&b""[..]));
    }

    #[test]
    fn cookie_value_extraction() {
        // the cookie value extractor shared with the raw backend
        assert_eq!(cookie_value_in(b"path=abc"), Some(&b"abc"[..]));
        assert_eq!(cookie_value_in(b"x=1; path=zz"), Some(&b"zz"[..]));
        assert_eq!(cookie_value_in(b"a=1"), None);
    }
}
