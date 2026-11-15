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

**No real TMX file was available when this was written.** The parser follows the public Tiled
TMX format that WorldEd's maps are based on: `<map width height>`, `<tileset firstgid name ...>`,
`<layer name visible>` with `<data encoding="base64|csv" compression="zlib|gzip|none">` holding
`width*height` little-endian u32 global tile ids; the top 3 bits (0xE0000000) are flip flags and
are masked off. A tile's name is `<tileset name>_<gid - firstgid>`. Unverified against WorldEd
output; fix here if a real file differs.

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
