"""Landscape pixel colour against the tile stack WorldEd put on that square (level 0).

    ground.py <variant> [...]
"""
import collections
import os
import struct
import sys

sys.path.insert(0, os.environ.get("KNOXMAP_DIR", "."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hdr2
from knoxbuild import bitmaps

H = os.path.dirname(os.path.abspath(__file__))
ORIGIN = 21000
NONE = 0xFFFFFFFF


def level0(lots, x, y):
    """{(tx, ty): tuple of tile names} for level 0 of one cell, tx/ty in cell tiles."""
    h = hdr2.parse(open(f"{lots}/{x}_{y}.lotheader", "rb").read())
    names = h["tiles"]
    d = open(f"{lots}/world_{x}_{y}.lotpack", "rb").read()
    offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(1024)] + [len(d)]
    out = {}
    for i in range(1024):
        cx, cy = i // 32, i % 32
        ch = d[offs[i]:offs[i + 1]]
        o = k = 0
        while o < len(ch) and k < 64:       # level 0 only: the first 64 squares
            n = struct.unpack_from("<I", ch, o)[0]
            o += 4
            if n == NONE:
                k += struct.unpack_from("<I", ch, o)[0]
                o += 4
            else:
                tiles = struct.unpack_from(f"<{n}I", ch, o)[1:]
                out[(cx * 8 + k // 8, cy * 8 + k % 8)] = tuple(names[t] for t in tiles)
                o += 4 * n
                k += 1
    return out


for v in sys.argv[1:]:
    base = f"{H}/exp/{v}"
    land = bitmaps.read_rgb(f"{base}/{v}.bmp")
    veg = bitmaps.read_rgb(f"{base}/{v}_veg.bmp")
    table = collections.defaultdict(collections.Counter)
    for f in os.listdir(f"{base}/lots"):
        if not f.endswith(".lotheader"):
            continue
        cx, cy = (int(t) for t in f[:-10].split("_"))
        sq = level0(f"{base}/lots", cx, cy)
        for (tx, ty), stack in sq.items():
            wx, wy = cx * 256 + tx - ORIGIN, cy * 256 + ty
            if 0 <= wy < land.shape[0] and 0 <= wx < land.shape[1]:
                key = (tuple(int(c) for c in land[wy, wx]), tuple(int(c) for c in veg[wy, wx]))
                table[key][stack] += 1
    print(f"== {v}: {land.shape[1]}x{land.shape[0]} tiles, {len(table)} (landscape, vegetation) colour pairs")
    for key, stacks in sorted(table.items(), key=lambda kv: -sum(kv[1].values()))[:8]:
        total = sum(stacks.values())
        top = stacks.most_common(3)
        print(f"  land{key[0]} veg{key[1]}: {total} squares, {len(stacks)} distinct stacks; top:",
              [(" + ".join(s) if s else "(none)", c) for s, c in top])
