"""Tiles for a building, from its .tbx (BuildingEd's building file).

What each part of a building becomes was worked out from WorldEd's own output for
small hand-made buildings (see the notes at each rule). Unfinished: stairs, furniture and
lights are not done.

A .tbx holds:
  tile_entry (many)   a numbered list (from 1) of tile sets; each maps names like West,
                      North, NorthWest, SouthEast to a tile; category says what it is for
  room (many)         numbered from 1: which tile entry is its floor and its interior walls
  floor (one a level) a grid of room numbers (0 for outside) and the objects on the level
The building itself names its exterior wall, trim and grime entries.
"""
from __future__ import annotations

import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field

from .buildroofs import upper_level


@dataclass
class Floor:
    rooms: list[list[int]]                  # [y][x]; 0 = outside
    objects: list[dict] = field(default_factory=list)


@dataclass
class Building:
    width: int
    height: int
    attrs: dict
    entries: list[dict]                     # 1-based: entries[n - 1], {enum: tile name}
    rooms: list[dict]                       # 1-based: rooms[n - 1]
    floors: list[Floor]
    # 0-based: furniture[n], {"layer": name or "", "orients": {orient: [(dx, dy, tile name)]}}
    furniture: list[dict] = field(default_factory=list)


def tile_name(name: str) -> str:
    """BuildingEd writes numbers padded to three digits, the lot files do not:
    walls_exterior_house_02_064 is walls_exterior_house_02_64 there."""
    m = re.match(r"^(.*_)(\d+)$", name)
    return f"{m.group(1)}{int(m.group(2))}" if m else name


def parse_tbx(text: str) -> Building:
    root = ET.fromstring(text)
    entries = [{t.get("enum"): tile_name(t.get("tile")) for t in e} for e in root if e.tag == "tile_entry"]
    rooms = [dict(r.attrib) for r in root if r.tag == "room"]
    w, h = int(root.get("width")), int(root.get("height"))
    floors = []
    for f in root:
        if f.tag != "floor":
            continue
        grid_el = f.find("rooms")
        values = [int(v) for v in re.split(r"[,\s]+", (grid_el.text or "").strip()) if v] if grid_el is not None else []
        if len(values) != w * h:
            raise ValueError(f"a floor has {len(values)} room numbers, expected {w * h}")
        grid = [values[y * w:(y + 1) * w] for y in range(h)]
        floors.append(Floor(grid, [dict(o.attrib) for o in f if o.tag == "object"]))
    furniture = [{"layer": f.get("layer", ""),
                  "orients": {e.get("orient"): [(int(t.get("x")), int(t.get("y")), tile_name(t.get("name")))
                                                for t in e if t.tag == "tile"]
                              for e in f if e.tag == "entry"}}
                 for f in root if f.tag == "furniture"]
    return Building(w, h, dict(root.attrib), entries, rooms, floors, furniture)


_WINDOW_FAMILIES = ("white", "metal", "wood")


def _window_variant(window_tile: str) -> int:
    """Which numbered Window variant of a wall holds this window: the wall art has a window
    cut for each sprite pair, numbered from 1 (white_16 and white_17 are variant 9, 10 and 11
    are 6, 24 and 25 are 13). Other window families sit in the plain window wall (0)."""
    m = re.match(r"^fixtures_windows_(?:%s)_(\d+)$" % "|".join(_WINDOW_FAMILIES), window_tile)
    return int(m.group(1)) // 2 + 1 if m else 0


def _stair_squares(o: dict) -> list[tuple[int, int]]:
    """The three squares a flight of stairs stands on, from the lowest step's end: a
    west-facing flight starts one square east of its own and runs east, a north-facing one
    starts one square south and runs south."""
    x, y = int(o["x"]), int(o["y"])
    if o.get("dir") == "N":
        return [(x, y + 1 + i) for i in range(3)]
    return [(x + 1 + i, y) for i in range(3)]


def _entry(b: Building, number) -> dict:
    n = int(number)
    return b.entries[n - 1] if 1 <= n <= len(b.entries) else {}


def level_tiles(b: Building, z: int) -> dict[tuple[int, int], list[str]]:
    """{(x, y): tiles, bottom first} for level z, over the building and the ring of
    squares outside its south and east sides, where its exterior walls stand.

    Rules (each seen in WorldEd's output):
    - A square with a room has that room's floor tile.
    - Walls stand on the north and west edge of a square, and exist where the room
      number on the other side of the edge differs. A square with a room uses that
      room's interior wall tile (and its trim, if the room has one), an empty square the building's exterior wall tile, with
      its trim and grime tiles on top (on the ground level only).
    - A square with both a north and a west wall has the NorthWest tile instead of the two.
    - An exterior post (SouthEast) stands on an empty square with no wall of its own
      where a wall ends on the square to its north and another on the square to its west.
    - A door on an edge makes the wall its Door variant and adds the door frame and door.
    - A window makes the wall the Window variant cut for its sprite (see _window_variant; an
      exterior wall has no trim or grime then) and adds the window. Curtains stand on the inside square; shutters
      only on exterior walls, two on the window's square and one on each side of it.
    """
    floor = b.floors[z]
    w, h = b.width, b.height

    def room(x: int, y: int) -> int:
        return floor.rooms[y][x] if 0 <= x < w and 0 <= y < h else 0

    ext = _entry(b, b.attrs.get("ExteriorWall", 0))
    trim = _entry(b, b.attrs.get("ExteriorWallTrim", 0))
    grime = _entry(b, b.attrs.get("GrimeWall", 0))
    ground = z == 0

    def west(x, y):
        return room(x - 1, y) != room(x, y)

    def north(x, y):
        return room(x, y - 1) != room(x, y)

    floors: dict[tuple[int, int], list[str]] = {}
    walls: dict[tuple[int, int], list[str]] = {}
    extra: dict[tuple[int, int], list[str]] = {}
    kind_at: dict[tuple[int, int], str] = {}

    def wall_tiles(x: int, y: int, kind: str, fallback: str = "") -> list[str]:
        r = room(x, y)
        if r > 0:
            room_def = b.rooms[r - 1]
            e = _entry(b, room_def["InteriorWall"])
            tiles = [e.get(kind) or e.get(fallback, "")]
            if int(room_def.get("InteriorWallTrim", 0)):
                tiles.append(_entry(b, room_def["InteriorWallTrim"]).get(kind if e.get(kind) else fallback, ""))
            return tiles
        return [ext.get(kind, "")] + ([trim.get(kind, ""), grime.get(kind, "")] if ground else [])

    # Stairs: three stair tiles on this level, and a hole (no floor tile) over them on the
    # level above. A west-facing flight runs east from its square, a north-facing one south.
    holes: set[tuple[int, int]] = set()
    if z >= 1:
        for o in b.floors[z - 1].objects:
            if o.get("type") == "stairs":
                holes.update(_stair_squares(o))

    for y in range(0, h + 1):
        for x in range(0, w + 1):
            r = room(x, y)
            if r > 0 and (x, y) not in holes:
                floors[(x, y)] = [_entry(b, b.rooms[r - 1]["Floor"]).get("Floor", "")]
            wl, nl = west(x, y), north(x, y)
            kind = "NorthWest" if wl and nl else ("West" if wl else ("North" if nl else ""))
            if kind:
                kind_at[(x, y)] = kind
                walls[(x, y)] = wall_tiles(x, y, kind)
            elif west(x, y - 1) and north(x - 1, y):
                walls[(x, y)] = wall_tiles(x, y, "SouthEast")

    def add(x, y, tile):
        extra.setdefault((x, y), []).append(tile)

    curtains: dict[tuple[int, int], list[str]] = {}

    def add_curtain(x, y, tile):
        curtains.setdefault((x, y), []).append(tile)

    for o in floor.objects:
        if o.get("type") == "stairs":
            steps = _entry(b, o.get("Tile", b.attrs.get("Stairs", 0)))
            name = "North" if o.get("dir") == "N" else "West"
            for i, pos in enumerate(_stair_squares(o)):
                add(pos[0], pos[1], steps.get(f"{name}{3 - i}", ""))

    for o in floor.objects:
        kind = o.get("type")
        if kind not in ("door", "window"):
            continue
        x, y, d = int(o["x"]), int(o["y"]), o.get("dir", "N")
        dname = "North" if d == "N" else "West"
        if (x, y) not in kind_at and kind_at.get((x, y)) is None:
            continue                                   # nothing to put it in
        r = room(x, y)
        if kind == "door":
            walls[(x, y)] = wall_tiles(x, y, dname + "Door")
            add(x, y, _entry(b, o.get("FrameTile", b.attrs.get("DoorFrame", 0))).get(dname, ""))
            add(x, y, _entry(b, o.get("Tile", b.attrs.get("Door", 0))).get(dname, ""))
            continue
        win = _entry(b, o.get("Tile", b.attrs.get("Window", 0))).get(dname, "")
        v = _window_variant(win)
        variant = f"{dname}Window{v}" if v else dname + "Window"
        if r > 0:
            walls[(x, y)] = wall_tiles(x, y, variant, fallback=dname + "Window")
        else:
            kind = variant if ext.get(variant) else dname + "Window"
            walls[(x, y)] = [ext.get(kind, "")] + ([trim.get(kind, ""), grime.get(kind, "")] if ground else [])
        if r == 0:
            sh = _entry(b, o.get("ShuttersTile", 0))
            if sh:
                first, second = ("Left", "Right") if d == "N" else ("Above", "Below")
                names = (("NorthLeft", "NorthRight") if d == "N" else ("WestAbove", "WestBelow"))
                add(x, y, sh.get(names[0], ""))
                add(x, y, sh.get(names[1], ""))
        add(x, y, _entry(b, o.get("Tile", b.attrs.get("Window", 0))).get(dname, ""))
        cur = _entry(b, o.get("CurtainsTile", 0))
        if cur:
            if r > 0:
                add_curtain(x, y, cur.get(dname, ""))
            else:                                       # the inside square is the one before
                ix, iy = (x, y - 1) if d == "N" else (x - 1, y)
                add_curtain(ix, iy, cur.get("South" if d == "N" else "East", ""))
        if r == 0 and _entry(b, o.get("ShuttersTile", 0)):
            sh = _entry(b, o.get("ShuttersTile", 0))
            if d == "N":
                add(x - 1, y, sh.get("NorthLeft", ""))
                add(x + 1, y, sh.get("NorthRight", ""))
            else:
                add(x, y - 1, sh.get("WestAbove", ""))
                add(x, y + 1, sh.get("WestBelow", ""))

    # Furniture goes onto the square in the order of BuildingEd's layers: floor furniture
    # under the walls; wall furniture facing north or west before ordinary furniture, facing
    # east or south after it; roof furniture last. Within a layer, in file order.
    furn_floor: dict[tuple[int, int], list[str]] = {}
    furn: dict[tuple[int, int], list[tuple[int, str]]] = {}
    for o in floor.objects:
        if o.get("type") != "furniture":
            continue
        idx = int(o.get("FurnitureTiles", -1))
        if not 0 <= idx < len(b.furniture):
            continue
        d = b.furniture[idx]
        orient = o.get("orient", "N")
        rank = {"FloorFurniture": -1, "Roof": 3}.get(d["layer"], 0)
        if d["layer"] == "WallFurniture":
            rank = 0 if orient in ("N", "W") else 2
        elif rank == 0:
            rank = 1
        for k, (dx, dy, name) in enumerate(d["orients"].get(orient, [])):
            pos = (int(o["x"]) + dx, int(o["y"]) + dy)
            # the other tiles of an ordinary multi-tile piece stand above the curtains
            r = 1.5 if rank == 1 and k > 0 else rank
            if r < 0:
                furn_floor.setdefault(pos, []).append(name)
            else:
                furn.setdefault(pos, []).append((r, name))
    furn_before = {pos: [n for r, n in items if r == 0] for pos, items in furn.items()}     # wall, north/west
    furn_mid = {pos: [n for r, n in items if r == 1] for pos, items in furn.items()}        # ordinary
    furn_after = {pos: [n for r, n in sorted((i for i in items if i[0] > 1), key=lambda i: i[0])]
                  for pos, items in furn.items()}       # more tiles of a piece, wall east/south, roof

    upper = upper_level(lambda n: _entry(b, n), b.attrs, w, h, b.rooms, b.floors, z)
    out: dict[tuple[int, int], list[str]] = {}
    # bottom to top: floor, rugs, walls, wall furniture (north/west), door frames, doors,
    # windows, ordinary furniture, curtains, wall furniture (east/south), roof furniture
    for pos in set(floors) | set(furn_floor) | set(walls) | set(extra) | set(furn_before) | set(furn_mid)             | set(furn_after) | set(curtains) | set(upper):
        stack = [t for t in floors.get(pos, []) + furn_floor.get(pos, []) + walls.get(pos, [])
                 + furn_before.get(pos, []) + extra.get(pos, []) + furn_mid.get(pos, [])
                 + curtains.get(pos, []) + furn_after.get(pos, []) + upper.get(pos, []) if t]
        if stack:
            out[pos] = stack
    return out
