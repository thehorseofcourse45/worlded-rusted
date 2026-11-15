//! A whole map: bitmaps + project + buildings to lot files (port of tools/map_source.py).
use crate::blend_layers::blend_layers;
use crate::blends::load_blends;
use crate::bmp::{bmp_info, read_rgb_rows};
use crate::buildtiles::compose_lots;
use crate::header::{cell_tables, placed_rooms};
use crate::rules::load_rules;
use crate::source::{cell_source, SIDE};
use crate::tbx::{parse_tbx, tile_name, Building};
use crate::terrain::{ground_layers, stack_order, Grid, Layers, Palette};
use crate::tmxground::{Generator, TmxWorld};
use crate::CompileError;
use knoxlots::compile::{compile_cell, Manifest};
use knoxlots::kcell::KCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The world's west edge in lot-file tiles (worldOrigin 70,0 of 300-tile cells), as the Python pipeline has it.
pub const ORIGIN_X: i64 = 70 * 300;
/// Tiles a building's output can reach past the lot the project gives it.
const MARGIN: i64 = 8;

pub struct Options {
    pub seed: i64,
    /// Blend each 300 x 300 map on its own, as WorldEd does.
    pub seams: bool,
    pub workers: Option<usize>,
    /// Also write the `.kcell` cell sources (and `source.txt`) here.
    pub keep_source: Option<PathBuf>,
    /// Rules.txt; Blends.txt must sit beside it.
    pub rules: PathBuf,
}

pub struct Lot {
    pub name: String,
    pub tx: i64,
    pub ty: i64,
    pub level: i64,
}

/// An error in one lot, named by its building file.
fn lot_error(project: &Path, l: &Lot, msg: String) -> CompileError {
    perr(&project.join("buildings").join(format!("{}.tbx", l.name)), msg)
}

fn perr(p: &Path, m: impl Into<String>) -> CompileError {
    CompileError::Parse(p.to_path_buf(), m.into())
}

/// `[(name, tx, ty, base level)]` in project order, with world tile positions.
pub fn read_lots(pzw: &Path) -> Result<Vec<Lot>, CompileError> {
    let text = std::fs::read_to_string(pzw).map_err(|e| CompileError::Io(pzw.to_path_buf(), e))?;
    let doc = roxmltree::Document::parse(&text).map_err(|e| perr(pzw, e.to_string()))?;
    let mut lots = Vec::new();
    for cell in doc.root_element().children().filter(|n| n.has_tag_name("cell")) {
        let num = |n: roxmltree::Node, k: &str| n.attribute(k).and_then(|v| v.parse::<u32>().ok()).map(i64::from);
        let (Some(cx), Some(cy)) = (num(cell, "x"), num(cell, "y")) else { continue };
        for l in cell.children().filter(|n| n.has_tag_name("lot")) {
            let name = l.attribute("map").and_then(|m| m.strip_prefix("buildings/")).and_then(|m| m.strip_suffix(".tbx"));
            if let (Some(name), Some(x), Some(y), Some(level)) = (name, num(l, "x"), num(l, "y"), num(l, "level")) {
                lots.push(Lot { name: name.to_string(), tx: cx * 300 + x, ty: cy * 300 + y, level });
            }
        }
    }
    Ok(lots)
}

/// One square's tiles from its ground and its building's.
fn merge_stack(ground: Vec<String>, building: &[String], has_room: bool) -> Vec<String> {
    if has_room {
        building.to_vec()
    } else {
        ground.into_iter().chain(building.iter().cloned()).collect()
    }
}

struct Shared<'a> {
    project: &'a Path,
    /// Set in `.pzw` mode: the ground comes from the cells' `.tmx` maps, not the bitmaps.
    tmx: Option<TmxWorld>,
    /// Squares whose tile id no tileset covered (TMX mode).
    skipped: AtomicUsize,
    name: String,
    opts: &'a Options,
    size: (i64, i64),
    lots: Vec<Lot>,
    buildings: Vec<Building>,
    placed: Vec<(i64, i64, Vec<crate::header::Room>)>,
    out: &'a Path,
    manifest: Manifest,
    spawn: Option<knoxlots::bmp::GreyImage>,
}

/// Compile the map in `project` into lot files in `out`. Returns the number of cells written.
/// `progress(done_rows, rows)` is called after each row of cells.
pub fn compile_map(input: &Path, out: &Path, opts: &Options, progress: &(dyn Fn(usize, usize) + Sync)) -> Result<usize, CompileError> {
    // A path ending in .pzw compiles from the cells' .tmx maps; a folder from the bitmaps.
    let tmx_mode = input.extension().is_some_and(|e| e.eq_ignore_ascii_case("pzw"));
    let (folder, name, pzw) = if tmx_mode {
        let folder = input.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_path_buf();
        (folder, input.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), input.to_path_buf())
    } else {
        let name = input.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        (input.to_path_buf(), name.clone(), input.join(format!("{name}.pzw")))
    };
    let project: &Path = &folder;
    let (tmx, size) = if tmx_mode {
        let t = TmxWorld::open(&pzw)?;
        if !t.has_maps() {
            return Err(perr(&pzw, "no cell has a .tmx file"));
        }
        for w in &t.warnings {
            eprintln!("warning: {w}");
        }
        let size = t.size_tiles();
        (Some(t), size)
    } else {
        let land = project.join(format!("{name}.bmp"));
        let info = bmp_info(&land)?;
        let veg = project.join(format!("{name}_veg.bmp"));
        let vi = bmp_info(&veg)?;
        if vi.width < info.width || vi.height < info.height {
            return Err(perr(&veg, "the vegetation picture is smaller than the landscape picture"));
        }
        (None, (info.width as i64, info.height as i64))
    };
    let lots = read_lots(&pzw)?;
    let mut buildings = Vec::new();
    for l in &lots {
        let p = project.join("buildings").join(format!("{}.tbx", l.name));
        let text = std::fs::read(&p).map_err(|e| CompileError::Io(p.clone(), e))?;
        buildings.push(parse_tbx(&String::from_utf8_lossy(&text)).map_err(|m| perr(&p, m))?);
    }
    // Room tables need every building at once (they touch, and a later lot takes squares).
    let with_floors: Vec<(&Building, i64, i64, i64)> = lots.iter().zip(&buildings).filter(|(_, b)| !b.floors.is_empty()).map(|(l, b)| (b, l.tx, l.ty, l.level)).collect();
    let floor_lots: Vec<usize> = lots.iter().zip(&buildings).enumerate().filter(|(_, (_, b))| !b.floors.is_empty()).map(|(i, _)| i).collect();
    let placed = placed_rooms(&with_floors).map_err(|e| lot_error(project, &lots[floor_lots[e.lot]], e.msg))?;

    let spawn_path = project.join(format!("{name}_ZombieSpawnMap.bmp"));
    let (manifest, spawn) = if spawn_path.exists() {
        let bytes = std::fs::read(&spawn_path).map_err(|e| CompileError::Io(spawn_path.clone(), e))?;
        (Manifest { origin_x: ORIGIN_X, origin_y: 0, spawn_map: Some("spawn.bmp".into()), ..Manifest::default() }, Some(knoxlots::bmp::read_grey(&bytes).map_err(|m| perr(&spawn_path, m))?))
    } else {
        (Manifest { origin_x: ORIGIN_X, origin_y: 0, ..Manifest::default() }, None)
    };
    std::fs::create_dir_all(out).map_err(|e| CompileError::Io(out.to_path_buf(), e))?;
    if let Some(dir) = &opts.keep_source {
        std::fs::create_dir_all(dir).map_err(|e| CompileError::Io(dir.clone(), e))?;
        let mut text = format!("format = knoxlots-source 1\norigin_x = {ORIGIN_X}\norigin_y = 0\n");
        if spawn_path.exists() {
            std::fs::copy(&spawn_path, dir.join("spawn.bmp")).map_err(|e| CompileError::Io(spawn_path.clone(), e))?;
            text += "spawn_map = spawn.bmp\n";
        }
        std::fs::write(dir.join("source.txt"), text).map_err(|e| CompileError::Io(dir.join("source.txt"), e))?;
    }

    let shared = Shared { project, tmx, skipped: AtomicUsize::new(0), name, opts, size, lots, buildings, placed, out, manifest, spawn };
    let rows = (shared.size.1 + SIDE as i64 - 1) / SIDE as i64;
    let workers = opts.workers.unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get())).clamp(1, rows.max(1) as usize);
    let (next, done_rows, written) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
    let first_error: Mutex<Option<CompileError>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let cy = next.fetch_add(1, Ordering::SeqCst) as i64;
                if cy >= rows || first_error.lock().unwrap().is_some() {
                    break;
                }
                match do_row(&shared, cy) {
                    Ok(n) => {
                        written.fetch_add(n, Ordering::SeqCst);
                        progress(done_rows.fetch_add(1, Ordering::SeqCst) + 1, rows as usize);
                    }
                    Err(e) => {
                        first_error.lock().unwrap().get_or_insert(e);
                        break;
                    }
                }
            });
        }
    });
    if let Some(e) = first_error.into_inner().unwrap() {
        return Err(e);
    }
    let skipped = shared.skipped.load(Ordering::SeqCst);
    if skipped > 0 {
        eprintln!("warning: {skipped} squares use a tile id that no tileset of their map covers; left empty");
    }
    Ok(written.load(Ordering::SeqCst))
}

/// Make and compile the cells of one row of cells.
fn do_row(s: &Shared, cy: i64) -> Result<usize, CompileError> {
    let side = SIDE as i64;
    let (w, h) = s.size;
    let (lo, hi) = (cy * side - MARGIN, cy * side + side + MARGIN);
    // the lots that reach this row, in project order: a square's tiles depend only on the lots covering it
    let band_idx: Vec<usize> = (0..s.lots.len()).filter(|&i| s.lots[i].ty - MARGIN < hi && s.lots[i].ty + s.buildings[i].height + MARGIN > lo).collect();
    let band: Vec<(&Building, i64, i64, i64)> = band_idx.iter().map(|&i| (&s.buildings[i], s.lots[i].tx, s.lots[i].ty, s.lots[i].level)).collect();
    let world = compose_lots(&band).map_err(|e| lot_error(s.project, &s.lots[band_idx[e.lot]], e.msg))?;
    let mut room_squares: HashSet<(i64, i64, i64)> = HashSet::new();
    for (b, tx, ty, base) in &band {
        for (z, floor) in b.floors.iter().enumerate() {
            for (y, row) in floor.rooms.iter().enumerate() {
                for (x, &v) in row.iter().enumerate() {
                    if v > 0 {
                        room_squares.insert((tx + x as i64, ty + y as i64, base + z as i64));
                    }
                }
            }
        }
    }
    let rules = load_rules(&s.opts.rules)?;
    let blends = load_blends(&s.opts.rules.with_file_name("Blends.txt"))?;
    let mut pal = Palette::default();
    let wy0 = cy * side;
    let (y_lo, y_hi) = ((wy0 - 1).max(0), (wy0 + side + 1).min(h));
    let strips = if s.tmx.is_none() {
        let land_path = s.project.join(format!("{}.bmp", s.name));
        let veg_path = s.project.join(format!("{}_veg.bmp", s.name));
        Some((read_rgb_rows(&land_path, y_lo, y_hi, Some(w as u32))?, read_rgb_rows(&veg_path, y_lo, y_hi, Some(w as u32))?))
    } else {
        None
    };
    let rows = s.tmx.as_ref().map(|t| t.load_rows(wy0 - 1, wy0 + side + 1, &Generator { rules: &rules, blends: &blends })).transpose()?;
    let win = SIDE + 2;
    let mut written = 0;
    for cx in ORIGIN_X / side..=(ORIGIN_X + w - 1) / side {
        let wx0 = cx * side - ORIGIN_X;
        let (layers, inside) = if let Some(rows) = &rows {
            // the editor already chose and blended every tile: take them as saved
            rows.window(wx0 - 1, wy0 - 1, win, &mut pal)
        } else {
            let (land_strip, veg_strip) = strips.as_ref().expect("bitmap mode reads strips");
            let (mut lw, mut vg) = (vec![0u8; win * win * 3], vec![0u8; win * win * 3]);
            let mut inside = vec![false; win * win];
            let (x_lo, x_hi) = ((wx0 - 1).max(0), (wx0 + side + 1).min(w));
            if x_lo < x_hi && y_lo < y_hi {
                let (sx, sy) = ((x_lo - (wx0 - 1)) as usize, (y_lo - (wy0 - 1)) as usize);
                for r in 0..(y_hi - y_lo) as usize {
                    for c in 0..(x_hi - x_lo) as usize {
                        let src = (r * land_strip.cols + x_lo as usize + c) * 3;
                        let dst = ((sy + r) * win + sx + c) * 3;
                        lw[dst..dst + 3].copy_from_slice(&land_strip.rgb[src..src + 3]);
                        vg[dst..dst + 3].copy_from_slice(&veg_strip.rgb[src..src + 3]);
                        inside[(sy + r) * win + sx + c] = true;
                    }
                }
            }
            let mut layers = ground_layers(&lw, &vg, win, win, &rules, &mut pal, s.opts.seed, wx0 - 1, wy0 - 1);
            for (_, g) in layers.0.iter_mut() {
                for (i, v) in g.data.iter_mut().enumerate() {
                    if !inside[i] {
                        *v = 0;
                    }
                }
            }
            if let Some(floor) = layers.get("0_Floor").cloned() {
                let b = blend_layers(&floor, &rules, &blends, &mut pal, s.opts.seed, wx0 - 1, wy0 - 1, if s.opts.seams { Some(300) } else { None }).map_err(|e| perr(s.project, e.to_string()))?;
                layers.update(b);
            }
            (layers, inside)
        };
        let cropped: Layers = Layers(
            layers
                .0
                .iter()
                .map(|(k, g)| {
                    let mut c = Grid::new(SIDE, SIDE);
                    for y in 0..SIDE {
                        for x in 0..SIDE {
                            c.set(x, y, g.get(x + 1, y + 1));
                        }
                    }
                    (k.clone(), c)
                })
                .collect(),
        );
        let names: Vec<String> = cropped.0.iter().map(|(k, _)| k.clone()).collect();
        // TMX layers are already in saving order (level, then file order); bitmap layers use the fixed table
        let order = if rows.is_some() { names.clone() } else { stack_order(&names) };

        // the stacks of the cell: level 0 is ground and buildings, above it buildings only
        let mut stacks: Vec<((usize, usize, usize), Vec<String>)> = Vec::new();
        for yy in 0..SIDE {
            for xx in 0..SIDE {
                let (wx, wy) = (wx0 + xx as i64, wy0 + yy as i64);
                let bt: &[String] = world.get(&(wx, wy, 0)).map(Vec::as_slice).unwrap_or(&[]);
                let mut g: Vec<String> = Vec::new();
                if inside[(yy + 1) * win + xx + 1] {
                    // a fence clears the ground's furniture (a garden bed under it); walls do not
                    let fenced = bt.iter().any(|t| t.starts_with("fencing_"));
                    for k in &order {
                        let v = cropped.get(k).unwrap().get(xx, yy);
                        if v != 0 && !(fenced && k == "0_Furniture") {
                            g.push(tile_name(&pal.names[v as usize]));
                        }
                    }
                }
                let st = merge_stack(g, bt, room_squares.contains(&(wx, wy, 0)));
                if !st.is_empty() {
                    stacks.push(((xx, yy, 0), st));
                }
            }
        }
        for ((wx, wy, z), bt) in world.range((wx0, i64::MIN, i64::MIN)..(wx0 + side, i64::MIN, i64::MIN)) {
            if *z > 0 && wy0 <= *wy && *wy < wy0 + side {
                stacks.push((((wx - wx0) as usize, (wy - wy0) as usize, *z as usize), bt.clone()));
            }
        }
        let nlev = stacks.iter().map(|((_, _, z), _)| *z).max().unwrap_or(0) + 1;
        if nlev > 64 {
            return Err(perr(&s.project.join(format!("{}.pzw", s.name)), format!("cell {cx}_{cy} would need {nlev} levels; the most a lot file holds is 64 (check the lot level attributes)")));
        }
        let mut tile_names: Vec<String> = Vec::new();
        let mut index: BTreeMap<&str, u32> = BTreeMap::new();
        let mut levels: Vec<Vec<Vec<u32>>> = vec![vec![Vec::new(); SIDE * SIDE]; nlev];
        for ((xx, yy, z), st) in &stacks {
            for t in st {
                let id = match index.get(t.as_str()) {
                    Some(&i) => i,
                    None => {
                        tile_names.push(t.clone());
                        let i = tile_names.len() as u32;
                        index.insert(t.as_str(), i);
                        i
                    }
                };
                levels[*z][yy * SIDE + xx].push(id);
            }
        }
        let (rooms, buildings) = cell_tables(cx, cy, &s.placed);
        let bytes = cell_source(cx, cy, &tile_names, &levels, &rooms, &buildings).map_err(|m| perr(s.project, m))?;
        if let Some(dir) = &s.opts.keep_source {
            let p = dir.join(format!("cell_{cx}_{cy}.kcell"));
            std::fs::write(&p, &bytes).map_err(|e| CompileError::Io(p, e))?;
        }
        let k = KCell::parse(&bytes).map_err(|m| perr(s.project, format!("cell {cx}_{cy}: {m}")))?;
        let f = compile_cell(&k, &s.manifest, s.spawn.as_ref());
        for (file, data) in [(format!("{cx}_{cy}.lotheader"), f.header), (format!("world_{cx}_{cy}.lotpack"), f.pack), (format!("chunkdata_{cx}_{cy}.bin"), f.chunkdata)] {
            let p = s.out.join(file);
            std::fs::write(&p, data).map_err(|e| CompileError::Io(p, e))?;
        }
        written += 1;
    }
    if let Some(rows) = &rows {
        s.skipped.fetch_add(rows.skipped(), Ordering::SeqCst);
    }
    Ok(written)
}
