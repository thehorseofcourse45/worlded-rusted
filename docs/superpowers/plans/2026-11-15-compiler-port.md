# Editor Stage 2: Compiler Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One Rust program (`worlded`) turns a map folder into lot files, byte-identical to the Python pipeline, with no Python or KnoxMap.

**Architecture:** New crate `rust/worlded-compile` (library plus `worlded` binary) ported module by module from `knoxbuild/` and `tools/map_source.py`, depending on `knoxlots` for cell sources to lot files. Python stays as the oracle: a differential test runs both on the same inputs and compares `.kcell` bytes.

**Tech Stack:** Rust 2021, `roxmltree` (tbx), existing `knoxlots`; no numpy equivalent: plain `Vec` grids with explicit `wrapping_*` integer maths. Python (miniconda, numpy, PIL) only in tests.

**Spec:** `docs/superpowers/specs/2026-11-15-compiler-port-design.md`

## Global Constraints

- Behaviour-preserving: output must equal the Python output byte for byte; no new features.
- Tile picks use the hash `mix` in `knoxbuild/terrain.py` (no RNG); port it with explicit `u64` wrapping arithmetic matching numpy `uint64`.
- Damaged or missing input is an error value naming the file, never a panic.
- Bitmaps are read in strips (rows needed by a row of cells), never whole.
- `knoxbuild/` and `tools/` Python stays in the repo unchanged as the oracle.
- Clean-room rule from the README still applies; this is a port of this repo's own MIT code.
- `worlded.exe` keeps v0.2.0 behaviour: double-click prompts, dragged folder works, window waits, `config/` next to the exe.

## Review Focus

- A `.tbx` that fails to parse: build fails with that file name, no partial output reported as success.
- A map folder missing `<name>_veg.bmp` or `Rules.txt`: clear error, no panic.
- Bitmap of 24-bit, 8-bit, top-down and bottom-up layouts, and width not a multiple of 4 (row padding).
- A building lot that reaches past the map edge or into a negative cell: clipped as Python does.
- Integer wraparound in `mix` for large coordinates (16,500 x 16,500 map).

## File Structure

- `rust/worlded-compile/src/{lib,bmp,rules,blends,terrain,tbx,buildtiles,buildroofs,header,source,pipeline}.rs`; `src/bin/worlded.rs`.
- `rust/worlded-compile/tests/`: one test file per module plus `differential.rs`.
- `tools/dump_for_rust.py`: runs the Python modules on given inputs and writes expected outputs (JSON or `.kcell`) for the Rust tests.
- Fixtures: `rust/knoxlots/fixtures/real/real1` (buildings, pzw), generated bitmaps from `tools/make_test_map.py` (new, deterministic).

### Task 1: Crate, bitmap reader, test map generator

**Files:**
- Create: `rust/worlded-compile/Cargo.toml`, `src/lib.rs`, `src/bmp.rs`, `tests/bmp.rs`, `tools/make_test_map.py`
- Modify: `rust/Cargo.toml` (add member)

**Interfaces:**
- Produces: `pub enum CompileError { Io(PathBuf, io::Error), Parse(PathBuf, String) }`; `pub struct BmpInfo { pub width: u32, pub height: u32 }`; `pub fn bmp_info(path: &Path) -> Result<BmpInfo, CompileError>`; `pub fn read_rgb_rows(path: &Path, top: i64, bottom: i64, width: u32) -> Result<Vec<u8>, CompileError>` returning `(bottom-top) x width x 3` RGB bytes, rows outside the image black, matching `bitmaps.read_rgb_rows`.

- [ ] **Step 1: Write failing tests** in `tests/bmp.rs`: `reads_24bit_bottom_up_with_row_padding` (a 5x3 BMP built in the test, width not a multiple of 4), `reads_8bit_palette`, `rows_outside_image_are_black`, `top_down_bmp_matches_bottom_up`, `truncated_and_non_bmp_are_errors`; plus `matches_python_on_generated_map` comparing against `tools/dump_for_rust.py bmp <file> top bottom` output (skips if `python` is absent).
- [ ] **Step 2: Run** `cargo test -p worlded-compile --test bmp`; expect FAIL (undefined).
- [ ] **Step 3: Implement** `bmp.rs` by porting `knoxbuild/bitmaps.py` (`_bmp_header`, `_fast`, `read_rgb_rows`); only uncompressed 8 and 24 bit are needed (same as Python's fast path).
- [ ] **Step 4: Write `tools/make_test_map.py <out> --size N --seed S`** making a deterministic project folder: `<name>.bmp` (colours taken from `Rules.txt`: grass, dirt, water, road), `<name>_veg.bmp`, `<name>.pzw` and copies of `real1` buildings. Run it for sizes 300 and 600.
- [ ] **Step 5: Run tests**; expect PASS. **Step 6: Commit** `feat(compile): crate and bitmap reader`.

### Task 2: Rules and Blends parsers

**Files:**
- Create: `src/rules.rs`, `src/blends.rs`, `tests/rules.rs`, `tests/blends.rs`

**Interfaces:**
- Produces: `pub struct Rule { pub label: String, pub bitmap: u8, pub colour: (u8,u8,u8), pub tiles: Vec<String>, pub layer: String, pub condition: Option<String> }` (fields exactly as `rules.Rule`), `pub struct Ruleset { pub rules: Vec<Rule>, pub aliases: HashMap<String, Vec<String>> }` with `for_bitmap(&self, u8) -> Vec<&Rule>`, `pub fn load_rules(&Path) -> Result<Ruleset, CompileError>`; `pub struct Blend { .. }` as `blends.Blend`, `pub fn load_blends(&Path) -> Result<Vec<Blend>, CompileError>`.

- [ ] **Step 1: Write failing tests** reusing the inputs of `tests/test_terrain.py` and `tests/test_blends.py` (copy their literal text and expected values), plus `real_rules_file_parses_every_rule` against `config/Rules.txt` and `Blends.txt` comparing rule and blend counts to `dump_for_rust.py rules`.
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** both parsers, porting `rules.py` and `blends.py` (`parse_*`, `_blocks`, `_triple`); unknown keys ignored as Python does.
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): Rules.txt and Blends.txt`.

### Task 3: Terrain (hash, picks, stack order)

**Files:**
- Create: `src/terrain.rs`, `tests/terrain.rs`

**Interfaces:**
- Consumes: `Ruleset`, `Rule`.
- Produces: `pub fn mix(x: u64, y: u64, salt: u64) -> u64`; `pub struct Palette` with `pub fn of(&mut self, name: &str) -> u32` (ids from 1, same order as Python); `pub struct Grid<T> { pub w: usize, pub h: usize, pub data: Vec<T> }`; `pub fn ground_layers(land: &[u8], veg: &[u8], w: usize, h: usize, rules: &Ruleset, pal: &mut Palette, seed: u64, x0: i64, y0: i64) -> BTreeMap<String, Grid<u32>>` (layer name to palette ids, 0 = empty); `pub fn stack_order(names: &[String]) -> Vec<String>`.

- [ ] **Step 1: Write failing tests**: `mix_matches_python_for_large_coordinates` (vectors from `dump_for_rust.py mix` including x,y near 16,500 and salt near `seed*1000003+7919`), `palette_ids_start_at_one_in_first_use_order`, `ground_layers_match_python_on_generated_map` (300x300 map, every layer grid equal), `stack_order_matches_python`.
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** by porting `terrain.py` (`mix`, `_pick`, `colour_keys`, `ground_layers`, `stack_order`); `mix` uses `wrapping_mul`/`wrapping_add`/shifts exactly as numpy `uint64`.
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): terrain`.

### Task 4: Edge blends

**Files:**
- Create: `src/blends_apply.rs` (or extend `blends.rs`), `tests/blend_layers.rs`

**Interfaces:**
- Consumes: `Grid<u32>` floor, `Ruleset`, `Vec<Blend>`, `Palette`, `seed`, `x0`, `y0`.
- Produces: `pub fn blend_layers(floor: &Grid<u32>, rules: &Ruleset, blends: &[Blend], pal: &mut Palette, seed: u64, x0: i64, y0: i64) -> BTreeMap<String, Grid<u32>>` equal to `blends.blend_layers`.

- [ ] **Step 1: Write failing tests**: `blend_layers_match_python_on_road_and_water_map` (map from `make_test_map.py` with roads and water edges; every returned layer grid equal to Python via `dump_for_rust.py blends`); `no_blends_gives_no_layers`.
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** porting `blend_layers`, `_members`, `_neighbour`, with neighbour shifts as bounds-checked index maths (edges treated as the Python does: `look` with out-of-grid = false).
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): edge blends`.

### Task 5: Buildings (tbx, tiles, roofs)

**Files:**
- Create: `src/tbx.rs`, `src/buildtiles.rs`, `src/buildroofs.rs`, `tests/buildings.rs`

**Interfaces:**
- Produces: `pub struct Building { pub width: usize, pub height: usize, pub floors: Vec<Floor>, .. }` and `pub fn parse_tbx(text: &str) -> Result<Building, CompileError>` (fields as `buildtiles.Building`/`Floor`); `pub fn level_tiles(b: &Building, z: usize) -> BTreeMap<(i32,i32), Vec<String>>`; `pub fn compose_lots(lots: &[(Building, i32, i32, i32)]) -> BTreeMap<(i32,i32,i32), Vec<String>>`; `pub fn tile_name(name: &str) -> String`.

- [ ] **Step 1: Write failing tests**: every case of `tests/test_buildtiles.py` and `tests/test_buildroofs.py` ported with the same inputs and expected tile lists; `all_real1_buildings_match_python` (each of the 113 `.tbx`: `level_tiles` for every level equals `dump_for_rust.py tiles <tbx>`); `compose_lots_matches_python_on_real1`; `damaged_tbx_names_the_file_in_the_error`.
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** by porting `buildtiles.py` and `buildroofs.py` function by function (`level_layers`, `wall_tiles`, `add_curtain`, `roof_squares`, `upper_layers`, `compose_lots`); keep Python function names as Rust names so a reader can diff them. Dict ordering matters: use `BTreeMap` or insertion-ordered `Vec` wherever Python relies on dict insertion order (check each).
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): buildings`.

### Task 6: Header tables

**Files:**
- Create: `src/header.rs`, `tests/header.rs`

**Interfaces:**
- Consumes: `Building`.
- Produces: `pub fn room_rects(squares: &BTreeSet<(i32,i32)>) -> Vec<(i32,i32,i32,i32)>`; `pub struct Room { pub name: String, pub z: i32, pub rects: Vec<(i32,i32,i32,i32)> }`; `pub fn placed_rooms(lots: &[(&Building, i32, i32, i32)]) -> Vec<(i32, i32, Vec<Room>)>`; `pub fn cell_tables(cx: i32, cy: i32, placed: &[(i32,i32,Vec<Room>)]) -> (Vec<Room>, Vec<Vec<i32>>)`.

- [ ] **Step 1: Write failing tests**: cases of `tests/test_buildheader.py`; `tables_match_python_for_all_real1_cells` (4 cells; rooms and building index lists equal to `dump_for_rust.py header`).
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** porting `buildheader.py` (`room_rects`, `building_rooms`, `placed_rooms`, `cell_tables`).
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): header tables`.

### Task 7: Cell source writer and pipeline

**Files:**
- Create: `src/source.rs`, `src/pipeline.rs`, `tests/source.rs`

**Interfaces:**
- Consumes: everything above; `knoxlots` cell reader/compiler (read `rust/knoxlots/src/kcell.rs` and `compile.rs` for the exact entry points).
- Produces: `pub fn cell_source(cx: i32, cy: i32, names: &[String], levels: &[Vec<Grid<u32>>], rooms: &[Room], buildings: &[Vec<i32>]) -> Vec<u8>` (a `.kcell`, equal to `kcell.cell_source`); `pub struct Options { pub seed: u64, pub seams: bool, pub workers: Option<usize>, pub keep_source: Option<PathBuf> }`; `pub fn compile_map(project: &Path, out: &Path, opts: &Options, progress: &dyn Fn(usize, usize)) -> Result<usize, CompileError>` (cells compiled), one thread per row of cells via `std::thread::scope`, porting `make_map_source` and `_row` including `merge_stack`, `MARGIN = 8`, `read_lots` of the `.pzw`, and spawn map copy.

- [ ] **Step 1: Write failing tests**: `cell_source_bytes_equal_python` (small synthetic inputs through `dump_for_rust.py kcell`); `compile_map_real1_cells_equal_python_kcells` (real1 buildings on a generated 600x600 map: `keep_source` folder files byte-equal to `python tools/map_source.py` output, all 4 cells); `missing_veg_bitmap_is_error_naming_it`; `stopping_early_returns_error` is not required (no cancel in CLI).
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** both modules; hand cell bytes to `knoxlots` in memory when its API allows, else write the `.kcell` files to a temp folder as Python does and call its compile function.
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): cell sources and pipeline`.

### Task 8: `worlded` binary

**Files:**
- Create: `src/bin/worlded.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `compile_map`.
- Produces: `worlded <map folder> [<out folder>] [--seed N] [--worlded-seams] [--keep-source DIR]`; default out `<map folder>/lots`; no arguments on a console prompts for the folder; waits for Enter before closing when launched by double-click; rules found via `KNOXLOTS_CONFIG`, `config/` next to the exe, then `config/` in the repo.

- [ ] **Step 1: Write failing tests** in `tests/cli.rs` (run the built binary): `compiles_generated_map_and_verify_passes` (output verified with `knoxlots verify`), `bad_folder_prints_error_and_nonzero_exit`, `missing_rules_file_message_names_config`.
- [ ] **Step 2: Run**; expect FAIL. **Step 3: Implement** the CLI; the double-click wait is on only when stdin is a console and no arguments were given.
- [ ] **Step 4: Run**; expect PASS. **Step 5: Commit** `feat(compile): worlded binary`.

### Task 9: Differential test, speed check, release

**Files:**
- Create: `rust/worlded-compile/tests/differential.rs`; modify `README.md`

**Interfaces:**
- Consumes: everything.

- [ ] **Step 1: Write failing test** `rust_lots_equal_python_pipeline`: generated 600x600 map (and a second with different seed and edge colours); run Python `tools/map_source.py` then `knoxlots compile`, run `worlded`, then `knoxlots compare` on the two output folders; expect 0 differences. Skips when `python` or the numpy/PIL modules are missing.
- [ ] **Step 2: Run**; expect FAIL or PASS (this test only passes when Tasks 1-8 are right; fix the module that differs, not the test).
- [ ] **Step 3: Speed check**: time both on a generated 2,400 x 2,400 map (64 cells); record the numbers in the README; Rust must be no slower than Python.
- [ ] **Step 4: Build release**: `cargo build --release -p worlded-compile`, zip `worlded.exe` with `config/`, `LICENSE`, `README.md` into `worlded-v0.4.0-windows-x64.zip`; publish a pre-release replacing the PyInstaller `worlded.exe` download; commit `feat: Rust worlded compiler v0.4.0`.
