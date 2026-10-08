"""Which squares does WorldEd give a blend tile?  Test hypotheses against its own output.

    blendtest.py <variant> [...]
"""
import collections
import os
import re
import struct
import sys

import numpy as np

sys.path.insert(0, os.environ.get("KNOXMAP_DIR", "."))
from knoxbuild.rules import load_rules

H = os.path.dirname(os.path.abspath(__file__))
CFG = os.environ.get("KNOXMAP_DIR", ".") + "/vendor/PZMappingTools/config"
NONE = 0xFFFFFFFF
ORIGIN = 21000
rs = load_rules(f"{CFG}/Rules.txt")


def parse_blends(path):
    out = []
    cur = None
    for line in open(path, encoding="utf-8"):
        s = line.strip()
        if s == "blend":
            cur = {}
        elif s == "}" and cur is not None:
            out.append(cur)
            cur = None
        elif cur is not None and "=" in s:
            k, v = s.split("=", 1)
            cur[k.strip()] = v.strip()
    return out


def expand(name):
    return set(rs.aliases.get(name, [name]))


blends = parse_blends(f"{CFG}/Blends.txt")
print(len(blends), "blend rules; layers", collections.Counter(b["layer"] for b in blends))


def decode(lots, w, h):
    """floor tile name array (h, w) of object dtype and {(x,y): stack} for stacks of 2+."""
    floor = np.full((h, w), "", dtype=object)
    multi = {}
    for f in os.listdir(lots):
        if not f.endswith(".lotheader"):
            continue
        cx, cy = (int(t) for t in f[:-10].split("_"))
        head = open(f"{lots}/{f}", "rb").read()
        n = struct.unpack_from("<I", head, 8)[0]
        o = 12
        names = []
        for _ in range(n):
            e = head.index(b"\n", o)
            names.append(head[o:e].decode())
            o = e + 1
        d = open(f"{lots}/world_{cx}_{cy}.lotpack", "rb").read()
        offs = [struct.unpack_from("<Q", d, 12 + 8 * i)[0] for i in range(1024)] + [len(d)]
        for i in range(1024):
            ch = d[offs[i]:offs[i + 1]]
            o = k = 0
            while o < len(ch) and k < 64:
                m = struct.unpack_from("<I", ch, o)[0]
                o += 4
                if m == NONE:
                    k += struct.unpack_from("<I", ch, o)[0]
                    o += 4
                else:
                    stack = [names[t] for t in struct.unpack_from(f"<{m}I", ch, o)[1:]]
                    o += 4 * m
                    wx = cx * 256 + i // 32 * 8 + k // 8 - ORIGIN
                    wy = cy * 256 + i % 32 * 8 + k % 8
                    if 0 <= wx < w and 0 <= wy < h and stack:
                        floor[wy, wx] = stack[0]
                        if len(stack) > 1:
                            multi[(wx, wy)] = stack
                    k += 1
    return floor, multi


DIRS = {"n": (0, -1), "s": (0, 1), "e": (1, 0), "w": (-1, 0), "ne": (1, -1), "nw": (-1, -1), "se": (1, 1), "sw": (-1, 1)}


def shifted(mask, dx, dy):
    """mask value of the neighbour at (+dx,+dy), for every square."""
    out = np.zeros_like(mask)
    h, w = mask.shape
    ys, yd = (slice(0, h - dy), slice(dy, h)) if dy >= 0 else (slice(-dy, h), slice(0, h + dy))
    xs, xd = (slice(0, w - dx), slice(dx, w)) if dx >= 0 else (slice(-dx, w), slice(0, w + dx))
    out[ys, xs] = mask[yd, xd]
    return out


for v in sys.argv[1:]:
    h = w = 900
    floor, multi = decode(f"{H}/exp/{v}/lots", w, h)
    print(f"== {v}: squares with 2+ tiles {len(multi)}")
    results = []
    for b in blends:
        main, blend, ex = expand(b["mainTile"]), expand(b["blendTile"]), set()
        for e in b["exclude"].split():
            ex |= expand(e)
        is_main = np.isin(floor, list(main))
        excl = np.isin(floor, list(ex)) if ex else np.zeros_like(is_main)
        has = np.zeros_like(is_main)
        for (x, y), st in multi.items():
            if any(t in blend for t in st[1:]):
                has[y, x] = True
        dx, dy = DIRS[b["dir"]]
        for label, sgn in (("neighbour in dir", 1), ("neighbour opposite", -1)):
            pred = ~is_main & ~excl & (floor != "") & shifted(is_main, sgn * dx, sgn * dy)
            results.append((b["layer"], b["mainTile"], b["blendTile"], b["dir"], label,
                            int((pred & has).sum()), int(pred.sum()), int(has.sum())))
    # best orientation per rule, only rules that have anything to say
    rows = [r for r in results if r[6] or r[7]]
    print("   rules with predictions or observations:", len({(r[1], r[2], r[3]) for r in rows}))
    tot = collections.defaultdict(lambda: [0, 0, 0])
    for r in rows:
        for i in range(3):
            tot[r[4]][i] += r[5 + i]
    for label, (hit, pred, obs) in tot.items():
        print(f"   {label}: predicted {pred}, observed {obs}, both {hit}")
    for r in sorted(rows, key=lambda r: -r[7])[:6]:
        print("  ", r)


# ---- per layer: which rule applies when several match? -------------------------------
def per_layer(v, semantics):
    h = w = 900
    floor, multi = decode(f"{H}/exp/{v}/lots", w, h)
    layers = sorted({b["layer"] for b in blends})
    tile_layers = collections.defaultdict(set)
    for b in blends:
        for t in expand(b["blendTile"]):
            tile_layers[t].add(b["layer"])
    res = {}
    for L in layers:
        rules_L = [b for b in blends if b["layer"] == L]
        pred_idx = np.full((h, w), -1)            # index of the rule that applies, -1 none
        order = range(len(rules_L)) if semantics == "first" else range(len(rules_L) - 1, -1, -1)
        for i in order:
            b = rules_L[i]
            main = expand(b["mainTile"])
            ex = set()
            for e in b["exclude"].split():
                ex |= expand(e)
            dx, dy = DIRS[b["dir"]]
            cond = ~np.isin(floor, list(main | ex)) & (floor != "") & shifted(np.isin(floor, list(main)), dx, dy)
            if semantics == "first":
                pred_idx = np.where((pred_idx < 0) & cond, i, pred_idx)
            else:
                pred_idx = np.where(cond, i, pred_idx)
        ok = fp = fn = 0
        obs_mask = np.zeros((h, w), bool)
        for (x, y), st in multi.items():
            ts = [t for t in st[1:] if L in tile_layers.get(t, ())]
            if ts:
                obs_mask[y, x] = True
                p = pred_idx[y, x]
                if p >= 0 and ts[0] in expand(rules_L[p]["blendTile"]):
                    ok += 1
                else:
                    fn += 1
        fp = int(((pred_idx >= 0) & ~obs_mask).sum())
        res[L] = (ok, fn, fp)
    return res


for v in sys.argv[1:]:
    for sem in ("first", "last"):
        r = per_layer(v, sem)
        print(f"{v} {sem}-match-wins:", {k: f"ok {a} missed {b} extra {c}" for k, (a, b, c) in r.items()})
