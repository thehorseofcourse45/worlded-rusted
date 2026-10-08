"""What do the header's 1024 bytes and chunkdata's 1024 bytes correlate with?"""
import os
import collections
import glob
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hdr2

LOTS = os.environ.get("KNOXMAP_DIR", ".") + "/output/knoxify_1791479055303/lots"
hv = collections.Counter()
cv = collections.Counter()
xtab = collections.defaultdict(collections.Counter)    # chunkdata value -> feature counts
blocks_vs = collections.Counter()
rows = []
for hp in sorted(glob.glob(LOTS + "/*.lotheader"))[:120]:
    x, y = hp.replace("\\", "/").split("/")[-1].split(".")[0].split("_")
    h = hdr2.parse(open(hp, "rb").read())
    hv.update(h["chunk_values"] if "chunk_values" in h else [])
    tail = open(hp, "rb").read()[-1024:]
    hv.update(tail)
    c = open(f"{LOTS}/chunkdata_{x}_{y}.bin", "rb").read()
    grid, nblk = c[2:1026], (len(c) - 1026) // 64
    cv.update(grid)
    blocks_vs[(nblk, sum(1 for v in grid if v not in (0, 16)))] += 1
    d = open(f"{LOTS}/world_{x}_{y}.lotpack", "rb").read()
    offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(1024)] + [len(d)]
    names = h["tiles"]
    for i in range(1024):
        ch = d[offs[i]:offs[i + 1]]
        o = 0
        tiles = set()
        while o < len(ch):
            n = struct.unpack_from("<I", ch, o)[0]
            o += 4
            if n == 0xFFFFFFFF:
                o += 4
            else:
                for k in range(n):
                    t = struct.unpack_from("<I", ch, o + 4 * k)[0]
                    if k > 0:
                        tiles.add(names[t].rsplit("_", 1)[0])
                o += 4 * n
        feat = ("walls" if any(t.startswith("walls") for t in tiles) else "") + \
               ("road" if any("street" in t or "road" in t for t in tiles) else "") + \
               ("veg" if any(t.startswith("vegetation") for t in tiles) else "") + \
               ("water" if any("water" in t for t in tiles) else "")
        xtab[grid[i]][feat or "-"] += 1
print("header tail byte histogram", dict(hv.most_common(8)))
print("chunkdata grid histogram", dict(cv.most_common(8)))
print("(blocks, chunks with value other than 0/16) per file:", dict(list(blocks_vs.most_common(6))))
for v in sorted(xtab):
    print("value", v, dict(xtab[v].most_common(6)))
