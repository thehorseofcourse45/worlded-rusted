"""Run the Python oracle for the Rust tests: python tools/dump_for_rust.py <what> <args...>.

Writes raw bytes (bmp) or JSON to stdout. Rust tests compare their own output with this.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def emit(lines):
    """Canonical text: lines joined with LF (never CRLF), UTF-8."""
    sys.stdout.buffer.write(("\n".join(lines) + "\n").encode("utf-8"))


def main(argv):
    what = argv[0]
    if what == "bmp":  # bmp <file> <top> <bottom> <width>  -> raw RGB bytes
        from knoxbuild import bitmaps
        a = bitmaps.read_rgb_rows(argv[1], int(argv[2]), int(argv[3]), int(argv[4]))
        sys.stdout.buffer.write(a.tobytes())
        return 0
    if what == "rules":  # rules <file> -> canonical text of aliases then rules
        from knoxbuild.rules import load_rules
        rs = load_rules(argv[1])
        out = [f"A|{n}|{','.join(e)}" for n, e in sorted(rs.aliases.items())]
        out += [f"R|{r.label}|{r.bitmap}|{r.color[0]} {r.color[1]} {r.color[2]}|{r.layer}|"
                f"{'' if r.condition is None else ' '.join(map(str, r.condition))}|{','.join(r.entries)}" for r in rs.rules]
        emit(out)
        return 0
    if what == "blends":  # blends <file> -> canonical text
        from knoxbuild.blends import load_blends
        out = [f"B|{b.layer}|{b.main}|{b.tile}|{b.dir}|{','.join(b.exclude)}|{';'.join(' '.join(p) for p in b.exclude2)}"
               for b in load_blends(argv[1])]
        emit(out)
        return 0
    if what == "mix":  # mix x,y,salt x,y,salt ... -> one u64 per line
        import numpy as np
        from knoxbuild.terrain import mix
        emit([str(int(mix(np.array([int(t.split(",")[0])]), np.array([int(t.split(",")[1])]), int(t.split(",")[2]))[0])) for t in argv[1:]])
        return 0
    if what == "stack":  # stack name... -> names in stack order
        from knoxbuild.terrain import stack_order
        emit(stack_order(argv[1:]))
        return 0
    if what == "terrain":  # terrain <rules> w h x0 y0 seed <land colours> <veg colours>
        import numpy as np
        from knoxbuild.rules import load_rules
        from knoxbuild.terrain import Palette, ground_layers
        rules = load_rules(argv[1])
        w, h, x0, y0, seed = (int(v) for v in argv[2:7])
        lc = [tuple(int(v) for v in c.split(",")) for c in argv[7].split(";")]
        vc = [tuple(int(v) for v in c.split(",")) for c in argv[8].split(";")]
        land = np.zeros((h, w, 3), np.uint8)
        veg = np.zeros((h, w, 3), np.uint8)
        for y in range(h):
            for x in range(w):
                land[y, x] = lc[(x * 7 + y * 13 + (x * y) % 5) % len(lc)]
                veg[y, x] = vc[(x * 3 + y * 5 + (x ^ y) % 7) % len(vc)]
        pal = Palette()
        out = []
        for name, arr in ground_layers(land, veg, rules, pal, seed, x0, y0).items():
            out.append("L|" + name)
            out += [" ".join(map(str, row.tolist())) for row in arr]
        out.append("P|" + "|".join(pal.names))
        emit(out)
        return 0
    if what == "blendapply":  # blendapply <rules> <blends> w h x0 y0 seed seam <floor names ; separated, - = none>
        import numpy as np
        from knoxbuild.blends import blend_layers, load_blends
        from knoxbuild.rules import load_rules
        from knoxbuild.terrain import Palette
        rules, blends = load_rules(argv[1]), load_blends(argv[2])
        w, h, x0, y0, seed, seam = (int(v) for v in argv[3:9])
        names = argv[9].split(";")
        pal = Palette()
        floor = np.zeros((h, w), np.int64)
        for y in range(h):
            for x in range(w):
                nm = names[((x // 2) * 5 + (y // 2) * 3 + ((x // 4) * (y // 3)) % 4) % len(names)]
                floor[y, x] = 0 if nm == "-" else pal.of(nm)
        out = []
        for name, arr in blend_layers(floor, rules, blends, pal, seed, x0, y0, seam or None).items():
            out.append("L|" + name)
            out += [" ".join(map(str, row.tolist())) for row in arr]
        out.append("P|" + "|".join(pal.names))
        emit(out)
        return 0
    if what == "tiles":  # tiles <tbx>... -> per file, per level: x y|layer=tile;layer=tile
        from knoxbuild.buildtiles import level_layers, parse_tbx
        out = []
        for path in argv[1:]:
            out.append("F|" + os.path.basename(path))
            try:
                b = parse_tbx(open(path, encoding="utf-8").read())
                for z in range(len(b.floors)):
                    out.append(f"Z|{z}")
                    for (x, y), items in sorted(level_layers(b, z).items()):
                        out.append(f"{x} {y}|" + ";".join(f"{l}={t}" for l, t in items))
            except Exception as e:  # noqa: BLE001
                out.append("ERR|" + type(e).__name__)
        emit(out)
        return 0
    if what == "compose":  # compose <tbx> x y base ... -> x y z|tile;tile
        from knoxbuild.buildtiles import compose_lots, parse_tbx
        lots = []
        a = argv[1:]
        for i in range(0, len(a), 4):
            lots.append((parse_tbx(open(a[i], encoding="utf-8").read()), int(a[i + 1]), int(a[i + 2]), int(a[i + 3])))
        emit([f"{x} {y} {z}|" + ";".join(t) for (x, y, z), t in sorted(compose_lots(lots).items())])
        return 0
    if what == "header":  # header <cx,cy;cx,cy> <tbx> x y base ... -> placed rooms and cell tables
        from knoxbuild.buildheader import cell_tables, placed_rooms
        from knoxbuild.buildtiles import parse_tbx
        cells = [tuple(int(v) for v in c.split(",")) for c in argv[1].split(";")]
        a = argv[2:]
        lots = [(parse_tbx(open(a[i], encoding="utf-8").read()), int(a[i + 1]), int(a[i + 2]), int(a[i + 3])) for i in range(0, len(a), 4)]
        placed = placed_rooms(lots)

        def rooms_text(rooms):
            return [f"R|{r['name']}|{r['z']}|" + ",".join(" ".join(map(str, q)) for q in r["rects"]) for r in rooms]
        out = []
        for tx, ty, entries in placed:
            out.append(f"P|{tx}|{ty}")
            out += rooms_text(entries)
        for cx, cy in cells:
            rooms, buildings = cell_tables(cx, cy, placed)
            out.append(f"C|{cx}|{cy}")
            out += rooms_text(rooms)
            out += ["B|" + ",".join(map(str, b)) for b in buildings]
        emit(out)
        return 0
    print("unknown dump", what, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
