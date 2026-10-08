"""Hypothesis parser for .lotheader; checks that every file parses to its exact end."""
import glob
import struct
import sys


class R:
    def __init__(self, d):
        self.d, self.o = d, 0

    def u32(self):
        v = struct.unpack_from("<I", self.d, self.o)[0]
        self.o += 4
        return v

    def line(self):
        e = self.d.index(b"\n", self.o)
        s = self.d[self.o:e].decode()
        self.o = e + 1
        return s


def parse(d):
    r = R(d)
    assert d[:4] == b"LOTH"
    r.o = 4
    h = {"version": r.u32()}
    n = r.u32()
    h["tiles"] = [r.line() for _ in range(n)]
    h["a"], h["b"] = r.u32(), r.u32()
    h["minz"], h["maxz"] = r.u32(), r.u32()
    nr = r.u32()
    rooms = []
    for _ in range(nr):
        name = r.line()
        z = r.u32()
        nrect = r.u32()
        rects = [(r.u32(), r.u32(), r.u32(), r.u32()) for _ in range(nrect)]
        nobj = r.u32()
        objs = [(r.u32(), r.u32(), r.u32()) for _ in range(nobj)]
        rooms.append((name, z, rects, objs))
    h["rooms"] = rooms
    nb = r.u32()
    bl = []
    for _ in range(nb):
        k = r.u32()
        bl.append([r.u32() for _ in range(k)])
    h["buildings"] = bl
    h["rest"] = len(d) - r.o
    return h


if __name__ == "__main__":
    bad = 0
    rests = {}
    for p in sorted(glob.glob(sys.argv[1] + "/*.lotheader")):
        d = open(p, "rb").read()
        try:
            h = parse(d)
        except Exception as e:  # noqa: BLE001
            bad += 1
            if bad < 4:
                print("FAIL", p, type(e).__name__, e)
            continue
        rests[h["rest"]] = rests.get(h["rest"], 0) + 1
    print("failed", bad, "rest-of-file sizes", dict(list(rests.items())[:8]))
