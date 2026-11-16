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
use std::cell::{Cell as Counter, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
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

#[derive(Clone)]
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
        if world.width == 0 || world.height == 0 || world.width > 10_000 || world.height > 10_000 {
            return Err(perr(pzw, format!("world size {} x {} cells is not supported (1 to 10000 each)", world.width, world.height)));
        }
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
                (Some(named), None) => warnings.push(format!("cell {},{}: {named} not found, no ground there", c.x, c.y)),
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

    /// A row of lot windows over this world. Nothing is read yet: each `.tmx` cell is read, generated
    /// and merged when a window first needs it, and dropped once the windows have moved past it, so a
    /// row holds at most the cells under one window (memory does not grow with the width of the world).
    /// The rows of tiles `[y_lo, y_hi)` are only a hint and no longer limit anything.
    pub fn load_rows<'a>(&self, _y_lo: i64, _y_hi: i64, gen: &'a Generator<'a>) -> Result<RowCells<'a>, CompileError> {
        Ok(RowCells { world: self.clone(), gen, cells: RefCell::new(HashMap::new()), skipped: Counter::new(0) })
    }
}

/// One cell after generation and merge: layers as palette numbers of the cell's own palette.
struct Effective {
    pal: Palette,
    /// `(key, level, grid)` in level order, file order within a level.
    layers: Vec<(String, i32, Grid)>,
    /// Whether the map embedded a landscape picture (the ground comes from the pictures then).
    pictures: bool,
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
    build(&text, path, i, j, gen)
}

/// The effective (generated + merged) layers of one cell's map text. Same as `build_cell` but on
/// text already in memory, so an editor can show what its unsaved edits would compile to.
fn build(text: &str, path: &Path, i: u32, j: u32, gen: &Generator) -> Result<(Effective, usize), CompileError> {
    let tmx: Tmx = parse_tmx_str(text).map_err(|e| perr(path, e.to_string()))?;
    if tmx.width as i64 != CELL || tmx.height as i64 != CELL {
        return Err(perr(path, format!("map is {} x {} tiles, a cell must be 300 x 300", tmx.width, tmx.height)));
    }
    if let Some(l) = tmx.layers.iter().find(|l| l.level < 0) {
        return Err(perr(path, format!("layer {} has level {}; levels below 0 are not supported", l.name, l.level)));
    }
    let images = parse_bmp_images(text).map_err(|e| perr(path, e.to_string()))?;
    let pictures = images.iter().any(|m| m.index == 0);

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
        // a name like `0_Floor` (level taken from it, no level attribute) already is the key
        let key = if l.name.starts_with(&format!("{}_", l.level)) { l.name.clone() } else { format!("{}_{}", l.level, l.name) };
        layers.push((key, l.level, grid));
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
    Ok((Effective { pal, layers, pictures }, skipped))
}

/// One cell's exported layers as gids of the map's own tilesets, so a viewer can draw exactly what
/// the compiler would write. `text` is the map's current text (unsaved edits included).
pub struct EffectiveCell {
    /// `(key "<level>_<name>", level, gids)`, same order as the compiled layers.
    pub layers: Vec<(String, i32, Vec<u32>)>,
    /// Squares whose tile no tileset of the map covered (left empty).
    pub skipped: usize,
    /// Whether the map embedded a landscape picture (ground was generated from it).
    pub pictures: bool,
}

pub fn effective_layers(text: &str, path: &Path, i: u32, j: u32, gen: &Generator) -> Result<EffectiveCell, CompileError> {
    let (eff, mut skipped) = build(text, path, i, j, gen)?;
    let tmx = parse_tmx_str(text).map_err(|e| perr(path, e.to_string()))?;
    let layers = eff
        .layers
        .iter()
        .map(|(key, level, grid)| {
            let mut gids = vec![0u32; grid.data.len()];
            for (k, &v) in grid.data.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                match worlded_core::gids::lookup_gid(&tmx, &eff.pal.names[v as usize]) {
                    Ok(Some(g)) => gids[k] = g,
                    _ => skipped += 1,
                }
            }
            (key.clone(), *level, gids)
        })
        .collect();
    Ok(EffectiveCell { layers, skipped, pictures: eff.pictures })
}

/// The `.tmx` cells one row of lot windows reads, built on demand and kept only while needed.
pub struct RowCells<'a> {
    world: TmxWorld,
    gen: &'a Generator<'a>,
    cells: RefCell<HashMap<(u32, u32), Rc<Effective>>>,
    skipped: Counter<usize>,
}

impl RowCells<'_> {
    /// Squares whose tile id no tileset of their map covers (left empty), in the cells built so far.
    pub fn skipped(&self) -> usize {
        self.skipped.get()
    }

    /// Cells currently held.
    pub fn cached(&self) -> usize {
        self.cells.borrow().len()
    }

    /// `win x win` squares starting at tile `(x0, y0)`: the layer grids, keyed `"<level>_<name>"`
    /// (union of the layers of the cells touched, level order then first-seen order), and a mask
    /// that is true where the square lies inside the world. A damaged `.tmx` is an error here.
    pub fn window(&self, x0: i64, y0: i64, win: usize, pal: &mut Palette) -> Result<(Layers, Vec<bool>), CompileError> {
        let size = self.world.size_tiles();
        // windows move left to right: cells that end before this one starts are not needed again
        self.cells.borrow_mut().retain(|&(i, _), _| i as i64 * CELL + CELL > x0);
        let (i0, i1) = (x0.max(0) / CELL, (x0 + win as i64 - 1).min(size.0 - 1) / CELL);
        let (j0, j1) = (y0.max(0) / CELL, (y0 + win as i64 - 1).min(size.1 - 1) / CELL);
        let mut touched: Vec<(u32, u32)> = Vec::new();
        if i1 >= i0 && j1 >= j0 {
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let key = (i as u32, j as u32);
                    let Some(path) = self.world.maps.get(&key) else { continue };
                    if !self.cells.borrow().contains_key(&key) {
                        let (eff, n) = build_cell(path, key.0, key.1, self.gen)?;
                        self.skipped.set(self.skipped.get() + n);
                        self.cells.borrow_mut().insert(key, Rc::new(eff));
                    }
                    touched.push(key);
                }
            }
        }
        let held: Vec<Rc<Effective>> = touched.iter().map(|k| self.cells.borrow()[k].clone()).collect();
        let mut keys: Vec<(i32, usize, String)> = Vec::new();
        for eff in &held {
            for (key, level, _) in &eff.layers {
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
        for (&(i, j), eff) in touched.iter().zip(&held) {
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
                inside[r * win + c] = tx >= 0 && ty >= 0 && tx < size.0 && ty < size.1;
            }
        }
        Ok((layers, inside))
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

    fn rows(w: &TmxWorld, y0: i64, y1: i64) -> RowCells<'static> {
        // the generator lives as long as the test process: a RowCells borrows it
        let (rules, blends) = env();
        let (rules, blends): (&'static Ruleset, &'static [Blend]) = (Box::leak(Box::new(rules)), Box::leak(blends.into_boxed_slice()));
        let gen: &'static Generator<'static> = Box::leak(Box::new(Generator { rules, blends }));
        w.load_rows(y0, y1, gen).unwrap()
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
        let (layers, _) = rows(&w, 0, 10).window(0, 0, 4, &mut Palette::default()).unwrap();
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
        let (layers, inside) = r.window(290, 0, 20, &mut pal).unwrap();
        assert_eq!(name_at(&pal, &layers, "0_Floor", 9, 5, 20), "t_1");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 10, 5, 20), "t_2");
        assert!(inside.iter().all(|v| *v));
        let (_, inside) = r.window(-5, -3, 10, &mut pal).unwrap();
        assert!(!inside[0] && !inside[4 * 10 + 4] && inside[3 * 10 + 5] && inside[9 * 10 + 9]);
        let (_, inside) = r.window(595, 295, 10, &mut pal).unwrap();
        assert!(inside[0] && !inside[5] && !inside[5 * 10]);
    }

    #[test]
    fn layer_sets_that_differ_between_cells_use_the_union() {
        let a = tmx(300, &[("Floor", 0, &[]), ("Walls2", 0, &[(299, 0, 1)])], "");
        let b = tmx(300, &[("Floor", 0, &[(0, 0, 2)])], "");
        let p = project("union", "70,0", 2, 1, &[(0, 0, a), (1, 0, b)]);
        let r = rows(&TmxWorld::open(&p).unwrap(), 0, 5);
        let mut pal = Palette::default();
        let (layers, _) = r.window(298, 0, 4, &mut pal).unwrap();
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
        let (layers, _) = r.window(0, 0, 5, &mut Palette::default()).unwrap();
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
        let e = TmxWorld::open(&p).unwrap().load_rows(0, 5, &gen).unwrap().window(0, 0, 4, &mut Palette::default()).err().unwrap().to_string();
        assert!(e.contains("c_0_0.tmx") && e.contains("300"), "{e}");
        let p = project("neg", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", -1, &[])], ""))]);
        assert!(TmxWorld::open(&p).unwrap().load_rows(0, 5, &gen).unwrap().window(0, 0, 4, &mut Palette::default()).err().unwrap().to_string().contains("level -1"));
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
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
        let n = name_at(&pal, &layers, "0_Floor", 5, 5, 20);
        assert!(n.starts_with("blends_natural_01_"), "generated ground, not the hand edit: {n}");
    }

    #[test]
    fn hand_edit_on_furniture_and_on_vegetation_without_a_generated_tile_stay() {
        let text = tmx(300, &[("Floor", 0, &[]), ("Furniture", 0, &[(6, 6, 2)]), ("Vegetation", 0, &[(7, 7, 3)])], &pictures());
        let r = rows(&TmxWorld::open(&project("stay", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
        assert_eq!(name_at(&pal, &layers, "0_Furniture", 6, 6, 20), "t_1");
        assert_eq!(name_at(&pal, &layers, "0_Vegetation", 7, 7, 20), "t_2", "the picture gives no vegetation there");
        assert!(name_at(&pal, &layers, "0_Floor", 0, 0, 20).starts_with("blends_natural_01_"), "ground still generated");
    }

    #[test]
    fn cell_without_pictures_uses_its_layers_alone() {
        let text = tmx(300, &[("Floor", 0, &[(5, 5, 6)])], "");
        let r = rows(&TmxWorld::open(&project("nopic", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
        assert_eq!(name_at(&pal, &layers, "0_Floor", 5, 5, 20), "t_5");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 6, 6, 20), "");
    }

    #[test]
    fn unknown_colour_gives_no_generated_tile_so_the_hand_edit_shows() {
        let text = tmx(300, &[("Floor", 0, &[(5, 5, 6)])], &(picture(0, &["1 2 3"]) + &picture(1, &["1 2 3"])));
        let r = rows(&TmxWorld::open(&project("unk", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
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

    #[test]
    fn cells_are_built_when_a_window_needs_them_and_dropped_behind_it() {
        let maps: Vec<(u32, u32, String)> = (0..12).map(|i| (i, 0, tmx(300, &[("Floor", 0, &[(1, 1, 2)])], ""))).collect();
        let r = rows(&TmxWorld::open(&project("lazy", "70,0", 12, 1, &maps)).unwrap(), 0, 300);
        assert_eq!(r.cached(), 0, "nothing is read before a window asks");
        let mut pal = Palette::default();
        let (mut peak, mut x0) = (0, -1);
        while x0 < 3600 {
            r.window(x0, -1, 258, &mut pal).unwrap();
            peak = peak.max(r.cached());
            x0 += 256;
        }
        assert!(peak <= 3, "at most the cells under one window are kept, peak {peak}");
        // a damaged map is found when a window needs it, and names the file
        let p = project("lazybad", "70,0", 1, 1, &[(0, 0, tmx(299, &[("Floor", 0, &[])], ""))]);
        let e = rows(&TmxWorld::open(&p).unwrap(), 0, 5).window(0, 0, 4, &mut Palette::default()).err().unwrap().to_string();
        assert!(e.contains("c_0_0.tmx") && e.contains("300"), "{e}");
    }

    #[test]
    fn a_layer_named_with_its_level_and_no_level_attribute_merges_with_the_generated_layer() {
        let text = tmx(300, &[("0_Floor", 0, &[(5, 5, 6)]), ("0_Furniture", 0, &[(6, 6, 2)])], &pictures()).replace(" level=\"0\"", "");
        let r = rows(&TmxWorld::open(&project("prefixed", "70,0", 1, 1, &[(0, 0, text)])).unwrap(), 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
        let keys: Vec<&str> = layers.0.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"0_Floor") && !keys.iter().any(|k| k.starts_with("0_0_")), "{keys:?}");
        assert!(name_at(&pal, &layers, "0_Floor", 5, 5, 20).starts_with("blends_natural_01_"), "the generated ground replaces the hand edit");
        assert_eq!(name_at(&pal, &layers, "0_Furniture", 6, 6, 20), "t_1");
    }

    #[test]
    fn world_sizes_of_zero_or_absurd_are_errors() {
        for (w, h) in [(0u32, 1u32), (1, 0), (20_000, 1)] {
            let p = project("size0", "70,0", 1, 1, &[]);
            let text = std::fs::read_to_string(&p).unwrap().replace("width=\"1\" height=\"1\"", &format!("width=\"{w}\" height=\"{h}\""));
            std::fs::write(&p, text).unwrap();
            let e = TmxWorld::open(&p).err().unwrap().to_string();
            assert!(e.contains("size") && e.contains(&w.to_string()), "{e}");
        }
        // a negative size is read as a huge one
        let p = project("sizeneg", "70,0", 1, 1, &[]);
        std::fs::write(&p, std::fs::read_to_string(&p).unwrap().replace("width=\"1\"", "width=\"-1\"")).unwrap();
        assert!(TmxWorld::open(&p).is_err());
    }

    #[test]
    fn effective_layers_match_the_compiled_window_and_follow_in_memory_edits() {
        // the test map lists only tileset `t`; add the blends tileset so generated names resolve to gids
        let with_blends = |text: String| text.replacen("<tileset firstgid=\"1\" name=\"t\"/>", "<tileset firstgid=\"1\" name=\"t\"/><tileset firstgid=\"1000\" name=\"blends_natural_01\"/>", 1);
        let text = with_blends(tmx(300, &[("Floor", 0, &[(5, 5, 6)]), ("Furniture", 0, &[(6, 6, 2)])], &pictures()));
        let p = project("eff", "70,0", 1, 1, &[(0, 0, text.clone())]);
        let w = TmxWorld::open(&p).unwrap();
        let r = rows(&w, 0, 20);
        let mut pal = Palette::default();
        let (layers, _) = r.window(0, 0, 20, &mut pal).unwrap();
        let (rules, blends) = env();
        let gen = Generator { rules: &rules, blends: &blends };
        let eff = effective_layers(&text, &p, 0, 0, &gen).unwrap();
        assert!(eff.pictures);
        let map = parse_tmx_str(&text).unwrap();
        for (key, _, gids) in &eff.layers {
            for y in 0..20usize {
                for x in 0..20usize {
                    let got = if gids[y * 300 + x] == 0 { String::new() } else { tile_name(&map, gids[y * 300 + x]).unwrap() };
                    assert_eq!(got, name_at(&pal, &layers, key, x, y, 20), "{key} at {x},{y}");
                }
            }
        }
        // an in-memory edit shows up (Furniture is not a generated layer, the edit stays)
        let edited = with_blends(tmx(300, &[("Floor", 0, &[]), ("Furniture", 0, &[(6, 6, 3)])], &pictures()));
        let eff2 = effective_layers(&edited, &p, 0, 0, &gen).unwrap();
        let map2 = parse_tmx_str(&edited).unwrap();
        let (_, _, g) = eff2.layers.iter().find(|(k, _, _)| k == "0_Furniture").unwrap();
        assert_eq!(tile_name(&map2, g[6 * 300 + 6]).as_deref(), Some("t_2"));
    }
}

