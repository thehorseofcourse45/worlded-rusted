"""A whole map as cell sources for `knoxlots compile`: ground, edge blends, buildings, tables.

    python tools/map_source.py <map folder> <out folder> [--seed N] [--worlded-seams]

The map folder is what KnoxMap writes: <name>.bmp, <name>_veg.bmp, <name>.pzw, buildings/*.tbx
and <name>_ZombieSpawnMap.bmp. For every 256-tile cell this makes the terrain tiles (Rules.txt,
Blends.txt), puts the buildings on them, and writes the room and building tables of the header.

How buildings go onto the ground (see merge_stack): a square with a room has the building's
tiles and none of the ground's; any other square keeps its ground and gets the building's tiles
on top (an outer wall stands on grass).
"""
from __future__ import annotations

import os
import re
import shutil
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from knoxbuild import bitmaps  # noqa: E402
from knoxbuild.blends import blend_layers, load_blends  # noqa: E402
from knoxbuild.buildheader import cell_tables, placed_rooms  # noqa: E402
from knoxbuild.buildtiles import compose_lots, parse_tbx, tile_name  # noqa: E402
from knoxbuild.kcell import SIDE, cell_source  # noqa: E402
from knoxbuild.rules import load_rules  # noqa: E402
from knoxbuild.terrain import Palette, ground_layers, stack_order  # noqa: E402
from tools.terrain_source import ORIGIN_X, rules_path  # noqa: E402


def read_lots(project: str):
    """[(name, tx, ty, base level, width, height)] in project order, with world tile positions."""
    name = os.path.basename(os.path.normpath(project))
    lots, cell = [], None
    for line in open(os.path.join(project, f"{name}.pzw"), encoding="utf-8"):
        m = re.match(r'\s*<cell x="(\d+)" y="(\d+)"', line)
        if m:
            cell = (int(m[1]), int(m[2]))
        m = re.search(r'<lot x="(\d+)" y="(\d+)" level="(\d+)" width="(\d+)" height="(\d+)" '
                      r'map="buildings/([^"]+)\.tbx"', line)
        if m and cell:
            lots.append((m[6], cell[0] * 300 + int(m[1]), cell[1] * 300 + int(m[2]), int(m[3]),
                         int(m[4]), int(m[5])))
    return lots


def merge_stack(ground: list[str], building: list[str], has_room: bool) -> list[str]:
    """One square's tiles from its ground and its building's."""
    return building if has_room else ground + building


def make_map_source(project: str, out: str, seed: int = 1, seams: bool = False) -> int:
    name = os.path.basename(os.path.normpath(project))
    land_path = os.path.join(project, f"{name}.bmp")
    veg_path = os.path.join(project, f"{name}_veg.bmp")
    head = bitmaps._bmp_header(land_path)
    w, h = head[0], head[1]
    rules_file = rules_path()
    rules = load_rules(rules_file)
    blends = load_blends(os.path.join(os.path.dirname(rules_file), "Blends.txt"))

    # buildings, composed as WorldEd places them
    lots = read_lots(project)
    parsed = [(parse_tbx(open(os.path.join(project, "buildings", f"{t}.tbx"), encoding="utf-8").read()),
               tx, ty, lv) for t, tx, ty, lv, _w, _h in lots]
    world = compose_lots(parsed)
    room_squares = set()
    for b, tx, ty, base in parsed:
        for z, floor in enumerate(b.floors):
            for y, row in enumerate(floor.rooms):
                for x, v in enumerate(row):
                    if v > 0:
                        room_squares.add((tx + x, ty + y, base + z))
    placed = placed_rooms([(b, tx, ty, lv) for b, tx, ty, lv in parsed if b.floors])

    os.makedirs(out, exist_ok=True)
    pal = Palette()
    levels_seen = max((z for _x, _y, z in world), default=0) + 1
    written = 0
    for cy in range((h + SIDE - 1) // SIDE):
        wy0 = cy * SIDE
        y_lo, y_hi = max(wy0 - 1, 0), min(wy0 + SIDE + 1, h)
        land_strip = bitmaps.read_rgb_rows(land_path, y_lo, y_hi, w)
        veg_strip = bitmaps.read_rgb_rows(veg_path, y_lo, y_hi, w)
        for cx in range(ORIGIN_X // SIDE, (ORIGIN_X + w - 1) // SIDE + 1):
            wx0 = cx * SIDE - ORIGIN_X
            win = SIDE + 2
            lw = np.zeros((win, win, 3), np.uint8)
            vg = np.zeros((win, win, 3), np.uint8)
            inside = np.zeros((win, win), bool)
            x_lo, x_hi = max(wx0 - 1, 0), min(wx0 + SIDE + 1, w)
            if x_lo < x_hi and y_lo < y_hi:
                sx, sy = x_lo - (wx0 - 1), y_lo - (wy0 - 1)
                rows, cols = slice(sy, sy + y_hi - y_lo), slice(sx, sx + x_hi - x_lo)
                lw[rows, cols] = land_strip[:, x_lo:x_hi]
                vg[rows, cols] = veg_strip[:, x_lo:x_hi]
                inside[rows, cols] = True
            layers = ground_layers(lw, vg, rules, pal, seed, x0=wx0 - 1, y0=wy0 - 1)
            layers = {k: np.where(inside, v, 0) for k, v in layers.items()}
            if "0_Floor" in layers:
                layers.update(blend_layers(layers["0_Floor"], rules, blends, pal, seed,
                                           x0=wx0 - 1, y0=wy0 - 1, seam=300 if seams else None))
            layers = {k: v[1:-1, 1:-1] for k, v in layers.items()}
            inside = inside[1:-1, 1:-1]
            order = stack_order(layers)

            # the stacks of the cell: level 0 is ground and buildings, above it buildings only
            stacks: dict[tuple[int, int, int], list[str]] = {}
            for yy in range(SIDE):
                for xx in range(SIDE):
                    wx, wy = wx0 + xx, wy0 + yy
                    g = []
                    bt = world.get((wx, wy, 0), [])
                    if inside[yy, xx]:
                        # a fence clears the ground's furniture (a garden bed under it); walls do not
                        fenced = any(t.startswith("fencing_") for t in bt)
                        g = [tile_name(pal.names[layers[k][yy, xx]]) for k in order
                             if layers[k][yy, xx] and not (fenced and k == "0_Furniture")]
                    s = merge_stack(g, bt, (wx, wy, 0) in room_squares)
                    if s:
                        stacks[(xx, yy, 0)] = s
            for (wx, wy, z), bt in world.items():
                if z > 0 and wx0 <= wx < wx0 + SIDE and wy0 <= wy < wy0 + SIDE:
                    stacks[(wx - wx0, wy - wy0, z)] = bt
            nlev = max((z for _, _, z in stacks), default=0) + 1
            depth = max((len(s) for s in stacks.values()), default=1)
            names: list[str] = []
            index: dict[str, int] = {}
            arrays = [[np.zeros((SIDE, SIDE), np.int64) for _ in range(depth)] for _ in range(nlev)]
            for (xx, yy, z), s in stacks.items():
                for k, t in enumerate(s):
                    i = index.get(t)
                    if i is None:
                        i = index[t] = len(names) + 1
                        names.append(t)
                    arrays[z][k][yy, xx] = i
            rooms, _buildings = cell_tables(cx, cy, placed)
            room_tuples = [(r["name"], r["z"], [tuple(x) for x in r["rects"]], []) for r in rooms]
            building_lists = _buildings
            data = cell_source(cx, cy, names, arrays, rooms=room_tuples, buildings=building_lists)
            with open(os.path.join(out, f"cell_{cx}_{cy}.kcell"), "wb") as f:
                f.write(data)
            written += 1
    spawn = f"{name}_ZombieSpawnMap.bmp"
    lines = ["format = knoxlots-source 1", f"origin_x = {ORIGIN_X}", "origin_y = 0"]
    if os.path.exists(os.path.join(project, spawn)):
        shutil.copy2(os.path.join(project, spawn), os.path.join(out, "spawn.bmp"))
        lines.append("spawn_map = spawn.bmp")
    with open(os.path.join(out, "source.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    return written


def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 1
    if "--seed" in argv:
        args.remove(argv[argv.index("--seed") + 1])
    if len(args) != 2:
        print(__doc__)
        return 2
    n = make_map_source(args[0], args[1], seed, seams="--worlded-seams" in argv)
    print(f"{n} cell sources written to {args[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
