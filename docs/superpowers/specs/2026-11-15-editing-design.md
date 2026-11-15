# worlded-rusted editor, sub-project 3: tile editing

Status: draft for review. Date: 2026-11-15. Roadmap: see `2026-11-15-viewer-design.md`.

## Goal

Turn the read-only viewer (`worlded-view`) into an editor for the tile layers of a cell map:
pick a tile, paint it, undo, save the `.tmx`, and have WorldEd or TileZed still open the result.
Written in Rust on `worlded-core` and `worlded-app`; MIT, clean-room as before.

## Scope

In:
- Tile palette: browse tiles by name from the game's `.pack` files (grouped by tileset prefix, with
  search), pick one, see its art.
- Tools: paint, erase, rectangle fill, bucket fill (same-tile region), eyedropper. A brush size of
  1 to 5 squares. All act on the selected layer of the open cell.
- Layers: select the active layer, show or hide it (exists), lock it, add and remove a tile layer.
- Undo and redo for every edit (unlimited within the session), with a dirty marker in the title.
- Save and Save As for the `.tmx`, written atomically (write a temp file, then rename), with a
  `.bak` of the previous file on the first save of a session.
- A cell open in the editor shows `*` when changed and asks before it is closed or replaced.

Out (later stages): building (`.tbx`) editing, objects and zones, roads and fences, zombie spawn,
`.pzw` editing, new cell creation, BMP to TMX import, Lua tools, exporting lot files from an edited
TMX (needs a TMX-to-cell-source step in `worlded-compile`; stage 4).

## The main risk: writing a TMX that WorldEd accepts

The stage 1 reader follows the public Tiled format and has never seen a real WorldEd `.tmx`. An
editor that saves must not damage what it does not understand. So:

1. **First task: get real files.** KnoxMap ships WorldEd's command line
   (`PZWorldEd_cli.exe`, in `vendor/PZMappingTools/bin`). Run its BMP to TMX step on a project made by
   `tools/make_test_map.py` to produce real `.tmx` files, keep two small ones as test fixtures
   (cell maps are big: trim or compress), and write the exact layout into `FORMATS.md`: tileset
   elements, layer attributes, data encoding and compression, object groups, properties.
2. **Lossless round trip.** The model keeps everything it does not edit as raw XML (tileset
   elements, object groups, properties, unknown attributes), in the original order. Opening and
   saving without edits must give a file equal to the input after XML normalisation, and WorldEd
   must open it (checked by running `PZWorldEd_cli.exe` on the saved result).
3. **Edits touch only layer data.** Saving re-encodes the changed layers with the same encoding and
   compression the file used.

## Architecture

- `worlded-core::tmx` grows a writer and a document model (`TmxDoc`) that holds the parsed layers
  plus the raw leftovers; the existing `Tmx` reader stays for the viewer or becomes a view of it.
- `worlded-core::edit` (no UI): `Edit` (a set of changed squares: layer, x, y, old gid, new gid),
  `History` (undo and redo stacks), tools as pure functions that turn a click and the document into
  an `Edit`: `paint`, `erase`, `rect_fill`, `bucket_fill`, `pick`. No UI types, fully testable.
- A tile name to gid mapping for the tilesets in the file; a tile not in any tileset of the file is
  added as a new tileset entry in the format WorldEd writes (found in step 1 above).
- `worlded-app`: `palette` panel, tool bar, cursor preview over the map, active-layer and lock
  controls, Save and Save As, close guard. The cell view maps a screen click to a tile with the
  existing `screen_to_tile` and the active layer's level (the stage 1 hover maths uses level 0; the
  tool must take the layer's level into account).

## Data flow

click or drag -> tool -> `Edit` (computed against the document) -> `History.apply` (changes the
document, pushes the inverse) -> view redraws only dirty squares from the document. Save -> encode
changed layers -> write temp file -> rename.

## Error handling

- A TMX with a layer or tileset the writer does not understand opens read-only with a banner
  ("this file has parts the editor cannot save safely") rather than risking damage.
- Save failure (disk full, file locked) leaves the old file untouched and the document dirty.
- Painting outside the map or on a locked or hidden layer does nothing and says why in the status bar.

## Testing

- Pure `edit` tests: each tool on small grids, undo restores exactly, redo re-applies, a drag is one
  history step, bucket fill respects the region boundary and a size limit.
- Round trip: open and save the real fixtures; compare XML after normalisation; edit one square,
  save, re-open, and confirm only that square changed.
- WorldEd acceptance (needs `PZWorldEd_cli.exe`, skipped without it): the saved file loads and
  generates lots without an error.
- App: headless unit tests for click-to-tile with levels; a manual checklist (paint, drag, undo,
  save, reopen in the viewer and in WorldEd).

## Open risks

- Real TMX layout unknown until step 1; if WorldEd uses features the writer cannot reproduce, the
  scope drops to "edit and save as plain Tiled TMX" and the plan says so.
- A 300 x 300 cell with many layers is large; edits must not copy whole layers (store the changed
  squares only) and saving must stream.
- Drag painting at high frame rates: apply an edit per frame, merge them into one history step.

## Success criteria

Open a real WorldEd cell `.tmx`, paint, erase and fill on several layers, undo and redo them, save,
and open the saved file in both `worlded-view` and WorldEd with only the intended squares changed.
