//! Static site assets, embedded at compile time.
//!
//! The site's pages reference a small set of `/static/...` resources. They
//! are compiled into the binary (`include_bytes!`), so the server stays a
//! single self-contained executable. Content types and the `Cache-Control`
//! header match what the site's static file handler sends.

use bytes::Bytes;

/// One static asset: exact bytes plus the response headers to send.
pub struct Asset {
    /// `Content-Type` value, including charset where the site sends one.
    pub content_type: &'static str,
    /// Raw asset bytes.
    pub body: Bytes,
}

const CSS_TYPE: &str = "text/css; charset=utf-8";
const JS_TYPE: &str = "text/javascript; charset=utf-8";
const PNG_TYPE: &str = "image/png";
const JPG_TYPE: &str = "image/jpeg";

macro_rules! asset {
    ($name:ident, $file:literal, $type:expr) => {
        pub static $name: Asset = Asset {
            content_type: $type,
            body: Bytes::from_static(include_bytes!($file)),
        };
    };
}

asset!(INDEX_CSS, "../assets/index.css", CSS_TYPE);
asset!(SEND_JS, "../assets/send.js", JS_TYPE);
asset!(TIRESIAS_JPG, "../assets/tiresias.jpg", JPG_TYPE);
asset!(PROGRESS_PNG, "../assets/progress.png", PNG_TYPE);
asset!(BONK_JPEG, "../assets/bonk.jpeg", JPG_TYPE);
asset!(SUCCESS_PNG, "../assets/success.png", PNG_TYPE);
asset!(NICE_JPG, "../assets/nice.jpg", JPG_TYPE);

/// `Cache-Control` sent with every static asset, matching the site.
pub const CACHE_CONTROL: &str = "no-cache";
