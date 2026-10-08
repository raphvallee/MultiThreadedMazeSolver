//! A* solver over the precomputed maze tables.
//!
//! The maze is static, so optimal paths are a search away: A* with the
//! Manhattan-distance heuristic over the same blocked-direction masks the
//! replay engine uses, which makes a solved A* path replay `Outcome::Solved`
//! (or `Outcome::Minotaur` for the minotaur route) by construction. The
//! search allocates freely (it is off the request hot path); the per-cell
//! state lives in flat arrays sized `W * H`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::maze::{step, EXIT, H, MINOTAUR, START, W};

/// Direction ids in the order [`DIR_CHARS`] spells them.
const DIRS: [u8; 4] = [0, 1, 2, 3];
/// Direction byte for each direction id, exactly the letters the site's UI
/// writes into the cookie (the inverse of [`crate::maze::dir_from_char`]).
const DIR_CHARS: [u8; 4] = *b"udlr";
/// came-from marker for cells that have no arrival direction (the start).
const NO_ARRIVAL: u8 = 0xff;

/// Manhattan distance from `pos` to `target`, in cells. Admissible and
/// consistent on a 4-connected grid, so the first target pop is optimal.
fn heuristic(pos: usize, target: usize) -> u16 {
    let (x, y) = ((pos % W) as u16, (pos / W) as u16);
    let (tx, ty) = ((target % W) as u16, (target / W) as u16);
    x.abs_diff(tx) + y.abs_diff(ty)
}

/// An optimal path from `from` to `to` as lowercase direction bytes
/// (`u`/`d`/`l`/`r`), or an empty vector if there is no route (impossible
/// between open cells of the bundled map). Ties are broken in direction
/// order, so the result is deterministic.
fn a_star(from: usize, to: usize) -> Vec<u8> {
    let mut dist = [u16::MAX; W * H];
    let mut came = [NO_ARRIVAL; W * H];
    let mut open: BinaryHeap<Reverse<(u16, usize)>> = BinaryHeap::new();
    dist[from] = 0;
    open.push(Reverse((heuristic(from, to), from)));

    while let Some(Reverse((f, pos))) = open.pop() {
        if pos == to {
            break;
        }
        // Stale entry: a cheaper route to `pos` was already expanded.
        if f - heuristic(pos, to) > dist[pos] {
            continue;
        }
        let g = dist[pos] + 1;
        for d in DIRS {
            let Some(next) = step(pos, d) else { continue };
            if g < dist[next] {
                dist[next] = g;
                came[next] = d;
                open.push(Reverse((g + heuristic(next, to), next)));
            }
        }
    }

    // Unreachable target: it was never reached.
    if came[to] == NO_ARRIVAL && to != from {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut pos = to;
    while pos != from {
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

/// An optimal path from the start cell to the exit as lowercase direction
/// bytes (`u`/`d`/`l`/`r`), or an empty vector if the maze has no solution
/// (impossible for the bundled map). Ties are broken in direction order, so
/// the result is deterministic.
#[must_use]
pub fn shortest_path() -> Vec<u8> {
    a_star(START, EXIT)
}

/// An optimal path from the start cell to the minotaur cell, in the same
/// format and with the same guarantees as [`shortest_path`].
#[must_use]
pub fn shortest_path_to_minotaur() -> Vec<u8> {
    a_star(START, MINOTAUR)
}

/// The least-moves plan to collect both outcomes the site rewards: the
/// minotaur encounter and the flag on the exit.
///
/// The site decides the outcome from the *final* cell of a walk, so no
/// single walk can earn both: it takes two `/move` requests. Walk 1 is the
/// A* start->minotaur walk. Walk 2 is the cheapest walk that ends on the
/// exit: either the direct A* start->exit route or the minotaur walk
/// extended with the A* minotaur->exit route, whichever sends fewer moves
/// overall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BothFlagsPlan {
    /// Walk 1: start -> minotaur. Its response is the minotaur page.
    pub minotaur_run: Vec<u8>,
    /// Walk 2: start -> exit. Its response is the flag page.
    pub exit_run: Vec<u8>,
    /// Whether `exit_run` detours through the minotaur cell (that variant
    /// wins only when it is strictly cheaper than the direct route).
    pub exit_run_via_minotaur: bool,
    /// Total moves sent across the two walks.
    pub total_moves: usize,
}

/// The least-moves two-walk plan that earns the minotaur encounter and the
/// exit flag, as documented on [`BothFlagsPlan`].
#[must_use]
pub fn both_flags_plan() -> BothFlagsPlan {
    let minotaur_run = a_star(START, MINOTAUR);
    let direct = a_star(START, EXIT);
    let tail = a_star(MINOTAUR, EXIT);
    let via = minotaur_run.len() + tail.len();
    if direct.len() <= via {
        BothFlagsPlan {
            total_moves: minotaur_run.len() + direct.len(),
            minotaur_run,
            exit_run: direct,
            exit_run_via_minotaur: false,
        }
    } else {
        let mut exit_run = minotaur_run.clone();
        exit_run.extend_from_slice(&tail);
        BothFlagsPlan {
            total_moves: minotaur_run.len() + exit_run.len(),
            minotaur_run,
            exit_run_via_minotaur: true,
            exit_run,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maze::{dir_from_char, trace, Outcome};

    /// Breadth-first distance from `from` to `to`, the true optimum. Used to
    /// prove A* optimality without hardcoding a path length.
    fn bfs_distance(from: usize, to: usize) -> usize {
        let mut dist = [usize::MAX; W * H];
        let mut queue = std::collections::VecDeque::new();
        dist[from] = 0;
        queue.push_back(from);
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
        dist[to]
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
        assert_eq!(shortest_path().len(), bfs_distance(START, EXIT));
    }

    #[test]
    fn shortest_path_uses_only_direction_bytes() {
        for &b in shortest_path().iter() {
            assert_ne!(dir_from_char(b), 0xff, "byte {b}");
        }
    }

    #[test]
    fn minotaur_path_replays_to_minotaur() {
        let p = shortest_path_to_minotaur();
        assert!(!p.is_empty(), "the minotaur cell is reachable");
        let t = trace(&p);
        assert_eq!(t.outcome, Outcome::Minotaur);
        assert_eq!(t.end, MINOTAUR);
        assert_eq!(t.steps, p.len());
        assert_eq!(p.len(), bfs_distance(START, MINOTAUR));
    }

    #[test]
    fn both_flags_plan_walks_replay_to_their_outcomes() {
        let plan = both_flags_plan();
        assert_eq!(trace(&plan.minotaur_run).outcome, Outcome::Minotaur);
        let exit = trace(&plan.exit_run);
        assert_eq!(exit.outcome, Outcome::Solved);
        assert_eq!(exit.steps, plan.exit_run.len());
        assert_eq!(
            plan.total_moves,
            plan.minotaur_run.len() + plan.exit_run.len()
        );
        // The plan is total-moves minimal: walk 1 is the optimal minotaur
        // walk and walk 2 is the optimal exit walk (direct or via minotaur).
        assert_eq!(plan.minotaur_run.len(), bfs_distance(START, MINOTAUR));
        let via_minotaur = bfs_distance(START, MINOTAUR) + bfs_distance(MINOTAUR, EXIT);
        assert_eq!(
            plan.exit_run.len(),
            bfs_distance(START, EXIT).min(via_minotaur)
        );
        assert_eq!(
            plan.exit_run_via_minotaur,
            via_minotaur < bfs_distance(START, EXIT)
        );
    }
}
