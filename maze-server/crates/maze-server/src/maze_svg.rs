//! SVG rendering of the full maze: every wall, the start, exit and minotaur
//! cells, and route overlays (the A* optimal path, the A* minotaur route,
//! and the least-moves both-flags plan).
//!
//! The maze is static, so the drawing is built once on first request and
//! served as a refcounted clone, exactly like the page bodies: zero
//! allocations per request in steady state. Geometry is derived from the same
//! `WALLS`/`NEXT` rodata the replay engine uses, so what is drawn cannot
//! disagree with what `/move` enforces.

use std::sync::LazyLock;

use bytes::Bytes;
use maze_core::maze::{self, dir_from_char, step};
use maze_core::{both_flags_plan, shortest_path, shortest_path_to_minotaur};

/// Cell edge length in SVG units.
const CELL: usize = 14;
/// Blank margin around the grid.
const PAD: usize = 8;
/// Legend strip height under the grid (one row per route line).
const LEGEND: usize = 72;
/// Height of one legend row.
const LEGEND_ROW: usize = 24;

/// `GET /maze` body: the full-maze visualization with the optimal path.
#[must_use]
pub fn maze_svg() -> Bytes {
    static SVG: LazyLock<Bytes> = LazyLock::new(render);
    SVG.clone()
}

fn render() -> Bytes {
    let width = maze::W * CELL + 2 * PAD;
    let height = maze::H * CELL + 2 * PAD + LEGEND;
    let mut s = String::with_capacity(16 << 10);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
         viewBox=\"0 0 {width} {height}\" font-family=\"monospace\">\n"
    ));
    s.push_str("<rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"#0f1220\"/>\n");
    walls(&mut s);
    labels(&mut s);
    routes(&mut s);
    s.push_str("</svg>\n");
    Bytes::from(s)
}

/// One filled rect per wall cell; corridors stay on the background.
fn walls(s: &mut String) {
    for y in 0..maze::H {
        for x in 0..maze::W {
            if maze::cell_at(x, y) == maze::WALL {
                rect(
                    s,
                    PAD + x * CELL,
                    PAD + y * CELL,
                    CELL,
                    CELL,
                    "#2c3157",
                    "#12152a",
                );
            }
        }
    }
}

/// Start/exit/minotaur fills and letters; the routes run over corridors, so
/// the special cells are marked on top of the wall pass and under the paths.
fn labels(s: &mut String) {
    let (sx, sy) = (maze::START % maze::W, maze::START / maze::W);
    let (ex, ey) = (maze::EXIT % maze::W, maze::EXIT / maze::W);
    let (mx, my) = (maze::MINOTAUR % maze::W, maze::MINOTAUR / maze::W);
    rect(
        s,
        PAD + sx * CELL,
        PAD + sy * CELL,
        CELL,
        CELL,
        "#1d5c38",
        "#1d5c38",
    );
    rect(
        s,
        PAD + ex * CELL,
        PAD + ey * CELL,
        CELL,
        CELL,
        "#6b5410",
        "#6b5410",
    );
    rect(
        s,
        PAD + mx * CELL,
        PAD + my * CELL,
        CELL,
        CELL,
        "#4a1d3f",
        "#4a1d3f",
    );
    for (cx, cy, letter, fill) in [
        (sx, sy, "S", "#8ef0b0"),
        (ex, ey, "E", "#f4d06f"),
        (mx, my, "M", "#ff9ecf"),
    ] {
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" fill=\"{fill}\" font-size=\"16\" font-weight=\"bold\" \
             text-anchor=\"middle\" dominant-baseline=\"central\">{letter}</text>\n",
            PAD + cx * CELL + CELL / 2,
            PAD + cy * CELL + CELL / 2,
        ));
    }
}

/// The route overlays plus their legend: the A* optimal path (solid red),
/// the A* minotaur route, and the least-moves both-flags plan. Dashed thin
/// lines sit on top of the solid path where they share cells, so every
/// overlay stays readable.
fn routes(s: &mut String) {
    let optimal = shortest_path();
    let minotaur = shortest_path_to_minotaur();
    let plan = both_flags_plan();

    overlay(s, &optimal, "#ff5d5d", 6, false);
    overlay(s, &minotaur, "#c084fc", 3, true);
    if plan.exit_run_via_minotaur {
        overlay(s, &plan.exit_run, "#4fd1c5", 3, true);
    }

    legend_row(
        s,
        0,
        "#ff5d5d",
        &format!("A* optimal path ({} moves)", optimal.len()),
    );
    legend_row(
        s,
        1,
        "#c084fc",
        &format!("A* minotaur route ({} moves)", minotaur.len()),
    );
    let tail = if plan.exit_run_via_minotaur {
        format!("exit run via minotaur ({} moves)", plan.exit_run.len())
    } else {
        format!("the optimal path again ({} moves)", plan.exit_run.len())
    };
    let swatch = if plan.exit_run_via_minotaur {
        "#4fd1c5"
    } else {
        "#ff5d5d"
    };
    legend_row(
        s,
        2,
        swatch,
        &format!(
            "both flags in {} moves: minotaur route, then {tail}",
            plan.total_moves
        ),
    );
}

/// One route polyline through the visited cell centers.
fn overlay(s: &mut String, dirs: &[u8], color: &str, width: usize, dashed: bool) {
    let mut points = String::new();
    let mut pos = maze::START;
    points.push_str(&center(pos));
    for &d in dirs.iter() {
        pos = step(pos, dir_from_char(d)).expect("an A* route never crosses a wall");
        points.push(' ');
        points.push_str(&center(pos));
    }
    let dash = if dashed {
        " stroke-dasharray=\"2 6\""
    } else {
        ""
    };
    s.push_str(&format!(
        "<polyline points=\"{points}\" fill=\"none\" stroke=\"{color}\" \
         stroke-width=\"{width}\"{dash} stroke-linecap=\"round\" \
         stroke-linejoin=\"round\" opacity=\"0.9\"/>\n"
    ));
}

/// One legend row: a short swatch line and the route description.
fn legend_row(s: &mut String, row: usize, color: &str, text: &str) {
    let ly = PAD + maze::H * CELL + row * LEGEND_ROW + LEGEND_ROW / 2;
    s.push_str(&format!(
        "<line x1=\"{PAD}\" y1=\"{ly}\" x2=\"{}\" y2=\"{ly}\" stroke=\"{color}\" \
         stroke-width=\"6\" stroke-linecap=\"round\" opacity=\"0.9\"/>\n",
        PAD + 28,
    ));
    s.push_str(&format!(
        "<text x=\"{}\" y=\"{ly}\" fill=\"#aab2d5\" font-size=\"12\" \
         dominant-baseline=\"central\">{text}</text>\n",
        PAD + 36,
    ));
}

fn center(pos: usize) -> String {
    let (x, y) = (pos % maze::W, pos / maze::W);
    format!(
        "{},{}",
        PAD + x * CELL + CELL / 2,
        PAD + y * CELL + CELL / 2
    )
}

fn rect(s: &mut String, x: usize, y: usize, w: usize, h: usize, fill: &str, stroke: &str) {
    s.push_str(&format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{fill}\" \
         stroke=\"{stroke}\"/>\n"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_contains_walls_start_exit_minotaur_and_routes() {
        let svg = String::from_utf8(maze_svg().to_vec()).unwrap();
        assert!(svg.starts_with("<svg "), "root element");
        assert!(svg.contains("<polyline"), "route overlay");
        assert!(svg.contains(">S</text>"), "start label");
        assert!(svg.contains(">E</text>"), "exit label");
        assert!(svg.contains(">M</text>"), "minotaur label");
        // one rect per wall cell (computed from the map, not hardcoded),
        // plus the background, the start fill, the exit fill, and the
        // minotaur fill
        let walls = (0..maze::W * maze::H)
            .filter(|&i| maze::cell_at(i % maze::W, i / maze::W) == maze::WALL)
            .count();
        assert_eq!(
            svg.matches("<rect").count(),
            walls + 4,
            "wall + background + start + exit + minotaur rects"
        );
        assert_eq!(
            svg.matches(&format!(
                "A* optimal path ({} moves)",
                shortest_path().len()
            ))
            .count(),
            1
        );
        assert_eq!(
            svg.matches(&format!(
                "A* minotaur route ({} moves)",
                shortest_path_to_minotaur().len()
            ))
            .count(),
            1
        );
        let plan = both_flags_plan();
        let expected = format!("both flags in {} moves", plan.total_moves);
        assert_eq!(svg.matches(&expected).count(), 1, "both-flags legend");
        // three legend rows means three swatch lines
        assert_eq!(svg.matches("<line ").count(), 3, "legend rows");
    }

    #[test]
    fn svg_is_cached() {
        let a = maze_svg();
        let b = maze_svg();
        assert_eq!(a.as_ptr(), b.as_ptr(), "steady-state clones share rodata");
    }
}
