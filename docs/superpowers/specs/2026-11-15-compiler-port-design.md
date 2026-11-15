# worlded-rusted editor, sub-project 2: the compiler port

Status: draft for review. Date: 2026-11-15. Roadmap: see `2026-11-15-viewer-design.md`.

## Goal

Port the Python map pipeline (`knoxbuild/` plus `tools/map_source.py`) to Rust, so one native
program turns a map folder (`<name>.bmp`, `<name>_veg.bmp`, `<name>.pzw`, `buildings/*.tbx`,
optional zombie spawn bmp) into lot files, with no Python, no PyInstaller and no KnoxMap. The
editor (stages 3 to 5) then builds on Rust code only.

## Scope

In (about 1,700 lines of Python):

| Python | Becomes |
|---|---|
| `bitmaps.py` (BMP strip reader) | `bmp` module |
| `rules.py`, `blends.py` (Rules.txt, Blends.txt) | `rules`, `blends` modules |
| `terrain.py` (colour to tile, hash picks, stack order) | `terrain` module |
| `buildtiles.py`, `buildroofs.py` (.tbx to tiles) | `buildings` module |
| `buildheader.py` (room and building tables) | `header` module |
| `kcell.py`, `map_source.py` (cell sources, merge, rows in parallel) | `source` and `pipeline` modules |
| `tools/worlded.py` | `worlded` binary, same command line |

Out: room objects, vehicle zones, world objects (not solved in Python either); any GUI; changing
what the output contains. The port is behaviour-preserving.

## Architecture

New crate `rust/worlded-compile` (library plus the `worlded` binary), depending on `knoxlots`
(already does `compile` from cell sources to lot files and `verify`). The pipeline stays two
steps in one process: build cell sources in memory, hand them to `knoxlots` directly instead of
writing `.kcell` files to a temp folder (a `--keep-source <dir>` flag still writes them for
debugging and for comparing with Python).

Parallelism: one thread per row of cells (`std::thread::scope`), worker count from CPU count and
free memory, as `make_map_source` does today. Only the strips of the bitmaps a row needs are read.

`worlded.exe` keeps the behaviour added in v0.2.0: double-click asks for the folder, a dragged
folder is used, the window stays open at the end, `config/Rules.txt` and `Blends.txt` are found
next to the program.

## How it is proven

Python is the oracle, kept in the repo (`knoxbuild/`, `tools/`) until parity holds, then kept as
the reference for tests.

- Differential test: for each of the `real1` buildings-only map and generated maps (grass, mixed
  colours, water edges, roads for blends; all 113 `real1` buildings), the Rust cell sources must
  be **byte-identical** to the Python `.kcell` files. Tile picks come from a deterministic hash
  (`mix`, no RNG), so exact equality is achievable; the hash and its integer widths must be
  ported exactly.
- Existing Python unit tests become vectors: for each module, the Rust unit tests reuse the same
  inputs and expected outputs.
- End to end: Rust `worlded` output (lot files) must equal the output of the Python pipeline
  plus `knoxlots compile` on the same inputs, checked with `knoxlots compare`.
- The WorldEd comparison fixtures stay gitignored as now; their check (`cell_check.py`) keeps
  working on Rust output.

## Order of work (each step ends green and committed)

1. `bmp`, `rules`, `blends` parsers with the Python vectors.
2. `terrain` (hash, picks, stack order) byte-equal to Python on generated bitmaps.
3. `buildings` (`.tbx` parse, floors, walls, roofs) equal on the 31 hand-made cases when the
   gitignored fixtures are present, and on all `real1` buildings always.
4. `header` tables, `source` merge, `pipeline`, `worlded` binary.
5. Differential and end-to-end tests; speed check against Python (target: at least as fast as
   Python's 60 s for the 1,500 m map, expected several times faster); release v0.4.0.

## Error handling

Damaged or missing inputs are error values with the file name, never panics (same rule as
`knoxlots`): bad `.bmp`, unreadable `.tbx`, missing `Rules.txt`, a `.pzw` that names a missing
building. A `.tbx` that cannot be read fails the build with its name, as Python does.

## Open risks

- Float and integer widths in `mix` and in the blend neighbour logic: numpy uses `uint64` and
  `int64` wraparound; Rust must use `wrapping_*` explicitly.
- `buildtiles.py` is 410 lines of rules fitted to WorldEd output; a faithful port is mostly
  transcription, and the Python tests are the only specification.
- Memory on very large maps: bitmaps are read in strips; the port must keep that and never load
  a 16,500 x 16,500 image whole.

## Success criteria

On the same inputs, `worlded.exe` (Rust only) writes lot files identical to the Python pipeline,
every comparison above passes, and a large map compiles with no Python installed.
