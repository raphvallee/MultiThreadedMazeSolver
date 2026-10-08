//! # maze-server
//!
//! The HTTP layer of the Maze Web Application: hyper 1.x on tokio, with a
//! fully synchronous router. Every handler is pure computation over
//! precomputed bodies, so the per-request work is: byte-level route match,
//! cookie scan, base64 decode into a stack buffer, table-driven replay, and
//! selection of a prebuilt page body.
//!
//! Statelessness: the only session state is the `path` cookie the client
//! carries; the server never stores anything and never rewrites it on
//! `/move`. This mirrors the Daedalus site being recreated, byte for byte
//! where the wire is concerned (page templates, sizes, error bodies).

pub mod assets;
pub mod bodies;
pub mod maze_svg;
pub mod router;
pub mod serve;
#[cfg(feature = "raw")]
pub mod raw;

pub use serve::serve;