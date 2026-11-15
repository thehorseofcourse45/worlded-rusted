mod common;
use worlded_compile::blend_layers::blend_layers;
use worlded_compile::blends::{load_blends, parse_blends};
use worlded_compile::rules::{load_rules, parse_rules};
use worlded_compile::terrain::{Grid, Palette};

fn data(name: &str) -> String {
    std::fs::read_to_string(common::repo().join("rust/worlded-compile/tests/data").join(name)).unwrap()
}

fn run_small(rows: &[&str], seam: Option<i64>, x0: i64) -> std::collections::BTreeMap<(usize, usize), std::collections::BTreeSet<String>> {
    let rules = parse_rules(&data("rules_for_blends.txt")).unwrap();
    let blends = parse_blends(&data("blends_all.txt")).unwrap();
    let mut pal = Palette::default();
    let (h, w) = (rows.len(), rows[0].len());
    let mut floor = Grid::new(w, h);
    for (y, r) in rows.iter().enumerate() {
        for (x, c) in r.chars().enumerate() {
            if c != '.' {
                floor.set(x, y, pal.of(if c == 'G' { "g_0" } else { "road" }));
            }
        }
    }
    let layers = blend_layers(&floor, &rules, &blends, &mut pal, 1, x0, 0, seam).unwrap();
    let mut out: std::collections::BTreeMap<(usize, usize), std::collections::BTreeSet<String>> = Default::default();
    for (_, g) in &layers.0 {
        for y in 0..h {
            for x in 0..w {
                if g.get(x, y) != 0 {
                    out.entry((x, y)).or_default().insert(pal.names[g.get(x, y) as usize].clone());
                }
            }
        }
    }
    out
}

fn set(v: &[&str]) -> std::collections::BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn cardinal_neighbour_and_opposites() {
    assert_eq!(run_small(&["...", "GR.", "..."], None, 0), [((1, 1), set(&["ov_w"]))].into());
    assert_eq!(run_small(&["GRG"], None, 0), [((1, 0), set(&["ov_w", "ov_e"]))].into());
}

#[test]
fn corner_replaces_the_two_cardinals() {
    let got = run_small(&["GG.", "GR.", "..."], None, 0);
    assert_eq!(got[&(1, 1)], set(&["ov_nw"]));
}

#[test]
fn seam_hides_the_neighbour_across_a_multiple_of_300() {
    // road at x=299 has grass at x=298 (west): seen. A seam at 300 between x=299|300 hides the east neighbour.
    assert!(run_small(&["GR"], Some(300), 299).is_empty());
    assert!(!run_small(&["GR"], Some(300), 298).is_empty());
}

#[test]
fn no_blends_gives_no_layers() {
    let rules = parse_rules(&data("rules_for_blends.txt")).unwrap();
    let mut pal = Palette::default();
    let mut floor = Grid::new(3, 1);
    floor.set(0, 0, pal.of("g_0"));
    assert!(blend_layers(&floor, &rules, &[], &mut pal, 1, 0, 0, None).unwrap().0.is_empty());
}

fn real_case(w: usize, h: usize, x0: i64, y0: i64, seed: i64, seam: i64) {
    let cfg = common::repo().join("config");
    let (rp, bp) = (cfg.join("Rules.txt"), cfg.join("Blends.txt"));
    let rules = load_rules(&rp).unwrap();
    let blends = load_blends(&bp).unwrap();
    let mut names: Vec<String> = Vec::new();
    for b in blends.iter().take(40) {
        let t = rules.aliases.get(&b.main).and_then(|v| v.first().cloned()).unwrap_or_else(|| b.main.clone());
        if !names.contains(&t) {
            names.push(t);
        }
        if names.len() == 6 {
            break;
        }
    }
    names.push("-".into());
    names.push("zz_other".into());
    let list = names.join(";");
    let args = [w.to_string(), h.to_string(), x0.to_string(), y0.to_string(), seed.to_string(), seam.to_string()];
    let mut a = vec!["blendapply", rp.to_str().unwrap(), bp.to_str().unwrap()];
    a.extend(args.iter().map(String::as_str));
    a.push(&list);
    let Some(want) = common::python(&a) else { return };

    let mut pal = Palette::default();
    let mut floor = Grid::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let nm = &names[((x / 2) * 5 + (y / 2) * 3 + ((x / 4) * (y / 3)) % 4) % names.len()];
            floor.set(x, y, if nm == "-" { 0 } else { pal.of(nm) });
        }
    }
    let layers = blend_layers(&floor, &rules, &blends, &mut pal, seed, x0, y0, if seam == 0 { None } else { Some(seam) }).unwrap();
    let mut out = String::new();
    for (name, g) in &layers.0 {
        out += &format!("L|{name}\n");
        for y in 0..h {
            out += &(0..w).map(|x| g.get(x, y).to_string()).collect::<Vec<_>>().join(" ");
            out.push('\n');
        }
    }
    out += &format!("P|{}\n", pal.names.join("|"));
    assert!(!layers.0.is_empty(), "test floor should produce some blends");
    assert_eq!(out, String::from_utf8(want).unwrap(), "case {w}x{h} at {x0},{y0} seed {seed} seam {seam}");
}

#[test]
fn blend_layers_match_python_with_real_blends() {
    real_case(40, 30, 0, 0, 1, 0);
    real_case(37, 41, 280, -3, 7, 300); // a seam crosses the window
    real_case(30, 30, 16490, 16480, 2, 300);
}
