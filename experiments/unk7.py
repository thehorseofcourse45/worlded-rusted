"""Where, in tile coordinates, are the flagged chunks?  unk7.py <variant>"""
import collections
import os
import sys

H = os.path.dirname(os.path.abspath(__file__))
v = sys.argv[1]
lots = f"{H}/exp/{v}/lots"
ORIGIN = 70 * 300      # world origin in tiles (70,0 in 300-cells)
found = collections.defaultdict(list)
for f in os.listdir(lots):
    if not f.startswith("chunkdata_"):
        continue
    cx, cy = (int(t) for t in f[10:-4].split("_"))
    g = open(f"{lots}/{f}", "rb").read()[2:1026]
    for i, val in enumerate(g):
        if val in (4, 16):
            for order, (a, b) in (("x-major", (i // 32, i % 32)), ("y-major", (i % 32, i // 32))):
                tx = cx * 256 + a * 8 - ORIGIN
                ty = cy * 256 + b * 8
                found[(val, order)].append((tx, ty))
for (val, order), pts in sorted(found.items()):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    print(val, order, "n", len(pts), "x", min(xs), max(xs) + 8, "y", min(ys), max(ys) + 8)
print("house should be near x 120..162, y 444..480 (tiles, y from north)")
