"""Run the Python oracle for the Rust tests: python tools/dump_for_rust.py <what> <args...>.

Writes raw bytes (bmp) or JSON to stdout. Rust tests compare their own output with this.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main(argv):
    what = argv[0]
    if what == "bmp":  # bmp <file> <top> <bottom> <width>  -> raw RGB bytes
        from knoxbuild import bitmaps
        a = bitmaps.read_rgb_rows(argv[1], int(argv[2]), int(argv[3]), int(argv[4]))
        sys.stdout.buffer.write(a.tobytes())
        return 0
    print("unknown dump", what, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
