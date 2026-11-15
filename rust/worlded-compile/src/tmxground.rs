//! Ground from the cells' `.tmx` maps, the way WorldEd's lot generation reads them: the ground is
//! generated from the pictures embedded in each map (landscape and vegetation) with the rules, and
//! merged with the map's own tile layers. Per layer the bitmap generates and per square, the
//! generated tile wins when there is one, else the layer's own tile stays (measured against
//! WorldEd). Layer key is `"<level>_<name>"`; within a level the order is the file order.
use crate::blend_layers::blend_layers;
use crate::blends::Blend;
use crate::rules::Ruleset;
use crate::terrain::{ground_layers, Grid, Layers, Palette};
use crate::CompileError;
use std::cell::Cell as Counter;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use worlded_core::pzw::{parse_pzw, resolve_map};
use worlded_core::tmx::{parse_bmp_images, parse_tmx_str, tile_name, BmpImage, Tmx};

/// Tiles on one side of a `.tmx` cell.
pub const CELL: i64 = 300;
const SIDE: usize = CELL as usize;

/// What the generation needs besides the maps themselves.
pub struct Generator<'a> {
    pub rules: &'a Ruleset,
    pub blends: &'a [Blend],
}

pub struct TmxWorld {
    pub cells_x: u32,
    pub cells_y: u32,
    pub warnings: Vec<String>,
    maps: HashMap<(u32, u32), PathBuf>,
}

fn perr(p: &Path, m: impl Into<String>) -> CompileError {
    CompileError::Parse(p.to_path_buf(), m.into())
}

/// Square by square: the generated tile if there is one, else the hand-made one.
pub fn merge_layer(generated: &Grid, manual: &Grid) -> Grid {
    let mut out = manual.clone();
    for (o, &g) in out.data.iter_mut().zip(&generated.data) {
        if g != 0 {
            *o = g;
        }
    }
    out
}

impl TmxWorld {
    /// Reads the project. A named `.tmx` that cannot be found is a warning and that cell has no
    /// ground; a world origin other than 70,0 is an error.
    pub fn open(pzw: &Path) -> Result<TmxWorld, CompileError> {
        let world = parse_pzw(pzw).map_err(|e| perr(pzw, e.to_string()))?;
        if world.origin != (70, 0) {
            return Err(perr(pzw, format!("world origin {},{} is not supported: only 70,0", world.origin.0, world.origin.1)));
        }
        let dir = pzw.parent().unwrap_or(Path::new("."));
        let (mut maps, mut warnings) = (HashMap::new(), Vec::new());
        for c in &world.cells {
            if c.x < 0 || c.y < 0 || c.x as u32 >= world.width || c.y as u32 >= world.height {
                continue;
            }
            match (&c.map, resolve_map(c, dir)) {
                (_, Some(p)) => {
                    maps.insert((c.x as u32, c.y as u32), p);
                }
                (Some(named), None) => warnings.push(format!("cell {},{}: {} not found, no ground there", c.x, c.y, named.display())),
                (None, None) => {}
            }
        }
        Ok(TmxWorld { cells_x: world.width, cells_y: world.height, warnings, maps })
    }

    /// The world in tiles: `(300 * cells_x, 300 * cells_y)`.
    pub fn size_tiles(&self) -> (i64, i64) {
        (CELL * self.cells_x as i64, CELL * self.cells_y as i64)
    }

    pub fn has_maps(&self) -> bool {
        !self.maps.is_empty()
    }

    /// Reads and prepares every `.tmx` cell that overlaps tile rows `[y_lo, y_hi)`.
    pub fn load_rows(&self, y_lo: i64, y_hi: i64, gen: &Generator) -> Result<RowCells, CompileError> {
        let mut cells = HashMap::new();
        let mut skipped = 0;
        if y_hi > y_lo {
            let (j0, j1) = (y_lo.max(0) / CELL, (y_hi - 1).max(0) / CELL);
            for j in j0..=j1.min(self.cells_y as i64 - 1) {
                for i in 0..self.cells_x {
                    let Some(path) = self.maps.get(&(i, j as u32)) else { continue };
                    let (eff, n) = build_cell(path, i, j as u32, gen)?;
                    skipped += n;
                    cells.insert((i, j as u32), eff);
                }
            }
        }
        Ok(RowCells { cells, size: self.size_tiles(), skipped: Counter::new(skipped) })
    }
}

/// One cell after generation and merge: layers as palette numbers of the cell's own palette.
struct Effective {
    pal: Palette,
    /// `(key, level, grid)` in level order, file order within a level.
    layers: Vec<(String, i32, Grid)>,
}

fn level_of(key: &str) -> i32 {
    key.split('_').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// RGB bytes of a picture (colour 0 = no colour = black, which no rule uses).
fn rgb(img: Option<&BmpImage>) -> Vec<u8> {
    let mut out = vec![0u8; SIDE * SIDE * 3];
    if let Some(img) = img {
        for (i, &p) in img.pixels.iter().enumerate() {
            if p != 0 {
                out[i * 3..i * 3 + 3].copy_from_slice(&img.colors[p as usize - 1]);
            }
        }
    }
    out
}

/// Reads one `.tmx`, generates its ground from the embedded pictures (when it has them) and
/// merges. The second value is the number of squares whose tile id no tileset covered.
fn build_cell(path: &Path, i: u32, j: u32, gen: &Generator) -> Result<(Effective, usize), CompileError> {
    let text = std::fs::read_to_string(path).map_err(|e| CompileError::Io(path.to_path_buf(), e))?;
    let tmx: Tmx = parse_tmx_str(&text).map_err(|e| perr(path, e.to_string()))?;
    if tmx.width as i64 != CELL || tmx.height as i64 != CELL {
        return Err(perr(path, format!("map is {} x {} tiles, a cell must be 300 x 300", tmx.width, tmx.height)));
    }
    if let Some(l) = tmx.layers.iter().find(|l| l.level < 0) {
        return Err(perr(path, format!("layer {} has level {}; levels below 0 are not supported", l.name, l.level)));
    }
    let images = parse_bmp_images(&text).map_err(|e| perr(path, e.to_string()))?;

    let mut pal = Palette::default();
    let mut skipped = 0;
    let mut cache: HashMap<u32, u32> = HashMap::new();
    let mut layers: Vec<(String, i32, Grid)> = Vec::new();
    for l in &tmx.layers {
        let mut grid = Grid::new(SIDE, SIDE);
        for (k, &gid) in l.gids.iter().enumerate() {
            if gid == 0 {
                continue;
            }
            let id = *cache.entry(gid).or_insert_with(|| tile_name(&tmx, gid).map_or(0, |n| pal.of(&n)));
            if id == 0 {
                skipped += 1;
            }
            grid.data[k] = id;
        }
        layers.push((format!("{}_{}", l.level, l.name), l.level, grid));
    }
    // keep the file's order, but within a level only (a stable sort by level)
    layers.sort_by_key(|(_, level, _)| *level);

    if let Some(land) = images.iter().find(|m| m.index == 0) {
        let veg = images.iter().find(|m| m.index == 1);
        let (x0, y0) = (CELL * i as i64, CELL * j as i64);
        let mut generated = ground_layers(&rgb(Some(land)), &rgb(veg), SIDE, SIDE, gen.rules, &mut pal, land.seed, x0, y0);
        if let Some(floor) = generated.get("0_Floor").cloned() {
            let b = blend_layers(&floor, gen.rules, gen.blends, &mut pal, land.seed, x0, y0, Some(CELL)).map_err(|e| perr(path, e.to_string()))?;
            generated.update(b);
        }
        for (key, g) in generated.0 {
            match layers.iter().position(|(k, _, _)| *k == key) {
                Some(p) => layers[p].2 = merge_layer(&g, &layers[p].2),
                None => {
                    // a layer only the generator has goes after the file's layers of its level
                    let level = level_of(&key);
                    let at = layers.iter().rposition(|(_, lv, _)| *lv <= level).map_or(0, |p| p + 1);
                    layers.insert(at, (key, level, g));
                }
            }
        }
    }
    Ok((Effective { pal, layers }, skipped))
}

/// The `.tmx` cells one row of lot windows needs, prepared.
pub struct RowCells {
    cells: HashMap<(u32, u32), Effective>,
    size: (i64, i64),
    skipped: Counter<usize>,
}

impl RowCells {
    /// Squares whose tile id no tileset of their map covers (left empty).
    pub fn skipped(&self) -> usize {
        self.skipped.get()
    }

    /// `win x win` squares starting at tile `(x0, y0)`: the layer grids, keyed `"<level>_<name>"`
    /// (union of the layers of the cells touched, level order then first-seen order), and a mask
    /// that is true where the square lies inside the world.
    pub fn window(&self, x0: i64, y0: i64, win: usize, pal: &mut Palette) -> (Layers, Vec<bool>) {
        let mut touched: Vec<(u32, u32)> = self
            .cells
            .keys()
            .copied()
            .filter(|&(i, j)| {
                let (cx, cy) = (i as i64 * CELL, j as i64 * CELL);
                cx < x0 + win as i64 && cx + CELL > x0 && cy < y0 + win as i64 && cy + CELL > y0
            })
            .collect();
        touched.sort_by_key(|&(i, j)| (j, i));
        let mut keys: Vec<(i32, usize, String)> = Vec::new();
        for c in &touched {
            for (key, level, _) in &self.cells[c].layers {
                if !keys.iter().any(|k| k.2 == *key) {
                    keys.push((*level, keys.len(), key.clone()));
                }
            }
        }
        keys.sort();
        let mut layers = Layers::default();
        for (_, _, k) in &keys {
            layers.entry(k, win, win);
        }
        for &(i, j) in &touched {
            let eff = &self.cells[&(i, j)];
            // the cell's palette numbers in the caller's palette
            let map: Vec<u32> = eff.pal.names.iter().map(|n| if n.is_empty() { 0 } else { pal.of(n) }).collect();
            let (cx, cy) = (i as i64 * CELL, j as i64 * CELL);
            let (xa, xb) = (x0.max(cx), (x0 + win as i64).min(cx + CELL));
            let (ya, yb) = (y0.max(cy), (y0 + win as i64).min(cy + CELL));
            for (key, _, src) in &eff.layers {
                let grid = layers.entry(key, win, win);
                for ty in ya..yb {
                    for tx in xa..xb {
                        let v = src.data[((ty - cy) * CELL + (tx - cx)) as usize];
                        if v != 0 {
                            grid.set((tx - x0) as usize, (ty - y0) as usize, map[v as usize]);
                        }
                    }
                }
            }
        }
        let mut inside = vec![false; win * win];
        for r in 0..win {
            for c in 0..win {
                let (tx, ty) = (x0 + c as i64, y0 + r as i64);
                inside[r * win + c] = tx >= 0 && ty >= 0 && tx < self.size.0 && ty < self.size.1;
            }
        }
        (layers, inside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blends::load_blends;
    use crate::rules::load_rules;
    use base64::Engine;
    use std::io::Write;

    fn env() -> (Ruleset, Vec<Blend>) {
        let cfg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
        (load_rules(&cfg.join("Rules.txt")).unwrap(), load_blends(&cfg.join("Blends.txt")).unwrap())
    }

    fn rows(w: &TmxWorld, y0: i64, y1: i64) -> RowCells {
        let (rules, blends) = env();
        w.load_rows(y0, y1, &Generator { rules: &rules, blends: &blends }).unwrap()
    }

    /// A project folder with one `.tmx` per `(cell x, cell y, tmx text)`; returns the `.pzw` path.
    fn project(tag: &str, origin: &str, cells_x: u32, cells_y: u32, maps: &[(u32, u32, String)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wcomp_tmxground_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut pzw = format!("<?xml version=\"1.0\"?>\n<world version=\"1.0\" width=\"{cells_x}\" height=\"{cells_y}\">\n <GenerateLots><worldOrigin origin=\"{origin}\"/></GenerateLots>\n");
        for y in 0..cells_y {
            for x in 0..cells_x {
                match maps.iter().find(|m| m.0 == x && m.1 == y) {
                    Some((_, _, text)) => {
                        let f = format!("c_{x}_{y}.tmx");
                        std::fs::write(dir.join(&f), text).unwrap();
                        pzw += &format!(" <cell x=\"{x}\" y=\"{y}\" map=\"{f}\"></cell>\n");
                    }
                    None => pzw += &format!(" <cell x=\"{x}\" y=\"{y}\" map=\"\"></cell>\n"),
                }
            }
        }
        pzw += "</world>\n";
        let p = dir.join("w.pzw");
        std::fs::write(&p, pzw).unwrap();
        p
    }

    /// A `<bmp-image>` whose every pixel is colour 1 of `colors`.
    fn picture(index: u32, colors: &[&str]) -> String {
        let raw: Vec<u8> = (0..SIDE * SIDE).flat_map(|_| 1u32.to_le_bytes()).collect();
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(&raw).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(e.finish().unwrap());
        let cols: String = colors.iter().map(|c| format!("<color rgb=\"{c}\"/>")).collect();
        format!("<bmp-image index=\"{index}\" seed=\"1\">{cols}<pixels>{b64}</pixels></bmp-image>")
    }

    type Edits<'a> = &'a [(usize, usize, u32)];

    /// A map with one tileset `t` (gid 1 is `t_0`), csv layers `(name, level, tiles)` and `extra` xml.
    fn tmx(width: usize, layers: &[(&str, i32, Edits)], extra: &str) -> String {
        let mut s = format!("<map width=\"{width}\" height=\"300\"><tileset firstgid=\"1\" name=\"t\"/>");
        for (name, level, tiles) in layers {
            let mut g = vec![0u32; width * 300];
            for &(x, y, gid) in *tiles {
                g[y * width + x] = gid;
            }
            let body: Vec<String> = g.iter().map(u32::to_string).collect();
            s += &format!("<layer name=\"{name}\" level=\"{level}\"><data encoding=\"csv\">{}</data></layer>", body.join(","));
        }
        s + extra + "</map>"
    }

    fn name_at(pal: &Palette, layers: &Layers, key: &str, x: usize, y: usize, win: usize) -> String {
        let g = layers.get(key).unwrap_or_else(|| panic!("no layer {key}"));
        assert_eq!(g.w, win);
        pal.names[g.get(x, y) as usize].clone()
    }

    #[test]
    fn layer_keys_use_level_and_file_order() {
        let p = project("keys", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[]), ("Walls", 1, &[]), ("Furniture", 0, &[])], ""))]);
        let w = TmxWorld::open(&p).unwrap();
        let (layers, _) = rows(&w, 0, 10).window(0, 0, 4, &mut Palette::default());
        let keys: Vec<&str> = layers.0.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["0_Floor", "0_Furniture", "1_Walls"]);
    }

    #[test]
    fn window_spans_two_cells_and_masks_outside_the_world() {
        let a = tmx(300, &[("Floor", 0, &[(299, 5, 2)])], "");
        let b = tmx(300, &[("Floor", 0, &[(0, 5, 3)])], "");
        let p = project("span", "70,0", 2, 1, &[(0, 0, a), (1, 0, b)]);
        let w = TmxWorld::open(&p).unwrap();
        assert_eq!(w.size_tiles(), (600, 300));
        let r = rows(&w, 0, 20);
        let mut pal = Palette::default();
        let (layers, inside) = r.window(290, 0, 20, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Floor", 9, 5, 20), "t_1");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 10, 5, 20), "t_2");
        assert!(inside.iter().all(|v| *v));
        let (_, inside) = r.window(-5, -3, 10, &mut pal);
        assert!(!inside[0] && !inside[4 * 10 + 4] && inside[3 * 10 + 5] && inside[9 * 10 + 9]);
        let (_, inside) = r.window(595, 295, 10, &mut pal);
        assert!(inside[0] && !inside[5] && !inside[5 * 10]);
    }

    #[test]
    fn layer_sets_that_differ_between_cells_use_the_union() {
        let a = tmx(300, &[("Floor", 0, &[]), ("Walls2", 0, &[(299, 0, 1)])], "");
        let b = tmx(300, &[("Floor", 0, &[(0, 0, 2)])], "");
        let p = project("union", "70,0", 2, 1, &[(0, 0, a), (1, 0, b)]);
        let r = rows(&TmxWorld::open(&p).unwrap(), 0, 5);
        let mut pal = Palette::default();
        let (layers, _) = r.window(298, 0, 4, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Walls2", 1, 0, 4), "t_0");
        assert_eq!(name_at(&pal, &layers, "0_Walls2", 2, 0, 4), "", "the right cell has no Walls2");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 2, 0, 4), "t_1");
    }

    #[test]
    fn unknown_gid_is_skipped_and_counted() {
        // a tileset covers every gid from its firstgid up, so a gid is unknown only below the first one
        let text = tmx(300, &[("Floor", 0, &[(1, 1, 5)])], "").replace("firstgid=\"1\"", "firstgid=\"9\"");
        let p = project("gid", "70,0", 1, 1, &[(0, 0, text)]);
        let r = rows(&TmxWorld::open(&p).unwrap(), 0, 10);
        let (layers, _) = r.window(0, 0, 5, &mut Palette::default());
        assert_eq!(layers.get("0_Floor").unwrap().get(1, 1), 0);
        assert_eq!(r.skipped(), 1);
    }

    #[test]
    fn wrong_origin_missing_file_wrong_size_and_negative_level() {
        let p = project("origin", "71,0", 1, 1, &[]);
        let e = TmxWorld::open(&p).err().unwrap().to_string();
        assert!(e.contains("71,0"), "{e}");
        // a named file that is missing: a warning, not an error
        let p = project("missing", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[])], ""))]);
        std::fs::remove_file(p.with_file_name("c_0_0.tmx")).unwrap();
        let w = TmxWorld::open(&p).unwrap();
        assert!(w.warnings.iter().any(|m| m.contains("c_0_0.tmx")), "{:?}", w.warnings);
        assert!(!w.has_maps());
        let (rules, blends) = env();
        let gen = Generator { rules: &rules, blends: &blends };
        let p = project("size", "70,0", 1, 1, &[(0, 0, tmx(299, &[("Floor", 0, &[])], ""))]);
        let e = TmxWorld::open(&p).unwrap().load_rows(0, 5, &gen).err().unwrap().to_string();
        assert!(e.contains("c_0_0.tmx") && e.contains("300"), "{e}");
        let p = project("neg", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", -1, &[])], ""))]);
        assert!(TmxWorld::open(&p).unwrap().load_rows(0, 5, &gen).err().unwrap().to_string().contains("level -1"));
    }

    /// Dark grass over the whole landscape, an unknown colour in the vegetation picture.
    fn pictures() -> String {
        picture(0, &["90 100 35"]) + &picture(1, &["1 2 3"])
    }

    #[test]
    fn hand_edit_on_floor_is_overwritten_where_the_picture_gives_ground() {
        let text = tmx(300, &[("Floor", 0, &[(5, 5, 6)])], &pictures());
        let r = rows(&TmxWorld::open(&project("floor", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal);
        let n = name_at(&pal, &layers, "0_Floor", 5, 5, 20);
        assert!(n.starts_with("blends_natural_01_"), "generated ground, not the hand edit: {n}");
    }

    #[test]
    fn hand_edit_on_furniture_and_on_vegetation_without_a_generated_tile_stay() {
        let text = tmx(300, &[("Floor", 0, &[]), ("Furniture", 0, &[(6, 6, 2)]), ("Vegetation", 0, &[(7, 7, 3)])], &pictures());
        let r = rows(&TmxWorld::open(&project("stay", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Furniture", 6, 6, 20), "t_1");
        assert_eq!(name_at(&pal, &layers, "0_Vegetation", 7, 7, 20), "t_2", "the picture gives no vegetation there");
        assert!(name_at(&pal, &layers, "0_Floor", 0, 0, 20).starts_with("blends_natural_01_"), "ground still generated");
    }

    #[test]
    fn cell_without_pictures_uses_its_layers_alone() {
        let text = tmx(300, &[("Floor", 0, &[(5, 5, 6)])], "");
        let r = rows(&TmxWorld::open(&project("nopic", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Floor", 5, 5, 20), "t_5");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 6, 6, 20), "");
    }

    #[test]
    fn unknown_colour_gives_no_generated_tile_so_the_hand_edit_shows() {
        let text = tmx(300, &[("Floor", 0, &[(5, 5, 6)])], &(picture(0, &["1 2 3"]) + &picture(1, &["1 2 3"])));
        let r = rows(&TmxWorld::open(&project("unk", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Floor", 5, 5, 20), "t_5");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 6, 6, 20), "");
    }

    #[test]
    fn merge_layer_prefers_generated_nonzero() {
        let mut g = Grid::new(2, 1);
        g.data = vec![0, 7];
        let mut m = Grid::new(2, 1);
        m.data = vec![3, 4];
        assert_eq!(merge_layer(&g, &m).data, vec![3, 7]);
    }
}
