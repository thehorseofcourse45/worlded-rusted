"""Does WorldEd's BMP to TMX load a landscape bitmap of this size?  sizetest.py W H"""
import os
import re
import shutil
import struct
import sys

import numpy as np

sys.path.insert(0, os.environ.get("KNOXMAP_DIR", "."))
W, H = int(sys.argv[1]), int(sys.argv[2])
SRC = os.environ.get("KNOXMAP_DIR", ".") + "/output/knoxify_1791479055303"
SN = "knoxify_1791479055303"
OUT = fos.environ.get("KNOXMAP_DIR", ".") + "/output/sz_{W}x{H}"
N = f"sz_{W}x{H}"


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
        row = (bgr * w).ljust(stride, b"\0")
        block = row * 256
        left = h
        while left > 0:
            n = min(256, left)
            f.write(block[: n * stride])
            left -= n


shutil.rmtree(OUT, ignore_errors=True)
os.makedirs(OUT)
land = pix(f"{SRC}/{SN}.bmp")
veg = pix(f"{SRC}/{SN}_veg.bmp")
write_bmp(f"{OUT}/{N}.bmp", W, H, land)
write_bmp(f"{OUT}/{N}_veg.bmp", W, H, veg)
write_bmp(f"{OUT}/{N}_ZombieSpawnMap.bmp", W // 10, H // 10, b"\0\0\0")
cx, cy = -(-W // 300), -(-H // 300)
s = open(f"{SRC}/{SN}.pzw", encoding="utf-8").read()
head = s[: s.index(" <bmp ")]
head = head.replace(SN, N).replace('width="20" height="17"', f'width="{cx}" height="{cy}"', 1)
head = re.sub(r"<world version=\"1.0\" width=\"\d+\" height=\"\d+\">", f'<world version="1.0" width="{cx}" height="{cy}">', head, 1)
with open(f"{OUT}/{N}.pzw", "w", encoding="utf-8") as f:
    f.write(head + f' <bmp path="{N}.bmp" x="0" y="0" width="{cx}" height="{cy}"/>\n</world>\n')
os.makedirs(f"{OUT}/tmx")
os.makedirs(f"{OUT}/lots")

from tools import compile_map as cm

try:
    n = cm.compile_map(OUT, batch=1, only_cells=[[0, 0, 0, 0]], incremental=False)
    print("RESULT", W, H, W * H, "compiled", n, "failed", cm.failed_cells(OUT))
except Exception as e:
    print("RESULT", W, H, W * H, "ERROR", type(e).__name__, str(e)[:300])
