"""worlded: compile a map folder into lot files, no WorldEd or KnoxMap needed.

    worlded <map folder> [<out folder>] [--seed N] [--worlded-seams]

The map folder holds <name>.bmp, <name>_veg.bmp, <name>.pzw and buildings/*.tbx.
Writes the .lotheader / .lotpack / chunkdata files into <out folder> (default <map folder>/lots).
Double-click to be asked for the folder, or drag a map folder onto worlded.exe.
"""
import multiprocessing
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from tools.map_source import make_map_source  # noqa: E402


def exe_dir() -> str:
    return os.path.dirname(sys.executable) if getattr(sys, "frozen", False) else \
        os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "rust", "knoxlots", "target", "release")


def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    seed = 1
    if "--seed" in argv:
        seed = int(argv[argv.index("--seed") + 1])
        args.remove(argv[argv.index("--seed") + 1])
    if not args:  # double-clicked: ask
        print(__doc__)
        args = [input("Map folder (or drag it here): ").strip().strip('"')]
    if len(args) == 1:  # one folder (typed, or dragged onto the exe): write beside it
        args.append(os.path.join(args[0], "lots"))
    if len(args) != 2:
        print(__doc__)
        return 2
    project, out = args
    with tempfile.TemporaryDirectory() as src:
        n = make_map_source(project, src, seed, seams="--worlded-seams" in argv)
        print(f"{n} cells built, compiling ...")
        rc = subprocess.call([os.path.join(exe_dir(), "knoxlots.exe"), "compile", src, out])
    print(f"Done, lot files are in {out}" if rc == 0 else f"Failed (code {rc})")
    return rc


if __name__ == "__main__":
    multiprocessing.freeze_support()
    try:
        rc = main(sys.argv[1:])
    except Exception as e:  # keep the window open so the message can be read
        print(f"Error: {e}")
        rc = 1
    if getattr(sys, "frozen", False):
        input("Press Enter to close ...")
    sys.exit(rc)
