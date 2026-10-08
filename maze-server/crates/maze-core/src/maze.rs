//! Maze map, precomputed transition tables, and path replay.
//!
//! All data structures below are `const`/`static` rodata built at compile
//! time. The replay loop is: one LUT load for the direction byte, one table
//! load for the transition, one compare. No allocation, no branches beyond
//! the two sentinel checks.
//!
//! The map is the surveyed layout of the live Daedalus site (v3105),
//! discovered by black-box probing with the mapper in `tools/map_remote.py`:
//! 121x119 playable cells inside a wall ring (123x121 with the ring), 7082
//! walkable cells including the start, the exit and one minotaur cell.

/// Grid width in cells, including the surrounding wall ring.
pub const W: usize = 123;
/// Grid height in cells, including the surrounding wall ring.
pub const H: usize = 121;

/// Spec cell value: empty space.
pub const EMPTY: u8 = 0;
/// Spec cell value: wall (inner obstacle).
pub const WALL: u8 = 1;
/// Spec cell value: start tile.
pub const START_CELL: u8 = 2;
/// Spec cell value: exit tile.
pub const EXIT_CELL: u8 = 3;
/// Spec cell value: minotaur tile (walkable; landing on it shows the
/// site's minotaur page instead of the going-well page).
pub const MINOTAUR_CELL: u8 = 4;

/// Direction id, also the bit position in the blocked-direction masks.
pub const DIR_UP: u8 = 0;
/// Direction id, also the bit position in the blocked-direction masks.
pub const DIR_DOWN: u8 = 1;
/// Direction id, also the bit position in the blocked-direction masks.
pub const DIR_LEFT: u8 = 2;
/// Direction id, also the bit position in the blocked-direction masks.
pub const DIR_RIGHT: u8 = 3;
/// Sentinel returned by the direction LUT for bytes that are not directions.
pub const INVALID_DIR: u8 = 0xff;

/// Longest raw path (in moves) accepted by [`replay`]. 3072 moves encode to
/// exactly 4096 base64 chars, which is the hard cookie budget browsers
/// enforce, so a browser-managed cookie can never carry a longer path.
/// Longer cookies still arrive from non-browser clients; the router answers
/// those with the bonk page rather than a decode error.
pub const MAX_PATH_LEN: usize = 3072;
/// Longest base64 cookie payload the server should accept before decoding.
pub const MAX_COOKIE_LEN: usize = 4096;

/// No-transition sentinel stored in [`NEXT`] for blocked moves.
const NO_STEP: u16 = u16::MAX;

/// The map. Legend: `#` wall, `.` empty, `S` start (the spec entry point,
/// logical (0, 0)), `E` exit, `M` minotaur. The outer ring of `#` is the
/// grid boundary; the surveyed maze is a perfect maze, so every wall is
/// either part of the ring or an inner obstacle between two corridors.
const MAZE_ART: &str = concat!(
        "###########################################################################################################################\n",
        "#.S........#...#.......#.....#.............#.........#.....#.........#...................#.....#.....#...........#.....#.##\n",
        "########.###.#.#.###.#.#.###.#.###.#######.#####.###.#.###.#####.###.#.#########.#######.#.###.###.#.#.###.#.#####.#.#.#.##\n",
        "##.......#...#.#.#...#...#...#.#...#.....#.......#.#...#.#.......#.#.#.#...#.....#.....#...#.#...#.#...#.#.#.#.....#.#...##\n",
        "##.###.###.###.#.#.#######.#####.###.#.#.#########.#####.#########.#.#.###.#.#####.###.#####.###.#####.#.#.#.#.#####.######\n",
        "##.#...#.....#...#.#.....#.#.....#.#.#.#...#...#.......#.........#...#.....#...#.....#.......#.#.....#...#.#.#.#...#.....##\n",
        "##.#####.#######.#.#.###.#.#.#####.#.#.###.#.#.#####.#.#####.###.#.#########.#.#####.#####.#.#.#####.#####.#.#.###.#####.##\n",
        "##.#...#.#.....#.#.....#.#...#.#.....#...#...#.......#.....#...#.#.....#...#.#.#...#.....#.#...#.#...#.....#.#...#.....#.##\n",
        "##.#.#.#.#.###.###.#####.#####.#.#######.#################.###.#.#####.#.#.###.#.#.#####.#.###.#.#.###.#########.#####.#.##\n",
        "##...#...#...#...#.#.....#.....#...#.....#.........#.....#...#.#.....#...#.....#.#.#.....#.#.#...#.#...........#...#...#.##\n",
        "##.#########.###.###.#######.#####.#.###.#####.#.###.###.###.#.###.#############.#.#######.#.###.#.#.#######.#.###.#.#.#.##\n",
        "##.#.#.......#...#...#.......#...#.#.#...#...#.#.#.....#...#...#...#.............#.........#.....#.#...#...#.#.......#.#.##\n",
        "##.#.#.#######.###.###.#.###.#.#.#.#.###.#.#.###.#.#####.###.#######.#########.###########.#######.###.#.#.###.#########.##\n",
        "##...#.#.....#.....#.#.#...#.#.#...#...#.#.#...#...#...#...#.#.......#.........#...#.....#.#.....#...#.#.#...#.#...#...#.##\n",
        "##.###.#.###########.#.###.###.#######.###.###.#.###.#.###.#.#.#######.#########.#.#.###.#.#.###.###.#.#.###.###.#.#.#.#.##\n",
        "##.#...#.#...........#.#.#.....#.....#.....#...#.#.#.#...#...#...#.#...#...#...#.#...#.#...#.#.#...#.#.#.#.......#...#...##\n",
        "####.###.#.#########.#.#.#########.#########.###.#.#.###.#######.#.#.###.###.#.#.#####.#####.#.###.#.#.#.#.#############.##\n",
        "##...#...#.#.......#...#...........#.........#.....#.#.#.#.......#.#...#.....#.......#...#...#...#...#.#.#.#...#.......#.##\n",
        "##.#####.#.#.#####.#####.#####.###.#.#########.#####.#.#.#.#######.###.#############.###.#.#####.#######.###.#.###.#.###.##\n",
        "##.#.....#.#.#...#.#.......#...#...#.......#...#...#...#...#.........#.#.....#.....#.#...#.#...#.....#...#...#...#.#.....##\n",
        "##.#.#####.#.#.#.#.#########.#.#######.###.#.###.#.###.###########.#.#.#.###.#.###.#.#.###.#.#.#.###.#.###.#####.#######.##\n",
        "##.#...#...#.#.#.#.#.........#.#.....#...#.#.#...#.....#...........#.#.....#.#.#...#...#...#.#...#.#.#.........#.......#.##\n",
        "##.#.#.#.###.#.#.#.#.#########.#.###.###.#.#.#.#############.#.#####.#######.#.#.#######.###.#####.#.#########.#######.#.##\n",
        "##.#.#...#...#.#.....#.......#.#.#...#...#.#.#.#...#.........#.#...#...#.....#.#.#.......#.......#.........#.....#.....#.##\n",
        "##.#.#####.###.#############.#.#.#.#####.#.###.#.#.#.###.#####.#.#.###.#.###.#.#.#.#######.#####.#.#######.#.#####.#####.##\n",
        "##.#.......#.#...#.....#...#...#.#.....#.#.#...#.#...#...#.....#.#.#...#.#.#.#.#.#.#.....#.....#.#.#.....#.#...#...#.....##\n",
        "##.#########.###.#.###.#.#.#####.#####.#.#.#.#######.#.###.#####.#.#####.#.#.#.#.#.#.#.#.#######.###.###.#.#####.##########\n",
        "##.....#.#.....#.#...#...#.....#...#...#.#.#...#...#.#.#.#...#...#.......#...#.#.#.#.#.#.......#.....#...#.....#...#.....##\n",
        "######.#.#.#.###.###.#########.###.#.###.#.###.#.#.###.#.###.#.###########.###.#.#.###.#######.#####.#.#######.###.#.###.##\n",
        "##.....#...#.......#.....#.....#...#.#...#.......#.....#.#...#.#...#...#.....#.#.#...#.#####.......#.#.#.....#...#.....#.##\n",
        "##.#######.#######.###.###.#####.###.#.#####.###########.#.###.#.#.#.#.#.#####.#.###.#.###########.#.#.###.#.###.#######.##\n",
        "##.......#.#.#...#...#.#...#...#.#.#.#.#...#...#.....#...#.#...#.#...#.#.#.....#.#...#.#########.#.#.#...#.#...#.......#.##\n",
        "##.#####.#.#.#.#.#####.#.###.#.#.#.#.#.#.#.#####.###.#.#.#.#.###.#####.#.#.#####.#.###.#########.#.#####.#.###########.#.##\n",
        "##...#...#...#.#.#.....#.....#.#...#.#.#.#.........#.#.#.#.#.....#...#.#.#...#.#...#...#########.#...#...#.........#...#.##\n",
        "######.#.#####.#.#.###########.###.#.###.###########.#.#.#.#######.###.#.###.#.#####.###########.###.#.#######.###.#.###.##\n",
        "##.....#.#.....#...#...#.....#.....#...#.......#.....#.#.#.#.......#...#...#.......#...#########...#.#.#.......#...#.#...##\n",
        "##.###.###.#########.#.###.#.#########.#######.#.#####.###.#.###.###.#####.#######.###.#########.#.#.#.#.#######.###.###.##\n",
        "##.#...#...#.........#...#.#...#.....#.....#...#...#.#.....#...#...#...#.......#...#...#########.#.#...#.#...#...#.#.#...##\n",
        "##.#####.###.###########.#.###.###.###.#####.#####.#.#.#######.###.###.###.#####.###.###########.#.#####.###.#.###.#.#.####\n",
        "##.#.....#.....#...#...#.#.#.#...#...#.#...#...#...#.#.....#.....#...#...#.#.....#.......#######.#.#.......#.......#.#...##\n",
        "##.#.#######.#.#.#.###.#.#.#.###.#.#.#.#.#.###.#.###.#####.#.#####.#.###.###.#####.#####.#######.###.#####.#.#######.###.##\n",
        "##...#.....#.#...#.#...#...#.#...#.#.#...#...#.#.#.........#...#...#...#.....#...#.#...#...###...#.......#.#...#...#...#.##\n",
        "##.###.###.#####.#.#.#######.#.#####.#######.#.#.###.#########.#.#############.###.#.###.#####.#.#.#######.###.#.#.###.#.##\n",
        "##...#.#...#...#.#.#.......#...........#.....#.#...#.....#.....#...#.......#.........#...#####.#...#.#.....#...#.#.....#.##\n",
        "####.#.#.###.#.###.#.###.###.###########.###.#.###.#####.#.#######.#.#.###.###########.#######.#####.#.###.#####.#######.##\n",
        "##.#...#.#...#.#...#...#.....#.....#.....#...#.#.#.....#.#.......#...#...#...#.........#####.#.....#.#.#...#...#.#...#...##\n",
        "##.#####.#.###.#.#############.###.#.#########.#.#####.#.###.#######.###.###.#.#####.#######.#####.#.#.###.#.#.#.#.#.#.#.##\n",
        "##.........#.#.#.....#.........#.....#.........#.....#.#.#...#.....#.#...#.#...#.#...#####.......#...#...#.#.#...#.#...#.##\n",
        "############.#.#####.#.#########.#####.#########.###.#.#.#.###.###.#.#.###.#####.#.#######.#####.###.###.#.#.###########.##\n",
        "##.....#...#...#.....#.....#.....#...#.#.........#...#.#.#...#.#...#.#...#...#.......#####...#.....#.#.#.#.#.#...........##\n",
        "##.###.#.#.###.#.#.#######.#######.#.#.#####.#####.###.#.#####.#.#######.#.#.#.###########.#.#####.#.#.#.###.#.###.########\n",
        "##...#.#.#...#...#.#.....#.#.......#...#...#.#...#.#...#.......#.#.......#.#...#...#######.#...#...#.#.#.....#...#.......##\n",
        "##.#.#.#.###.#####.#.###.#.#.###########.#.#.###.#.#.###########.#.#.#####.#####.#.#######.###.#.###.#.#########.#######.##\n",
        "##.#.#.#...#...#...#.#.#...#.#.........#.#.#...#...#.#.....#...#.#.#.....#.....#.#.#######...#.#...#...#...#.....#.....#.##\n",
        "##.#.#.###.###.#####.#.###.#.#####.###.#.#.###.###.#.#.#####.#.#.#.#####.#####.###.#########.#.#.#####.#.#.#.#########.#.##\n",
        "##.#.#.....#...#...#.#...#.#...#...#.#...#...#...#.#.#.#.....#...#.....#...#.#.......#####...#.#.#.....#.#.#...#.......#.##\n",
        "####.#######.###.#.#.#.#.#.###.#.###.#######.#.#.###.#.#.#######.#####.###.#.#######.#########.###.#####.#####.#.###.###.##\n",
        "##...#...#.....#.#.#.#.#...#.#...#...#...#...#.#...#.#...#.....#...#...#...#.....#...#########.......#.#.....#.#.#.#.#...##\n",
        "##.###.#.#####.#.#.#.#.#####.#####.###.#.#.###.###.#.#.###.###.###.#####.#.#####.#.#################.#.#.###.#.#.#.#.#.#.##\n",
        "##.#...#.....#.#.#.#.#.#.........#...#.#...#...#...#.#...#.#...#...#.....#.#...#.#...###############.#.#.#.#.#...#...#.#.##\n",
        "##.#.#######.#.#.#.#.#.###.#####.#.#.#.#####.###.###.#.###.#.###.###.###.###.#.#.###.###############.#.#.#.#.#####.###.#.##\n",
        "##...#.....#.#...#...#.#...#...#.#.#.#...#...#.#.#...#.#...#...#.#...#...#...#.#.....#...###########.#.....#...#...#.#.#.##\n",
        "##.###.#.#.#.#########.#.#####.#.###.###.###.#.#.#.#####.#####.#.#.#####.#.###.#####.#.#.###########.#########.#.###.#.#.##\n",
        "##...#.#.#.#...#...#.#.#...#...#.#.....#...#...#.#...........#.#.#.#...#.#...#.#...#.#.#..##.......#.#...#...#...#...#.#.##\n",
        "####.###.#.###.#.#.#.#.###.#.###.#.#######.#####.###########.#.#.#.#.#.###.###.#.#.###.#######.###.#.#.#.#.#.#.###.#.#.#.##\n",
        "##.#...#.#.#...#.#...#...#.#.....#.#.......#.....#...#.....#.#.#...#.#.....#...#.#...#.....#...#...#.#.#...#.#.....#.#.#.##\n",
        "##.###.#.#.#.###.###.###.#.#.#####.#.#######.###.#.###.###.###.###.#.#######.###.###.#####.#.#######.#.#####.#######.#.#.##\n",
        "##.#.....#.#.#.....#...#...#.......#.......#.#...#...#...#...#.#.#.#.#######.....#.#...#...#.........#...#.........#.#.#.##\n",
        "##.#.#######.#.###.#############.#########.#.###.###.###.###.#.#.#.#.#############.#.###.###.#########.#.#########.#.#.#.##\n",
        "##.#.#.......#.#...#.......#.....#.......#.#...#...#...#...#...#...#.#M#####.......#...#.#.#.....#...#.#.........#.#.#.#.##\n",
        "##.#.#.#########.#.#.#.#####.#####.###.###.###.###.#.#.###.###.#####.#.#####.###.#.###.#.#.#####.#.#.#####.#####.#.###.#.##\n",
        "##...#.#.......#.#.#.#.#.....#...#...#...#...#.#.#.#.#...#.#...#.....#.......#...#...#...#.....#...#.....#.#.#...#.....#.##\n",
        "######.#.#####.#.#.#.#.#.#####.#.###.###.###.#.#.#.#.#####.#####.#############.#####.#######.#.#########.#.#.#.#.##########\n",
        "##.....#.#...#...#.#.#...#.....#...#.#.....#.#.#.#.#...#...#.....#...........#.#.....#.......#...#.......#...#.#.#.......##\n",
        "##.#####.#.#.#####.#.#######.#####.#.#.#####.#.#.#.###.#.#.#.#######.#######.#.#.#####.#.#####.###.#########.#.###.#####.##\n",
        "##.#.......#.#.....#.#.......#...#.#.#...#...#.#.......#.#.#.#.......#.....#.#.#.#.....#...#...#...#.....#.#.#.....#.....##\n",
        "##.###.#####.#.#####.#.###.###.#.###.###.#.###.#.#######.#.#.#.#########.###.#.###.#######.#####.###.###.#.#.#######.######\n",
        "##...#.#.....#.....#.#...#.#...#.....#.#...#...#.#.......#.#...#.........#...#.......#...#.#...#...#.#.#.#...#.....#.#...##\n",
        "##.#.###.###########.###.###.#########.#####.#.###.#################.###.#.#####.#####.#.#.#.#.###.#.#.#.#.###.###.#.#.#.##\n",
        "##.#...#.......#.........#...#.....#.....#...#...#...#...........#...#...#.#...#...#...#...#.#...#.#.#.#.#.....#...#...#.##\n",
        "######.###.###.#.#########.###.#####.###.#.#####.###.#.#######.#.#.###.###.#.#.#####.#######.#.#.#.#.#.#.#####.#########.##\n",
        "##...#...#...#.#.#...#.....#...........#...#.....#.#...#.....#.#...#.....#...#.#.....#.......#.#.#...#.#...#...#.......#.##\n",
        "##.#.#.#.#####.#.###.#.#####################.###.#.#########.#.###############.#.#####.#######.#######.###.###.#.#####.#.##\n",
        "##.#.#.#.....#.......#.#.....................#...#.........#.#.#...........#...#.........#...#...#.......#...#.#.#...#.#.##\n",
        "##.#.#######.###.#####.###.#.#################.###.#######.#.#.#.#######.#.#.###########.#.#.###.#.#####.###.#.#.#.###.#.##\n",
        "##.#...#...#...#.#...#...#.#.#.#.....#.....#...#.#.....#.....#...#.....#.#.#.........#.#...#...#.......#.#...#.#.#...#.#.##\n",
        "##.###.#.#.###.###.#.###.###.#.#.#.#.#.#####.###.#.###.###############.#.###########.#.###.#############.#.#####.#.#.#.#.##\n",
        "##.#.#...#...#...#.#...#.....#.#.#.#.#.#.....#.#...#.#.................#.......#...#.#.....#.......#...#.#.#...#.#.#...#.##\n",
        "##.#.#######.###.#.#.#########.#.#.#.#.#.#####.#.###.#####.#######.###.#####.#.###.#.#.#####.#####.#.#.#.#.#.#.#.#####.#.##\n",
        "##.#.....#...#...#.#.#.....#.#...#.#.#.#.#.....#.#...#...#.......#...#.....#.#.....#.#.#...#...#.....#.#.#...#.#.#...#.#.##\n",
        "##.#.###.#.###.###.#.#.#.#.#.#.###.#.#.#.###.###.#.#.#.#.###########.#####.#.#######.#.###.###.#######.#.#####.#.#.#.#.#.##\n",
        "##.#...#.#.#...#...#...#.#.#.....#.#...#...#...#...#.#.#.............#.#...#.........#.......#.....#.......#...#...#.#...##\n",
        "##.###.#.#.###.#.#######.#.#####.#.#######.###.#####.#.###############.#.###################.#####.#.#######.#####.#.###.##\n",
        "##.#...#.#.#...#.#...#...#...#...#.#.....#...#.......#.#.........#.#.....#...#.............#.#.....#.#.....#.....#.#.#...##\n",
        "##.#####.#.#.###.#.###.#####.#####.#.###.###.#.#######.###.###.#.#.#.#####.#.#.#####.#####.###.#####.#.###.#####.###.#.####\n",
        "##.....#.#.#.#...#...#.#.#...#...#.#...#.#...#.#...#.#...#.#...#.#.#...#...#.#.....#.....#...#.#.#...#...#.....#...#.#.#.##\n",
        "######.#.#.#.#.#####.#.#.#.###.#.#.###.#.#.###.#.#.#.###.#.#.###.#.###.#.###.###########.###.#.#.#.#####.#.###.###.#.#.#.##\n",
        "##.....#...#.....#...#.#...#...#.#...#.#...#.....#.#.#...#.#.#.#.#.#...#...#...#.......#...#...#...#.....#.#.#...#.#.#...##\n",
        "##.#######.#####.#.#.#.#.###.#.#.#.#.#.###########.#.#.###.#.#.#.#.#.#####.###.#.#####.###.#.#####.#.#####.#.###.#.#.######\n",
        "##.#.....#...#.#...#.#.#...#.#.#.#.#.#.......#...#...#...#.#.#.....#...#...#...#.#...#.....#.#...#.#.....#.#.#...#.#.#...##\n",
        "##.#.###.###.#.#.#####.###.###.#.#.#.#######.#.#####.###.#.#.#####.###.#.###.###.#.#######.###.#.#####.###.#.#.###.#.#.#.##\n",
        "##.#...#.#...#...#.....#.#.....#.#.#.#.....#.#...#...#...#.#.....#.#...#.#.......#.#.......#...#.......#...#...#...#...#.##\n",
        "##.###.#.#.#####.#.#####.#######.#.###.#.###.#.#.#.###.#########.###.###.#########.#.#######.###########.###.###.#######.##\n",
        "##.#...#.#.....#.#.#...#.......#.#...#.#.....#.#...#.#.........#.#...#...#...#.......#...#...#.......#...#.....#.........##\n",
        "##.#.###.#####.###.#.#.#.#####.#.###.#.###########.#.#########.#.#.###.###.#.###.#####.#.#.#####.#.###.#################.##\n",
        "##...#.#.....#.....#.#.....#.#.#...#.#.......#...#...#.....#...#...#...#...#...#.......#.#.......#.#...#.................##\n",
        "##.###.#####.#############.#.#.###.#.#######.#.#.#####.###.#.#####.#.###.#####.###################.#.###.##################\n",
        "##.#.......#.#...#.......#...#.....#...#...#...#...#...#...#.....#...#...#...#.....#.........#...#.#.....#.............#.##\n",
        "##.#.#####.#.#.#.#.#####.#########.#.#.###.#######.#.###.###.###.###.#.###.#.#####.#.#######.#.#.#.#######.###########.#.##\n",
        "##...#.....#.#.#.#...#.#.........#.#.#.#.....#...#.#.#...#.#.#...#...#...#.#.....#.#.#.....#...#.#...#.....#.#...#...#...##\n",
        "##.#####.###.#.#.###.#.#########.#.#.#.#.#.###.#.#.#.###.#.#.#.###.#####.#.#####.#.#.#.#.#######.###.###.###.#.#.#.#.###.##\n",
        "##.#...#...#...#...#.....#...#...#.#.#...#.#...#...#...#.#...#...#.#...#.#.#.....#...#.#.#.....#.....#...#.#...#...#...#.##\n",
        "##.#.#.#########.#.#####.#.#.#.###.#.#.#####.#########.#.#.#####.#.#.#.#.#.#.###.#####.###.#.#########.###.#.#########.#.##\n",
        "##.#.#.......#...#.....#...#.#...#.#.#.#...#.....#.....#.#.....#.#...#.#.#.#.#...#...#.#...#.......#.....#...#.#.....#.#.##\n",
        "##.#.#######.#.#####.#######.###.#.#.###.#.#####.###.###.#####.#.#######.###.#.###.#.#.#.#######.#.###.#.#.###.#.###.#.#.##\n",
        "##.#.....#.....#.....#.......#.#.#...#...#.....#.....#...#.....#.........#...#.....#.#...#.......#...#.#.#.#...#...#...#.##\n",
        "##.#####.#####.#####.#.#######.#.###.#.###.###.#######.#######.###########.#########.#.###.#########.###.#.#.#.###.#####.##\n",
        "##.....#.....#.#...#.#.#.....#.#...#.#...#.#.........#.......#.#.........#...#.....#.#...#.........#.#...#.#.#...#.#.....##\n",
        "######.#####.###.#.###.#.###.#.###.#####.#.#################.#.#####.###.###.#.#####.#############.#.#.###.#####.#.#.######\n",
        "##.........#.....#.......#.......#.......#...................#.......#.......#.....................#...#.........#.......E#\n",
        "###########################################################################################################################\n",
    );

const fn cell_value(c: u8) -> u8 {
    match c {
        b'.' => EMPTY,
        b'#' => WALL,
        b'S' => START_CELL,
        b'E' => EXIT_CELL,
        b'M' => MINOTAUR_CELL,
        _ => panic!("invalid character in maze art"),
    }
}

/// Parse [`MAZE_ART`] into a flat row-major cell array at compile time.
const fn parse_cells() -> [u8; W * H] {
    let mut out = [0u8; W * H];
    let bytes = MAZE_ART.as_bytes();
    let mut i = 0usize;
    let mut row = 0usize;
    let mut col = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        if c == b'\r' {
            continue;
        }
        if c == b'\n' {
            assert!(col == W, "maze art row has wrong width");
            row += 1;
            col = 0;
            continue;
        }
        assert!(row < H, "maze art has too many rows");
        assert!(col < W, "maze art row is too wide");
        out[row * W + col] = cell_value(c);
        col += 1;
    }
    assert!(row == H, "maze art has too few rows");
    assert!(col == 0, "maze art last row has wrong width");
    out
}

const CELLS: [u8; W * H] = parse_cells();

/// A cell value a player may stand on.
const fn walkable(v: u8) -> bool {
    v != WALL
}

/// Per-cell bitmask of blocked directions (bit `d` set = step `d` is blocked):
/// bit 0 up, bit 1 down, bit 2 left, bit 3 right. Inner walls and the outer
/// boundary both land here, so the legality check is one load and one AND.
const fn build_walls() -> [u8; W * H] {
    let mut out = [0u8; W * H];
    let mut y = 0usize;
    while y < H {
        let mut x = 0usize;
        while x < W {
            let idx = y * W + x;
            let mut mask = 0u8;
            if y == 0 || !walkable(CELLS[idx - W]) {
                mask |= 1 << DIR_UP;
            }
            if y + 1 >= H || !walkable(CELLS[idx + W]) {
                mask |= 1 << DIR_DOWN;
            }
            if x == 0 || !walkable(CELLS[idx - 1]) {
                mask |= 1 << DIR_LEFT;
            }
            if x + 1 >= W || !walkable(CELLS[idx + 1]) {
                mask |= 1 << DIR_RIGHT;
            }
            // Wall cells can never be stood on; seal them completely so the
            // table is total and needs no bounds reasoning at replay time.
            out[idx] = if CELLS[idx] == WALL { 0xff } else { mask };
            x += 1;
        }
        y += 1;
    }
    out
}

const WALLS: [u8; W * H] = build_walls();

/// Cell index delta for each direction: up, down, left, right.
const DELTA: [isize; 4] = [-(W as isize), W as isize, -1, 1];

/// Flat transition table: `NEXT[pos * 4 + dir]` is the cell reached from
/// `pos` by stepping `dir`, or [`NO_STEP`] when the move is blocked. This
/// removes all index arithmetic from the replay loop.
const fn build_next() -> [u16; W * H * 4] {
    let mut out = [NO_STEP; W * H * 4];
    let mut pos = 0usize;
    while pos < W * H {
        let mut d = 0usize;
        while d < 4 {
            out[pos * 4 + d] = if WALLS[pos] & (1u8 << d) != 0 {
                NO_STEP
            } else {
                (pos as isize + DELTA[d]) as u16
            };
            d += 1;
        }
        pos += 1;
    }
    out
}

const NEXT: [u16; W * H * 4] = build_next();

/// Direction id for a path byte, or [`INVALID_DIR`]. Exactly the four
/// lowercase letters the Daedalus UI sends (`u`, `d`, `l`, `r`) are valid;
/// any other byte, including the uppercase spellings, is not a direction.
/// Exposed so the HTTP layer can classify path bytes before replay.
pub const fn dir_from_char(c: u8) -> u8 {
    match c {
        b'u' => DIR_UP,
        b'd' => DIR_DOWN,
        b'l' => DIR_LEFT,
        b'r' => DIR_RIGHT,
        _ => INVALID_DIR,
    }
}

static DIR_LUT: [u8; 256] = {
    let mut t = [INVALID_DIR; 256];
    let mut i = 0usize;
    while i < 256 {
        t[i] = dir_from_char(i as u8);
        i += 1;
    }
    t
};

/// Flat cell index holding `target`; const panic if the map lacks it.
const fn find_cell(target: u8) -> usize {
    let mut i = 0usize;
    while i < CELLS.len() {
        if CELLS[i] == target {
            return i;
        }
        i += 1;
    }
    panic!("maze art is missing a required cell")
}

/// Flat cell index of the start cell (spec entry point, logical (0, 0)).
pub const START: usize = find_cell(START_CELL);
/// Flat cell index of the exit cell.
pub const EXIT: usize = find_cell(EXIT_CELL);
/// Flat cell index of the minotaur cell.
pub const MINOTAUR: usize = find_cell(MINOTAUR_CELL);

/// Result of replaying a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every step was legal and the final cell is a corridor tile.
    GoingWell,
    /// A step tried to cross an inner wall, leave the grid, or was not one
    /// of the four direction bytes at all. On the Daedalus site any byte
    /// that is not `u`/`d`/`l`/`r` plays exactly like a wall: the client
    /// never learns the difference, so there is no separate invalid state.
    Bonk,
    /// Every step was legal and the final cell is the minotaur cell. The
    /// site serves its minotaur page for that (the move itself succeeds and
    /// the player can keep walking).
    Minotaur,
    /// Every step was legal and the final cell is the exit.
    Solved,
}

/// Detailed result of [`trace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trace {
    /// Overall outcome.
    pub outcome: Outcome,
    /// Flat index of the cell reached after the executed steps.
    pub end: usize,
    /// Number of moves actually executed. On `Bonk`/`Invalid` this is the
    /// length of the last valid prefix, so `path[..steps]` is always legal.
    pub steps: usize,
}

/// Replays `path` from the start cell and reports the outcome plus the exact
/// stopping point.
///
/// Semantics follow the Daedalus site: the first step that would cross a
/// wall or the boundary stops the replay (`Bonk`), a byte that is not one of
/// the four lowercase directions also stops the replay as a `Bonk` (the site
/// renders its bonk page for those), and the special cells only matter as
/// the *final* cell of the path (minotaur page / flag page). Paths longer
/// than [`MAX_PATH_LEN`] bonk on the first step; the cookie format cannot
/// reach that length in a browser.
#[must_use]
pub fn trace(path: &[u8]) -> Trace {
    if path.len() > MAX_PATH_LEN {
        return Trace {
            outcome: Outcome::Bonk,
            end: START,
            steps: 0,
        };
    }
    let mut pos = START;
    let mut steps = 0usize;
    while steps < path.len() {
        let d = DIR_LUT[path[steps] as usize];
        if d == INVALID_DIR {
            return Trace {
                outcome: Outcome::Bonk,
                end: pos,
                steps,
            };
        }
        let next = NEXT[pos * 4 + d as usize];
        if next == NO_STEP {
            return Trace {
                outcome: Outcome::Bonk,
                end: pos,
                steps,
            };
        }
        pos = next as usize;
        steps += 1;
    }
    let outcome = if pos == EXIT {
        Outcome::Solved
    } else if pos == MINOTAUR {
        Outcome::Minotaur
    } else {
        Outcome::GoingWell
    };
    Trace {
        outcome,
        end: pos,
        steps,
    }
}

/// Outcome-only replay.
#[must_use]
pub fn replay(path: &[u8]) -> Outcome {
    trace(path).outcome
}

/// Final cell after a fully successful replay, or `None` on `Bonk`.
#[must_use]
pub fn replay_end(path: &[u8]) -> Option<usize> {
    let t = trace(path);
    match t.outcome {
        Outcome::Bonk => None,
        Outcome::GoingWell | Outcome::Minotaur | Outcome::Solved => Some(t.end),
    }
}

/// The cell reached by stepping `dir` from `pos`, or `None` when the move is
/// blocked (inner wall, boundary, or a wall cell's sealed mask). One table
/// lookup over [`NEXT`], the same transition the replay engine takes.
#[must_use]
pub fn step(pos: usize, dir: u8) -> Option<usize> {
    let next = NEXT[pos * 4 + dir as usize];
    (next != NO_STEP).then_some(next as usize)
}

/// Cell value at array coordinates (`x` right, `y` down). Out-of-bounds reads
/// as wall, which keeps total-table reasoning intact at the borders.
#[must_use]
pub const fn cell_at(x: usize, y: usize) -> u8 {
    if x >= W || y >= H {
        WALL
    } else {
        CELLS[y * W + x]
    }
}

/// Map character for rendering a cell: `#`, `.`, `S`, `M`, or `E`. Used by
/// the UI to render the map; the player marker is drawn by the caller.
#[must_use]
pub const fn cell_char(x: usize, y: usize) -> u8 {
    match cell_at(x, y) {
        EMPTY => b'.',
        WALL => b'#',
        START_CELL => b'S',
        MINOTAUR_CELL => b'M',
        _ => b'E',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base64;

    /// The surveyed solution: a shortest path from the start to the exit,
    /// discovered by the mapper and confirmed by the site's flag page.
    /// Lowercase, exactly as the Daedalus UI writes directions into the
    /// cookie.
    fn solution() -> Vec<u8> {
        let mut s = String::new();
        for (c, n) in [
            ('r', 6), ('d', 2), ('l', 6), ('d', 6), ('r', 2), ('u', 2), ('r', 2), ('d', 2), ('r', 2), ('u', 4), ('r', 2), ('u', 2), ('r', 2), ('u', 2), ('r', 2), ('d', 4), ('r', 2), ('u', 4), ('r', 6), ('d', 2), ('r', 2), ('u', 2), ('r', 4), ('d', 2), ('l', 2), ('d', 4), ('r', 2), ('u', 2), ('r', 4), ('u', 2), ('r', 2), ('u', 2), ('r', 8), ('d', 2), ('r', 6), ('u', 2), ('r', 4), ('d', 2), ('r', 2), ('u', 2), ('r', 4), ('d', 2), ('r', 6), ('u', 2), ('r', 4), ('d', 4), ('l', 2), ('d', 2), ('r', 4), ('d', 2), ('r', 2), ('u', 2), ('r', 2), ('d', 2), ('r', 4), ('u', 4), ('l', 2), ('u', 2), ('r', 4), ('u', 2), ('r', 8), ('d', 2), ('r', 2), ('u', 2), ('r', 4), ('d', 2), ('r', 2), ('d', 2), ('r', 4), ('d', 2), ('l', 2), ('d', 6), ('r', 2), ('d', 4), ('l', 2), ('u', 2), ('l', 2), ('u', 2), ('l', 4), ('d', 4), ('l', 2), ('d', 4), ('l', 2), ('d', 2), ('l', 6), ('d', 6), ('r', 2), ('d', 2), ('l', 2), ('d', 2), ('l', 2), ('u', 12), ('r', 2), ('u', 2), ('l', 4), ('d', 12), ('l', 4), ('d', 2), ('r', 2), ('d', 2), ('r', 6), ('d', 2), ('l', 2), ('d', 2), ('l', 4), ('d', 2), ('l', 4), ('u', 2), ('l', 2), ('u', 2), ('l', 2), ('u', 2), ('r', 2), ('u', 6), ('l', 2), ('d', 2), ('l', 2), ('u', 2), ('l', 2), ('d', 4), ('l', 4), ('u', 2), ('r', 2), ('u', 4), ('r', 2), ('u', 4), ('r', 2), ('d', 4), ('r', 6), ('u', 4), ('r', 4), ('u', 4), ('l', 4), ('d', 2), ('l', 2), ('u', 4), ('l', 2), ('u', 2), ('r', 2), ('u', 2), ('r', 8), ('u', 2), ('l', 10), ('d', 2), ('l', 6), ('d', 2), ('r', 2), ('d', 2), ('l', 6), ('d', 2), ('l', 2), ('u', 4), ('l', 2), ('u', 2), ('l', 2), ('d', 6), ('r', 2), ('d', 2), ('l', 4), ('u', 2), ('l', 2), ('d', 2), ('l', 2), ('d', 4), ('l', 2), ('d', 2), ('r', 2), ('d', 2), ('r', 2), ('u', 2), ('r', 2), ('d', 2), ('r', 4), ('u', 4), ('r', 2), ('u', 2), ('r', 4), ('u', 2), ('r', 2), ('d', 4), ('l', 4), ('d', 2), ('r', 2), ('d', 2), ('l', 2), ('d', 8), ('l', 4), ('d', 2), ('r', 4), ('d', 2), ('l', 6), ('d', 2), ('r', 4), ('d', 8), ('r', 6), ('u', 4), ('r', 4), ('d', 2), ('l', 2), ('d', 8), ('r', 2), ('d', 2), ('l', 2), ('d', 6), ('r', 2), ('u', 4), ('r', 2), ('u', 2), ('r', 4), ('u', 2), ('r', 2), ('u', 2), ('l', 2), ('u', 2), ('l', 4), ('u', 2), ('r', 4), ('u', 2), ('l', 2), ('u', 2), ('r', 2), ('u', 2), ('l', 2), ('u', 2), ('r', 4), ('d', 2), ('r', 2), ('d', 2), ('r', 2), ('u', 2), ('r', 8), ('u', 2), ('r', 2), ('u', 4), ('l', 4), ('u', 2), ('r', 2), ('u', 2), ('l', 2), ('u', 2), ('r', 2), ('u', 8), ('r', 2), ('d', 2), ('r', 6), ('d', 2), ('r', 4), ('d', 4), ('r', 2), ('d', 4), ('r', 2), ('u', 4), ('r', 2), ('u', 2), ('l', 2), ('u', 4), ('r', 2), ('u', 2), ('l', 4), ('d', 2), ('l', 4), ('u', 4), ('l', 4), ('u', 4), ('r', 2), ('d', 2), ('r', 2), ('u', 2), ('r', 4), ('d', 4), ('r', 6), ('d', 4), ('r', 4), ('d', 2), ('r', 2), ('d', 2), ('r', 6), ('d', 2), ('l', 2), ('d', 8), ('r', 2), ('d', 2), ('l', 4), ('u', 2), ('l', 2), ('d', 6), ('l', 2), ('u', 2), ('l', 2), ('d', 6), ('l', 4), ('u', 4), ('l', 2), ('u', 4), ('r', 4), ('u', 4), ('l', 6), ('d', 2), ('l', 2), ('d', 2), ('l', 2), ('u', 2), ('l', 2), ('d', 4), ('r', 4), ('d', 2), ('r', 2), ('d', 6), ('r', 2), ('d', 2), ('l', 4), ('d', 2), ('r', 2), ('d', 10), ('l', 8), ('d', 2), ('r', 4), ('d', 2), ('r', 2), ('u', 2), ('r', 2), ('d', 2), ('r', 4), ('d', 2), ('l', 6), ('d', 2), ('l', 2), ('d', 2), ('r', 2), ('d', 4), ('r', 2), ('u', 6), ('r', 4), ('d', 6), ('r', 2), ('d', 2), ('r', 2), ('d', 2), ('l', 2), ('d', 4), ('r', 2), ('u', 2), ('r', 2), ('d', 4), ('l', 2), ('d', 2), ('r', 4), ('d', 2), ('r', 2), ('d', 6), ('l', 2), ('d', 2), ('r', 8), ('d', 2), ('l', 16), ('d', 2), ('l', 4), ('u', 2), ('r', 2), ('u', 2), ('r', 2), ('u', 2), ('r', 2), ('u', 8), ('l', 4), ('d', 2), ('r', 2), ('d', 2), ('l', 4), ('d', 2), ('r', 2), ('d', 2), ('l', 6), ('u', 2), ('l', 2), ('d', 2), ('l', 2), ('d', 2), ('l', 2), ('d', 2), ('r', 6), ('u', 2), ('r', 2), ('d', 6), ('r', 2), ('d', 2), ('l', 4), ('u', 4), ('l', 2), ('d', 2), ('l', 2), ('u', 2), ('l', 8), ('d', 4), ('l', 2), ('u', 4), ('l', 4), ('u', 2), ('l', 2), ('u', 2), ('l', 2), ('d', 2), ('l', 2), ('d', 2), ('l', 2), ('d', 2), ('r', 2), ('d', 6), ('l', 8), ('u', 4), ('l', 2), ('u', 2), ('r', 2), ('u', 2), ('l', 4), ('d', 4), ('l', 2), ('d', 2), ('r', 4), ('d', 6), ('r', 6), ('u', 2), ('r', 4), ('d', 2), ('r', 4), ('u', 2), ('l', 2), ('u', 2), ('r', 2), ('u', 4), ('r', 4), ('d', 2), ('l', 2), ('d', 2), ('r', 4), ('u', 2), ('r', 2), ('d', 6), ('r', 14), ('u', 2), ('l', 8), ('u', 2), ('r', 6), ('u', 2), ('r', 2), ('d', 2), ('r', 2), ('d', 4), ('r', 2), ('u', 2), ('r', 2), ('u', 4), ('l', 2), ('u', 2), ('r', 2), ('u', 2), ('r', 2), ('u', 2), ('r', 12), ('d', 2), ('r', 2), ('d', 8), ('l', 4), ('d', 2), ('r', 5)
        ] {
            for _ in 0..n {
                s.push(c);
            }
        }
        s.into_bytes()
    }

    #[test]
    fn solution_reaches_exit() {
        let p = solution();
        let t = trace(&p);
        assert_eq!(t.outcome, Outcome::Solved);
        assert_eq!(t.end, EXIT);
        assert_eq!(t.steps, p.len());
    }

    #[test]
    fn empty_path_sits_at_start() {
        let t = trace(b"");
        assert_eq!(t.outcome, Outcome::GoingWell);
        assert_eq!(t.end, START);
        assert_eq!(t.steps, 0);
    }

    #[test]
    fn first_step_into_border_bonks() {
        // Start cell (2, 1): up and down are walls, left and right are open.
        assert_eq!(replay(b"u"), Outcome::Bonk);
        assert_eq!(replay(b"d"), Outcome::Bonk);
        assert_eq!(replay(b"l"), Outcome::GoingWell);
        assert_eq!(replay(b"r"), Outcome::GoingWell);
    }

    #[test]
    fn bonk_reports_last_good_prefix() {
        // Row 1 is a corridor from (1, 1) to (10, 1); (11, 1) is a wall.
        let t = trace(&[b'r'; 9]);
        assert_eq!(t.outcome, Outcome::Bonk);
        assert_eq!(t.steps, 8);
        assert_eq!(t.end, W + 10); // (10, 1), row 1
        assert_eq!(replay(&[b'r'; 9]), Outcome::Bonk);
        assert_eq!(replay(&[b'r'; 8]), Outcome::GoingWell);
    }

    #[test]
    fn any_non_direction_byte_bonks_like_a_wall() {
        // The Daedalus site has no separate invalid state: uppercase
        // spellings, punctuation, anything unknown plays as a wall. The
        // replay stops at the first such byte and reports a bonk.
        for bad in [&b"X"[..], b"R", b"U", b"D", b"N", b" ", b"r r"] {
            assert_eq!(replay(bad), Outcome::Bonk, "path {bad:?}");
        }
        // the byte stops the replay exactly where it occurs
        let t = trace(b"rrX");
        assert_eq!(t.outcome, Outcome::Bonk);
        assert_eq!(t.steps, 2);
        assert_eq!(t.end, W + 4); // (4, 1), row 1
                                  // a good prefix followed by the bad byte still ends in a bonk page
        assert_eq!(replay(b"rrrrrrrrrX"), Outcome::Bonk);
    }

    #[test]
    fn landing_on_the_minotaur_is_a_minotaur_page() {
        // A shortest path onto the minotaur cell: the move is legal, the
        // site shows its minotaur page instead of the going-well page.
        let p = solution_to(MINOTAUR);
        let t = trace(&p);
        assert_eq!(t.outcome, Outcome::Minotaur);
        assert_eq!(t.end, MINOTAUR);
        // The minotaur cell is walkable: stepping back the way we came is a
        // normal move again (only the final cell decides the page).
        let mut back = p.clone();
        back.push(match p[p.len() - 1] {
            b'u' => b'd',
            b'd' => b'u',
            b'l' => b'r',
            _ => b'l',
        });
        assert_eq!(trace(&back).outcome, Outcome::GoingWell);
        assert_eq!(trace(&back).end, back.iter().fold(START, |pos, &c| {
            NEXT[pos * 4 + dir_from_char(c) as usize] as usize
        }));
    }

    /// Builds a legal path to `target` by BFS over the transition table.
    fn solution_to(target: usize) -> Vec<u8> {
        let mut prev = [u16::MAX; W * H];
        let mut prev_dir = [0xffu8; W * H];
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(START);
        prev[START] = START as u16;
        while let Some(pos) = queue.pop_front() {
            if pos == target {
                break;
            }
            for d in 0..4u8 {
                let next = NEXT[pos * 4 + d as usize];
                if next != NO_STEP && prev[next as usize] == u16::MAX {
                    prev[next as usize] = pos as u16;
                    prev_dir[next as usize] = d;
                    queue.push_back(next as usize);
                }
            }
        }
        assert_ne!(prev[target], u16::MAX, "target unreachable");
        let mut path = Vec::new();
        let mut pos = target;
        let letters = [b'u', b'd', b'l', b'r'];
        while pos != START {
            path.push(letters[prev_dir[pos] as usize]);
            pos = prev[pos] as usize;
        }
        path.reverse();
        path
    }

    #[test]
    fn path_length_cap() {
        let ok = vec![b'd'; MAX_PATH_LEN];
        assert_eq!(replay(&ok), Outcome::Bonk); // legal chars, bonks on step 1
        let too_long = vec![b'd'; MAX_PATH_LEN + 1];
        assert_eq!(replay(&too_long), Outcome::Bonk); // capped: bonk, step 0
        assert_eq!(trace(&too_long).steps, 0);
    }

    #[test]
    fn cookie_round_trip() {
        let p = solution();
        let mut enc = [0u8; MAX_COOKIE_LEN];
        let n = base64::encode_into(&p, &mut enc).unwrap();
        assert_eq!(n, base64::encoded_len(p.len()));
        let mut dec = [0u8; MAX_PATH_LEN];
        let m = base64::decode_into(&enc[..n], &mut dec).unwrap();
        assert_eq!(&dec[..m], &p[..]);
        assert_eq!(replay(&dec[..m]), Outcome::Solved);
    }

    #[test]
    fn maze_layout_invariants() {
        assert_eq!(CELLS[START], START_CELL);
        assert_eq!(CELLS[EXIT], EXIT_CELL);
        assert_eq!(CELLS[MINOTAUR], MINOTAUR_CELL);
        assert_eq!(cell_at(2, 1), START_CELL);
        for x in 0..W {
            assert_eq!(cell_at(x, 0), WALL);
            assert_eq!(cell_at(x, H - 1), WALL);
        }
        for y in 0..H {
            assert_eq!(cell_at(0, y), WALL);
            assert_eq!(cell_at(W - 1, y), WALL);
        }
    }

    #[test]
    fn cell_chars_render_the_map() {
        assert_eq!(cell_char(0, 0), b'#');
        assert_eq!(cell_char(2, 1), b'S');
        assert_eq!(cell_char(3, 1), b'.');
        assert_eq!(cell_char(121, 119), b'E');
        assert_eq!(cell_char(70, 69), b'M');
        assert_eq!(cell_char(W, 0), b'#'); // out of bounds reads as wall
    }
}
