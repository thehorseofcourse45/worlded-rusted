"""Make real WorldEd .tmx files by running the WorldEd command line on a generated map.

    python tools/make_real_tmx.py <out folder> [--exe PATH] [--size N] [--lots]

Builds a test project with make_test_map.py, runs `PZWorldEd_cli --generate-map=<pzw>
--cells=0,0,0,0` (the same call KnoxMap makes; it also converts the bitmap to tmx/*.tmx),
and reports the tmx files produced. Never pass --help to the exe: it opens the GUI.
"""
import glob
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_EXE = r"C:\KnoxMap\vendor\PZMappingTools\bin\PZWorldEd_cli.exe"


PZW = """<?xml version="1.0" encoding="UTF-8"?>
<world version="1.0" width="{n}" height="{n}">
 <BMPToTMX>
  <tmxexportdir path="{d}/tmx"/>
  <rulesfile path=""/>
  <blendsfile path=""/>
  <mapbasefile path=""/>
  <assign-maps-to-world checked="true"/>
  <warn-unknown-colors checked="true"/>
  <compress checked="true"/>
  <copy-pixels checked="true"/>
  <update-existing checked="false"/>
 </BMPToTMX>
 <TMXToBMP>
  <mainImage generate="true"/>
  <vegetationImage generate="true"/>
  <buildingsImage path="" generate="false"/>
 </TMXToBMP>
 <GenerateLots>
  <exportdir path="{d}/lots"/>
  <ZombieSpawnMap path=""/>
  <TileDefFolder path=""/>
  <worldOrigin origin="70,0"/>
  <numberOfThreads count="4"/>
 </GenerateLots>
 <LuaSettings>
  <spawnPointsFile path="spawnpoints.lua"/>
  <worldObjectsFile path="objects.lua"/>
 </LuaSettings>
  <objecttype name="ParkingStall"/>
  <objecttype name="TownZone"/>
  <objectgroup name="ParkingStall" color="#ff007f" defaulttype="ParkingStall"/>
  <objectgroup name="TownZone" color="#aa0000" defaulttype="TownZone"/>
 <bmp path="rt.bmp" x="0" y="0" width="{n}" height="{n}"/>
{cells}</world>
"""


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    exe = argv[argv.index("--exe") + 1] if "--exe" in argv else DEFAULT_EXE
    size = argv[argv.index("--size") + 1] if "--size" in argv else "300"
    args = [a for a in args if a not in (exe, size)]
    if len(args) != 1:
        sys.exit(__doc__)
    project = os.path.join(os.path.abspath(args[0]), "rt")
    subprocess.run([sys.executable, os.path.join(HERE, "make_test_map.py"), project, "--size", size], check=True)
    pzw = os.path.join(project, "rt.pzw")
    n = (int(size) + 299) // 300
    lots = {}
    if "--lots" in argv:  # keep the building placements make_test_map.py wrote (real1's lots)
        for m in re.finditer(r'<cell x="(\d+)" y="(\d+)"[^>]*>(.*?)</cell>', open(pzw, encoding="utf-8").read(), re.S):
            lots[(int(m.group(1)), int(m.group(2)))] = "".join(f"  {l}\n" for l in re.findall(r"<lot [^>]*/>", m.group(3)))
    cells = "".join(f' <cell x="{x}" y="{y}" map="">\n{lots.get((x, y), "")} </cell>\n' for y in range(n) for x in range(n))
    text = PZW.format(n=n, d=project.replace("\\", "/"), cells=cells)
    open(pzw, "w", encoding="utf-8", newline="\n").write(text)
    for d in ("tmx", "lots"):
        os.makedirs(os.path.join(project, d), exist_ok=True)
    r = subprocess.run([exe, f"--generate-map={pzw}", f"--cells=0,0,{n - 1},{n - 1}"], timeout=900, capture_output=True, text=True)
    print((r.stdout + r.stderr)[-1500:])
    tmx = glob.glob(os.path.join(project, "tmx", "*.tmx"))
    print(f"exit {r.returncode}; {len(tmx)} tmx file(s) in {os.path.join(project, 'tmx')}")
    for t in tmx:
        print(" ", t, os.path.getsize(t))
    return 0 if tmx else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
