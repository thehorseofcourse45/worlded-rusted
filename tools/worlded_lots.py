"""Run WorldEd's lot generation on a project made by make_real_tmx.py (after its tmx was edited).

    python tools/worlded_lots.py <project folder> [--exe PATH]

Points the .pzw at the project's own tmx, lots folder and a blank zombie spawn map, runs
`PZWorldEd_cli --generate-map ... --cells=...`, and exits 0 only if WorldEd exited 0 and
lot files appeared. Never pass --help to the exe: it opens the GUI.
"""
import glob
import os
import re
import subprocess
import sys

from PIL import Image

DEFAULT_EXE = r"C:\KnoxMap\vendor\PZMappingTools\bin\PZWorldEd_cli.exe"


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    exe = argv[argv.index("--exe") + 1] if "--exe" in argv else DEFAULT_EXE
    args = [a for a in args if a != exe]
    if len(args) != 1:
        sys.exit(__doc__)
    project = os.path.abspath(args[0]).replace("\\", "/")
    pzw = project + "/rt.pzw"
    text = open(pzw, encoding="utf-8").read()
    n = int(re.search(r'<world version="1.0" width="(\d+)"', text).group(1))
    spawn = project + "/rt_ZombieSpawnMap.bmp"
    Image.new("RGB", (30 * n, 30 * n), (0, 0, 0)).save(spawn)
    text = re.sub(r'<ZombieSpawnMap path="[^"]*"/>', f'<ZombieSpawnMap path="{spawn}"/>', text)
    text = re.sub(r'<exportdir path="[^"]*"/>', f'<exportdir path="{project}/lots"/>', text)
    for t in sorted(glob.glob(project + "/tmx/*.tmx")):
        # WorldEd names a cell's map <name>_<origin x + x>_<origin y + y>.tmx; the origin is 70,0
        found = re.search(r"_(\d+)_(\d+)\.tmx$", t)
        if not found:
            continue
        x, y = int(found.group(1)) - 70, int(found.group(2))
        text = re.sub(rf'(<cell x="{x}" y="{y}" map=")[^"]*"', lambda m: m.group(1) + t.replace("\\", "/") + '"', text, count=1)
    open(pzw, "w", encoding="utf-8", newline="\n").write(text)
    for old in glob.glob(project + "/lots/*"):
        os.remove(old)
    os.makedirs(project + "/lots", exist_ok=True)
    r = subprocess.run([exe, f"--generate-map={pzw}", f"--cells=0,0,{n - 1},{n - 1}"], timeout=900, capture_output=True, text=True)
    lots = glob.glob(project + "/lots/*.lotheader")
    print((r.stdout + r.stderr)[-600:])
    print(f"exit {r.returncode}; {len(lots)} lotheader file(s)")
    return 0 if r.returncode == 0 and lots else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
