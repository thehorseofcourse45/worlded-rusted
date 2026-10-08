"""Hypothesis parser for .lotpack chunks; every chunk must parse to its exact end."""
import os
import glob
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hdr2

lots = sys.argv[1]
bad = tot = 0
levels_seen = {}
for hp in sorted(glob.glob(lots + "/*.lotheader"))[:int(sys.argv[2])]:
    x, y = hp.replace("\\", "/").split("/")[-1].split(".")[0].split("_")
    h = hdr2.parse(open(hp, "rb").read())
    d = open(f"{lots}/world_{x}_{y}.lotpack", "rb").read()
    assert d[:4] == b"LOTP"
    cnt = struct.unpack_from("<I", d, 8)[0]
    offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(cnt)] + [len(d)]
    levels = h["maxz"] - h["minz"] + 1
    levels_seen[(h["minz"], h["maxz"])] = levels_seen.get((h["minz"], h["maxz"]), 0) + 1
    for c in range(cnt):
        ch = d[offs[c]:offs[c + 1]]
        tot += 1
        o = 0
        done = 0
        want = 64 * levels
        try:
            while done < want:
                n = struct.unpack_from("<I", ch, o)[0]
                o += 4
                if n == 0xFFFFFFFF:
                    done += struct.unpack_from("<I", ch, o)[0]
                    o += 4
                else:
                    o += 4 * n
                    done += 1
            ok = (o == len(ch) and done == want)
        except struct.error:
            ok = False
        if not ok:
            bad += 1
            if bad < 4:
                print("BAD chunk", hp, c, "o", o, "len", len(ch), "done", done, "want", want)
print("chunks", tot, "bad", bad, "levels", levels_seen)
