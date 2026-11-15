"""Make a deterministic test project: bitmaps in real Rules.txt colours plus the real1 buildings.

    python tools/make_test_map.py <out folder> [--size N] [--seed S]

The folder is named like the map (its last path part): <name>.bmp, <name>_veg.bmp, <name>.pzw,
buildings/*.tbx. Blocks of colour from the rules (grass, dirt, road, water ...) so blends,
vegetation and buildings all have something to do. N is the picture size (default 600).
"""
import os
import random
import re
import shutil
import sys

import numpy as np
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)


def main(argv):
    from knoxbuild.rules import load_rules
    args = [a for a in argv if not a.startswith("--")]
    size = int(argv[argv.index("--size") + 1]) if "--size" in argv else 600
    seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 1
    if "--size" in argv:
        args.remove(argv[argv.index("--size") + 1])
    if "--seed" in argv:
        args.remove(argv[argv.index("--seed") + 1])
    out = args[0]
    name = os.path.basename(os.path.normpath(out))
    os.makedirs(out, exist_ok=True)
    rules = load_rules(os.path.join(ROOT, "config", "Rules.txt"))
    rnd = random.Random(seed)
    land_cols = [r.color for r in rules.rules if r.bitmap == 0]
    cond_rules = [r for r in rules.rules if r.condition is not None]
    veg_cols = [r.color for r in rules.rules if r.bitmap != 0 and r.condition is None]
    rnd.shuffle(land_cols)
    rnd.shuffle(veg_cols)
    land_cols = land_cols[:10]
    veg_cols = veg_cols[:6]
    for r in cond_rules[:3]:
        land_cols.append(r.condition)
        veg_cols.append(r.color)
    block = 12
    n = (size + block - 1) // block
    lgrid = np.array([[rnd.randrange(len(land_cols)) for _ in range(n)] for _ in range(n)])
    vgrid = np.array([[rnd.randrange(len(veg_cols) * 3) for _ in range(n)] for _ in range(n)])
    land = np.zeros((n * block, n * block, 3), np.uint8)
    veg = np.full((n * block, n * block, 3), 255, np.uint8)  # 255 255 255 is no rule colour
    for by in range(n):
        for bx in range(n):
            land[by * block:(by + 1) * block, bx * block:(bx + 1) * block] = land_cols[lgrid[by, bx]]
            v = vgrid[by, bx]
            if v < len(veg_cols):
                veg[by * block:(by + 1) * block, bx * block:(bx + 1) * block] = veg_cols[v]
    Image.fromarray(land[:size, :size]).save(os.path.join(out, f"{name}.bmp"))
    Image.fromarray(veg[:size, :size]).save(os.path.join(out, f"{name}_veg.bmp"))
    src = os.path.join(ROOT, "rust", "knoxlots", "fixtures", "real", "real1")
    shutil.rmtree(os.path.join(out, "buildings"), ignore_errors=True)
    shutil.copytree(os.path.join(src, "buildings"), os.path.join(out, "buildings"))
    text = open(os.path.join(src, "real1.pzw"), encoding="utf-8").read()
    text = re.sub(r'<world version="1.0" width="\d+" height="\d+"', f'<world version="1.0" width="{(size + 299) // 300}" height="{(size + 299) // 300}"', text, 1)
    open(os.path.join(out, f"{name}.pzw"), "w", encoding="utf-8", newline="\n").write(text)


if __name__ == "__main__":
    main(sys.argv[1:])
