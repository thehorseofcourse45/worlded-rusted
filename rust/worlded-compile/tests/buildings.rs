mod common;
use std::path::PathBuf;
use worlded_compile::buildtiles::{compose_lots, level_layers, window_variant};
use worlded_compile::tbx::{parse_tbx, tile_name, Building};

fn real1() -> PathBuf {
    common::repo().join("rust/knoxlots/fixtures/real/real1")
}

fn tbx_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(real1().join("buildings")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "tbx")).collect();
    v.sort();
    v
}

/// The Python dump format for one building.
fn dump(path: &std::path::Path, b: &Building) -> String {
    let mut out = format!("F|{}\n", path.file_name().unwrap().to_string_lossy());
    for z in 0..b.floors.len() {
        out += &format!("Z|{z}\n");
        for ((x, y), items) in level_layers(b, z).unwrap() {
            out += &format!("{x} {y}|{}\n", items.iter().map(|(l, t)| format!("{l}={t}")).collect::<Vec<_>>().join(";"));
        }
    }
    out
}

#[test]
fn tile_names_lose_their_padding() {
    assert_eq!(tile_name("walls_exterior_house_02_064"), "walls_exterior_house_02_64");
    assert_eq!(tile_name("a_000"), "a_0");
    assert_eq!(tile_name("no_digits_x"), "no_digits_x");
    assert_eq!(tile_name("trailing_"), "trailing_");
}

#[test]
fn window_variants() {
    assert_eq!(window_variant("fixtures_windows_white_16"), 9);
    assert_eq!(window_variant("fixtures_windows_white_17"), 9);
    assert_eq!(window_variant("fixtures_windows_wood_10"), 6);
    assert_eq!(window_variant("fixtures_windows_01_40"), 17);
    assert_eq!(window_variant("fixtures_windows_01_41"), 18);
    assert_eq!(window_variant("fixtures_windows_01_99"), 0);
    assert_eq!(window_variant("something_else_3"), 0);
}

#[test]
fn damaged_tbx_is_an_error_with_a_message() {
    assert!(parse_tbx("<building").is_err());
    assert!(parse_tbx(r#"<building width="2" height="2"><floor><rooms>1,1,1</rooms></floor></building>"#).unwrap_err().contains("room numbers"));
    assert!(parse_tbx(r#"<building height="2"/>"#).is_err());
}

#[test]
fn all_real1_buildings_equal_python() {
    let files = tbx_files();
    assert!(files.len() > 100);
    let paths: Vec<String> = files.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let mut a = vec!["tiles"];
    a.extend(paths.iter().map(String::as_str));
    let Some(want) = common::python(&a) else { return };
    let mut got = String::new();
    for f in &files {
        let b = parse_tbx(&std::fs::read_to_string(f).unwrap()).unwrap();
        got += &dump(f, &b);
    }
    let want = String::from_utf8(want).unwrap();
    assert!(want.lines().count() > 10_000, "the oracle output should be large");
    if got != want {
        let (g, w): (Vec<&str>, Vec<&str>) = (got.lines().collect(), want.lines().collect());
        let i = (0..g.len().min(w.len())).find(|&i| g[i] != w[i]).unwrap_or(g.len().min(w.len()));
        panic!("first difference at line {i}:\nrust:   {:?}\npython: {:?}", g.get(i), w.get(i));
    }
}

#[test]
fn compose_lots_equal_python_on_real1() {
    // the lots of the first cell of the project, in project order
    let text = std::fs::read_to_string(real1().join("real1.pzw")).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut lots: Vec<(PathBuf, i64, i64, i64)> = Vec::new();
    for cell in doc.root_element().children().filter(|n| n.has_tag_name("cell")) {
        for l in cell.children().filter(|n| n.has_tag_name("lot")) {
            let f = |k: &str| l.attribute(k).unwrap().parse::<i64>().unwrap();
            let (cx, cy) = (cell.attribute("x").unwrap().parse::<i64>().unwrap(), cell.attribute("y").unwrap().parse::<i64>().unwrap());
            lots.push((real1().join(l.attribute("map").unwrap()), cx * 300 + f("x"), cy * 300 + f("y"), f("level")));
        }
    }
    lots.truncate(60);
    let nums: Vec<String> = lots.iter().flat_map(|(p, x, y, l)| [p.to_string_lossy().to_string(), x.to_string(), y.to_string(), l.to_string()]).collect();
    let mut a = vec!["compose"];
    a.extend(nums.iter().map(String::as_str));
    let Some(want) = common::python(&a) else { return };
    let buildings: Vec<Building> = lots.iter().map(|(p, ..)| parse_tbx(&std::fs::read_to_string(p).unwrap()).unwrap()).collect();
    let refs: Vec<(&Building, i64, i64, i64)> = buildings.iter().zip(&lots).map(|(b, (_, x, y, l))| (b, *x, *y, *l)).collect();
    let world = compose_lots(&refs).unwrap();
    let got: String = world.iter().map(|((x, y, z), t)| format!("{x} {y} {z}|{}\n", t.join(";"))).collect();
    assert_eq!(got, String::from_utf8(want).unwrap());
}

/// Every roof kind, depth, several sizes and cap patterns, in one big building.
fn roof_sweep_tbx() -> String {
    let kinds = ["PeakWE", "PeakNS", "Peak30WE", "Peak30NS", "Peak30Quad"];
    let depths = ["Point5", "One", "OnePoint5", "Two", "TwoPoint5", "Three"];
    let sizes = [(1, 1), (2, 3), (3, 2), (5, 5), (6, 4), (7, 9), (9, 7), (11, 3), (12, 12), (13, 5)];
    let caps = [("true", "true", "true", "true"), ("true", "false", "false", "true"), ("false", "true", "true", "false")];
    // every tile enum the roof code can ask for, each mapped to a tile named after it
    let mut enums: Vec<String> = Vec::new();
    for i in 1..=12 {
        for p in ["S", "E", "N", "W"] {
            for pre in ["Slope", "CapFall", "CapRise", "Slope30", "CapSlope30Fall", "CapSlope30Rise", "OuterSlope30NW", "OuterSlope30NE", "OuterSlope30SW", "OuterSlope30SE", "Peak30Quad", "Peak30NS", "Peak30WE", "CapPeak30"] {
                enums.push(format!("{pre}{p}{i}"));
                enums.push(format!("{pre}{i}"));
            }
        }
        enums.push(format!("OuterSlope30NW{i}"));
    }
    for p in ["Pt5", "OnePt5", "TwoPt5"] {
        for d in ["S", "E"] {
            enums.push(format!("Slope{p}{d}"));
            enums.push(format!("Peak{p}{d}"));
        }
    }
    enums.extend(["CapGapE3", "CapGapS3", "West1", "North1"].map(String::from));
    enums.sort();
    enums.dedup();
    let entry = |name: &str| -> String { format!("<tile_entry category=\"{name}\">{}</tile_entry>", enums.iter().map(|e| format!("<tile enum=\"{e}\" tile=\"{name}_{e}_001\"/>")).collect::<String>()) };
    let (w, h) = (336, 700);
    let mut objs = String::new();
    let mut n = 0;
    for kind in kinds {
        for depth in depths {
            for (sw, sh) in sizes {
                for (cw, cn, ce, cs) in caps {
                    let (ox, oy) = ((n % 20) * 16 + 1, (n / 20) * 16 + 1);
                    objs += &format!("<object type=\"roof\" RoofType=\"{kind}\" Depth=\"{depth}\" x=\"{ox}\" y=\"{oy}\" width=\"{sw}\" height=\"{sh}\" cappedW=\"{cw}\" cappedN=\"{cn}\" cappedE=\"{ce}\" cappedS=\"{cs}\"/>");
                    n += 1;
                }
            }
        }
    }
    let zeros = vec!["0"; w * h].join(",");
    format!("<building width=\"{w}\" height=\"{h}\" RoofSlope=\"1\" RoofCap=\"2\">{}{}<floor><rooms>{zeros}</rooms>{objs}</floor></building>", entry("slope"), entry("cap"))
}

#[test]
fn pitched_roofs_of_every_kind_equal_python() {
    let path = std::env::temp_dir().join("wcc_roof_sweep.tbx");
    std::fs::write(&path, roof_sweep_tbx()).unwrap();
    let Some(want) = common::python(&["tiles", path.to_str().unwrap()]) else { return };
    let b = parse_tbx(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let got = dump(&path, &b);
    let want = String::from_utf8(want).unwrap();
    assert!(got.lines().count() > 1000, "the sweep should produce roof tiles");
    assert_eq!(got.len(), want.len(), "outputs differ in size");
    assert!(got == want, "roof sweep differs from Python");
}

fn tiny(extra: &str, w: i64, h: i64, rooms: &str) -> String {
    format!("<building width=\"{w}\" height=\"{h}\"><floor><rooms>{rooms}</rooms>{extra}</floor></building>")
}

#[test]
fn absurd_sizes_are_errors_not_aborts() {
    assert!(parse_tbx(&tiny("", -1, -1, "0")).is_err());
    assert!(parse_tbx(&tiny("", -2, -3, "0,0,0,0,0,0")).is_err());
    assert!(parse_tbx(&tiny("", 4_294_967_296, 4_294_967_296, "")).is_err());
    assert!(parse_tbx(&tiny("", 0, 5_000_000, "")).is_err());
    // a wall of two billion squares and a roof of 100000 x 100000 are refused when used
    let wall = parse_tbx(&tiny(r#"<object type="wall" x="1" y="1" dir="W" length="2000000000"/>"#, 2, 2, "0,0,0,0")).unwrap();
    assert!(level_layers(&wall, 0).is_err());
    let roof = parse_tbx(&tiny(r#"<object type="roof" RoofType="PeakWE" Depth="One" x="0" y="0" width="100000" height="100000"/>"#, 2, 2, "0,0,0,0")).unwrap();
    assert!(level_layers(&roof, 0).is_err());
}
