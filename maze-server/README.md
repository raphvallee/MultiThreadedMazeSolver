# maze

A faithful Rust recreation of the Daedalus maze site
(`daedalus.defi.info.cegepmontpetit.ca`): sub-millisecond, fully stateless
(spec: `maze_application_specification.md`). The wire behavior was captured
from the original server and is reproduced byte for byte - page templates,
response sizes, cookies, redirects and error bodies. All heavy work is
precomputed at compile time and the hot path is allocation-free.

The bundled map is the surveyed layout of the live site (v3105), discovered
by black-box probing with the mapper in `tools/` (see "Mapper" below):
123x121 cells including the wall ring, 7082 walkable cells, one minotaur
cell and one exit in the bottom-right corner.

## Layout

```
crates/
  maze-core/   Stateless maze engine: compile-time map + transition tables,
               path replay, zero-allocation base64 cookie codec.
  maze-server/ HTTP layer: byte-exact page templates (templates/), embedded
               site assets (assets/), shared router, hyper + raw backends.
```

## Milestones

- [x] 1. Core lib (map, tables, replay, base64, full test coverage)
- [x] 2. HTTP server (hyper, exact-size spec responses, cookie handling)
- [x] 3. UI page (Daedalus clone: D-pad UI, images, full-page navigation)
- [x] 4. Optimization pass (criterion benches, allocation audit, load numbers)
- [x] 5. Optional raw-socket server behind a feature flag
- [x] 6. Daedalus clone pass (byte-identical pages, site semantics)

## Run

```
cargo run --release -p maze-server -- 127.0.0.1:8173
# or the hand-rolled HTTP/1.1 backend (no hyper):
cargo run --release -p maze-server --features raw -- 127.0.0.1:8173
```

Endpoints, exactly like the original site:

- `GET /` - start page (French intro, D-pad, tiresias image); sets a fresh
  `path` cookie: `path=; Path=/` (session scope, no Max-Age)
- `GET /move` - the whole path travels base64-encoded in the `path` cookie;
  the URL carries no parameters, the server never rewrites the cookie, and
  the client (`send.js`, embedded) appends the move and navigates here.
  Outcome pages: `BONK!` exactly 1730 bytes, `Ça avance bien!` exactly 1744
  bytes, `le minotaure` exactly 1774 bytes (legal move onto the minotaur
  cell), flag page on the exit
- `GET /static/...` - the seven embedded assets the pages reference
  (css, js, tiresias.jpg, progress.png, bonk.jpeg, success.png, nice.jpg)

There is no `/reset` (the site 404s it): restarting is `GET /`, which is
what the "Je me suis perdu :(" button links to. Site semantics, verified
against the original server:

- `/move` without any cookie header: 302 to `/` (Flask body, no Set-Cookie)
- `/move` with an empty cookie: going-well page (start cell)
- a non-direction byte plays as a wall: bonk page (uppercase spellings,
  punctuation, anything unknown - only lowercase `u d l r` are directions)
- base64 decoding is lenient exactly like Python's `b64decode(...,
  validate=False)`: non-alphabet bytes are discarded; structurally broken
  values (missing padding, misplaced `=`) answer 500 with the site's body
- a bonked move stays in the cookie forever (the client never truncates):
  after a bonk every further move still bonks until you restart

Spec byte sizes hold: bonk 1730, going well 1744, minotaur 1774, flag any
other length.
The flag lives in `bodies::FLAG_MESSAGE` (the site's real flag:
`CEM{D3D4L3_3T_L3_F1L_D3_4R14DNE}`).

## Mapper

`tools/map_remote.py` black-box maps the live site. The server is stateless
(the whole path travels in the cookie), so the mapper walks a known-good
path to a discovered cell and appends one probe move per direction:
bonk = wall, any success page = open passage. It classifies the four
outcome pages by exact byte size (1730/1744/1774 + flag detection), BFSes
until every reachable cell has all four probes answered, checkpoints to
`tools/remote_map/state.json`, and can seed a run from a previous session's
log (`--prime`). Artifacts in `tools/remote_map/`:

- `maze.txt` / `maze.json` - the surveyed map (ASCII art + full cell graph)
- `state.json` - resumable mapper state: cells, edges, per-cell probes
- `solution.txt` - a verified 1457-move shortest path to the flag

`tools/cross_check.py` replays sampled paths against both the live site and
a running clone and asserts the outcome sizes match.

## Performance (measured, see BENCHMARKS section)

Engine and handler are allocation-free or near-free; end-to-end loopback
latency is far below the sub-millisecond goal:

| Path | Throughput | p50 | p99 |
|---|---|---|---|
| `GET /move` (well page, 1744 B) | 97,800 rps | 37 µs | 93 µs |
| `GET /move` (16 workers, peak) | 151,700 rps | 101 µs | 213 µs |
| `GET /move` (bonk page, 1730 B) | 98,000 rps | 37 µs | 94 µs |
| `GET /` (start page) | 120,700 rps | 29 µs | 78 µs |
| `GET /reset` (404) | 124,000 rps | 29 µs | 75 µs |

Pure handler cost (criterion, no parse/socket layers): 119-272 ns per request.
Replaying the maximum 3072-move path: 4.1 µs; typical paths: 6-120 ns.
Zero heap allocations in the engine; 2-4 allocations per HTTP request
(response header map + one page buffer).

## Benchmarks & load testing

```
cargo bench -p maze-core                       # engine: replay + base64
cargo bench -p maze-server                     # handler: route_parts
cargo test -p maze-core --test alloc -- --nocapture        # alloc audit
cargo test -p maze-server --test alloc_audit -- --nocapture
cargo run --release -p maze-server --example loadgen -- 127.0.0.1:8091 /move ZGRk 4 8
```

For best results build with `RUSTFLAGS="-C target-cpu=native"`. The loadgen
holds one keep-alive connection per worker and records end-to-end latency per
request; numbers above were taken on loopback (Windows 11, release profile).

## Backends

Two HTTP backends share one router (`route_ctx` over a backend-neutral
`RequestCtx`), so behavior and response bytes are identical:

- **hyper** (default): hyper 1.x, IOCP, `writev` responses.
- **raw** (`--features raw`): hand-rolled HTTP/1.1 parser + serializer over
  tokio sockets, one task per connection. An earlier blocking-`std::net`
  version measured ~4x worse latency (blocked threads must be scheduled back
  in per request; overlapped reads do not), so the raw backend uses async I/O
  on purpose.

Raw backend, same load as the table above: `/move` 97,000 rps (p50 35.5 µs,
p99 119 µs), peak 145,000 rps, `/` 100,400 rps, `/reset` (404) 101,500 rps.
Within
measurement noise of hyper on `/move`; hyper keeps a small edge (~15%) on the
small-body endpoints thanks to its zero-copy `writev` path. All 23
maze-server tests, including the full wire suite, run against both backends:

```
cargo test -p maze-server                  # hyper backend
cargo test -p maze-server --features raw   # raw backend
```