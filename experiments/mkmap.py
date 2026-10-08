"""A real KnoxMap map from the cached downloads, compiled by WorldEd, as a test set for buildings.

    mkmap.py <name> [size in metres, default 500]
"""
import contextlib
import glob
import io
import os
import shutil
import sys

sys.path.insert(0, os.environ.get("KNOXMAP_DIR", "."))
from generator import osm, renderer
from knoxbuild.build import build
from knoxbuild.settings import Settings
from tools import compile_map as cm

H = os.path.dirname(os.path.abspath(__file__))
name = sys.argv[1]
size_m = float(sys.argv[2]) if len(sys.argv) > 2 else 500.0

import gzip
import json
from generator.osm import OSMFeature


def read_tile(path):
    with gzip.open(path, "rt", encoding="utf-8") as fh:
        payload = json.load(fh)
    return [OSMFeature(d["osm_id"], d["kind"], d.get("tags") or {}, [tuple(c) for c in d.get("geometry") or []],
                       [(role, [tuple(c) for c in ring]) for role, ring in d.get("role_geoms") or []])
            for d in payload.get("features", [])]


best = None
for f in glob.glob(os.environ.get("KNOXMAP_DIR", ".") + "/cache/overpass_tiles/*.json.gz"):
    try:
        feats = read_tile(f)
    except Exception:
        continue
    if not feats:
        continue
    n = sum(1 for x in feats if "building" in x.tags)
    if best is None or n > best[0]:
        best = (n, f, feats)
n, f, feats = best
print("densest cached tile", os.path.basename(f), n, "buildings of", len(feats), "features")

# a square of size_m around the centre of mass of the buildings
def coords(x):
    g = x.geometry
    if g and isinstance(g[0][0], (list, tuple)):  # a relation: rings of points
        return [tuple(p) for ring in g for p in ring]
    return [tuple(p) for p in g]


pts = [p for x in feats if "building" in x.tags for p in coords(x)]
lat = sorted(p[0] for p in pts)[len(pts) // 2]
lon = sorted(p[1] for p in pts)[len(pts) // 2]
dlat = size_m / 111320 / 2
import math
dlon = size_m / (111320 * math.cos(math.radians(lat))) / 2
S, W, N, E = lat - dlat, lon - dlon, lat + dlat, lon + dlon
inside = [x for x in feats if any(S <= p[0] <= N and W <= p[1] <= E for p in coords(x))]
print("area", round(S, 5), round(W, 5), round(N, 5), round(E, 5), "->", len(inside), "features,",
      sum(1 for x in inside if "building" in x.tags), "buildings")
out = f"{H}/real/{name}"
shutil.rmtree(out, ignore_errors=True)
os.makedirs(out)
renderer.render(inside, S, W, N, E, meters_per_tile=1.0, output_dir=out, map_name=name)
os.makedirs(f"{out}/buildings", exist_ok=True)
for g in glob.glob(f"{out}/*.geojson"):
    if os.path.getsize(g) == 0:                  # the render step writes some of these later
        open(g, "w").write('{"type": "FeatureCollection", "features": []}')
with contextlib.redirect_stdout(io.StringIO()):
    build(out, settings=Settings(seed=1, true_map=1))
print("built; compiling with WorldEd ...")
with contextlib.redirect_stdout(io.StringIO()):
    cm.compile_map(out, batch=4, incremental=False)
print("done", len(os.listdir(f"{out}/lots")), "files in lots,", len(os.listdir(f"{out}/buildings")), "building files")
