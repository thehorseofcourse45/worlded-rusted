mod common;
use std::path::PathBuf;
use worlded_compile::blends::{load_blends, parse_blends};
use worlded_compile::rules::{load_rules, parse_rules, Ruleset};

fn data(name: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)).unwrap()
}

fn canonical(rs: &Ruleset) -> String {
    let mut a: Vec<_> = rs.aliases.iter().collect();
    a.sort();
    let mut out: Vec<String> = a.iter().map(|(n, e)| format!("A|{n}|{}", e.join(","))).collect();
    for r in &rs.rules {
        let cond = r.condition.map_or(String::new(), |c| format!("{} {} {}", c.0, c.1, c.2));
        out.push(format!("R|{}|{}|{} {} {}|{}|{}|{}", r.label, r.bitmap, r.color.0, r.color.1, r.color.2, r.layer, cond, r.entries.join(",")));
    }
    out.join("\n") + "\n"
}

#[test]
fn small_rules_parse() {
    let rs = parse_rules(&data("rules_small.txt")).unwrap();
    assert_eq!(rs.aliases["grass"], vec!["g_0", "g_1", "g_2", "g_3"]);
    assert_eq!(rs.rules.iter().map(|r| r.layer.as_str()).collect::<Vec<_>>(), vec!["0_Floor", "0_Vegetation"]);
    assert_eq!(rs.rules[1].condition, Some((10, 20, 30)));
    assert_eq!(rs.rules[1].entries, vec!["tree_a", "tree_b", "grass"]);
    assert_eq!(rs.for_bitmap(rs.rules[0].bitmap).len() >= 1, true);
}

#[test]
fn comments_blank_lines_and_single_tile_values() {
    let rs = parse_rules("// c\nalias\n{\n name = a // trailing\n tiles = t_1\n}\nrule\n{\n bitmap = 1\n color = 1 2 3\n tiles = a\n layer = 0_Floor\n}\n").unwrap();
    assert_eq!(rs.aliases["a"], vec!["t_1"]);
    assert_eq!(rs.rules[0].label, "");
    assert_eq!(rs.rules[0].color, (1, 2, 3));
}

#[test]
fn damaged_rules_are_errors_not_panics() {
    assert!(parse_rules("rule\n{\n bitmap = x\n color = 1 2 3\n layer = a\n}\n").is_err());
    assert!(parse_rules("rule\n{\n bitmap = 1\n color = 1 2\n layer = a\n}\n").is_err());
    assert!(parse_rules("rule\n{\n bitmap = 1\n color = 1 2 3\n}\n").is_err()); // no layer
    assert!(parse_rules("alias\n{\n tiles = a\n}\n").is_err()); // no name
    assert!(load_rules(&PathBuf::from("Z:/no/such/Rules.txt")).is_err());
}

#[test]
fn blends_parse() {
    let b = parse_blends(&data("blends_all.txt")).unwrap();
    assert_eq!(b.len(), 8);
    assert_eq!((b[4].dir.as_str(), b[4].tile.as_str(), b[4].layer.as_str()), ("nw", "ov_nw", "0_FloorOverlay"));
    assert!(parse_blends("blend\n{\n layer = a\n}\n").is_err()); // missing keys
}

#[test]
fn real_config_files_equal_python() {
    let cfg = common::repo().join("config");
    let Some(want) = common::python(&["rules", cfg.join("Rules.txt").to_str().unwrap()]) else { return };
    let rs = load_rules(&cfg.join("Rules.txt")).unwrap();
    assert!(rs.rules.len() > 100 && rs.aliases.len() > 50);
    assert_eq!(canonical(&rs), String::from_utf8(want).unwrap());
    let want = common::python(&["blends", cfg.join("Blends.txt").to_str().unwrap()]).unwrap();
    let got: String = load_blends(&cfg.join("Blends.txt"))
        .unwrap()
        .iter()
        .map(|b| format!("B|{}|{}|{}|{}|{}|{}\n", b.layer, b.main, b.tile, b.dir, b.exclude.join(","), b.exclude2.iter().map(|p| p.join(" ")).collect::<Vec<_>>().join(";")))
        .collect();
    assert_eq!(got, String::from_utf8(want).unwrap());
}
