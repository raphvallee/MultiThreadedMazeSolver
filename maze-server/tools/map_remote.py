#!/usr/bin/env python3
"""Black-box mapper for the Daedalus maze site.

The server is stateless: the whole path travels base64-encoded in the
`path` cookie, and every request replays it from the start cell. That
lets us probe arbitrary positions directly:

  - take the known-good path to a discovered cell,
  - append one candidate move (u/d/l/r),
  - a bonk page (1730 B) means the move is blocked, any other success
    page means the move is open:
      * 1744 B  going-well page ("Ca avance bien!")
      * 1774 B  minotaur page ("Oh non, le minotaure!...")
      * flag    page containing CEM{...} - the exit

The map is rebuilt from probe facts, not from a queue replay: each
logged request `P+c -> outcome` directly fixes the edge from the cell
reached by `P` in direction `c`. A cell is complete once all four of
its probes are known; incomplete cells are probed live. State is
checkpointed to remote_map/state.json every 500 requests.
"""

import base64
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

BASE_URL = "https://daedalus.defi.info.cegepmontpetit.ca/move"
OUT_DIR = Path(__file__).parent / "remote_map"
STATE_FILE = OUT_DIR / "state.json"

# Outcome signatures per the site spec.
BONK_LEN = 1730
WELL_LEN = 1744  # "Ca avance bien!"
MINOTAUR_LEN = 1774  # "Oh non, le minotaure!"

# Past the remote's path cap every probe would falsely read as bonk.
PATH_WARN = 2500

DELTA = {"u": (0, -1), "d": (0, 1), "l": (-1, 0), "r": (1, 0)}
ORDER = "udlr"

request_count = 0


def classify(body: bytes) -> str:
    n = len(body)
    if n == BONK_LEN:
        return "bonk"
    if n == WELL_LEN:
        return "well"
    if n == MINOTAUR_LEN:
        return "minotaur"
    if b"CEM{" in body:
        return "flag"
    OUT_DIR.mkdir(exist_ok=True)
    (OUT_DIR / f"odd_{n}.html").write_bytes(body)
    raise RuntimeError(f"unexpected response of {n} bytes")


def fetch(path: str) -> str:
    """Play `path` (raw move letters) and classify the outcome."""
    global request_count
    cookie = base64.b64encode(path.encode()).decode()
    req = urllib.request.Request(BASE_URL, headers={"Cookie": f"path={cookie}"})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                body = resp.read()
            break
        except Exception:
            if attempt == 4:
                raise
            time.sleep(0.5 * (attempt + 1))
    request_count += 1
    outcome = classify(body)
    print(
        f"[{request_count:6d}] len={len(path):5d} ...{path[-20:]} -> {outcome}",
        file=sys.stderr,
    )
    return outcome


def walk(path: str):
    """Absolute cell reached by playing `path` from the start (0, 0)."""
    x, y = 0, 0
    for c in path:
        dx, dy = DELTA[c]
        x += dx
        y += dy
    return (x, y)


class Map:
    def __init__(self):
        self.cells = {}  # pos -> type: start|path|minotaur|exit
        self.edges = {}  # pos -> {letter: neighbor pos} for open moves
        self.probed = {}  # pos -> {letter: outcome} for all 4 moves
        self.paths = {}  # pos -> known-good path from start

    # ---- fact application --------------------------------------------
    def add_probe(self, parent_path: str, letter: str, outcome: str):
        parent = walk(parent_path)
        known = self.paths.setdefault(parent, parent_path)
        assert known == parent_path or len(known) >= len(parent_path)
        bucket = self.probed.setdefault(parent, {})
        if letter in bucket and bucket[letter] != outcome:
            raise RuntimeError(
                f"contradiction at {parent} move {letter}: "
                f"{bucket[letter]} vs {outcome}"
            )
        bucket[letter] = outcome
        if outcome == "bonk":
            return
        child = walk(parent_path + letter)
        self.edges.setdefault(parent, {})[letter] = child
        ctype = {"well": "path", "minotaur": "minotaur", "flag": "exit"}[outcome]
        if child not in self.cells:
            self.cells[child] = ctype
            self.paths[child] = parent_path + letter
        elif ctype != "path" and self.cells[child] == "path":
            self.cells[child] = ctype

    def build_from_log(self, log_path: str) -> None:
        """Seed all state from a previous run's stderr log."""
        log = open(log_path, encoding="utf-8", errors="replace").read()
        entries = re.findall(r"\[\s*\d+\] '([udlr]*)' -> (bonk|well)", log)
        print(f"building from {len(entries)} logged probes")
        for path, outcome in entries:
            if not path:
                self.cells[(0, 0)] = "start"
                self.paths[(0, 0)] = ""
                continue
            self.add_probe(path[:-1], path[-1], outcome)
        self.cells[(0, 0)] = "start"
        self.paths.setdefault((0, 0), "")
        print(
            f"cells={len(self.cells)} complete-cells="
            f"{sum(1 for p in self.cells if len(self.probed.get(p, {})) == 4)}"
        )

    # ---- persistence -------------------------------------------------
    def save(self):
        OUT_DIR.mkdir(exist_ok=True)
        STATE_FILE.write_text(
            json.dumps(
                {
                    "cells": {f"{x},{y}": t for (x, y), t in self.cells.items()},
                    "edges": {
                        f"{x},{y}": {d: f"{n[0]},{n[1]}" for d, n in e.items()}
                        for (x, y), e in self.edges.items()
                    },
                    "probed": {f"{x},{y}": p for (x, y), p in self.probed.items()},
                    "paths": {f"{x},{y}": p for (x, y), p in self.paths.items()},
                }
            ),
            encoding="utf-8",
        )

    def load(self) -> bool:
        if not STATE_FILE.exists():
            return False
        data = json.loads(STATE_FILE.read_text(encoding="utf-8"))
        self.cells = {
            tuple(map(int, k.split(","))): v for k, v in data["cells"].items()
        }
        self.edges = {
            tuple(map(int, k.split(","))): {
                d: tuple(map(int, n.split(","))) for d, n in e.items()
            }
            for k, e in data["edges"].items()
        }
        self.probed = {
            tuple(map(int, k.split(","))): v for k, v in data["probed"].items()
        }
        self.paths = {
            tuple(map(int, k.split(","))): v for k, v in data["paths"].items()
        }
        return True

    # ---- live probing ------------------------------------------------
    def pending(self):
        """Cells whose four probes are not all known yet."""
        for pos in self.cells:
            got = self.probed.get(pos, {})
            missing = [c for c in ORDER if c not in got]
            if missing:
                yield pos, self.paths[pos], missing

    def finish(self):
        last_save = request_count
        for pos, path, missing in list(self.pending()):
            if len(path) >= PATH_WARN:
                print(
                    f"WARNING: probe path is {len(path)} moves; the remote "
                    f"may cap path length and fake bonks",
                    file=sys.stderr,
                )
            for letter in missing:
                outcome = fetch(path + letter)
                self.add_probe(path, letter, outcome)
            if request_count - last_save >= 500:
                self.save()
                last_save = request_count
        self.save()

    # ---- output ------------------------------------------------------
    def render(self):
        pts = list(self.cells)
        for e in self.edges.values():
            pts.extend(e.values())
        min_x = min(p[0] for p in pts) - 1
        min_y = min(p[1] for p in pts) - 1
        max_x = max(p[0] for p in pts) + 1
        max_y = max(p[1] for p in pts) + 1
        w, h = max_x - min_x + 1, max_y - min_y + 1
        grid = [["#"] * w for _ in range(h)]
        glyph = {"start": "S", "exit": "E", "minotaur": "M", "path": "."}
        for (x, y), t in self.cells.items():
            grid[y - min_y][x - min_x] = glyph[t]
        return "\n".join("".join(row) for row in grid), w, h

    def report(self):
        art, w, h = self.render()
        print()
        print(art)
        print()
        types = {}
        for t in self.cells.values():
            types[t] = types.get(t, 0) + 1
        exits = [p for p, t in self.cells.items() if t == "exit"]
        print(f"cells discovered: {len(self.cells)} by type: {types}")
        print(f"grid size (incl. wall ring): {w}x{h}")
        print(f"exit cells: {exits}")
        print(f"requests sent this session: {request_count}")
        OUT_DIR.mkdir(exist_ok=True)
        (OUT_DIR / "maze.txt").write_text(art + "\n", encoding="utf-8")
        (OUT_DIR / "maze.json").write_text(
            json.dumps(
                {
                    "width": w,
                    "height": h,
                    "cells": {f"{x},{y}": t for (x, y), t in self.cells.items()},
                    "edges": {
                        f"{x},{y}": {d: f"{n[0]},{n[1]}" for d, n in e.items()}
                        for (x, y), e in self.edges.items()
                    },
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"saved maze.txt / maze.json / state.json under {OUT_DIR}")


def main():
    m = Map()
    primed = False
    if "--prime" in sys.argv:
        m.build_from_log(sys.argv[sys.argv.index("--prime") + 1])
        primed = True
    elif not m.load():
        print("no checkpoint and no --prime; probing from scratch")
        m.cells[(0, 0)] = "start"
        m.paths[(0, 0)] = ""
    m.finish()
    m.report()


if __name__ == "__main__":
    main()
