"""Make a small demo project for the viewer: demo.pzw + demo_0_0.tmx (300 x 300, two layers).

    python tools/make_demo_tmx.py <out folder>

Uses real PZ floor tile names, so the viewer shows art once the Project Zomboid folder is set.
"""
import base64
import os
import random
import struct
import sys
import zlib

W = 300


def layer(name, gids):
    raw = struct.pack(f"<{len(gids)}I", *gids)
    data = base64.b64encode(zlib.compress(raw)).decode()
    return f' <layer name="{name}" width="{W}" height="{W}"><data encoding="base64" compression="zlib">{data}</data></layer>\n'


def main(out: str) -> None:
    os.makedirs(out, exist_ok=True)
    rnd = random.Random(1)
    grass = [8, 9, 10, 11, 16]
    floor = [1 + rnd.choice(grass) for _ in range(W * W)]
    road = [0] * (W * W)
    for y in range(W):
        for x in range(140, 160):
            road[y * W + x] = 1 + 16
    tmx = (f'<?xml version="1.0" encoding="UTF-8"?>\n<map version="1.0" orientation="isometric" width="{W}" height="{W}" tilewidth="64" tileheight="32">\n'
           ' <tileset firstgid="1" name="floors_exterior_natural_01" tilewidth="64" tileheight="128"/>\n'
           + layer("0_Floor", floor) + layer("0_FloorOverlay", road) + "</map>\n")
    open(os.path.join(out, "demo_0_0.tmx"), "w", encoding="utf-8").write(tmx)
    open(os.path.join(out, "demo.pzw"), "w", encoding="utf-8").write(
        '<?xml version="1.0" encoding="UTF-8"?>\n<world version="1.0" width="2" height="1">\n'
        ' <GenerateLots><worldOrigin origin="70,0"/></GenerateLots>\n'
        ' <cell x="0" y="0" map="Z:/gone/demo_0_0.tmx"/>\n <cell x="1" y="0" map="Z:/gone/demo_1_0.tmx"/>\n</world>\n')


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "viewer_demo")
