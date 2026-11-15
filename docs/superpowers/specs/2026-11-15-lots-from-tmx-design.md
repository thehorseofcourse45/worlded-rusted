# Editor Stage 4a: Lot files from the cells' TMX maps

Status: design for review. Date: 2026-11-15. Follows `2026-11-15-editing-design.md` (stage 3, built and released as v0.5.1).

## Purpose

Stage 3 lets a user edit the tile layers of a cell `.tmx` in the viewer. Nothing yet turns those edits into something the game reads. Stage 4a closes that loop: the compiler reads the cells' `.tmx` files, so tile edits end up in the lot files, with no WorldEd, no bitmaps and no Python.

Success: for the same project, the lot files compiled from WorldEd's own `.tmx` output are identical to the lot files WorldEd generates from those `.tmx` files; and a tile edited with the stage 3 editor shows up in the compiled lot files, in that square only.

## Roadmap context

The viewer spec's stage 4 ("buildings, objects, roads, zombie spawn, lots export from the GUI") is too large for one spec. It is split:

- **4a (this spec):** lots from the cells' TMX, command line.
- **4b:** a viewer "Export lots" button calling 4a.
- Later: building (`.tbx`) editing; objects, zones, roads and fences; zombie spawn editing.

## Scope

In:
- `worlded <project>.pzw [<out folder>]` compiles from the `.tmx` each cell of the project lists. The existing `worlded <map folder>` bitmap mode is unchanged.
- Ground, blends, vegetation and every other tile layer come from the `.tmx` files. Buildings (`.tbx` lots listed in the `.pzw`), room and building tables, and zombie bytes (from `<name>_ZombieSpawnMap.bmp` when it exists) come exactly as they do today.
- Layers above level 0 in a `.tmx` produce tiles on those levels.
- Same options as bitmap mode where they still make sense: output folder, `--threads`, `--keep-source`. `--seed` and `--worlded-seams` are accepted only in bitmap mode (the tiles are already chosen and blended in the `.tmx`); giving them in `.pzw` mode is an error.

Out: editing anything other than tile layers, a GUI trigger (4b), a changed world origin, writing `.pzw` or `.tmx` from the compiler.

## Assumptions to check early (risks)

1. The order of layers in a `.tmx` is the order of tiles in a square's stack in the lot files. The first oracle comparison settles this.
2. WorldEd gives each cell's `.tmx` the same tiles the compiler's own ground step would; the compiler's bitmap mode is already proven equal to WorldEd, so the oracle should match except where the bitmap mode picks a different random tile from an alias (it is compared by alias today). The TMX mode picks nothing, so it should match WorldEd exactly.
3. Fences and building squares interact with the ground the way the bitmap mode already handles (a fence clears `0_Furniture` ground tiles; a room square drops the ground).
4. A `.tmx` cell is 300 x 300 tiles, the lot window 256 x 256: a window can touch up to four `.tmx` cells.

If assumption 1 or 2 fails, the design changes before any code beyond the oracle test is written.

## Design

### Entry and input

- `worlded <path>`: a path ending in `.pzw` selects TMX mode; anything else is a map folder as today.
- The `.pzw` supplies: the world size in cells (`width`, `height`), the origin, each cell's `map` path, and the `<lot>` placements. The origin must be `70,0` (the pipeline's fixed `ORIGIN_X`); any other origin is an error naming it.
- A cell's `.tmx` path is resolved as the viewer does (absolute, or relative to the `.pzw`'s folder, then the `tmx` folder beside it). That resolver moves from `worlded-app` into `worlded-core` so both use one copy.
- A cell with no `map`, or whose file is missing, has no ground. A missing file that the project names is a warning on stderr, not an error, so a half-built map still compiles.

### TMX ground source (`worlded-compile/src/tmxground.rs`)

- Depends on `worlded-core` for `parse_tmx` (a new dependency of `worlded-compile`).
- For one lot window it returns the same thing the bitmap path builds from the pictures: a set of layer grids keyed `"<level>_<name>"`, with the window's squares filled from the `.tmx` cells it overlaps. A `Floor` layer with `level="0"` becomes `0_Floor`, so `stack_order` and the fence rule keep working on the names they know.
- Tile ids become tile names with `worlded-core`'s `tile_name` and are interned in the window's `Palette`.
- Layer order is the file order of the `<layer>` elements.
- A row of windows loads the `.tmx` cells it needs and drops them when the row is done, so memory stays bounded (a 300 x 300 cell with 15 layers is about 5 MB decoded). A `.tmx` cell is read by at most three window rows.

### Pipeline change (`pipeline.rs`)

- `do_row` takes its ground layers from either the bitmap step (as now) or `tmxground`; everything after is shared.
- The stack step today puts every ground layer on level 0 because bitmap layers are all `0_*`. It now routes each layer to the level in its key prefix. At each level a square's stack is ground tiles in layer order, then building tiles; a room square keeps only building tiles (existing `merge_stack` rule, now applied on every level).
- World size, the number of window rows and columns, and the "inside" mask come from the `.pzw` cell grid (300 x width, 300 x height) instead of a bitmap's size.
- The 64-level limit error stays.

### Errors

- A damaged `.tmx` is an error naming the file and what was wrong; no panic.
- A `.tmx` whose width or height is not 300 is an error naming the file.
- Output is written only after a window's cell is fully built, as today; a failed run leaves the files already written (the existing behaviour).

## Testing

Tests are written before the code they test.

1. **Oracle, ground only (committed).** Fixture: the real WorldEd `.tmx` already in `worlded-core/tests/data/real_cell_a.tmx`, a project around it (`.pzw` with that one cell, no lots), and the four lot cells WorldEd generated from it (about 1.5 MB, from the acceptance run's output). Test: compile in TMX mode, compare every output file byte for byte with WorldEd's. Runs without WorldEd or Python.
2. **Oracle with buildings (generated, opt-in).** `tools/make_tmx_oracle.py` builds a project with `<lot>` placements from the real1 buildings, has WorldEd convert it and generate lots, and keeps the `.tmx` and lots. Test (`WORLDED_ACCEPTANCE=1`): TMX-mode compile equals WorldEd's lots. Run by hand, results recorded in the spec's checklist.
3. **Edit round trip.** Open the fixture with the stage 3 `TmxDoc`, set one square on one layer, compile, and check the lot output differs from the unedited compile in that square's stack only (read back with `knoxlots`).
4. **Levels.** A synthetic `.tmx` with a layer at `level="2"`: its tiles land on level 2; a room square drops ground on every level.
5. **Windows across cells.** A synthetic two-cell project: tiles on both sides of the 300-tile seam end up in the right lot cells.
6. **Errors.** Damaged `.tmx`, wrong size, wrong origin, missing file (warning), and a TMX-mode run with `--seed`.
7. The existing bitmap-mode tests keep passing unchanged.

## Deliverables

`worlded.exe` v0.6.0 with TMX mode, README and `FORMATS.md` notes, the spec checklist filled in with what was and was not compared, a pre-release.

## Not verified until the user does it

Lot files from either mode have never been tried in Project Zomboid. 4a proves equality with WorldEd's lot output, not that the game loads it.
