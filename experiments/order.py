"""Square order inside a chunk, and chunk order inside a pack, from room rectangles.

A header lists rooms as rectangles (x y w h in cell tiles, plus level z). A square that
is in room r carries r as its first value. So for the right ordering, every square
carrying room r lies inside a rectangle of room r.
"""
import os
import itertools
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hdr2

NONE = 0xFFFFFFFF
D = sys.argv[1]
cell = sys.argv[2]
h = hdr2.parse(open(f"{D}/{cell}.lotheader", "rb").read())
d = open(f"{D}/world_{cell}.lotpack", "rb").read()
offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(1024)] + [len(d)]
levels = h["maxz"] - h["minz"] + 1


def squares(i):
    ch = d[offs[i]:offs[i + 1]]
    o = 0
    out = []
    while o < len(ch):
        n = struct.unpack_from("<I", ch, o)[0]
        o += 4
        if n == NONE:
            out += [None] * struct.unpack_from("<I", ch, o)[0]
            o += 4
        else:
            out.append(struct.unpack_from(f"<{n}I", ch, o))
            o += 4 * n
    return out


def inside(room, x, y, z):
    name, rz, rects, _ = h["rooms"][room]
    return rz == z and any(rx <= x < rx + rw and ry <= y < ry + rh for rx, ry, rw, rh in rects)


rows = []
for chunk_order in ("x-major", "y-major"):
    for sq_order in ("x-major", "y-major"):
        for z_order in ("z-outer", "z-inner"):
            ok = bad = 0
            for i in range(1024):
                s = squares(i)
                cx, cy = (i // 32, i % 32) if chunk_order == "x-major" else (i % 32, i // 32)
                for k, sq in enumerate(s):
                    if sq is None or sq[0] == NONE:
                        continue
                    if z_order == "z-outer":
                        z, r = divmod(k, 64)
                        a, b = divmod(r, 8)
                    else:
                        r, z = divmod(k, levels)
                        a, b = divmod(r, 8)
                    sx, sy = (a, b) if sq_order == "x-major" else (b, a)
                    if inside(sq[0], cx * 8 + sx, cy * 8 + sy, z + h["minz"]):
                        ok += 1
                    else:
                        bad += 1
            rows.append((bad, ok, chunk_order, sq_order, z_order))
for r in sorted(rows):
    print(r)
