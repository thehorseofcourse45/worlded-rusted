# worlded-rusted

Work in progress toward a Project Zomboid (Build 42) map compiler for
[KnoxMap](https://github.com/spytheeuclidean-a11y/knoxmap) that does not depend on WorldEd:
no bitmap size limit, no per-batch start-up cost, and much faster.

**This is unfinished.** Nothing here compiles a complete map yet. It is shared so others can
help and so the findings are not lost. MIT licensed.

## Why

WorldEd's BMP to TMX step cannot load a landscape bitmap past about 2^28 pixels (16,200 x 16,500
loads, 16,500 x 16,500 fails with "The image file couldn't be loaded"), and a compile of a large
town is hours of batches that each reload the tile catalogue.

## Clean-room

WorldEd is GPL-2.0; this project is MIT. So it is written from **file formats and observed
behaviour**, not from WorldEd's code:

- file layouts were found by reading real lot files and checking every file of several maps;
- what WorldEd does with a map was found by running WorldEd as a program on small hand-made
  inputs and comparing its output;
- data files that ship with the mapping tools (`Rules.txt`, `Blends.txt`, BuildingEd's `.tbx`)
  are read as data.

Before the clean-room decision the author's assistant read the declarations and function names of
WorldEd's lot writer, and one source comment; no layouts or logic were taken. Contributors who
read WorldEd's source should say so in their pull request, so the record stays honest.

## What exists

| Part | Where | State |
|---|---|---|
| Lot files: read, write, round-trip | `rust/knoxlots` | `.lotheader`, `.lotpack`, `chunkdata` read and written byte-for-byte on 9 maps (6,578 cells), including two by other authors |
| Cell source format and `compile` | `rust/knoxlots`, `SOURCE_FORMAT.md` | export from a real map then compile gives identical `.lotheader` and `.lotpack` on 4 maps (2,284 cells); zombie bytes computed from the spawn map match WorldEd exactly |
| Blank `chunkdata` | | tested in-game (Build 42): a map with every `chunkdata` blank played normally, so it need not be reproduced |
| Ground tiles from the landscape and vegetation pictures | `knoxbuild/rules.py`, `terrain.py`, `tools/terrain_source.py` | per-colour tile choice matches WorldEd's shares; about 49 ms a cell, parallel across rows, strips of the picture only |
| Edge blends | `knoxbuild/blends.py` | overlay squares and kinds identical to WorldEd on a road map (3,126 squares) and a water map (603) |
| Building tiles from a `.tbx` | `knoxbuild/buildtiles.py`, `buildroofs.py` | floors, walls, corners, doors, windows, curtains, shutters, furniture, stairs, ceilings, roofs, and painted tile layers (porch lights) match WorldEd on the 113-building test map (99.4% of building squares before ground is added; 100% of squares of the whole map once it is) |
| Header room and building tables | `knoxbuild/buildheader.py`, `fixtures/headercheck.py` | rooms and buildings built from the `.tbx` files equal WorldEd's headers on 8 of 9 cells of the test map; room objects are not written |
| A whole map from a project folder | `tools/map_source.py`, `tools/cell_check.py` | ground, blends, buildings, fences, lights and header tables for all 9 cells of `fixtures/real/real1`; compiled by `knoxlots compile`, **every square of every level equals WorldEd's** (422,696 squares; random tile picks compared by Rules.txt alias). A 1,500 m map of 1,177 buildings (25 cells compared): 2,020,658 of 2,020,734 squares equal (the rest are window/wall details on upper floors); Rust 60 s for 64 cells, WorldEd 271 s |

## What does not exist yet

- Room objects in headers (a few markers in about 7% of buildings, no rule found), two rare WorldEd
  room merges, vehicle zones, world objects.
- Only one real map is compared whole (9 cells); the rules are fitted to it. More maps, other
  tilesets and window families, and 30-degree roof shapes seen only there may still differ.
- A `compile_map.py` backend in KnoxMap that calls `knoxlots` instead of WorldEd.
- The meaning of two header fields (always 8, 8), `chunkdata`'s layout (unneeded).
- Validation that the game accepts a map made entirely by these tools.

## Layout

```
rust/knoxlots/     the Rust crate and command line (verify, rebuild, export, compile, compare)
knoxbuild/         Python: rules.py, terrain.py, blends.py, kcell.py, buildtiles.py, buildroofs.py, buildheader.py
tools/             map_source.py (a whole map), cell_check.py (compare with WorldEd), terrain_source.py, terrain_check.py, blend_check.py
tests/             unit tests for the Python parts
experiments/       the scripts used to find all of the above (see below)
```

The Python parts are meant to be laid over a [KnoxMap](https://github.com/spytheeuclidean-a11y/knoxmap)
checkout (they import `knoxbuild.bitmaps` and `knoxpaths` from it); copy these folders into it, or
set `KNOXMAP_DIR` for the experiment scripts. The mapping tools (WorldEd) must be installed as KnoxMap
installs them for the experiments and for `Rules.txt` / `Blends.txt`.

## Build and test

```
cd rust/knoxlots && cargo test --release && cargo build --release
python -m unittest discover -s tests -p "test_[bt]*.py"
python tools/map_source.py rust/knoxlots/fixtures/real/real1 out.src --worlded-seams
knoxlots compile out.src out.lots
python tools/cell_check.py out.lots rust/knoxlots/fixtures/real/real1/lots
```

The Rust command line: see `rust/knoxlots/README.md`.

## The experiments

`experiments/` holds the scripts that produced each finding. They write into folders next to
themselves and expect a KnoxMap install with the mapping tools. In brief:
`sizetest*.py` the bitmap size limit; `exp.py` small maps differing in one feature, compiled by
WorldEd; `ground.py`, `blendtest.py`, `blendtable.py` ground tiles and blend rules; `bx.py`,
`bxshow.py`, `bxcheck.py`, `housecheck.py` hand-made buildings and the comparison with a real one;
`hdr2.py`, `pack.py`, `order.py`, `tiletable.py` the lot file layouts.

## Method, so it can be continued

Change one thing, have WorldEd compile it, read exactly what it wrote, state the rule, implement it,
and compare square by square. A rule is only kept when it reproduces WorldEd on every square tested.

Fixture maps under `rust/knoxlots/fixtures` hold geometry derived from OpenStreetMap data (ODbL, (c) OpenStreetMap contributors).
