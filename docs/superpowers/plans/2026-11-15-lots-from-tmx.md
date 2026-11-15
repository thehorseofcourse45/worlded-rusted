# Stage 4a: Lots from the Cells' TMX Implementation Plan (revised)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `worlded <project>.pzw` compiles lot files from the cells' `.tmx` maps the way WorldEd does: ground generated from the pictures embedded in each `.tmx`, merged with the tile layers, plus buildings, so tile edits reach the game files without WorldEd.

**Architecture:** `worlded-core` decodes the embedded pictures. `tmxground` in `worlded-compile` builds each cell's effective layers (generate with the existing bitmap code, merge with the `.tmx` layers by WorldEd's rule) and serves padded per-window layer grids. `pipeline::compile_map` picks the ground source by input (`.pzw` path = TMX mode); the stack step routes layers to levels. Buildings, rooms, spawn and `compile_cell` are shared and untouched.

**Tech Stack:** Rust 2021; `worlded-core` (`parse_tmx_str`, `parse_pzw`, `resolve_map`, `flate2`), `knoxlots`, existing `worlded-compile` modules.

**Spec:** `docs/superpowers/specs/2026-11-15-lots-from-tmx-design.md` (read its Findings first).

## Global Constraints

- TMX mode is selected by a path ending in `.pzw`; any other path is a map folder (bitmap mode unchanged, existing tests pass untouched).
- World origin must be `70,0` (`pipeline::ORIGIN_X = 70 * 300`).
- A `.tmx` cell is 300 x 300 tiles; a lot window is `SIDE` = 256 x 256; a `.tmx` cell is read by at most three window rows.
- Layer key is `"<level>_<name>"`; order is level, then file order.
- Merge rule per layer the bitmap generates, per square: generated tile if nonzero, else the `.tmx` layer's tile; layers it does not generate are the `.tmx` layer alone.
- Rules come from `Rules.txt` / `Blends.txt` in the compiler's config folder; the seed is the embedded picture's `seed`; blends are per cell (`seams = Some(300)`), global tile origin `(300 i, 300 j)`.
- `--seed` and `--worlded-seams` are errors in `.pzw` mode.
- A missing named `.tmx` is a warning; a damaged `.tmx` or picture is an error naming the file; never a panic.
- The 64-level limit error stays.
- Tests come first. Clean-room. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Editing multi-line source with Python heredocs corrupts newlines in this shell: use the Edit tool.

## Review Focus

- A cell whose `.tmx` has no `<bmp-image>`: layers alone, no error (Task 3).
- A picture colour no rule knows: no generated tile, the hand-edited tile shows (Task 3).
- Cells whose layer sets differ, and a generated layer the file does not list: union of keys, empty where missing (Task 3).
- A window at the world edge or partly outside the world: no tiles outside, no panic (Task 3).
- A project where no cell has a `.tmx`: clear error, nothing written (Task 6).

## Status

- Task 1 (`resolve_map` in core): DONE, commit `df752ec`.
- `tmxground.rs` exists on `main` from the first plan (layers only: `TmxWorld::open`, `size_tiles`, `has_maps`, `load_rows`, `RowCells::window`/`skipped`, 5 tests). Task 3 extends it.
- Branch `stage4a-wip` holds the pipeline TMX-mode edits (compile_map entry, `strips`/`rows` ground switch, level-0 only) and the oracle fixture project `tests/data/tmx_oracle/`; Task 4 restores them with `git checkout stage4a-wip -- <paths>` and replaces the failing byte-for-WorldEd test.

### Task 2: Embedded pictures in worlded-core

**Files:** Modify `rust/worlded-core/src/tmx.rs`, Test `rust/worlded-core/tests/tmx.rs` (or a new `tests/bmpimage.rs`).

**Interfaces:**
- Produces: `pub struct BmpImage { pub index: u32, pub seed: i64, pub colors: Vec<[u8; 3]>, pub width: u32, pub height: u32, pub pixels: Vec<u32> }` (pixels: 1-based index into `colors`, 0 = none) and `pub fn parse_bmp_images(text: &str) -> Result<Vec<BmpImage>, CoreError>`. Pixel data is base64 of gzip; decoded size must be `4 * width * height` where `width = height = 300` (the cell size); gzip output is capped at that size + 1 (zip bomb = error); an index beyond `colors.len()` is an error.

- [ ] **Step 1: Write failing tests** on `real_cell_a.tmx`: `real_fixture_has_two_pictures` (indexes 0 and 1, seed 1, 11 and 10 colours, 300 x 300, every pixel index <= colour count, first pixel of image 0 maps to colour `(201, 31, 201)`); `file_without_pictures_gives_empty_list`; `damaged_gzip_wrong_size_and_bad_index_are_errors` (synthetic text built with `flate2` in the test).
- [ ] **Step 2: Run** `cd rust && cargo test --release -p worlded-core --test bmpimage`; expect FAIL.
- [ ] **Step 3: Implement** with `roxmltree` (`<bmp-image>` children `<color rgb>` and `<pixels>`) and `flate2::read::GzDecoder` with `take`.
- [ ] **Step 4: Run** the whole workspace; PASS. **Step 5: Commit** `feat(core): embedded bitmap pictures of a TMX`.

### Task 3: Effective layers: generate, then merge

**Files:** Modify `rust/worlded-compile/src/tmxground.rs`.

**Interfaces:**
- Consumes: `parse_bmp_images`, `terrain::{ground_layers, Palette, Layers, Grid}`, `blend_layers::blend_layers`, `rules::Ruleset`, `blends::Blend`.
- Produces: `TmxWorld::load_rows(&self, y_lo: i64, y_hi: i64, gen: &Generator) -> Result<RowCells, CompileError>` where `pub struct Generator<'a> { pub rules: &'a Ruleset, pub blends: &'a [Blend] }`; `RowCells::window` unchanged in signature but its layers are the effective layers; `RowCells` keeps its own palette for the generated tile ids and re-interns names in the caller's palette. Public pure helper for testing: `pub fn merge_layer(generated: &Grid, manual: &Grid) -> Grid` (square-wise: generated if nonzero else manual; both same size).

- [ ] **Step 1: Write failing tests** (synthetic `.tmx` with pictures: build with a helper that gzips and base64-encodes `u32` indexes; tiny rules text in the test, as `tests/data/rules_small.txt` is used by existing tests, or the repo `config/Rules.txt` with a known colour such as `90 100 35` = Dark Grass):
  - `hand_edit_on_floor_is_overwritten_where_the_picture_gives_ground`.
  - `hand_edit_on_furniture_stays`.
  - `hand_edit_on_vegetation_stays_where_the_picture_gives_none` (vegetation picture colour with no rule tile).
  - `cell_without_pictures_uses_its_layers_alone`.
  - `unknown_colour_gives_no_generated_tile`.
  - `merge_layer_prefers_generated_nonzero`.
  - The five `tmxground` tests from the first plan keep passing (adapted to pass a `Generator` made from `common`/config rules).
- [ ] **Step 2: Run** `cargo test --release -p worlded-compile --lib tmxground`; expect FAIL.
- [ ] **Step 3: Implement**: per cell with `index 0` and `index 1` pictures, RGB from colour lists, `ground_layers(land, veg, 300, 300, rules, &mut cellpal, seed, 300 i, 300 j)`, then `blend_layers(floor, rules, blends, &mut cellpal, seed, 300 i, 300 j, Some(300))` and `layers.update`; merge per key with the cell's `.tmx` layers (key `"<level>_<name>"`); a key only the generator has is added after the file's layers of its level. Use the seed of image 0 for both (they agree in WorldEd files).
- [ ] **Step 4: Run** the lib tests; PASS. **Step 5: Commit** `feat(compile): effective layers from embedded pictures and layers`.

### Task 4: TMX mode in the pipeline, oracles

**Files:** Modify `rust/worlded-compile/src/pipeline.rs`; Create `rust/worlded-compile/tests/tmx_mode.rs`, fixture `rust/worlded-compile/tests/data/tmx_oracle/` (restore from `stage4a-wip`: `rt.pzw`, `tmx/rt_70_0.tmx`, `rt_ZombieSpawnMap.bmp` (force-add, `*.bmp` is ignored), `lots/` = WorldEd's output).

**Interfaces:** `compile_map(input: &Path, out, opts, progress)` as on `stage4a-wip` (`.pzw` entry, `Shared.tmx`/`skipped`, `rows` ground switch, stack order from the window's layers), plus `Generator` built from the rules/blends `do_row` already loads.

- [ ] **Step 1: Restore** the wip pipeline edits and fixture: `git checkout stage4a-wip -- rust/worlded-compile/src/pipeline.rs rust/worlded-compile/tests/data/tmx_oracle`; adapt `load_rows` call to pass the `Generator`.
- [ ] **Step 2: Write failing tests** in `tests/tmx_mode.rs`:
  - `tmx_mode_equals_bitmap_mode_byte_for_byte`: write the fixture's two pictures as 24-bit BMPs (`rt.bmp`, `rt_veg.bmp`, helper in the test) into a temp project named `rt` with the same `.pzw`; compile in bitmap mode (`seams: true`, `seed: 1`) and in TMX mode; same file list, every file byte-identical.
  - `tmx_mode_matches_worlded_by_alias`: compile the fixture in TMX mode; for every square of the four lot cells compare tile names with WorldEd's lots (`lots/`) after mapping each name to its alias group from `Rules.txt` (a name in the same alias as the other counts as equal; names not in any alias must be equal), reading cells with `knoxlots::kcell`/lot readers or the existing helpers in `tests/header.rs`/`tests/common`; `chunkdata` files ignored. Report the number of squares compared and differing.
- [ ] **Step 3: Run**; expect FAIL. **Step 4: Implement/fix** until both pass. If the bitmap-vs-TMX bytes differ, find the first differing square and fix the generation (origin, seams, seed) before anything else; if only the WorldEd alias test differs, report the squares and stop for the user (the generation rule is then wrong).
- [ ] **Step 5: Run** the whole `worlded-compile` suite incl. bitmap tests; PASS. **Step 6: Commit** `feat(compile): compile lots from cell TMX files`.

### Task 5: Upper levels and rooms

**Files:** Modify `rust/worlded-compile/src/pipeline.rs`, Test `tests/tmx_mode.rs`.

**Interfaces:** the stack step routes each ground layer to the level in its key prefix; at each level a square's stack is ground tiles in layer order then building tiles; a room square at that level keeps only building tiles (`merge_stack` on every level); the `0_Furniture` fence rule unchanged.

- [ ] **Step 1: Write failing tests** (synthetic project; read `.kcell` via `keep_source`): `tmx_layer_at_level_2_lands_on_level_2`; `room_square_drops_ground_on_its_level_and_keeps_building_tiles`; `tiles_across_the_300_tile_seam_land_in_the_right_lot_cells` (2 x 1 cell world, tiles at x = 299 and 300); `more_than_64_levels_is_still_an_error`.
- [ ] **Step 2: Run**; FAIL. **Step 3: Implement.** **Step 4: Run** the suite; PASS. **Step 5: Commit** `feat(compile): upper levels from TMX layers`.

### Task 6: Command line and errors

**Files:** Modify `rust/worlded-compile/src/bin/worlded.rs`, Test `tests/cli.rs`, `tests/tmx_mode.rs`.

**Interfaces:** `worlded <x>.pzw [<out>] [--threads N] [--keep-source DIR]` runs TMX mode (default out `<pzw folder>/lots`); `--seed N` / `--worlded-seams` with a `.pzw` exit with an error naming the option; USAGE documents both modes.

- [ ] **Step 1: Write failing tests**: `cli_compiles_a_pzw_into_its_lots_folder` (fixture copy, exit 0, four cells); `cli_rejects_seed_and_seams_in_pzw_mode`; `project_without_any_tmx_is_an_error`; `damaged_tmx_and_damaged_picture_name_the_file`; `missing_named_tmx_warns_and_compiles_the_rest`.
- [ ] **Step 2: Run**; FAIL. **Step 3: Implement** (make `--seed`/`--worlded-seams` detectable). **Step 4: Run** the suite; PASS. **Step 5: Commit** `feat(worlded): .pzw mode on the command line`.

### Task 7: Edit round trip and the buildings oracle

**Files:** Create `tools/make_tmx_oracle.py`; Test `tests/tmx_mode.rs`.

**Interfaces:** `python tools/make_tmx_oracle.py <out folder>` builds a test map with `<lot>` placements from the real1 buildings, runs WorldEd's BMP to TMX and lot generation (helpers of `make_real_tmx.py` / `worlded_lots.py`), leaves `rt/` with `tmx/`, `lots/`, `buildings/`, `.pzw`, spawn map.

- [ ] **Step 1: Write failing tests**: `edit_changes_only_where_worlded_lets_it` (fixture opened with `worlded_core::tmxdoc::open`; set one `Furniture` square and one `Floor` square; compile; the `Furniture` tile is in that stack, the `Floor` edit is not, every other square's stack equals the unedited compile); `tmx_mode_matches_worlded_with_buildings` (opt-in `WORLDED_ACCEPTANCE=1`: runs the script, compiles the result in TMX mode, compares with WorldEd's lots by alias as in Task 4).
- [ ] **Step 2: Run** the first; PASS or fix. **Step 3: Write** the script; run the opt-in test once; record the outcome in the spec (identical or the differing squares; fix real differences in the shared building code or report them).
- [ ] **Step 4: Commit** `test: TMX edit round trip and WorldEd oracle with buildings`.

### Task 8: Docs and release

**Files:** Modify `README.md`, `FORMATS.md` (embedded picture format, merge rule), `HANDOFF.md`, the spec (a "Results" section: what was compared and what not).
**Release:** `worlded.exe` zip (with `config/`, README, LICENSE as the v0.4.0 zip), tag `v0.6.0` pre-release.

- [ ] **Step 1: Write** the docs. **Step 2: Run** `cd rust && cargo test --release` (all PASS) and the zip from a clean folder on the fixture `.pzw` (four cells compiled). **Step 3: Build** `cargo build --release -p worlded-compile`, zip, commit `feat: lots from TMX, v0.6.0`, push, `gh release create v0.6.0 ... --prerelease` with notes saying what was and was not compared (never loaded in the game; hand edits to generated layers are overwritten, as in WorldEd).
