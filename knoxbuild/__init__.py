"""Own modules first; bitmaps etc. come from a KnoxMap checkout (KNOXMAP_DIR, ../knoxmap, or C:/KnoxMap)."""
import os
import sys
from pathlib import Path

_here = Path(__file__).resolve().parent
for _c in (os.environ.get("KNOXMAP_DIR"), _here.parent / "knoxmap", "C:/KnoxMap"):
    if _c and (Path(_c) / "knoxbuild" / "bitmaps.py").exists():
        __path__.append(str(Path(_c) / "knoxbuild"))  # own modules win; the rest fall through
        if str(_c) not in sys.path:
            sys.path.append(str(_c))
        break
