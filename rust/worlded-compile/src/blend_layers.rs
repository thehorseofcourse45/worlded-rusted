//! Edge blends applied to a window of floor tiles (port of `blend_layers` in knoxbuild/blends.py).
use crate::blends::Blend;
use crate::rules::Ruleset;
use crate::terrain::{mix, Grid, Layers, Palette};
use crate::CompileError;
use std::collections::HashMap;
use std::path::PathBuf;

fn cardinal(d: &str) -> Option<(i64, i64)> {
    match d {
        "n" => Some((0, -1)),
        "s" => Some((0, 1)),
        "e" => Some((1, 0)),
        "w" => Some((-1, 0)),
        _ => None,
    }
}

fn corner(d: &str) -> Option<(&'static str, &'static str)> {
    match d {
        "nw" => Some(("n", "w")),
        "ne" => Some(("n", "e")),
        "sw" => Some(("s", "w")),
        "se" => Some(("s", "e")),
        _ => None,
    }
}

/// Which palette entries are `kind` (a tile, or an alias of tiles).
fn members(rules: &Ruleset, pal: &Palette, kind: &str) -> Vec<bool> {
    let wanted: Vec<&str> = match rules.aliases.get(kind) {
        Some(v) => v.iter().map(String::as_str).collect(),
        None => vec![kind],
    };
    pal.names.iter().map(|n| wanted.contains(&n.as_str())).collect()
}

/// `mask` at the square (+dx, +dy) from each square; false off the edge.
fn neighbour(mask: &[bool], w: usize, h: usize, dx: i64, dy: i64) -> Vec<bool> {
    let mut out = vec![false; w * h];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let (sx, sy) = (x + dx, y + dy);
            if sx >= 0 && sy >= 0 && sx < w as i64 && sy < h as i64 {
                out[(y as usize) * w + x as usize] = mask[(sy as usize) * w + sx as usize];
            }
        }
    }
    out
}

fn and(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(x, y)| *x && *y).collect()
}

/// Overlay tiles for a window of floor tiles (`floor` holds palette numbers, 0 = no tile).
/// Squares past the window edge have no ground. With `seam`, squares either side of every multiple
/// of that many tiles do not see each other (WorldEd blends each 300 x 300 map alone).
#[allow(clippy::too_many_arguments)]
pub fn blend_layers(floor: &Grid, rules: &Ruleset, blends: &[Blend], pal: &mut Palette, seed: i64, x0: i64, y0: i64, seam: Option<i64>) -> Result<Layers, CompileError> {
    let (w, h) = (floor.w, floor.h);
    let has_tile: Vec<bool> = floor.data.iter().map(|&v| v > 0).collect();
    let mut reach: HashMap<(i64, i64), Vec<bool>> = HashMap::new();
    let mut layers = Layers::default();
    let mut main_cache: HashMap<String, Vec<bool>> = HashMap::new();
    let mut takes_cache: HashMap<(String, Vec<String>), Vec<bool>> = HashMap::new();
    let mut seen_cache: HashMap<(String, i64, i64), Vec<bool>> = HashMap::new();

    // `mask` at the neighbour (+dx, +dy), unseen across a seam.
    let mut look = |mask: &[bool], dx: i64, dy: i64| -> Vec<bool> {
        let out = neighbour(mask, w, h, dx, dy);
        match seam.filter(|s| *s != 0) {
            Some(s) => {
                let r = reach.entry((dx, dy)).or_insert_with(|| {
                    (0..w * h)
                        .map(|i| {
                            let (gx, gy) = ((i % w) as i64 + x0, (i / w) as i64 + y0);
                            gx.div_euclid(s) == (gx + dx).div_euclid(s) && gy.div_euclid(s) == (gy + dy).div_euclid(s)
                        })
                        .collect()
                });
                and(&out, r)
            }
            None => out,
        }
    };

    for (n, b) in blends.iter().enumerate() {
        if !main_cache.contains_key(&b.main) {
            let m = members(rules, pal, &b.main);
            let v: Vec<bool> = (0..w * h).map(|i| m[floor.data[i] as usize] && has_tile[i]).collect();
            main_cache.insert(b.main.clone(), v);
        }
        let is_main = main_cache[&b.main].clone();
        if !is_main.iter().any(|&v| v) {
            continue; // none of this ground in the window
        }
        let tkey = (b.main.clone(), b.exclude.clone());
        if !takes_cache.contains_key(&tkey) {
            let mut excl = vec![false; pal.names.len()];
            for e in &b.exclude {
                for (i, m) in members(rules, pal, e).into_iter().enumerate() {
                    excl[i] = excl[i] || m;
                }
            }
            let v: Vec<bool> = (0..w * h).map(|i| has_tile[i] && !is_main[i] && !excl[floor.data[i] as usize]).collect();
            takes_cache.insert(tkey.clone(), v);
        }
        let takes = takes_cache[&tkey].clone();
        let mut see = |dx: i64, dy: i64| -> Vec<bool> {
            let k = (b.main.clone(), dx, dy);
            if let Some(v) = seen_cache.get(&k) {
                return v.clone();
            }
            let v = look(&is_main, dx, dy);
            seen_cache.insert(k, v.clone());
            v
        };
        let cond: Vec<bool> = if let Some((dx, dy)) = cardinal(&b.dir) {
            let mut cond = and(&takes, &see(dx, dy));
            // dropped where a corner overlay of the same ground takes over
            for name in ["nw", "ne", "sw", "se"] {
                let (a, c) = corner(name).unwrap();
                if b.dir == a || b.dir == c {
                    let (ax, ay) = cardinal(a).unwrap();
                    let (cx, cy) = cardinal(c).unwrap();
                    let both = and(&see(ax, ay), &see(cx, cy));
                    cond = cond.iter().zip(&both).map(|(c, b)| *c && !*b).collect();
                }
            }
            cond
        } else {
            let (a, c) = corner(&b.dir).ok_or_else(|| CompileError::Parse(PathBuf::from("Blends.txt"), format!("unknown dir {:?}", b.dir)))?;
            let (ax, ay) = cardinal(a).unwrap();
            let (cx, cy) = cardinal(c).unwrap();
            and(&and(&takes, &see(ax, ay)), &see(cx, cy))
        };
        if !cond.iter().any(|&v| v) {
            continue;
        }
        let layer = layers.entry(&b.layer, w, h);
        let tiles: Vec<String> = rules.aliases.get(&b.tile).cloned().unwrap_or_else(|| vec![b.tile.clone()]);
        let ids: Vec<u32> = tiles.iter().map(|t| pal.of(t)).collect();
        if ids.is_empty() {
            continue;
        }
        for i in 0..w * h {
            if cond[i] {
                let (gx, gy) = ((i % w) as i64 + x0, (i / w) as i64 + y0);
                let p = mix(gx, gy, seed.wrapping_mul(1_000_003).wrapping_add(5000).wrapping_add(n as i64)) % ids.len() as u64;
                layer.data[i] = ids[p as usize];
            }
        }
    }
    Ok(layers)
}
