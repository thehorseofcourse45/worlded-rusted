"""Does WorldEd compile a world made of two bitmaps, each under the size limit?  sizetest2.py W H"""
import os
import re
import shutil
import struct
import sys

sys.path.insert(0, os.environ.get("KNOXMAP_DIR", "."))
W, H = int(sys.argv[1]), int(sys.argv[2])
SRC = os.environ.get("KNOXMAP_DIR", ".") + "/output/knoxify_1791479055303"
SN = "knoxify_1791479055303"
N = f"sz2_{W}x{H}"
OUT = fos.environ.get("KNOXMAP_DIR", ".") + "/output/{N}"


def pix(path):
    with open(path, "rb") as f:
        head = f.read(54)
        off = struct.unpack_from("<I", head, 10)[0]
        f.seek(off)
        return f.read(3)


def write_bmp(path, w, h, bgr):
    stride = (w * 3 + 3) // 4 * 4
    with open(path, "wb") as f:
        size = 54 + stride * h
        f.write(b"BM" + struct.pack("<IHHI", size, 0, 0, 54))
        f.write(struct.pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, stride * h, 2835, 2835, 0, 0))
        block = (bgr * w).ljust(stride, bytes(1)) * 256
        left = h
        while left > 0:
            n = min(256, left)
            f.write(block[: n * stride])
            left -= n


shutil.rmtree(OUT, ignore_errors=True)
os.makedirs(OUT)
land = pix(f"{SRC}/{SN}.bmp")
veg = pix(f"{SRC}/{SN}_veg.bmp")
cx, cy = W // 300, H // 300
cells_a = cx // 2
wa = cells_a * 300
wb = W - wa
for nm, w in ((N + "_a", wa), (N + "_b", wb)):
    write_bmp(f"{OUT}/{nm}.bmp", w, H, land)
    write_bmp(f"{OUT}/{nm}_veg.bmp", w, H, veg)
write_bmp(f"{OUT}/{N}_ZombieSpawnMap.bmp", W // 10, H // 10, bytes(3))
s = open(f"{SRC}/{SN}.pzw", encoding="utf-8").read()
head = s[: s.index(" <bmp ")].replace(SN, N)
head = re.sub(r'<world version="1.0" width="\d+" height="\d+">',
              f'<world version="1.0" width="{cx}" height="{cy}">', head, count=1)
with open(f"{OUT}/{N}.pzw", "w", encoding="utf-8") as f:
    f.write(head + f' <bmp path="{N}_a.bmp" x="0" y="0" width="{cells_a}" height="{cy}"/>\n'
            f' <bmp path="{N}_b.bmp" x="{cells_a}" y="0" width="{cx - cells_a}" height="{cy}"/>\n</world>\n')
os.makedirs(f"{OUT}/tmx")
os.makedirs(f"{OUT}/lots")

from tools import compile_map as cm

print("BITMAPS", wa * H, wb * H, "pixels each; world", W * H)
try:
    n = cm.compile_map(OUT, batch=1, only_cells=[[0, 0, 0, 0], [cx - 2, cy - 2, cx - 2, cy - 2]],
                       incremental=False)
    print("RESULT compiled", n, "failed", cm.failed_cells(OUT))
except Exception as e:
    print("RESULT ERROR", type(e).__name__, str(e)[:400])
print("TMX", len(os.listdir(f"{OUT}/tmx")), "LOTS", sorted(os.listdir(f"{OUT}/lots"))[:6])
