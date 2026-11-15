//! Ground from the cells' `.tmx` maps instead of the bitmaps: the same padded layer grids per
//! lot window that `terrain::ground_layers` makes, filled from the tiles the map editor saved.
//! Layer key is `"<level>_<name>"`; within a level the order is the file order of the layers.
use crate::terrain::{Layers, Palette};
use crate::CompileError;
use std::cell::Cell as Counter;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use worlded_core::pzw::{parse_pzw, resolve_map};
use worlded_core::tmx::{parse_tmx, tile_name, Tmx};

/// Tiles on one side of a `.tmx` cell.
pub const CELL: i64 = 300;

pub struct TmxWorld {
    pub cells_x: u32,
    pub cells_y: u32,
    pub warnings: Vec<String>,
    maps: HashMap<(u32, u32), PathBuf>,
}

fn perr(p: &Path, m: impl Into<String>) -> CompileError {
    CompileError::Parse(p.to_path_buf(), m.into())
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

    /// Parses every `.tmx` cell that overlaps tile rows `[y_lo, y_hi)`.
    pub fn load_rows(&self, y_lo: i64, y_hi: i64) -> Result<RowCells, CompileError> {
        let mut cells = HashMap::new();
        if y_hi > y_lo {
            let (j0, j1) = (y_lo.max(0) / CELL, (y_hi - 1).max(0) / CELL);
            for j in j0..=j1.min(self.cells_y as i64 - 1) {
                for i in 0..self.cells_x {
                    let Some(path) = self.maps.get(&(i, j as u32)) else { continue };
                    let tmx = parse_tmx(path).map_err(|e| perr(path, e.to_string()))?;
                    if tmx.width as i64 != CELL || tmx.height as i64 != CELL {
                        return Err(perr(path, format!("map is {} x {} tiles, a cell must be 300 x 300", tmx.width, tmx.height)));
                    }
                    if let Some(l) = tmx.layers.iter().find(|l| l.level < 0) {
                        return Err(perr(path, format!("layer {} has level {}; levels below 0 are not supported", l.name, l.level)));
                    }
                    cells.insert((i, j as u32), tmx);
                }
            }
        }
        Ok(RowCells { cells, size: self.size_tiles(), skipped: Counter::new(0) })
    }
}

/// The `.tmx` cells one row of lot windows needs.
pub struct RowCells {
    cells: HashMap<(u32, u32), Tmx>,
    size: (i64, i64),
    skipped: Counter<usize>,
}

impl RowCells {
    /// Squares whose tile id no tileset of their map covers (left empty), so far.
    pub fn skipped(&self) -> usize {
        self.skipped.get()
    }

    /// `win x win` squares starting at tile `(x0, y0)`: the layer grids, keyed `"<level>_<name>"`
    /// (union of the layers of the cells touched, level order then first-seen file order), and a
    /// mask that is true where the square lies inside the world.
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
            for l in &self.cells[c].layers {
                let key = format!("{}_{}", l.level, l.name);
                if !keys.iter().any(|k| k.2 == key) {
                    keys.push((l.level, keys.len(), key));
                }
            }
        }
        keys.sort();
        let mut layers = Layers::default();
        for (_, _, k) in &keys {
            layers.entry(k, win, win);
        }
        let mut gid_names: HashMap<(u32, u32, u32), u32> = HashMap::new();
        for &(i, j) in &touched {
            let tmx = &self.cells[&(i, j)];
            let (cx, cy) = (i as i64 * CELL, j as i64 * CELL);
            let (xa, xb) = (x0.max(cx), (x0 + win as i64).min(cx + CELL));
            let (ya, yb) = (y0.max(cy), (y0 + win as i64).min(cy + CELL));
            for l in &tmx.layers {
                let key = format!("{}_{}", l.level, l.name);
                let grid = layers.entry(&key, win, win);
                for ty in ya..yb {
                    for tx in xa..xb {
                        let gid = l.gids[((ty - cy) * CELL + (tx - cx)) as usize];
                        if gid == 0 {
                            continue;
                        }
                        let id = *gid_names.entry((i, j, gid)).or_insert_with(|| tile_name(tmx, gid).map_or(0, |n| pal.of(&n)));
                        if id == 0 {
                            self.skipped.set(self.skipped.get() + 1);
                            continue;
                        }
                        grid.set((tx - x0) as usize, (ty - y0) as usize, id);
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

    /// A 300 x 300 map with one tileset `t` (gid 1 is `t_0`) and csv layers `(name, level, tiles)`.
    fn tmx(width: usize, layers: &[(&str, i32, &[(usize, usize, u32)])]) -> String {
        let mut s = format!("<map width=\"{width}\" height=\"300\"><tileset firstgid=\"1\" name=\"t\"/>");
        for (name, level, tiles) in layers {
            let mut g = vec![0u32; width * 300];
            for &(x, y, gid) in *tiles {
                g[y * width + x] = gid;
            }
            let body: Vec<String> = g.iter().map(u32::to_string).collect();
            s += &format!("<layer name=\"{name}\" level=\"{level}\"><data encoding=\"csv\">{}</data></layer>", body.join(","));
        }
        s + "</map>"
    }

    fn name_at(pal: &Palette, layers: &Layers, key: &str, x: usize, y: usize, win: usize) -> String {
        let g = layers.get(key).unwrap_or_else(|| panic!("no layer {key}"));
        assert_eq!(g.w, win);
        pal.names[g.get(x, y) as usize].clone()
    }

    #[test]
    fn layer_keys_use_level_and_file_order() {
        let p = project("keys", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[]), ("Walls", 1, &[]), ("Furniture", 0, &[])]))]);
        let w = TmxWorld::open(&p).unwrap();
        let rows = w.load_rows(0, 10).unwrap();
        let (layers, _) = rows.window(0, 0, 4, &mut Palette::default());
        let keys: Vec<&str> = layers.0.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["0_Floor", "0_Furniture", "1_Walls"]);
    }

    #[test]
    fn window_spans_two_cells_and_masks_outside_the_world() {
        let a = tmx(300, &[("Floor", 0, &[(299, 5, 2)])]);
        let b = tmx(300, &[("Floor", 0, &[(0, 5, 3)])]);
        let p = project("span", "70,0", 2, 1, &[(0, 0, a), (1, 0, b)]);
        let w = TmxWorld::open(&p).unwrap();
        assert_eq!(w.size_tiles(), (600, 300));
        let rows = w.load_rows(0, 20).unwrap();
        let mut pal = Palette::default();
        let (layers, inside) = rows.window(290, 0, 20, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Floor", 9, 5, 20), "t_1");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 10, 5, 20), "t_2");
        assert!(inside.iter().all(|v| *v));
        let (_, inside) = rows.window(-5, -3, 10, &mut pal);
        assert!(!inside[0] && !inside[4 * 10 + 4] && inside[3 * 10 + 5] && inside[9 * 10 + 9]);
        let (_, inside) = rows.window(595, 295, 10, &mut pal);
        assert!(inside[0] && !inside[5] && !inside[5 * 10]);
    }

    #[test]
    fn layer_sets_that_differ_between_cells_use_the_union() {
        let a = tmx(300, &[("Floor", 0, &[]), ("Walls2", 0, &[(299, 0, 1)])]);
        let b = tmx(300, &[("Floor", 0, &[(0, 0, 2)])]);
        let p = project("union", "70,0", 2, 1, &[(0, 0, a), (1, 0, b)]);
        let rows = TmxWorld::open(&p).unwrap().load_rows(0, 5).unwrap();
        let mut pal = Palette::default();
        let (layers, _) = rows.window(298, 0, 4, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Walls2", 1, 0, 4), "t_0");
        assert_eq!(name_at(&pal, &layers, "0_Walls2", 2, 0, 4), "", "the right cell has no Walls2");
        assert_eq!(name_at(&pal, &layers, "0_Floor", 2, 0, 4), "t_1");
    }

    #[test]
    fn unknown_gid_is_skipped_and_counted() {
        let p = project("gid", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[(1, 1, 999), (2, 2, 1)])]))]);
        let rows = TmxWorld::open(&p).unwrap().load_rows(0, 10).unwrap();
        // a tileset covers every gid from its firstgid up, so a gid is unknown only below the first one
        let mut pal = Palette::default();
        let (layers, _) = rows.window(0, 0, 5, &mut pal);
        assert_eq!(name_at(&pal, &layers, "0_Floor", 2, 2, 5), "t_0");
        let p0 = project("gid0", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[(1, 1, 5)])]).replace("firstgid=\"1\"", "firstgid=\"9\""))]);
        let rows = TmxWorld::open(&p0).unwrap().load_rows(0, 10).unwrap();
        let (layers, _) = rows.window(0, 0, 5, &mut Palette::default());
        assert_eq!(layers.get("0_Floor").unwrap().get(1, 1), 0);
        assert_eq!(rows.skipped(), 1);
    }

    #[test]
    fn wrong_origin_missing_file_wrong_size_and_negative_level() {
        let p = project("origin", "71,0", 1, 1, &[]);
        let e = TmxWorld::open(&p).err().unwrap().to_string();
        assert!(e.contains("71,0"), "{e}");
        // a named file that is missing: a warning, not an error
        let p = project("missing", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", 0, &[])]))]);
        std::fs::remove_file(p.with_file_name("c_0_0.tmx")).unwrap();
        let w = TmxWorld::open(&p).unwrap();
        assert!(w.warnings.iter().any(|m| m.contains("c_0_0.tmx")), "{:?}", w.warnings);
        assert!(!w.has_maps());
        let p = project("size", "70,0", 1, 1, &[(0, 0, tmx(299, &[("Floor", 0, &[])]))]);
        let e = TmxWorld::open(&p).unwrap().load_rows(0, 5).err().unwrap().to_string();
        assert!(e.contains("c_0_0.tmx") && e.contains("300"), "{e}");
        let p = project("neg", "70,0", 1, 1, &[(0, 0, tmx(300, &[("Floor", -1, &[])]))]);
        assert!(TmxWorld::open(&p).unwrap().load_rows(0, 5).err().unwrap().to_string().contains("level -1"));
    }
}
