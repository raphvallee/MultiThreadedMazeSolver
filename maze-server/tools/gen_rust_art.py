"""Generate the Rust MAZE_ART source from the mapper's maze.json."""

import json

m = json.load(open(r"tools\remote_map\maze.json"))
cells = {tuple(map(int, k.split(","))): v for k, v in m["cells"].items()}
edges = {tuple(map(int, k.split(","))): e for k, e in m["edges"].items()}

pts = list(cells)
for e in edges.values():
    for n in e.values():
        pts.append(tuple(map(int, n.split(","))))
min_x = min(p[0] for p in pts) - 1
min_y = min(p[1] for p in pts) - 1
max_x = max(p[0] for p in pts) + 1
max_y = max(p[1] for p in pts) + 1
w, h = max_x - min_x + 1, max_y - min_y + 1

grid = [["#"] * w for _ in range(h)]
glyph = {"start": "S", "exit": "E", "minotaur": "M", "path": "."}
for (x, y), t in cells.items():
    grid[y - min_y][x - min_x] = glyph[t]

assert all(c == "#" for c in grid[0]) and all(c == "#" for c in grid[-1])
assert all(r[0] == "#" and r[-1] == "#" for r in grid)

rows = "\n".join('        "' + "".join(r) + '\\n",' for r in grid)
body = "concat!(\n" + rows + "\n    )"
open(r"tools\remote_map\maze_art.rs.txt", "w", encoding="utf-8").write(body)

print("w", w, "h", h)
print("S at", [(r.index("S"), i) for i, r in enumerate(grid) if "S" in r])
print("E at", [(r.index("E"), i) for i, r in enumerate(grid) if "E" in r])
print("M at", [(r.index("M"), i) for i, r in enumerate(grid) if "M" in r])
print("open cells", sum(r.count(".") for r in grid) + 3)

# also the RLE solution for the Rust tests
sol = open(r"tools\remote_map\solution.txt").read().strip()
runs = []
for c in sol:
    if runs and runs[-1][0] == c:
        runs[-1][1] += 1
    else:
        runs.append([c, 1])
rle = ", ".join(f"('{c}', {n})" for c, n in runs)
open(r"tools\remote_map\solution_rle.txt", "w").write(rle)
print("solution moves:", len(sol), "runs:", len(runs))
print(rle[:200], "...")
