# File formats found for the viewer

Found by reading real files (clean-room). Nothing here comes from WorldEd's source.

## `.pzw` (world project, XML)

Root `<world version="1.0" width="W" height="H">` (W x H in 300-tile cells). Children seen:
`<BMPToTMX>`, `<TMXToBMP>`, `<GenerateLots>` (settings; `<worldOrigin origin="70,0"/>` is the
world's west/north cell offset), `<bmp path width height x y>`, one `<cell x y map="...tmx">` per
cell with child `<lot x y level width height map="buildings/NAME.tbx"/>` and `<object .../>`.
`map` is an absolute path as written on the author's machine and may not exist; a cell with an
empty or missing `map` has no TMX.

## `.tmx` (one 300x300 cell map)

Checked on a real WorldEd file (`rust/worlded-core/tests/data/real_cell_a.tmx`, made by
`tools/make_real_tmx.py` from `PZWorldEd_cli --generate-map`; named `<bmp>_<originX+x>_<originY+y>.tmx`).
UTF-8, LF line endings, one space of indent per level, 2695 lines, 327 KB.

```
<map version="2.0" orientation="levelisometric" width="300" height="300" tilewidth="64" tileheight="32">
 <tileset firstgid="1" name="Fire_01" tilewidth="64" tileheight="128">   x646, in firstgid order
  [<tileoffset x="-128" y="0"/>]        only on the oversized tree tilesets (tilewidth 320/448)
  <image source="<relative path>/Fire_01.png" width="512" height="512"/>
 </tileset>
 <layer name="Floor" level="0" width="300" height="300">   x15: Floor, FloorOverlay..FloorOverlay6,
  <data encoding="base64" compression="zlib">                Curbs, Walls, Walls2, Furniture..Furniture4, Vegetation
   (base64 of zlib of 300*300 little-endian u32 gids, one unbroken line, indented 3 spaces)
  </data>
 </layer>
 <objectgroup name="Lots" level="0" width="300" height="300"/>
 <bmp-settings version="1">  rules-file, blends-file, aliases, rules, blends (WorldEd BMP-to-TMX settings)
 <bmp-image index="0" seed="1"><pixels>base64 zlib</pixels></bmp-image>   index 0 and 1
</map>
```

- Layers carry `level` (floor); there is no `visible` attribute. `<data>` is base64 + zlib in
  every layer; csv and gzip stay supported by the reader but were not seen.
- A tile's name is `<tileset name>_<gid - firstgid>`, as before. Gids are not dense: the file lists
  only the tilesets WorldEd loaded (646 here); each `firstgid` is the previous one plus that tileset's tile count (8 or 32 in the first entries).
- No `<properties>` elements. Everything after the layers (`objectgroup`, `bmp-settings`,
  `bmp-image`) is WorldEd state that an editor must keep byte for byte.
- `parse_tmx` reads this file unchanged (15 layers, 300x300; test `real_tmx.rs`).
- Only one real cell was made so far (a second one would be the same shape: every 300x300 cell
  has all 15 layers).

### What a TMX from BMP to TMX really holds (measured against WorldEd, 2026-11-15)

- **The tile layers are empty.** The ground is not in them. Each `<bmp-image index="0|1" seed="N">`
  (0 landscape, 1 vegetation) holds `<color rgb="r g b"/>` lines and `<pixels>`: base64 of gzip of
  300 x 300 little-endian `u32`, each a 1-based index into the colour list (0 = no colour). WorldEd
  generates the ground from them with the rules and blends (`<bmp-settings>` embeds copies; the file
  names `Rules.txt`/`Blends.txt`) each time it makes lot files. Read by `worlded_core::tmx::parse_bmp_images`.
- **Merge with the layers**, per square, for the layers the pictures generate (`Floor`,
  `FloorOverlay`, `FloorOverlay2` to `4`, `FloorOverlay6`, `Curbs`, `Vegetation`): the generated tile
  if there is one, else the layer's own tile. The other layers (`FloorOverlay5`, `Walls`, `Walls2`,
  `Furniture` to `Furniture4`) are the layers alone. A tile edit on `Floor` is therefore lost wherever
  the picture gives ground (everywhere, normally). Tiles of a square come in layer (file) order.
- **Rule order.** All landscape-picture rules are applied before all vegetation-picture rules, so
  where both give a tile on the same layer the vegetation one wins (file order does not decide it).
- **Not reproducible:** which entry a rule picks at random (WorldEd's random numbers), and WorldEd's
  real `chunkdata`. Comparisons with WorldEd are by alias group, `chunkdata` ignored.
- `worlded <project>.pzw` (stage 4a) compiles lots from such maps: ground from the pictures, merged
  with the layers, plus buildings. Rules are read from `Rules.txt` (the embedded copies are ignored),
  `obsolete = true` rules are not skipped (not checked against WorldEd), blends are per 300 x 300 cell.

## `.pack` (Project Zomboid texture pack, `media/texturepacks/*.pack`)

All integers little-endian u32.

```
"PZPK"  u32 version (1)  u32 page_count
page_count times:
    u32 name_len, name            page name, e.g. "Tiles2x0"
    u32 entry_count, u32 flag     flag was 1 in every pack looked at
    entry_count times:
        u32 name_len, name        tile name, e.g. "blends_natural_01_0"
        u32 x, y, w, h            trimmed rectangle inside the page image
        u32 ox, oy                where the trimmed rectangle sits inside the full frame
        u32 fw, fh                full frame size (128 x 256 for 2x tiles)
    u32 png_len, png bytes        the page image (RGBA PNG)
```

Checked on `Tiles2x.floor.pack` (11 pages, every png_len matches the next page's start).
Tile names are the tile's own name, so no `Tilesets.txt` lookup is needed to find art:
draw the entry named `floors_burnt_01_8` directly.
