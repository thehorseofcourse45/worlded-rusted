mod common;
use worlded_compile::rules::load_rules;
use worlded_compile::terrain::{ground_layers, mix, stack_order, Palette};

#[test]
fn mix_matches_python_for_large_and_negative_coordinates() {
    let cases = [(0, 0, 0), (5, 7, 1_000_003), (-1, -3, 7919), (16500, 16500, 5_000_000), (16499, -2, 123_456_789_012)];
    let args: Vec<String> = cases.iter().map(|(x, y, s)| format!("{x},{y},{s}")).collect();
    let mut a = vec!["mix"];
    a.extend(args.iter().map(String::as_str));
    let Some(want) = common::python(&a) else { return };
    let got: String = cases.iter().map(|&(x, y, s)| format!("{}\n", mix(x, y, s))).collect();
    assert_eq!(got, String::from_utf8(want).unwrap());
}

#[test]
fn palette_ids_start_at_one_in_first_use_order() {
    let mut p = Palette::default();
    assert_eq!((p.of("b"), p.of("a"), p.of("b"), p.of("")), (1, 2, 1, 0));
    assert_eq!(p.names, vec!["", "b", "a"]);
}

#[test]
fn stack_order_matches_python() {
    let names: Vec<String> = ["0_Vegetation", "zzz", "0_Floor", "aaa", "0_Curbs", "0_FloorOverlay3"].iter().map(|s| s.to_string()).collect();
    let mut a = vec!["stack"];
    a.extend(names.iter().map(String::as_str));
    let got = stack_order(&names);
    assert_eq!(got, vec!["0_Floor", "0_FloorOverlay3", "0_Curbs", "0_Vegetation", "aaa", "zzz"]);
    if let Some(want) = common::python(&a) {
        assert_eq!(got.join("\n") + "\n", String::from_utf8(want).unwrap());
    }
}

fn colours(cs: &[(i32, i32, i32)]) -> String {
    cs.iter().map(|c| format!("{},{},{}", c.0, c.1, c.2)).collect::<Vec<_>>().join(";")
}

fn run_case(w: usize, h: usize, x0: i64, y0: i64, seed: i64) {
    let cfg = common::repo().join("config").join("Rules.txt");
    let rules = load_rules(&cfg).unwrap();
    let filler = (1, 2, 3);
    let mut land: Vec<(i32, i32, i32)> = Vec::new();
    let mut veg: Vec<(i32, i32, i32)> = Vec::new();
    for r in rules.rules.iter().filter(|r| r.bitmap == 0).take(12) {
        land.push(r.color);
    }
    for r in rules.rules.iter().filter(|r| r.condition.is_some()).take(4) {
        land.push(r.condition.unwrap());
        veg.push(r.color);
    }
    for r in rules.rules.iter().filter(|r| r.bitmap != 0 && r.condition.is_none()).take(6) {
        veg.push(r.color);
    }
    land.push(filler);
    veg.push(filler);
    land.dedup();
    let (lc, vc) = (colours(&land), colours(&veg));
    let args = [w.to_string(), h.to_string(), x0.to_string(), y0.to_string(), seed.to_string()];
    let mut a = vec!["terrain", cfg.to_str().unwrap()];
    a.extend(args.iter().map(String::as_str));
    a.push(&lc);
    a.push(&vc);
    let Some(want) = common::python(&a) else { return };

    let (mut lp, mut vp) = (vec![0u8; w * h * 3], vec![0u8; w * h * 3]);
    for y in 0..h {
        for x in 0..w {
            let l = land[(x * 7 + y * 13 + (x * y) % 5) % land.len()];
            let v = veg[(x * 3 + y * 5 + (x ^ y) % 7) % veg.len()];
            lp[(y * w + x) * 3..][..3].copy_from_slice(&[l.0 as u8, l.1 as u8, l.2 as u8]);
            vp[(y * w + x) * 3..][..3].copy_from_slice(&[v.0 as u8, v.1 as u8, v.2 as u8]);
        }
    }
    let mut pal = Palette::default();
    let layers = ground_layers(&lp, &vp, w, h, &rules, &mut pal, seed, x0, y0);
    let mut out = String::new();
    for (name, g) in &layers.0 {
        out += &format!("L|{name}\n");
        for y in 0..h {
            out += &(0..w).map(|x| g.get(x, y).to_string()).collect::<Vec<_>>().join(" ");
            out.push('\n');
        }
    }
    out += &format!("P|{}\n", pal.names.join("|"));
    let want = String::from_utf8(want).unwrap();
    assert!(!layers.0.is_empty(), "test pictures should hit some rules");
    assert_eq!(out, want, "case w={w} h={h} x0={x0} y0={y0} seed={seed}");
}

#[test]
fn ground_layers_match_python() {
    run_case(40, 30, 0, 0, 1);
    run_case(37, 41, 16490, -5, 7); // large and negative world coordinates, odd size
    run_case(25, 25, 300, 600, 3);
}
