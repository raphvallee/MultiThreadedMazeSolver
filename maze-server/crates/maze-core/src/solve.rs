//! A* solver over the precomputed maze tables.
//!
//! The maze is static, so the optimal path is a search away: A* with the
//! Manhattan-distance heuristic over the same blocked-direction masks the
//! replay engine uses, which makes a solved A* path replay `Outcome::Solved`
//! by construction. The search allocates freely (it is off the request hot
//! path); the per-cell state lives in flat arrays sized `W * H`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::maze::{step, EXIT, H, START, W};

/// Direction ids in the order [`DIR_CHARS`] spells them.
const DIRS: [u8; 4] = [0, 1, 2, 3];
/// Direction byte for each direction id, exactly the letters the site's UI
/// writes into the cookie (the inverse of [`crate::maze::dir_from_char`]).
const DIR_CHARS: [u8; 4] = *b"udlr";
/// came-from marker for cells that have no arrival direction (the start).
const NO_ARRIVAL: u8 = 0xff;

/// Manhattan distance from `pos` to the exit, in cells. Admissible and
/// consistent on a 4-connected grid, so the first exit pop is optimal.
fn heuristic(pos: usize) -> u16 {
    let (x, y, ex, ey) = (
        (pos % W) as u16,
        (pos / W) as u16,
        (EXIT % W) as u16,
        (EXIT / W) as u16,
    );
    x.abs_diff(ex) + y.abs_diff(ey)
}

/// An optimal path from the start cell to the exit as lowercase direction
/// bytes (`u`/`d`/`l`/`r`), or an empty vector if the maze has no solution
/// (impossible for the bundled map). Ties are broken in direction order, so
/// the result is deterministic.
#[must_use]
pub fn shortest_path() -> Vec<u8> {
    let mut dist = [u16::MAX; W * H];
    let mut came = [NO_ARRIVAL; W * H];
    let mut open: BinaryHeap<Reverse<(u16, usize)>> = BinaryHeap::new();
    dist[START] = 0;
    open.push(Reverse((heuristic(START), START)));

    while let Some(Reverse((f, pos))) = open.pop() {
        if pos == EXIT {
            break;
        }
        // Stale entry: a cheaper route to `pos` was already expanded.
        if f - heuristic(pos) > dist[pos] {
            continue;
        }
        let g = dist[pos] + 1;
        for d in DIRS {
            let Some(next) = step(pos, d) else { continue };
            if g < dist[next] {
                dist[next] = g;
                came[next] = d;
                open.push(Reverse((g + heuristic(next), next)));
            }
        }
    }

    // Unsolvable maze: the exit was never reached.
    if came[EXIT] == NO_ARRIVAL && EXIT != START {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut pos = EXIT;
    while pos != START {
        // `came` stores the arrival direction; the step back to the previous
        // cell is its opposite (up<->down are ids 0/1, left<->right are 2/3).
        let d = came[pos];
        debug_assert!(d != NO_ARRIVAL, "reconstruction left the came-from graph");
        path.push(DIR_CHARS[d as usize]);
        pos = step(pos, d ^ 1).expect("the opposite of a legal step is legal");
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maze::{dir_from_char, trace, Outcome};

    /// Breadth-first distance from start to exit, the true optimum. Used to
    /// prove A* optimality without hardcoding a path length.
    fn bfs_distance() -> usize {
        let mut dist = [usize::MAX; W * H];
        let mut queue = std::collections::VecDeque::new();
        dist[START] = 0;
        queue.push_back(START);
        while let Some(pos) = queue.pop_front() {
            for d in DIRS {
                if let Some(next) = step(pos, d) {
                    if dist[next] == usize::MAX {
                        dist[next] = dist[pos] + 1;
                        queue.push_back(next);
                    }
                }
            }
        }
        dist[EXIT]
    }

    #[test]
    fn shortest_path_replays_to_solved() {
        let p = shortest_path();
        assert!(!p.is_empty(), "the bundled maze is solvable");
        let t = trace(&p);
        assert_eq!(t.outcome, Outcome::Solved);
        assert_eq!(t.end, EXIT);
        assert_eq!(t.steps, p.len());
    }

    #[test]
    fn shortest_path_is_optimal() {
        assert_eq!(shortest_path().len(), bfs_distance());
    }

    #[test]
    fn shortest_path_uses_only_direction_bytes() {
        for &b in shortest_path().iter() {
            assert_ne!(dir_from_char(b), 0xff, "byte {b}");
        }
    }
}
