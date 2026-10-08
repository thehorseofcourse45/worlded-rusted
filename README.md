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
| Building tiles from a `.tbx` | `knoxbuild/buildtiles.py`, `buildroofs.py` | floors, interior and exterior walls, corners, doors, windows, curtains, shutters, ceilings and every roof type KnoxMap writes (flat, 45-degree peaks of every depth with caps, 30-degree peaks and hips) match WorldEd exactly on hand-made buildings. On a real 4-level house the two roof levels match on every square (1,548 and 1,260); the two storeys match on about two thirds of squares, the rest being furniture and lights |

## What does not exist yet

- Building: furniture and lights (the largest gap), stairs, indoor vegetation, floor and wall grime
  on rooms that ask for it; `Peak30Quad` roofs that are not square; windows other than variant 9
  are an assumption (every sample used it).
- Fences, vehicles zones, world objects, the `.pzw` conversion of KnoxMap's buildings into tile layers.
- A `compile_map.py` backend that calls `knoxlots` instead of WorldEd.
- The meaning of two header fields (always 8, 8), the room-object numbers, `chunkdata`'s layout
  (found to be unneeded, not decoded), and the room values inside squares (KnoxMap maps carry
  none; another author's map carries numbers that do not match the header's room table).
- Validation that the game accepts a map made entirely by these tools.

## Layout

```
rust/knoxlots/     the Rust crate and command line (verify, rebuild, export, compile, compare)
knoxbuild/         Python: rules.py, terrain.py, blends.py, kcell.py, buildtiles.py
tools/             terrain_source.py (make cell sources), terrain_check.py, blend_check.py
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
python -m unittest tests.test_terrain tests.test_blends tests.test_terrain_source tests.test_buildtiles
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
