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

#[test]
fn tmx_mode_matches_worlded_by_alias() {
    let dir = project("worlded");
    let out = tmp("worlded_out");
    compile_map(&dir.join("rt.pzw"), &out, &opts(false), &|_, _| {}).unwrap();
    let canon = canonical();
    let (mut squares, mut differing) = (0usize, 0usize);
    let mut first: Option<String> = None;
    for (cx, cy) in [(82, 0), (82, 1), (83, 0), (83, 1)] {
        let (ours, theirs) = (read_cell(&out, cx, cy), read_cell(&fixture().join("lots"), cx, cy));
        let mut keys: Vec<_> = ours.keys().chain(theirs.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            squares += 1;
            let (a, b) = (ours.get(&k).map(|v| v.iter().map(|t| canon(t)).collect::<Vec<_>>()), theirs.get(&k).map(|v| v.iter().map(|t| canon(t)).collect::<Vec<_>>()));
            if a != b {
                differing += 1;
                first.get_or_insert_with(|| format!("cell {cx}_{cy} square {k:?}: ours {:?}, WorldEd {:?}", ours.get(&k), theirs.get(&k)));
            }
        }
    }
    assert!(squares > 50_000, "compared {squares} squares");
    assert_eq!(differing, 0, "{differing} of {squares} squares differ from WorldEd by alias; first: {first:?}");
}
