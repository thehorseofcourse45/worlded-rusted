import collections, os, sys
sys.path.insert(0, os.environ.get("KNOXMAP_DIR", ".")); sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bxshow import load
from knoxbuild.buildtiles import parse_tbx
H = os.path.dirname(os.path.abspath(__file__)); ORIGIN = 21000
b = parse_tbx(open(f"{H}/exp/house/buildings/house_0000.tbx", encoding="utf-8").read())
lx, ly = 269, 592
cx, cy = (ORIGIN + lx) // 256, ly // 256
sq, levels = load(f"{H}/exp/house/lots", cx, cy)
res = collections.Counter()
for z, fl in enumerate(b.floors[:levels]):
    for o in fl.objects:
        if o.get("type") != "furniture": continue
        d = b.furniture[int(o["FurnitureTiles"])]
        tiles = d["orients"].get(o["orient"], [])
        for k, (dx, dy, name) in enumerate(tiles):
            st = sq.get((ORIGIN + lx + int(o["x"]) + dx - cx * 256, ly + int(o["y"]) + dy - cy * 256, z), [])
            cur = [i for i, t in enumerate(st) if t.startswith("fixtures_windows_curtains")]
            if cur and name in st:
                res[(d["layer"] or "-", f"tile{k}" if k else "first", "furniture before curtains" if st.index(name) < cur[0] else "furniture after curtains")] += 1
for k, v in sorted(res.items()): print(v, k)
