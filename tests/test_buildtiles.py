import unittest

from knoxbuild.buildtiles import level_tiles, parse_tbx, tile_name


def entry(category, **tiles):
    body = "".join(f'<tile enum="{k}" tile="{v}"/>' for k, v in tiles.items())
    return f'<tile_entry category="{category}">{body}</tile_entry>'


def tbx(width, height, floors, rooms=2, extra_attrs=""):
    """Entries, 1-based: 1 exterior wall, 2 trim, 3 grime, 4 door, 5 frame, 6 window, 7 curtains,
    8 shutters, 9 floor A, 10 interior wall A, 11 floor B, 12 interior wall B, 13 interior trim."""
    entries = [
        entry("exterior_walls", West="ew_000", North="ew_001", NorthWest="ew_002", SouthEast="ew_003",
              WestDoor="ew_010", NorthDoor="ew_011", WestWindow9="ew_020", NorthWindow9="ew_021"),
        entry("exterior_wall_trim", West="tr_000", North="tr_001", NorthWest="tr_002", SouthEast="tr_003",
              WestDoor="tr_010", NorthDoor="tr_011"),
        entry("grime_wall", West="gr_000", North="gr_001", NorthWest="gr_002", SouthEast="gr_003",
              WestDoor="gr_010", NorthDoor="gr_011"),
        entry("doors", West="door_000", North="door_001"),
        entry("door_frames", West="frame_000", North="frame_001"),
        entry("windows", West="win_000", North="win_001"),
        entry("curtains", West="cur_000", East="cur_001", North="cur_002", South="cur_003"),
        entry("shutters", WestBelow="sh_000", WestAbove="sh_001", NorthLeft="sh_002", NorthRight="sh_003"),
        entry("floors", Floor="floorA_000"),
        entry("interior_walls", West="iwA_000", North="iwA_001", NorthWest="iwA_002", WestDoor="iwA_010",
              NorthDoor="iwA_011", WestWindow="iwA_020", NorthWindow="iwA_021"),
        entry("floors", Floor="floorB_000"),
        entry("interior_walls", West="iwB_000", North="iwB_001", NorthWest="iwB_002"),
        entry("interior_wall_trim", West="it_000", North="it_001", NorthWest="it_002", NorthWindow="it_021"),
    ]
    room_defs = (
        '<room Name="a" InteriorWall="10" InteriorWallTrim="0" Floor="9" GrimeFloor="0" GrimeWall="0" Ceiling="0"/>'
        '<room Name="b" InteriorWall="12" InteriorWallTrim="13" Floor="11" GrimeFloor="0" GrimeWall="0" Ceiling="0"/>')
    fl = ""
    for rooms_grid, objects in floors:
        rows = ",\n".join(",".join(str(v) for v in row) for row in rooms_grid)
        objs = "".join("<object " + " ".join(f'{k}="{v}"' for k, v in o.items()) + "/>" for o in objects)
        fl += f"<floor>{objs}<rooms>\n{rows}\n</rooms></floor>"
    return (f'<building version="4" width="{width}" height="{height}" ExteriorWall="1" ExteriorWallTrim="2" '
            f'GrimeWall="3" Door="4" DoorFrame="5" Window="6" {extra_attrs}>' + "".join(entries) + room_defs + fl
            + "</building>")


def grid(w, h, fill=1, **cells):
    g = [[fill] * w for _ in range(h)]
    for (x, y), v in cells.items():
        g[y][x] = v
    return g


class BuildTilesTests(unittest.TestCase):
    def square(self, text, x, y, z=0):
        return level_tiles(parse_tbx(text), z).get((x, y), [])

    def test_names_lose_their_padding(self):
        self.assertEqual(tile_name("walls_exterior_house_02_064"), "walls_exterior_house_02_64")
        self.assertEqual(tile_name("floors_interior_carpet_01_000"), "floors_interior_carpet_01_0")
        self.assertEqual(tile_name("nothing"), "nothing")

    def test_a_room_has_floor_and_interior_walls_on_its_north_and_west_edges(self):
        t = tbx(4, 3, [(grid(4, 3), [])])
        self.assertEqual(self.square(t, 0, 0), ["floorA_0", "iwA_2"])              # both: the corner tile
        self.assertEqual(self.square(t, 2, 0), ["floorA_0", "iwA_1"])
        self.assertEqual(self.square(t, 0, 2), ["floorA_0", "iwA_0"])
        self.assertEqual(self.square(t, 2, 2), ["floorA_0"])

    def test_exterior_walls_stand_on_the_squares_outside_the_south_and_east(self):
        t = tbx(4, 3, [(grid(4, 3), [])])
        self.assertEqual(self.square(t, 1, 3), ["ew_1", "tr_1", "gr_1"])           # south side
        self.assertEqual(self.square(t, 4, 1), ["ew_0", "tr_0", "gr_0"])           # east side
        self.assertEqual(self.square(t, 4, 3), ["ew_3", "tr_3", "gr_3"])           # the post at the corner
        self.assertEqual(self.square(t, 5, 5), [])

    def test_upper_levels_have_no_trim_or_grime_outside(self):
        t = tbx(4, 3, [(grid(4, 3), []), (grid(4, 3), [])])
        self.assertEqual(self.square(t, 1, 3, z=1), ["ew_1"])
        self.assertEqual(self.square(t, 1, 3, z=0), ["ew_1", "tr_1", "gr_1"])

    def test_a_wall_between_two_rooms_belongs_to_the_room_on_its_south_or_east(self):
        g = [[1, 1, 2, 2], [1, 1, 2, 2], [1, 1, 2, 2]]
        t = tbx(4, 3, [(g, [])])
        self.assertEqual(self.square(t, 2, 1), ["floorB_0", "iwB_0", "it_0"])      # room b has trim
        self.assertEqual(self.square(t, 1, 1), ["floorA_0"])
        self.assertEqual(self.square(t, 2, 0), ["floorB_0", "iwB_2", "it_2"])

    def test_a_door_swaps_the_wall_and_adds_frame_and_door(self):
        t = tbx(4, 3, [(grid(4, 3), [dict(type="door", x=2, y=0, dir="N")])])
        self.assertEqual(self.square(t, 2, 0), ["floorA_0", "iwA_11", "frame_1", "door_1"])
        t = tbx(4, 3, [(grid(4, 3), [dict(type="door", x=1, y=3, dir="N")])])      # exterior
        self.assertEqual(self.square(t, 1, 3), ["ew_11", "tr_11", "gr_11", "frame_1", "door_1"])

    def test_a_window_with_curtains_and_shutters(self):
        obj = dict(type="window", x=2, y=3, dir="N", CurtainsTile=7, ShuttersTile=8)
        t = tbx(5, 3, [(grid(5, 3), [obj])])
        self.assertEqual(self.square(t, 2, 3), ["ew_21", "sh_2", "sh_3", "win_1"])
        self.assertEqual(self.square(t, 1, 3), ["ew_1", "tr_1", "gr_1", "sh_2"])
        self.assertEqual(self.square(t, 3, 3), ["ew_1", "tr_1", "gr_1", "sh_3"])
        self.assertEqual(self.square(t, 2, 2), ["floorA_0", "cur_3"])               # on the inside
        inside = dict(type="window", x=2, y=0, dir="N", CurtainsTile=7)
        t = tbx(5, 3, [(grid(5, 3), [inside])])
        self.assertEqual(self.square(t, 2, 0), ["floorA_0", "iwA_21", "win_1", "cur_2"])

    def test_a_floor_with_the_wrong_number_of_rooms_is_refused(self):
        with self.assertRaises(ValueError):
            parse_tbx(tbx(4, 3, [(grid(3, 3), [])]))


if __name__ == "__main__":
    unittest.main()
