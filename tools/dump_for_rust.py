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
    print("unknown dump", what, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
