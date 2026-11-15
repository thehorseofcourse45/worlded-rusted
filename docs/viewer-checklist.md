# Viewer manual checklist

Make a demo project: `python tools/make_demo_tmx.py viewer_demo`, then run
`worlded-view.exe viewer_demo/demo.pzw` (optional 2nd argument: cell index to open at once).

| Check | Result (2026-11-15) |
|---|---|
| Opens a `.pzw`; cells listed, one with a missing `.tmx` is marked | done: 2 cells, project path remembered in settings |
| Set the Project Zomboid folder; tiles load from `media/texturepacks` | done: 9 packs read, real floor art drawn |
| Open a cell; isometric tiles drawn in layer order | done: screenshot checked, demo floor + road band |
| Layer checkboxes hide/show layers | **not yet checked by hand** |
| Pan (drag), zoom (wheel, about the cursor) | **not yet checked by hand** |
| Tile coordinates under the cursor in the status bar | **not yet checked by hand** |
| Tiles match WorldEd's view of the same cell | **not checked**: no real TMX available, and no WorldEd view to compare |
| Double-click a cell in the world view opens it | **not yet checked by hand** (the 2nd-argument path was used instead) |
| No PZ folder set: placeholders and a banner, no crash | **not yet checked by hand** (covered by the atlas unit test) |
