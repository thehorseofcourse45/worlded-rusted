# worlded-rusted editor, sub-project 1: the viewer

Status: draft for review. Date: 2026-11-15.

## Goal

A standalone Rust clone of PZWorldEd (the Project Zomboid map editor), built in stages. This spec
covers stage 1 only: a **read-only viewer** that opens a WorldEd project and shows its map.
No Qt, no WorldEd, no KnoxMap at runtime. MIT licensed, clean-room (file formats and observed
behaviour only, no WorldEd source; see the README).

## Whole-editor roadmap (each stage gets its own spec, plan and build)

1. **Viewer** (this spec).
2. Compiler port: move the Python map-to-lots code into Rust, so one binary does everything.
3. Editing: tile painting, brushes, undo/redo, save `.tmx` and `.pzw`.
4. Buildings, objects, roads, zombie spawn, lots export from the GUI.
5. BMP to TMX import, Lua tools, remaining tool windows.

## Scope of the viewer

In:
- Open a `.pzw` project. World view: grid of 300x300-tile cells that have a map, with names.
- Double-click a cell to open its `.tmx` at tile level.
- Draw tile layers in order; per-layer visibility toggle; pan; zoom; tile coordinates under the
  cursor in a status bar.
- Tiles looked up by name (e.g. `floors_burnt_01_8`) through the tileset list and the game's
  `.pack` texture packs.
- Settings: path to the user's Project Zomboid install (file picker, remembered).

Out (later stages): any editing or saving, undo, BMP import, Lua, objects/roads editing,
isometric polish (orthogonal or simple isometric rendering is enough).

## Architecture

Cargo workspace in this repo:

- `rust/knoxlots` (exists): lot files and the compiler. Untouched.
- `rust/worlded-core` (new, no UI): the project model and file formats.
  - `pzw`: parse `.pzw` (cells, map paths, world size, origin).
  - `tmx`: parse `.tmx` (tilesets, tile layers, object groups read but not drawn).
  - `tilesets`: parse `Tilesets.txt`; map a tile name to (pack, sheet, index).
  - `pack`: read `.pack` texture packs from the PZ install; decode a sheet to RGBA.
  - Each module is usable and testable alone; none depends on egui.
- `rust/worlded-app` (new): egui/eframe window. Depends on `worlded-core`.
  - `world_view`, `cell_view`, `layers_panel`, `settings`, `atlas` (GPU texture cache).

## Data flow

open `.pzw` -> list cells -> user opens a cell -> parse its `.tmx` -> collect the tile names
used -> resolve each to a sheet in `.pack` -> decode needed sheets once, cache as GPU textures
-> draw visible layers each frame, culled to the visible rectangle.

## Error handling

- Missing PZ install or pack: viewer still opens; tiles draw as labelled placeholder squares and
  a banner asks for the install path.
- Unknown tile name or damaged file: skip that tile or file, log once, never panic (same rule as
  `knoxlots`: damaged input is an error value, not a crash).
- Large maps: only parse/decode what is on screen or opened.

## Testing

- `worlded-core`: unit tests per module on the existing fixtures (`rust/knoxlots/fixtures/real/real1`
  for `.pzw`); damaged-file tests as in `knoxlots`.
- `pack` and `tmx` formats are not yet documented here. **First task of the plan is to work them
  out from real files** (clean-room) and write `FORMATS.md` before any code depends on them.
- App: a headless render test (draw a known small cell to an image, compare to a stored
  reference), plus a manual checklist: open real1, pan/zoom, toggle layers.
- No test needs a PZ install; tests that do skip, as in the Python suite.

## Open risks

- `.pack` layout is unverified; if it is awkward, fall back to the loose tile PNGs some installs
  and KnoxMap's `Tiles/` folder provide.
- egui has no built-in huge-canvas widget; the cell view is a custom painter with culling.

## Success criteria

Open `real1.pzw`, see its 9 cells, open one, and see building and ground tiles with real art
matching how WorldEd shows the same cell, at interactive frame rates.
