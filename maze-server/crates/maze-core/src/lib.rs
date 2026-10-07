//! # maze-core
//!
//! Stateless maze engine for the Maze Web Application (see
//! `maze_application_specification.md`). Pure computation: no I/O, zero
//! dependencies, zero framework code. The HTTP layer (milestone 2) sits on top
//! of this crate.
//!
//! ## Hot path design
//!
//! Everything that can be precomputed is precomputed at compile time:
//!
//! - The map is parsed from ASCII art into spec cell values (0/1/2/3) by a
//!   `const fn`, so there is no runtime parsing at all.
//! - A per-cell blocked-direction bitmask (`WALLS`) folds inner walls and the
//!   outer boundary into rodata, so "is this move legal" is one load + one AND.
//! - A flat transition table (`NEXT`) turns a legal step into one more load;
//!   replaying a path never does index arithmetic, only lookups.
//!
//! A full worst-case [`maze::MAX_PATH_LEN`] path replays in a few
//! microseconds; typical paths in well under 100 ns. Nothing in this crate
//! allocates on the hot path: base64 decodes and encodes write into
//! caller-provided buffers.
//!
//! ## Behavior mapping
//!
//! - `Outcome::GoingWell` -> the "Ça avance bien!" page (1744 bytes).
//! - `Outcome::Bonk` -> the "BONK!" page (1730 bytes). A step crossed a wall
//!   or the boundary, or the byte was not one of the four lowercase
//!   directions at all: the site renders the same bonk page for both.
//! - `Outcome::Solved` -> flag page (the final cell is the exit).
//!
//! The exit is only checked on the *final* cell of the path, exactly as the
//! site defines the outcome; walking over the exit and ending elsewhere is a
//! normal valid move.

pub mod base64;
pub mod maze;
pub mod solve;

pub use base64::{decode_into, decoded_len, encode_into, encoded_len, Base64Error};
pub use maze::{
    cell_at, cell_char, dir_from_char, replay, replay_end, step, trace, Outcome, Trace, EXIT, H,
    MAX_COOKIE_LEN, MAX_PATH_LEN, START, W,
};
pub use solve::shortest_path;
