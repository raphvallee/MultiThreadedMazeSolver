//! Page bodies: byte-exact replicas of the Daedalus site's responses.
//!
//! Every page the site serves is a fully static template; the only per-page
//! variation is which template is sent. The four game pages therefore have
//! fixed sizes, and the two outcome sizes the spec fixes are asserted at
//! compile time:
//!
//! - start page (`GET /`): 1843 bytes, tiresias image, sets a fresh cookie
//! - going-well page (`GET /move`): exactly 1744 bytes, progress image
//! - bonk page (`GET /move`): exactly 1730 bytes, bonk image
//! - minotaur page (`GET /move`): exactly 1774 bytes, the minotaur
//!   encounter message and the nice image; the move itself is legal
//! - flag page (solved path): well page with the flag message and the
//!   success image (1759 bytes with the bundled 32-byte flag)
//!
//! All pages are compile-time constants, built once and served as
//! refcounted clones: zero allocations per request in steady state.

use bytes::Bytes;

/// The flag the site renders on the win page. Change it here; the win page
/// is rebuilt around it at startup, so its size tracks the flag length.
pub const FLAG_MESSAGE: &str = "CEM{D3D4L3_3T_L3_F1L_D3_4R14DNE}";

/// Fixed outcome page sizes (spec 3.2). They must differ: the Content-Length
/// of a move response is what makes a good move distinguishable from a bad
/// one at the protocol level.
pub const BONK_SIZE: usize = 1730;
pub const WELL_SIZE: usize = 1744;
pub const MINOTAUR_SIZE: usize = 1774;
const _: () = assert!(BONK_SIZE != WELL_SIZE);
const _: () = assert!(WELL_SIZE != MINOTAUR_SIZE);
const _: () = assert!(BONK_SIZE != MINOTAUR_SIZE);

const ROOT: &str = include_str!("../templates/root.html");
const BONK: &str = include_str!("../templates/bonk.html");
const WELL: &str = include_str!("../templates/well.html");
const MINOTAUR: &str = include_str!("../templates/minotaur.html");

/// The site's Flask error/redirect bodies, byte for byte.
const E302: &str = include_str!("../templates/e302.html");
const E404: &str = include_str!("../templates/e404.html");
const E405: &str = include_str!("../templates/e405.html");
const E500: &str = include_str!("../templates/e500.html");

const _: () = assert!(BONK.len() == BONK_SIZE);
const _: () = assert!(WELL.len() == WELL_SIZE);
const _: () = assert!(MINOTAUR.len() == MINOTAUR_SIZE);
const _: () = assert!(ROOT.len() == 1843);

/// The message slot and the image slot of the well page, used to build the
/// flag page from the same template (they each occur exactly once).
const WELL_MSG: &str = "Ça avance bien!";
const WELL_IMG: &str = "/static/img/progress.png";
const WIN_IMG: &str = "/static/img/success.png";

/// Start page (`GET /`): intro message, controls, tiresias image.
pub fn start_page() -> Bytes {
    static PAGE: Bytes = Bytes::from_static(ROOT.as_bytes());
    PAGE.clone()
}

/// Bonk page: the path crossed a wall (or carried a non-direction byte).
pub fn bonk_page() -> Bytes {
    static PAGE: Bytes = Bytes::from_static(BONK.as_bytes());
    PAGE.clone()
}

/// Going-well page: every step legal, final cell not the exit or the
/// minotaur.
pub fn well_page() -> Bytes {
    static PAGE: Bytes = Bytes::from_static(WELL.as_bytes());
    PAGE.clone()
}

/// Minotaur page: every step legal and the final cell is the minotaur
/// cell. The move itself succeeds - the page is a going-well variant with
/// the encounter message and the nice image; the player can keep walking.
pub fn minotaur_page() -> Bytes {
    static PAGE: Bytes = Bytes::from_static(MINOTAUR.as_bytes());
    PAGE.clone()
}

/// Flag page: the path ends on the exit. Same layout as the well page with
/// the flag in the message slot and the success image; built once around
/// [`FLAG_MESSAGE`].
pub fn flag_page() -> Bytes {
    static PAGE: std::sync::LazyLock<Bytes> = std::sync::LazyLock::new(|| {
        let s = WELL
            .replace(WELL_MSG, FLAG_MESSAGE)
            .replace(WELL_IMG, WIN_IMG);
        Bytes::from(s)
    });
    PAGE.clone()
}

/// Flask-style redirect body (`/move` without any cookie header).
pub fn redirect_body() -> &'static str {
    E302
}

/// Flask-style 404 body (unknown paths, `/reset` - the site has no reset).
pub fn not_found_body() -> &'static str {
    E404
}

/// Flask-style 405 body (non-GET on a GET route).
pub fn method_not_allowed_body() -> &'static str {
    E405
}

/// Flask-style 500 body (structurally broken base64 cookie).
pub fn server_error_body() -> &'static str {
    E500
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_pages_exact_sizes() {
        assert_eq!(bonk_page().len(), BONK_SIZE);
        assert_eq!(well_page().len(), WELL_SIZE);
        assert_eq!(minotaur_page().len(), MINOTAUR_SIZE);
    }

    #[test]
    fn start_page_contents() {
        let s = String::from_utf8(start_page().to_vec()).unwrap();
        assert!(s.contains("Tirésias"), "intro message");
        assert!(s.contains("send('u')"), "controls call the JS move helper");
        for d in ['u', 'd', 'l', 'r'] {
            assert!(s.contains(&format!("send('{d}')")), "direction {d}");
        }
        assert!(s.contains("Je me suis perdu :("), "lost button");
        assert!(s.contains("/static/img/tiresias.jpg"), "start image");
        assert!(s.contains("Daedalus v3105"), "title");
        assert!(!s.contains("/move?"), "the URL must carry no parameters");
        assert!(!s.contains("<pre"), "no maze map on the site's pages");
    }

    #[test]
    fn outcome_page_contents() {
        let b = String::from_utf8(bonk_page().to_vec()).unwrap();
        assert!(b.contains("BONK!"), "bonk message");
        assert!(b.contains("/static/img/bonk.jpeg"), "bonk image");
        assert!(b.contains("send('u')"), "controls still present");
        let w = String::from_utf8(well_page().to_vec()).unwrap();
        assert!(w.contains("Ça avance bien!"), "well message");
        assert!(w.contains("/static/img/progress.png"), "progress image");
        assert_eq!(w.matches("send('").count(), 4, "four arrows");
    }

    #[test]
    fn minotaur_page_contents() {
        let m = String::from_utf8(minotaur_page().to_vec()).unwrap();
        assert!(m.contains("le minotaure"), "minotaur message");
        assert!(m.contains("/static/img/nice.jpg"), "nice image");
        assert!(
            !m.contains("Ça avance bien!"),
            "the well message is replaced"
        );
        assert!(
            !m.contains("/static/img/progress.png"),
            "the progress image is replaced"
        );
        assert_eq!(m.matches("send('").count(), 4, "four arrows");
    }

    #[test]
    fn flag_page_contents() {
        let s = String::from_utf8(flag_page().to_vec()).unwrap();
        assert!(s.contains(FLAG_MESSAGE), "flag message");
        assert!(s.contains(WIN_IMG), "success image");
        assert!(!s.contains(WELL_MSG), "the well message is replaced");
        let n = flag_page().len();
        assert_ne!(n, BONK_SIZE);
        assert_ne!(n, WELL_SIZE);
    }

    #[test]
    fn error_bodies_are_flask_shaped() {
        assert!(redirect_body().contains("Redirecting..."));
        assert!(not_found_body().contains("404 Not Found"));
        assert!(method_not_allowed_body().contains("405 Method Not Allowed"));
        assert!(server_error_body().contains("500 Internal Server Error"));
    }
}
