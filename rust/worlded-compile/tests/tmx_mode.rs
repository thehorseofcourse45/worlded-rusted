mod common;
use knoxlots::cell::Cell;
use knoxlots::header::Header;
use knoxlots::pack::Pack;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use worlded_compile::pipeline::{compile_map, Options};
use worlded_compile::rules::load_rules;
use worlded_core::tmx::{parse_bmp_images, BmpImage};

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("wcomp_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn rules_path() -> PathBuf {
    common::repo().join("config/Rules.txt")
}

fn opts(seams: bool) -> Options {
    Options { seed: 1, seams, workers: Some(2), keep_source: None, rules: rules_path() }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tmx_oracle")
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

/// A 24-bit BMP of an embedded picture.
fn write_bmp(img: &BmpImage, path: &Path) {
    let (w, h) = (img.width as usize, img.height as usize);
    let row = (w * 3 + 3) / 4 * 4;
    let mut out = b"BM".to_vec();
    out.extend_from_slice(&((54 + row * h) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as u32).to_le_bytes());
    out.extend_from_slice(&(h as u32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        let mut line = Vec::with_capacity(row);
        for x in 0..w {
            let p = img.pixels[y * w + x] as usize;
            let c = if p == 0 { [0, 0, 0] } else { img.colors[p - 1] };
            line.extend_from_slice(&[c[2], c[1], c[0]]);
        }
        line.resize(row, 0);
        out.extend_from_slice(&line);
    }
    std::fs::write(path, out).unwrap();
}

/// The oracle project as a folder both modes can read: the `.tmx`, a `.pzw`, the spawn map and the two
/// embedded pictures written out as the bitmaps bitmap mode wants.
fn project(tag: &str) -> PathBuf {
    let dir = tmp(tag).join("rt");
    std::fs::create_dir_all(dir.join("tmx")).unwrap();
    for f in ["rt.pzw", "rt_ZombieSpawnMap.bmp"] {
        std::fs::copy(fixture().join(f), dir.join(f)).unwrap();
    }
    std::fs::copy(fixture().join("tmx/rt_70_0.tmx"), dir.join("tmx/rt_70_0.tmx")).unwrap();
    let imgs = parse_bmp_images(&std::fs::read_to_string(dir.join("tmx/rt_70_0.tmx")).unwrap()).unwrap();
    write_bmp(&imgs[0], &dir.join("rt.bmp"));
    write_bmp(&imgs[1], &dir.join("rt_veg.bmp"));
    dir
}

#[test]
fn tmx_mode_equals_bitmap_mode_byte_for_byte() {
    let dir = project("modes");
    let (a, b) = (tmp("modes_bitmap"), tmp("modes_tmx"));
    assert_eq!(compile_map(&dir, &a, &opts(true), &|_, _| {}).unwrap(), 4);
    assert_eq!(compile_map(&dir.join("rt.pzw"), &b, &opts(false), &|_, _| {}).unwrap(), 4);
    assert_eq!(files(&a), files(&b));
    for f in files(&a) {
        let (x, y) = (std::fs::read(a.join(&f)).unwrap(), std::fs::read(b.join(&f)).unwrap());
        if x != y {
            let at = x.iter().zip(&y).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
            panic!("{f}: bitmap mode {} bytes, TMX mode {} bytes, first difference at byte {at}", x.len(), y.len());
        }
    }
}

/// Every tile of a lot cell by square: `(x, y, z) -> tile names bottom first`.
fn read_cell(lots: &Path, cx: i64, cy: i64) -> HashMap<(usize, usize, usize), Vec<String>> {
    let h = Header::parse(&std::fs::read(lots.join(format!("{cx}_{cy}.lotheader"))).unwrap()).unwrap();
    let pack = Pack::parse(&std::fs::read(lots.join(format!("world_{cx}_{cy}.lotpack"))).unwrap()).unwrap();
    let cell = Cell::from_pack(&h.tiles, &pack, (h.max_z - h.min_z + 1) as usize).unwrap();
    let mut out = HashMap::new();
    for z in 0..cell.levels {
        for x in 0..256 {
            for y in 0..256 {
                if let Some(sq) = cell.get(z, x, y) {
                    out.insert((x, y, z), sq.tiles.iter().map(|&t| cell.names[t as usize].clone()).collect());
                }
            }
        }
    }
    out
}

/// A tile name's group: its alias, else the layer a rule puts it on, else itself (as `tools/cell_check.py`).
fn canonical() -> impl Fn(&str) -> String {
    let rules = load_rules(&rules_path()).unwrap();
    let mut alias_of: HashMap<String, String> = HashMap::new();
    let mut names: Vec<&String> = rules.aliases.keys().collect();
    names.sort();
    for n in names {
        for t in &rules.aliases[n] {
            alias_of.entry(t.clone()).or_insert_with(|| format!("@{n}"));
        }
    }
    let mut layer_of: HashMap<String, String> = HashMap::new();
    for r in &rules.rules {
        for e in &r.entries {
            if !rules.aliases.contains_key(e) {
                layer_of.entry(e.clone()).or_insert_with(|| format!("#{}", r.layer));
            }
        }
    }
    let base = move |t: &str| alias_of.get(t).or_else(|| layer_of.get(t)).cloned().unwrap_or_else(|| t.to_string());
    // A rule that picks one of several entries at random (trees, flowers, grass of one rule) gives a
    // different tile in every run: its groups count as one, since WorldEd's random numbers are not ours.
    let mut parent: HashMap<String, String> = HashMap::new();
    fn find(parent: &mut HashMap<String, String>, x: &str) -> String {
        let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
        if p == x {
            return p;
        }
        let root = find(parent, &p);
        parent.insert(x.to_string(), root.clone());
        root
    }
    for r in &rules.rules {
        let groups: Vec<String> = r.entries.iter().map(|e| if rules.aliases.contains_key(e) { format!("@{e}") } else { base(e) }).collect();
        for g in groups.iter().skip(1) {
            let (a, b) = (find(&mut parent, &groups[0]), find(&mut parent, g));
            if a != b {
                parent.insert(b, a);
            }
        }
    }
    let parent = std::cell::RefCell::new(parent);
    move |t: &str| find(&mut parent.borrow_mut(), &base(t))
}

/// Squares compared, squares that differ by alias, and the first difference.
fn compare_by_alias(ours: &Path, worlded: &Path, cells: &[(i64, i64)]) -> (usize, usize, Option<String>) {
    let canon = canonical();
    let (mut squares, mut differing, mut first) = (0usize, 0usize, None);
    for &(cx, cy) in cells {
        let (a, b) = (read_cell(ours, cx, cy), read_cell(worlded, cx, cy));
        let mut keys: Vec<_> = a.keys().chain(b.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            squares += 1;
            let (x, y) = (a.get(&k).map(|v| v.iter().map(|t| canon(t)).collect::<Vec<_>>()), b.get(&k).map(|v| v.iter().map(|t| canon(t)).collect::<Vec<_>>()));
            if x != y {
                differing += 1;
                first.get_or_insert_with(|| format!("cell {cx}_{cy} square {k:?}: ours {:?}, WorldEd {:?}", a.get(&k), b.get(&k)));
            }
        }
    }
    (squares, differing, first)
}

#[test]
fn tmx_mode_matches_worlded_by_alias() {
    let dir = project("worlded");
    let out = tmp("worlded_out");
    compile_map(&dir.join("rt.pzw"), &out, &opts(false), &|_, _| {}).unwrap();
    let (squares, differing, first) = compare_by_alias(&out, &fixture().join("lots"), &[(82, 0), (82, 1), (83, 0), (83, 1)]);
    assert!(squares > 50_000, "compared {squares} squares");
    assert_eq!(differing, 0, "{differing} of {squares} squares differ from WorldEd by alias; first: {first:?}");
}

/// Opt-in (about two minutes): WorldEd converts a generated 600 x 600 map with the real1 buildings
/// and generates lots; TMX mode must equal them by alias. `WORLDED_ACCEPTANCE=1`, the WorldEd command
/// line (or `PZWORLDED_CLI`) and Python with Pillow and numpy are needed.
#[test]
fn tmx_mode_matches_worlded_with_buildings() {
    let exe = std::env::var("PZWORLDED_CLI").unwrap_or_else(|_| r"C:\KnoxMap\vendor\PZMappingTools\bin\PZWorldEd_cli.exe".to_string());
    if std::env::var("WORLDED_ACCEPTANCE").is_err() || !Path::new(&exe).exists() || std::process::Command::new("python").arg("--version").output().is_err() {
        eprintln!("skipped: set WORLDED_ACCEPTANCE=1 with the WorldEd CLI and Python available");
        return;
    }
    let dir = tmp("oracle_buildings");
    let tools = common::repo().join("tools");
    let run = |script: &str, args: &[&str]| {
        let out = std::process::Command::new("python").arg(tools.join(script)).args(args).arg("--exe").arg(&exe).output().unwrap();
        (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
    };
    // the first WorldEd run ends with an error about the spawn map but leaves the converted maps
    let _ = run("make_real_tmx.py", &[dir.to_str().unwrap(), "--size", "600", "--lots"]);
    let project = dir.join("rt");
    assert!(project.join("tmx").read_dir().unwrap().count() >= 4, "WorldEd made the maps");
    let (ok, log) = run("worlded_lots.py", &[project.to_str().unwrap()]);
    assert!(ok, "WorldEd lot generation: {log}");
    let out = tmp("oracle_buildings_out");
    compile_map(&project.join("rt.pzw"), &out, &opts(false), &|_, _| {}).unwrap();
    let mut cells: Vec<(i64, i64)> = std::fs::read_dir(project.join("lots"))
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().to_string_lossy().to_string();
            let (a, b) = n.strip_suffix(".lotheader")?.split_once('_')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect();
    cells.sort();
    // TMX mode equals bitmap mode on the same project (the pictures WorldEd embedded are the bitmaps), byte for byte
    let bitmap_out = tmp("oracle_buildings_bitmap");
    compile_map(&project, &bitmap_out, &opts(true), &|_, _| {}).unwrap();
    assert_eq!(files(&out), files(&bitmap_out));
    for f in files(&out) {
        assert!(std::fs::read(out.join(&f)).unwrap() == std::fs::read(bitmap_out.join(&f)).unwrap(), "{f}: TMX mode differs from bitmap mode");
    }
    // and WorldEd by alias. 1,776 of 422,696 squares (0.42%) differ on this map: building wall tiles
    // (for example walls_exterior_house_08 where WorldEd has _02). That is in the shared building
    // code of stage 2, not in TMX mode (both modes give the same bytes); the ceiling stops it growing.
    let (squares, differing, first) = compare_by_alias(&out, &project.join("lots"), &cells);
    eprintln!("compared {squares} squares in {} cells, {differing} differ from WorldEd by alias; first: {first:?}", cells.len());
    assert!(differing * 200 < squares, "{differing} of {squares} squares differ from WorldEd by alias; first: {first:?}");
}

// ---- synthetic projects -------------------------------------------------------------------

/// A 300 x 300 map: tileset `t` (gid 1 is `t_0`) and csv layers `(name, level, tiles)`, no pictures.
fn tmx_text(layers: &[(&str, i32, &[(usize, usize, u32)])]) -> String {
    let mut s = String::from("<map width=\"300\" height=\"300\"><tileset firstgid=\"1\" name=\"t\"/>");
    for (name, level, tiles) in layers {
        let mut g = vec![0u32; 300 * 300];
        for &(x, y, gid) in *tiles {
            g[y * 300 + x] = gid;
        }
        let body: Vec<String> = g.iter().map(u32::to_string).collect();
        s += &format!("<layer name=\"{name}\" level=\"{level}\"><data encoding=\"csv\">{}</data></layer>", body.join(","));
    }
    s + "</map>"
}

/// A project folder `p` with `<name>.pzw` (cells `cells_x` x `cells_y`, one `.tmx` per `(x, y, text)`, plus `lots_xml`).
fn synth(tag: &str, cells_x: u32, cells_y: u32, maps: &[(u32, u32, String)], lots_xml: &str) -> PathBuf {
    let dir = tmp(tag).join("p");
    std::fs::create_dir_all(&dir).unwrap();
    let mut pzw = format!("<?xml version=\"1.0\"?>\n<world version=\"1.0\" width=\"{cells_x}\" height=\"{cells_y}\">\n <GenerateLots><worldOrigin origin=\"70,0\"/></GenerateLots>\n");
    for y in 0..cells_y {
        for x in 0..cells_x {
            let file = maps.iter().find(|m| m.0 == x && m.1 == y).map(|(_, _, t)| {
                let f = format!("c_{x}_{y}.tmx");
                std::fs::write(dir.join(&f), t).unwrap();
                f
            });
            pzw += &format!(" <cell x=\"{x}\" y=\"{y}\" map=\"{}\">\n", file.unwrap_or_default());
            if x == 0 && y == 0 {
                pzw += lots_xml;
            }
            pzw += " </cell>\n";
        }
    }
    pzw += "</world>\n";
    std::fs::write(dir.join("p.pzw"), pzw).unwrap();
    dir.join("p.pzw")
}

/// Compile in TMX mode keeping the cell sources; returns the output folder of the `.kcell` files.
fn compile_keep(pzw: &Path) -> Result<PathBuf, String> {
    let base = pzw.parent().unwrap().parent().unwrap(); // the project's own temp folder: tests run in parallel
    let (out, src) = (base.join("out"), base.join("src"));
    let o = Options { keep_source: Some(src.clone()), ..opts(false) };
    compile_map(pzw, &out, &o, &|_, _| {}).map_err(|e| e.to_string())?;
    Ok(src)
}

fn kcell(src: &Path, cx: i64, cy: i64) -> knoxlots::kcell::KCell {
    knoxlots::kcell::KCell::parse(&std::fs::read(src.join(format!("cell_{cx}_{cy}.kcell"))).unwrap()).unwrap()
}

/// Tile names of one square, bottom first (empty when there is none).
fn stack(k: &knoxlots::kcell::KCell, z: usize, x: usize, y: usize) -> Vec<String> {
    if z >= k.squares.levels {
        return Vec::new();
    }
    k.squares.get(z, x, y).map(|s| s.tiles.iter().map(|&t| k.squares.names[t as usize].clone()).collect()).unwrap_or_default()
}

#[test]
fn tmx_layer_at_level_2_lands_on_level_2() {
    let pzw = synth("lv2", 1, 1, &[(0, 0, tmx_text(&[("Floor", 0, &[]), ("Walls", 2, &[(5, 5, 2)])]))], "");
    let k = kcell(&compile_keep(&pzw).unwrap(), 82, 0);
    // map x 5 is world 21005: lot cell 82, column 13
    assert_eq!(stack(&k, 2, 13, 5), vec!["t_1"]);
    assert!(stack(&k, 1, 13, 5).is_empty() && stack(&k, 0, 13, 5).is_empty());
}

#[test]
fn more_than_64_levels_is_still_an_error() {
    let pzw = synth("lv70", 1, 1, &[(0, 0, tmx_text(&[("Walls", 70, &[(5, 5, 2)])]))], "");
    let e = compile_keep(&pzw).unwrap_err();
    assert!(e.contains("levels"), "{e}");
}

#[test]
fn tiles_across_the_cell_seam_and_the_lot_cell_edge_land_in_the_right_places() {
    // two 300-tile cells side by side: map x 247/248 straddle the lot cells 82/83, x 299/300 the cell seam
    let a = tmx_text(&[("Floor", 0, &[(247, 7, 2), (248, 7, 3), (299, 8, 4)])]);
    let b = tmx_text(&[("Floor", 0, &[(0, 8, 5)])]);
    let pzw = synth("seam", 2, 1, &[(0, 0, a), (1, 0, b)], "");
    let src = compile_keep(&pzw).unwrap();
    let (k82, k83) = (kcell(&src, 82, 0), kcell(&src, 83, 0));
    assert_eq!(stack(&k82, 0, 255, 7), vec!["t_1"]);
    assert_eq!(stack(&k83, 0, 0, 7), vec!["t_2"]);
    assert_eq!(stack(&k83, 0, 51, 8), vec!["t_3"]);
    assert_eq!(stack(&k83, 0, 52, 8), vec!["t_4"]);
}

#[test]
fn room_squares_keep_building_tiles_only_and_other_building_squares_keep_ground_too() {
    // the real1 fixture's buildings in cell 0,0, with a map that has ground (t_0) on every square
    let real1 = common::repo().join("rust/knoxlots/fixtures/real/real1");
    let text = std::fs::read_to_string(real1.join("real1.pzw")).unwrap();
    let lots: String = text.lines().filter(|l| l.trim_start().starts_with("<lot ")).take_while(|_| true).map(|l| format!("{l}\n")).collect();
    let all: Vec<(usize, usize, u32)> = (0..300).flat_map(|y| (0..300).map(move |x| (x, y, 1u32))).collect();
    let pzw = synth("rooms", 1, 1, &[(0, 0, tmx_text(&[("Floor", 0, &all)]))], &lots);
    let dir = pzw.parent().unwrap().to_path_buf();
    copy_dir(&real1.join("buildings"), &dir.join("buildings"));
    let with = kcell(&compile_keep(&pzw).unwrap(), 82, 0);
    // the same project with an empty map: what the buildings alone put on each square
    let bare = synth("rooms_bare", 1, 1, &[(0, 0, tmx_text(&[("Floor", 0, &[])]))], &lots);
    copy_dir(&real1.join("buildings"), &bare.parent().unwrap().join("buildings"));
    let none = kcell(&compile_keep(&bare).unwrap(), 82, 0);
    let (mut rooms, mut open) = (0, 0);
    for x in 0..256usize {
        for y in 0..256usize {
            let (mx, my) = (x as i64 + 82 * 256 - 21000, y);
            if !(0..300).contains(&mx) {
                continue;
            }
            let (a, b) = (stack(&with, 0, x, y), stack(&none, 0, x, y));
            if b.is_empty() {
                assert_eq!(a, vec!["t_0"], "ground alone at ({mx},{my})");
            } else if a == b {
                rooms += 1; // building tiles only: the ground was dropped
            } else {
                assert_eq!(a[0], "t_0", "ground goes first at ({mx},{my})");
                assert_eq!(a[1..], b[..], "then the building tiles");
                open += 1;
            }
        }
    }
    assert!(rooms > 100 && open > 10, "rooms {rooms}, open building squares {open}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

// ---- editing ----------------------------------------------------------------------------------

#[test]
fn edit_changes_only_where_worlded_lets_it() {
    use worlded_core::gids::gid_for;
    use worlded_core::tmxdoc::open;
    let dir = project("edit");
    let tmx_path = dir.join("tmx/rt_70_0.tmx");
    let mut doc = open(std::fs::read_to_string(&tmx_path).unwrap()).unwrap();
    let tile = gid_for(&mut doc, "Fire_01_3", 8).unwrap();
    let furniture = doc.tmx.layers.iter().position(|l| l.name == "Furniture").unwrap();
    // map (100,100) is lot cell 82 column 108; map (110,100) column 118
    assert!(doc.set_gid(furniture, 100, 100, tile));
    assert!(doc.set_gid(0, 110, 100, tile)); // Floor: the picture gives ground on every square
    let before = compile_keep_dir(&dir.join("rt.pzw"), "edit_before");
    std::fs::write(&tmx_path, doc.to_text().unwrap()).unwrap();
    let after = compile_keep_dir(&dir.join("rt.pzw"), "edit_after");

    let (a, b) = (kcell(&before, 82, 0), kcell(&after, 82, 0));
    let mut changed = Vec::new();
    for x in 0..256 {
        for y in 0..256 {
            if stack(&a, 0, x, y) != stack(&b, 0, x, y) {
                changed.push((x, y));
            }
        }
    }
    assert_eq!(changed, vec![(108, 100)], "only the Furniture square changes; the Floor edit is overwritten by the picture");
    let (old, new) = (stack(&a, 0, 108, 100), stack(&b, 0, 108, 100));
    assert!(new.contains(&"Fire_01_3".to_string()));
    let mut without = new.clone();
    without.retain(|t| t != "Fire_01_3");
    assert_eq!(without, old, "the rest of the stack is as before");
}

fn compile_keep_dir(pzw: &Path, tag: &str) -> PathBuf {
    let (out, src) = (tmp(&format!("{tag}_out")), tmp(&format!("{tag}_src")));
    let o = Options { keep_source: Some(src.clone()), ..opts(false) };
    compile_map(pzw, &out, &o, &|_, _| {}).unwrap();
    src
}
