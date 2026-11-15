//! Ground tiles from the landscape and vegetation pictures and Rules.txt (port of knoxbuild/terrain.py).
//! Every pick is a hash of the pixel position, the rule and a seed, so a cell can be made alone.
use crate::rules::{Rule, Ruleset};
use std::collections::{HashMap, HashSet};

/// Bottom of the stack first. Layers not listed go after these, by name.
pub const LAYER_ORDER: [&str; 10] = [
    "0_Floor",
    "0_FloorOverlay",
    "0_FloorOverlay2",
    "0_FloorOverlay3",
    "0_FloorOverlay4",
    "0_FloorOverlay5",
    "0_FloorOverlay6",
    "0_Curbs",
    "0_Furniture",
    "0_Vegetation",
];

/// splitmix64 on the position; identical to numpy uint64 arithmetic (everything wraps).
pub fn mix(x: i64, y: i64, salt: i64) -> u64 {
    let mut z = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = z.wrapping_add((salt as u64).wrapping_mul(0x1656_67B1_9E37_79F9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Tile names to small numbers, shared by every cell of a map. 0 means no tile.
pub struct Palette {
    pub names: Vec<String>,
    index: HashMap<String, u32>,
}

impl Default for Palette {
    fn default() -> Self {
        Palette { names: vec![String::new()], index: HashMap::from([(String::new(), 0)]) }
    }
}

impl Palette {
    pub fn of(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.index.insert(name.to_string(), i);
        self.names.push(name.to_string());
        i
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u32>,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Grid {
        Grid { w, h, data: vec![0; w * h] }
    }
    pub fn get(&self, x: usize, y: usize) -> u32 {
        self.data[y * self.w + x]
    }
    pub fn set(&mut self, x: usize, y: usize, v: u32) {
        self.data[y * self.w + x] = v;
    }
}

/// Layers by name, in the order they were first painted (like a Python dict).
#[derive(Default, Debug, Clone)]
pub struct Layers(pub Vec<(String, Grid)>);

impl Layers {
    pub fn get(&self, name: &str) -> Option<&Grid> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, g)| g)
    }
    pub fn entry(&mut self, name: &str, w: usize, h: usize) -> &mut Grid {
        if let Some(i) = self.0.iter().position(|(n, _)| n == name) {
            return &mut self.0[i].1;
        }
        self.0.push((name.to_string(), Grid::new(w, h)));
        &mut self.0.last_mut().unwrap().1
    }
    /// Like dict.update: replaces a layer of the same name in place, else appends.
    pub fn update(&mut self, other: Layers) {
        for (n, g) in other.0 {
            match self.0.iter().position(|(m, _)| *m == n) {
                Some(i) => self.0[i].1 = g,
                None => self.0.push((n, g)),
            }
        }
    }
}

/// The palette number of the tile of `rule` at (x, y).
fn pick(rule: &Rule, rules: &Ruleset, pal: &mut Palette, x: i64, y: i64, seed: i64, salt: i64, cache: &mut HashMap<usize, Vec<u32>>) -> u32 {
    let n = rule.entries.len() as u64;
    if n == 0 {
        return 0;
    }
    let h1 = mix(x, y, seed.wrapping_mul(1_000_003).wrapping_add(salt));
    let e = (h1 % n) as usize;
    let name = &rule.entries[e];
    match rules.aliases.get(name) {
        Some(sub) => {
            let h2 = mix(x, y, seed.wrapping_mul(1_000_003).wrapping_add(salt).wrapping_add(7919));
            if sub.is_empty() {
                return 0;
            }
            // Python builds the whole sub-list in the palette the first time the entry is picked
            let ids = cache.entry(e).or_insert_with(|| sub.iter().map(|t| pal.of(t)).collect());
            ids[(h2 % sub.len() as u64) as usize]
        }
        None => pal.of(name),
    }
}

fn key(c: (i32, i32, i32)) -> i64 {
    ((c.0 as i64) << 16) | ((c.1 as i64) << 8) | c.2 as i64
}

fn colour_keys(img: &[u8]) -> Vec<i64> {
    img.chunks_exact(3).map(|p| ((p[0] as i64) << 16) | ((p[1] as i64) << 8) | p[2] as i64).collect()
}

/// `{layer: (h, w) palette numbers}` for a window of the pictures. `land` and `veg` are w*h*3 RGB;
/// (x0, y0) is where the window starts in the world, so picks do not depend on how it was cut up.
pub fn ground_layers(land: &[u8], veg: &[u8], w: usize, h: usize, rules: &Ruleset, pal: &mut Palette, seed: i64, x0: i64, y0: i64) -> Layers {
    let mut layers = Layers::default();
    assert!(land.len() == w * h * 3 && veg.len() == w * h * 3, "pictures must be w x h x 3");
    let (land_key, veg_key) = (colour_keys(land), colour_keys(veg));
    let in_land: HashSet<i64> = land_key.iter().copied().collect();
    let in_veg: HashSet<i64> = veg_key.iter().copied().collect();
    // WorldEd applies every landscape rule, then every vegetation rule: where both pictures give a
    // tile on the same layer the vegetation one wins (measured against WorldEd, not file order).
    // `n` stays the rule's place in the file: it salts the random picks.
    let mut order: Vec<usize> = (0..rules.rules.len()).collect();
    order.sort_by_key(|&i| rules.rules[i].bitmap != 0);
    for n in order {
        let rule = &rules.rules[n];
        let k = key(rule.color);
        if !(if rule.bitmap == 0 { &in_land } else { &in_veg }).contains(&k) {
            continue;
        }
        if let Some(c) = rule.condition {
            if !in_land.contains(&key(c)) {
                continue;
            }
        }
        let src = if rule.bitmap == 0 { &land_key } else { &veg_key };
        let cond = rule.condition.map(key);
        let hit: Vec<usize> = (0..w * h).filter(|&i| src[i] == k && cond.map_or(true, |c| land_key[i] == c)).collect();
        if hit.is_empty() {
            continue;
        }
        let grid = layers.entry(&rule.layer, w, h);
        let mut cache = HashMap::new();
        // Python picks all pixels of a rule entry-by-entry (palette ids in entry order), so the
        // order palette names are first used is by entry, not by pixel: do the same.
        let mut picks = vec![0u32; hit.len()];
        let mut by_entry: Vec<Vec<usize>> = vec![Vec::new(); rule.entries.len().max(1)];
        for (j, &i) in hit.iter().enumerate() {
            let (x, y) = ((i % w) as i64 + x0, (i / w) as i64 + y0);
            if !rule.entries.is_empty() {
                let h1 = mix(x, y, seed.wrapping_mul(1_000_003).wrapping_add(n as i64));
                by_entry[(h1 % rule.entries.len() as u64) as usize].push(j);
            }
        }
        for js in by_entry {
            for j in js {
                let i = hit[j];
                let (x, y) = ((i % w) as i64 + x0, (i / w) as i64 + y0);
                picks[j] = pick(rule, rules, pal, x, y, seed, n as i64, &mut cache);
            }
        }
        for (j, &i) in hit.iter().enumerate() {
            grid.data[i] = picks[j];
        }
    }
    layers
}

pub fn stack_order(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = LAYER_ORDER.iter().filter(|n| names.iter().any(|m| m == *n)).map(|s| s.to_string()).collect();
    let mut rest: Vec<String> = names.iter().filter(|n| !LAYER_ORDER.contains(&n.as_str())).cloned().collect();
    rest.sort();
    out.extend(rest);
    out
}
