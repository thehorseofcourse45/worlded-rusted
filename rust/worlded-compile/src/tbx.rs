//! BuildingEd `.tbx` files: the building model and parser (port of the top of knoxbuild/buildtiles.py).
//! Errors are plain messages; the pipeline adds the file name.
use std::collections::HashMap;

pub type R<T> = Result<T, String>;

/// An error from one lot of a map: its position in the list of lots handed to the function.
#[derive(Debug)]
pub struct LotError {
    pub lot: usize,
    pub msg: String,
}

/// Largest building (squares each way) and largest roof or wall run the parser accepts.
pub const MAX_SIZE: i64 = 4096;
pub type Attrs = HashMap<String, String>;

/// One tile entry: `{enum: tile name}`, keeping the order the file lists them in
/// (some rules take the first value).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry(pub Vec<(String, String)>);

impl Entry {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    /// Python `entry.get(key, "")`.
    pub fn get_or_empty(&self, key: &str) -> &str {
        self.get(key).unwrap_or("")
    }
    pub fn insert(&mut self, key: String, value: String) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value, // a repeated key keeps its first position
            None => self.0.push((key, value)),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn values(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(_, v)| v)
    }
}

#[derive(Debug, Clone)]
pub struct Floor {
    /// `[y][x]`; 0 = outside.
    pub rooms: Vec<Vec<i64>>,
    pub objects: Vec<Attrs>,
    /// (layer, `[y][x]` tile or "")
    pub tile_layers: Vec<(String, Vec<Vec<String>>)>,
}

#[derive(Debug, Clone)]
pub struct Furniture {
    pub layer: String,
    /// orient -> [(dx, dy, tile name)]
    pub orients: HashMap<String, Vec<(i64, i64, String)>>,
}

#[derive(Debug, Clone)]
pub struct Building {
    pub width: i64,
    pub height: i64,
    pub attrs: Attrs,
    /// 1-based in the file: `entries[n - 1]`
    pub entries: Vec<Entry>,
    /// 1-based in the file: `rooms[n - 1]`
    pub rooms: Vec<Attrs>,
    pub floors: Vec<Floor>,
    /// 0-based
    pub furniture: Vec<Furniture>,
}

/// Python `int(text)`: surrounding whitespace allowed.
pub fn int(text: &str) -> R<i64> {
    text.trim().parse::<i64>().map_err(|_| format!("not a whole number: {text:?}"))
}

pub fn attr_int(a: &Attrs, key: &str) -> R<i64> {
    int(a.get(key).ok_or_else(|| format!("missing attribute {key}"))?)
}

/// BuildingEd writes numbers padded to three digits, the lot files do not:
/// walls_exterior_house_02_064 is walls_exterior_house_02_64 there.
pub fn tile_name(name: &str) -> String {
    if let Some(i) = name.rfind('_') {
        let (head, digits) = (&name[..=i], &name[i + 1..]);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            let t = digits.trim_start_matches('0');
            return format!("{head}{}", if t.is_empty() { "0" } else { t });
        }
    }
    name.to_string()
}

/// Whitespace/comma separated whole numbers (Python `re.split(r"[,\s]+", text.strip())`).
fn numbers(text: &str) -> R<Vec<i64>> {
    text.split(|c: char| c == ',' || c.is_whitespace()).filter(|v| !v.is_empty()).map(int).collect()
}

fn tile_grid(text: &str, user_tiles: &[String], w: i64, h: i64) -> R<Vec<Vec<String>>> {
    let values = numbers(text)?;
    if values.len() as i64 != (w + 1) * (h + 1) {
        return Err(format!("a tile layer has {} values, expected {}", values.len(), (w + 1) * (h + 1)));
    }
    let names: Vec<String> = values.iter().map(|&v| if 0 < v && v <= user_tiles.len() as i64 { user_tiles[(v - 1) as usize].clone() } else { String::new() }).collect();
    Ok((0..=h as usize).map(|y| names[y * (w as usize + 1)..(y + 1) * (w as usize + 1)].to_vec()).collect())
}

fn attrs_of(n: roxmltree::Node) -> Attrs {
    n.attributes().map(|a| (a.name().to_string(), a.value().to_string())).collect()
}

pub fn parse_tbx(text: &str) -> R<Building> {
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let kids: Vec<roxmltree::Node> = root.children().filter(|n| n.is_element()).collect();
    let tag = |n: &roxmltree::Node, t: &str| n.tag_name().name() == t;
    let mut entries = Vec::new();
    for e in kids.iter().filter(|n| tag(n, "tile_entry")) {
        let mut entry = Entry::default();
        for t in e.children().filter(|n| n.is_element()) {
            let en = t.attribute("enum").ok_or("tile_entry child without enum")?;
            entry.insert(en.to_string(), tile_name(t.attribute("tile").ok_or("tile_entry child without tile")?));
        }
        entries.push(entry);
    }
    let rooms: Vec<Attrs> = kids.iter().filter(|n| tag(n, "room")).map(|n| attrs_of(*n)).collect();
    let mut user_tiles = Vec::new();
    for u in kids.iter().filter(|n| tag(n, "user_tiles")) {
        for t in u.children().filter(|n| n.is_element()) {
            user_tiles.push(tile_name(t.attribute("tile").ok_or("user tile without tile")?));
        }
    }
    let w = int(root.attribute("width").ok_or("missing width")?)?;
    let h = int(root.attribute("height").ok_or("missing height")?)?;
    if !(0..=MAX_SIZE).contains(&w) || !(0..=MAX_SIZE).contains(&h) {
        return Err(format!("building size {w} x {h} is outside 0..={MAX_SIZE}"));
    }
    let mut floors = Vec::new();
    for f in kids.iter().filter(|n| tag(n, "floor")) {
        let grid_el = f.children().find(|n| n.is_element() && tag(n, "rooms"));
        let values = match grid_el {
            Some(g) => numbers(g.text().unwrap_or(""))?,
            None => Vec::new(),
        };
        if values.len() as i64 != w * h {
            return Err(format!("a floor has {} room numbers, expected {}", values.len(), w * h));
        }
        let grid = (0..h as usize).map(|y| values[y * w as usize..(y + 1) * w as usize].to_vec()).collect();
        let fk: Vec<roxmltree::Node> = f.children().filter(|n| n.is_element()).collect();
        let objects = fk.iter().filter(|n| tag(n, "object")).map(|n| attrs_of(*n)).collect();
        let mut tile_layers = Vec::new();
        for t in fk.iter().filter(|n| tag(n, "tiles")) {
            tile_layers.push((t.attribute("layer").unwrap_or("").to_string(), tile_grid(t.text().unwrap_or(""), &user_tiles, w, h)?));
        }
        floors.push(Floor { rooms: grid, objects, tile_layers });
    }
    let mut furniture = Vec::new();
    for f in kids.iter().filter(|n| tag(n, "furniture")) {
        let mut orients = HashMap::new();
        for e in f.children().filter(|n| n.is_element() && tag(n, "entry")) {
            let mut tiles = Vec::new();
            for t in e.children().filter(|n| n.is_element() && tag(n, "tile")) {
                tiles.push((
                    int(t.attribute("x").ok_or("furniture tile without x")?)?,
                    int(t.attribute("y").ok_or("furniture tile without y")?)?,
                    tile_name(t.attribute("name").ok_or("furniture tile without name")?),
                ));
            }
            orients.insert(e.attribute("orient").ok_or("furniture entry without orient")?.to_string(), tiles);
        }
        furniture.push(Furniture { layer: f.attribute("layer").unwrap_or("").to_string(), orients });
    }
    Ok(Building { width: w, height: h, attrs: attrs_of(root), entries, rooms, floors, furniture })
}
