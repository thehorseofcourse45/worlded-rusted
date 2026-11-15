# Editing manual checklist

Make a project with real WorldEd tmx files: `python tools/make_real_tmx.py C:\wr_demo\edit_demo`
(about 90 s, needs the WorldEd command line), then
`worlded-view.exe C:\wr_demo\edit_demo\rt\rt.pzw 0`. Set the Project Zomboid folder in the app
for real tile art and the palette.

Automated results (2026-11-15): unit tests for tools, history, document, save, gids, palette
grouping, tile-under-cursor at every level, and the paint/fill/pick/refusal flow of the cell
state, all passing. WorldEd loading an edited and saved map: see the last row.

| Check | Result |
|---|---|
| Palette lists tilesets; search narrows them; thumbnails show real art | **not checked by hand** |
| Click a palette tile: it becomes the selected tile (highlighted, name in the tile box) | **not checked by hand** |
| Paint: click and drag lay tiles on the active layer, with a yellow preview of the brush | **not checked by hand** |
| Brush sizes 1 to 9; even sizes extend right and down | logic tested; **not checked by hand** |
| Erase, Rectangle, Fill, Eyedropper | logic tested; **not checked by hand** |
| One drag is one Ctrl+Z; Ctrl+Y / Ctrl+Shift+Z redo | logic tested; **keys not checked by hand** |
| Middle-button or space + drag pans; wheel zooms about the cursor | **not checked by hand** |
| Clicking a tile lands under the cursor on a layer with level above 0 | maths tested; **not checked by hand** |
| Hidden or locked active layer: edit refused, status bar says why | logic tested; **not checked by hand** |
| Ctrl+S saves; first save makes `<file>.bak`; title and layer heading lose `*` | logic tested; **keys and title not checked by hand** |
| Ctrl+Shift+S saves under a new name | **not checked by hand** |
| Reopen the saved file in the viewer: edits are there, nothing else moved | tested on the real fixture (`edit_then_reopen_changes_only_the_edited_squares`); **viewer reopen not checked by hand** |
| A map with an unknown data encoding opens read-only with a banner | logic tested; **banner not checked by hand** |
| Close or Back to world with unsaved edits asks Save / Discard / Cancel | **not checked by hand** |
| WorldEd generates lots from an edited, saved map | **done** (`saved_edit_loads_in_worlded`, 39 s): 10 + 10 squares edited on layers 0 and 14 of a real cell, saved, WorldEd exited 0 and wrote 4 lot cells whose headers list both edited tiles. Not tried in the game. |
