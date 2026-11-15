# Handoff: worlded-rusted

Project: Rust rewrite of the Project Zomboid map editor WorldEd. Folder `C:\worlded-rusted`, branch `main` (also local branches `viewer`, `compiler-port`, both merged). Repo: github.com/thehorseofcourse45/worlded-rusted (MIT, clean-room: formats come from real files, never WorldEd source).

## Goal
Full editor in five stages (roadmap in `docs/superpowers/specs/2026-11-15-viewer-design.md`): viewer, compiler port, tile editing, buildings/objects/roads/lots export, BMP import and tools.

## Done (checked by `cd rust && cargo test --release`, all green; Python on PATH needed for the comparison tests)
- Stage 0, `rust/knoxlots`: lot file reader/writer and `compile` (pre-existing, unchanged).
- Stage 1 viewer, `rust/worlded-core` (pzw, tmx, pack) + `rust/worlded-app` (egui, binary `worlded-view`). Release v0.3.0. Screenshot-checked on a generated demo cell only.
- Stage 2 compiler, `rust/worlded-compile` (binary `worlded`). Cell sources byte-identical to the Python pipeline (`knoxbuild/`, `tools/map_source.py`) on generated maps, 113 real buildings and a roof sweep; 64 cells in 2.3 s vs 18.7 s Python. Release v0.4.0 (zip tested from a clean folder).
- Both reviewed by a fresh subagent; Important findings fixed.
- Test helpers: `tools/dump_for_rust.py` (Python oracle), `tools/make_test_map.py`, `tools/make_demo_tmx.py`.

## Not done
- Stage 3 (tile editing): spec and plan written, no code. Blocker: no real WorldEd `.tmx` exists on this machine; the TMX reader follows public Tiled format and is unverified.
- Never tested in the game: lot files from either compiler.
- Viewer UI not hand-checked: pan, zoom, layer toggles, hover readout, double-click open (`docs/viewer-checklist.md`).
- Deferred minors: output folder keeps stale cells after a failed run; `worlded .` gives a bad name; list-valued `condition` in Rules.txt ignored; stray `./config` can override the exe's config; empty alias accepted silently; worker panic lets other threads continue; viewer decodes tile pages on the UI thread.

## Git (as of last check)
- Working tree clean except this new `HANDOFF.md` (uncommitted). `main` = `origin/main` at `7ed7d90`. No open PRs. Pre-releases v0.2.0, v0.3.0, v0.4.0.

## Next steps
1. Start stage 3 with execution method chosen by user (needs user: Native recommended, or subagent-driven). Plan: `docs/superpowers/plans/2026-11-15-editing.md`; first task = real TMX fixtures: find how KnoxMap calls `PZWorldEd_cli.exe` (`C:\KnoxMap\vendor\PZMappingTools\bin`) and write `tools/make_real_tmx.py`.
2. If WorldEd cannot be run headless, decide the fallback: plain Tiled TMX only (needs user).
3. Hand-check the viewer and fill `docs/viewer-checklist.md` (needs user, needs a GUI).
4. Try a compiled map in Project Zomboid (needs user).
5. Stage 4 needs a TMX to cell-source step in `worlded-compile` before an edited TMX can become lot files.

## To resume
- `cd C:\worlded-rusted\rust && cargo test --release`; viewer: `python tools/make_demo_tmx.py C:\wr_demo\viewer_demo`, then `cargo run --release -p worlded-app -- C:\wr_demo\viewer_demo\demo.pzw 0` (needs the PZ folder set in the app).
- Compiler: `rust\target\release\worlded.exe <map folder>`.
- Permission: commit/push/merge/PR/release are allowed without asking (user preference). Never launch the Firestorm viewer. Ask before touching files outside `C:\worlded-rusted`.
- Shell quirk: bash heredocs with an odd number of apostrophes (Rust char literals) fail; write such files with the Write tool.
- `Rules.txt`/`Blends.txt` in `config/` are GPL data from PZ Mapping Tools; the user should decide if shipping them is fine.

## Do not redo
- `Tilesets.txt` reader dropped: pack entries are named by tile.
- `.pack` format found (PZPK): see `FORMATS.md`.
- Python made order-canonical (`sorted(world.items())` in `tools/map_source.py`) so byte comparison is possible.
- Bitmap reader supports only 24-bit and 8-bit BMPs; `.pzw` read as XML; origin hard-coded 21000 as in Python.
- Running the reviewer with `model: opus` failed; default model works.
