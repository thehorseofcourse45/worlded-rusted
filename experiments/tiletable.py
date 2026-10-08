import glob
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hdr2

SETS = (("house exp", "exp/house/lots"), ("ches", "ches"),
        ("galway", os.path.expanduser("~/Zomboid/mods/Galway_Ireland/common/media/maps/Galway_Ireland")))
for label, pat in SETS:
    ps = sorted(glob.glob(pat + "/*.lotheader"))[:25]
    srt = unique = mismatched = first_use = 0
    for p in ps:
        h = hdr2.parse(open(p, "rb").read())
        t = h["tiles"]
        srt += t == sorted(t)
        unique += len(set(t)) == len(t)
        x, y = os.path.basename(p)[:-10].split("_")
        d = open(f"{pat}/world_{x}_{y}.lotpack", "rb").read()
        offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(1024)] + [len(d)]
        used = set()
        order = []
        for i in range(1024):
            ch = d[offs[i]:offs[i + 1]]
            o = 0
            while o < len(ch):
                n = struct.unpack_from("<I", ch, o)[0]
                o += 4
                if n == 0xFFFFFFFF:
                    o += 4
                else:
                    for v in struct.unpack_from(f"<{n - 1}I", ch, o + 4):
                        if v not in used:
                            used.add(v)
                            order.append(v)
                    o += 4 * n
        mismatched += len(used) != len(t)
        first_use += order == sorted(order)
    print(label, "cells", len(ps), "| table sorted by name:", srt, "| names unique:", unique,
          "| table size != tiles used:", mismatched, "| first use in index order:", first_use)
