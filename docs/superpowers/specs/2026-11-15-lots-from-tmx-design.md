# Editor Stage 4a: Lot files from the cells' TMX maps

Status: revised 2026-11-15 after the first oracle check failed (see "Findings"); the user chose the embedded-bitmaps design. Follows `2026-11-15-editing-design.md` (stage 3, released v0.5.1).

## Purpose

Stage 3 lets a user edit the tile layers of a cell `.tmx` in the viewer. Nothing yet turns a project's `.tmx` files into something the game reads. Stage 4a closes that loop: the compiler reads the cells' `.tmx` files the way WorldEd's lot generation does, with no WorldEd, no separate bitmap files and no Python.

Success: for a project, the lot files compiled in TMX mode equal what the compiler's own bitmap mode produces from the same pictures (byte for byte), and equal WorldEd's lot files up to the two things nobody can reproduce (WorldEd's random alias picks, which the earlier stages already compare by alias, and WorldEd's real `chunkdata`, which the game does not need). A tile edited with the stage 3 editor shows up in the compiled lot files wherever WorldEd's own rule lets it show.

## Findings (checked against WorldEd on 2026-11-15)

These replace the assumptions of the first version of this spec, which were wrong.

1. **A WorldEd `.tmx` made by BMP to TMX has empty tile layers.** All 15 layers of the real cell are zeros. The ground lives in two embedded pictures, `<bmp-image index="0">` (landscape) and `<bmp-image index="1">` (vegetation), plus the rules, aliases and blends in `<bmp-settings>`. WorldEd generates the ground from them each time it exports lots.
2. **Embedded picture format.** `<color rgb="r g b"/>` lines list the picture's colours; `<pixels>` is base64 of gzip of `width x height` little-endian `u32`, each a 1-based index into that colour list (checked pixel for pixel against the source bitmap: 0 differences). `seed` is the random seed WorldEd used.
3. **Merge rule between generated and hand-edited tiles** (experiment: every square of every layer filled with its own marker tile, WorldEd lot generation, markers looked up in the lot files):
   - For the seven layers the bitmap generates (`Floor`, `FloorOverlay`, `FloorOverlay2` to `4`, `FloorOverlay6`, `Curbs`, `Vegetation`), per square: the generated tile if there is one, else the hand-edited tile of that layer. The marker survived on exactly the squares where the generated layer is empty, and nowhere else (for example `Floor`: 0 of 90,000, because ground is generated on every square).
   - For the other layers (`FloorOverlay5`, `Walls`, `Walls2`, `Furniture` to `Furniture4`) the hand-edited tiles stay on all 90,000 squares.
   - Tiles of a square appear in layer order (file order), hand-edited and generated mixed.
4. **Exact byte equality with WorldEd is impossible**, even for the existing bitmap mode: random alias picks differ and WorldEd writes real `chunkdata`. Earlier stages compare by alias; this one does too.
5. **The viewer and editor of stages 1 and 3 draw only the layers**, so a cell made by BMP to TMX looks blank in them. Not fixed by 4a (listed under follow-ups).

## Roadmap context

- **4a (this spec):** lots from the cells' TMX, command line.
- **4b:** a viewer "Export lots" button calling 4a, and drawing the generated ground in the cell view (so what you see is what is compiled).
- Later: building (`.tbx`) editing; objects, zones, roads and fences; zombie spawn editing.

## Scope

In:
- `worlded <project>.pzw [<out folder>]` compiles from the `.tmx` each cell of the project lists. The existing `worlded <map folder>` bitmap mode is unchanged.
- Per cell: ground generated from the embedded pictures (when present), merged with the cell's tile layers by the rule in Finding 3. A cell with no embedded pictures uses its layers alone; a cell without a `.tmx` has no ground.
- Buildings (`.tbx` lots listed in the `.pzw`), room and building tables, zombie bytes (from `<name>_ZombieSpawnMap.bmp` when it exists) come exactly as today.
- Layers above level 0 produce tiles on those levels.
- Options: output folder, `--threads`, `--keep-source`. `--seed` and `--worlded-seams` are errors in `.pzw` mode (the seed comes from each embedded picture; each cell is blended on its own, as WorldEd does).

Out: editing anything but tile layers, a GUI trigger and drawing generated ground in the viewer (4b), a world origin other than 70,0, reading rules from the `.tmx` (the compiler uses `Rules.txt` and `Blends.txt` from its `config` folder, as bitmap mode does; a project that embeds changed rules would compile with the standard ones), writing `.pzw` or `.tmx` from the compiler.

## Design

### Entry and input

- `worlded <path>`: a path ending in `.pzw` selects TMX mode; anything else is a map folder as today.
- The `.pzw` supplies the world size in cells, the origin (must be `70,0`, else an error naming it), each cell's `map` path and the `<lot>` placements.
- A cell's `.tmx` path is resolved by `worlded_core::pzw::resolve_map` (path as written, else the file name beside the `.pzw`, else in a `tmx` folder beside it). A named file that cannot be found is a warning on stderr and that cell has no ground. If no cell has a `.tmx`, an error ("no cell has a .tmx file").

### Embedded pictures (worlded-core)

- `worlded_core::tmx::parse_bmp_images(text) -> Result<Vec<BmpImage>, CoreError>`: for each `<bmp-image>`: `index`, `seed`, the colour list, and the decoded `u32` pixels (300 x 300 expected; any other size, or an index beyond the colour list, is an error). Gzip decoding is bounded (a zip bomb is an error).

### TMX ground source (`worlded-compile/src/tmxground.rs`)

- `load_rows` reads each `.tmx` cell a row of lot windows needs (text once; `parse_tmx_str` for layers, `parse_bmp_images` for pictures) and builds the cell's effective layers:
  1. Generate with the existing bitmap code on the cell's own 300 x 300 area: `ground_layers` from the landscape and vegetation pictures (RGB from the colour lists), then `blend_layers` on `0_Floor` with seams at 300, global tile origin `(300 i, 300 j)` and the picture's `seed`.
  2. Merge with the cell's tile layers per Finding 3: per layer key, per square, generated tile if nonzero else the hand-edited one.
- `window(x0, y0, win, pal)` is as before: it returns the padded layer grids and the inside mask, assembled from the cells' effective layers (tile names interned in the caller's palette). Layer key is `"<level>_<name>"`; order is level, then first-seen file order, with generated-only layers placed where the file lists them (a layer the file lacks is appended after the file's layers of its level, in the bitmap code's order).
- Memory stays bounded as before (a row of windows loads what it needs and drops it).

### Pipeline change (`pipeline.rs`)

- `do_row` takes its ground layers from either the bitmap step or `tmxground`; everything after is shared. The stack order in TMX mode is the layer order the window returned.
- The stack step routes each ground layer to the level in its key prefix; at each level a square's stack is ground tiles in layer order, then building tiles; a room square keeps only building tiles (existing `merge_stack` rule on every level). The fence rule on `0_Furniture` is unchanged.
- World size comes from the `.pzw` cell grid. The 64-level limit error stays.

### Errors

A damaged `.tmx` or embedded picture is an error naming the file; a `.tmx` not 300 x 300, or with a layer below level 0, is an error naming the file. No panics.

## Testing

Tests are written before the code they test.

1. **Embedded pictures.** `parse_bmp_images` on the real fixture (`worlded-core/tests/data/real_cell_a.tmx`): two images, 300 x 300, colour counts 11 and 10, every index in range; damaged gzip, wrong size and out-of-range index are errors.
2. **TMX mode equals bitmap mode (committed, no WorldEd).** Test writes the fixture's two pictures as 24-bit BMPs into a project folder (same name rules as bitmap mode) and a `.pzw`; compiles it in bitmap mode (`seams: true`, seed 1) and in TMX mode; every lot file is byte-identical.
3. **Merge rule.** Synthetic `.tmx` files with pictures and layers: a hand edit on `Floor` where the picture gives ground is overwritten; a hand edit on `Furniture` stays; a hand edit on `Vegetation` where the picture gives no vegetation stays; stacks follow layer order.
4. **Against WorldEd (committed fixture, alias level).** The fixture project's lot files from WorldEd are in the repo; a test compares the TMX-mode output with them by alias using the same canonicalisation as `tools/cell_check.py`'s Rust equivalent (tile names mapped through `Rules.txt` aliases), ignoring `chunkdata`.
5. **Edit round trip.** Open the fixture with the stage 3 `TmxDoc`, set a square on `Furniture` and one on `Floor`; compile; the `Furniture` tile is in that square's stack and the `Floor` edit is not (Finding 3); nothing else changed.
6. **Levels, rooms, seams, errors.** As in the plan: a layer at `level="2"`; a room square drops ground; tiles across a 300-tile seam land in the right lot cells; damaged and missing files; `--seed` rejected in `.pzw` mode; more than 64 levels.
7. The existing bitmap-mode tests keep passing unchanged.

## Deliverables

`worlded.exe` v0.6.0 with TMX mode, README and `FORMATS.md` notes (embedded picture format and merge rule), a pre-release.

## Not verified until the user does it

Lot files from either mode have never been tried in Project Zomboid. The merge rule was measured on one generated map. The editor's `Floor` edits do not survive compilation wherever a bitmap generates ground, as in WorldEd; if you want hand edits to win instead, that is a later option, not part of 4a.

## Results (2026-11-15)

Compared, all passing:
- TMX mode equals bitmap mode byte for byte on the one-cell oracle project (ground only), and on a generated 600 x 600 map with the 121 building lots of the real1 fixture (9 lot cells, 27 files; opt-in test `tmx_mode_matches_worlded_with_buildings`).
- Against WorldEd by alias group: 0 of 90,000 squares differ on the one-cell project (committed test `tmx_mode_matches_worlded_by_alias`); 1,776 of 422,696 (0.42%) differ on the building map. The differing squares are building wall tiles (for example `walls_exterior_house_08` where WorldEd has `_02`). Both modes give the same bytes, so this is in the shared building code of stage 2, not in TMX mode. Not investigated further.
- Edit round trip: a `Furniture` edit changes exactly its square's stack; a `Floor` edit changes nothing, as WorldEd does.
- Levels, rooms, the 300-tile cell seam, the 64-level limit, the command-line errors: tests in `tests/tmx_mode.rs` and `tests/cli.rs`.

Found on the way:
- The comparison with WorldEd exposed a stage 2 ground bug: where the landscape and the vegetation picture both give a tile on one layer, WorldEd lets the vegetation tile win; the compiler used file order. Fixed in `terrain.rs` and in the Python reference `knoxbuild/terrain.py` (unit test added). It changed which tile wins on about 5,900 of 90,000 squares of the oracle map.
- `worlded_lots.py` assigned maps to cells by sorted file name (wrong on 2 x 2 maps); fixed.

Not checked:
- Rules marked `obsolete = true` in `Rules.txt` (40 of them) are applied; whether WorldEd skips them could not be seen on the test map.
- Rules come from `Rules.txt`, not from the copy embedded in each `.tmx`.
- Random picks inside a rule's entry list (trees, flowers, grass) cannot match WorldEd's; the WorldEd comparison treats the groups of one such rule as one class.
- The merge rule was measured on one generated map (seven generated layers).
- No lot file from either mode has been loaded in Project Zomboid.
