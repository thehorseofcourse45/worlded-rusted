//! `.pzw` world project (XML): size, origin, and the cells with their `.tmx` paths, lots and
//! objects. Parsing is lossless: every child element keeps the whitespace in front of it, so
//! `to_xml(parse(text)) == text` on files WorldEd wrote. (Assumes the settings blocks come before
//! the cells, which is how WorldEd writes projects.)
use crate::CoreError;
use std::path::{Path, PathBuf};

pub struct World {
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    /// Everything up to and including the `<world ...>` open tag, verbatim.
    pub head: String,
    /// Newline style of the file (`\n` or `\r\n`), used for everything this crate generates.
    pub nl: String,
    /// Root children that are not cells, verbatim including the whitespace in front of each one.
    pub raw: Vec<String>,
    pub cells: Vec<Cell>,
    /// Everything after the last child element, verbatim (`</world>` and any trailing newline).
    pub closing: String,
}

pub struct Cell {
    pub x: i32,
    pub y: i32,
    /// The `map` attribute as written (`None` when absent).
    pub map: Option<String>,
    pub children: Vec<CellChild>,
    /// Whitespace in front of this `<cell>` element, verbatim.
    pub lead: String,
    /// Whether an empty cell was written `<cell .../>` (as opposed to `<cell ...></cell>`).
    pub self_closing: bool,
}

pub enum CellChild {
    Lot(Lot),
    Object(Obj),
    /// Anything else inside a cell, verbatim including the whitespace in front of it.
    Raw(String),
}

pub struct Lot {
    pub x: i64,
    pub y: i64,
    pub level: i64,
    pub width: i64,
    pub height: i64,
    pub map: String,
}

pub struct Obj {
    pub name: String,
    pub group: String,
    pub kind: String,
    pub x: i64,
    pub y: i64,
    pub level: i64,
    pub width: i64,
    pub height: i64,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn parse_pzw(path: &Path) -> Result<World, CoreError> {
    let text = std::fs::read_to_string(path)?;
    parse_pzw_str(&text)
}

pub fn parse_pzw_str(text: &str) -> Result<World, CoreError> {
    let doc = roxmltree::Document::parse(text).map_err(|e| CoreError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "world" {
        return Err(CoreError::Parse("root element is not <world>".into()));
    }
    let num = |n: roxmltree::Node, k: &str| n.attribute(k).and_then(|v| v.parse::<i64>().ok());
    let origin = root
        .descendants()
        .find(|n| n.has_tag_name("worldOrigin"))
        .and_then(|n| n.attribute("origin"))
        .and_then(|s| s.split_once(','))
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
        .unwrap_or((0, 0));
    let root_open_end = root.range().start + text[root.range().start..].find('>').map(|i| i + 1).unwrap_or(0);
    let head = text[..root_open_end].to_string();
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let kids: Vec<roxmltree::Node> = root.children().filter(|n| n.is_element()).collect();
    let mut raw = Vec::new();
    let mut cells = Vec::new();
    let mut prev = root_open_end;
    for n in &kids {
        let lead = text[prev..n.range().start].to_string();
        prev = n.range().end;
        if n.has_tag_name("cell") {
            let cell_open_end = n.range().start + text[n.range().start..].find('>').map(|i| i + 1).unwrap_or(n.range().end);
            let mut children = Vec::new();
            let mut cprev = cell_open_end;
            let ekids: Vec<roxmltree::Node> = n.children().filter(|c| c.is_element()).collect();
            for c in &ekids {
                let clead = text[cprev..c.range().start].to_string();
                cprev = c.range().end;
                if c.has_tag_name("lot") {
                    children.push(CellChild::Lot(Lot {
                        x: num(*c, "x").unwrap_or(0),
                        y: num(*c, "y").unwrap_or(0),
                        level: num(*c, "level").unwrap_or(0),
                        width: num(*c, "width").unwrap_or(0),
                        height: num(*c, "height").unwrap_or(0),
                        map: c.attribute("map").unwrap_or("").to_string(),
                    }));
                } else if c.has_tag_name("object") {
                    children.push(CellChild::Object(Obj {
                        name: c.attribute("name").unwrap_or("").to_string(),
                        group: c.attribute("group").unwrap_or("").to_string(),
                        kind: c.attribute("type").unwrap_or("").to_string(),
                        x: num(*c, "x").unwrap_or(0),
                        y: num(*c, "y").unwrap_or(0),
                        level: num(*c, "level").unwrap_or(0),
                        width: num(*c, "width").unwrap_or(0),
                        height: num(*c, "height").unwrap_or(0),
                    }));
                } else {
                    children.push(CellChild::Raw(clead + &text[c.range()]));
                }
            }
            let self_closing = ekids.is_empty() && text[n.range()].trim_end().ends_with("/>");
            cells.push(Cell { x: num(*n, "x").unwrap_or(0) as i32, y: num(*n, "y").unwrap_or(0) as i32, map: n.attribute("map").map(str::to_string), children, lead, self_closing });
        } else {
            raw.push(lead + &text[n.range()]);
        }
    }
    let closing = text[prev.min(text.len())..].to_string();
    Ok(World { width: num(root, "width").unwrap_or(0) as u32, height: num(root, "height").unwrap_or(0) as u32, origin, head, nl: nl.to_string(), raw, cells, closing })
}

impl World {
    /// Serializes back in WorldEd's format; a parsed file round-trips byte for byte.
    pub fn to_xml(&self) -> String {
        let nl = self.nl.as_str();
        let mut s = self.head.clone();
        for r in &self.raw {
            s.push_str(r);
        }
        for c in &self.cells {
            s.push_str(&c.lead);
            let map = c.map.as_deref().unwrap_or("");
            if c.children.is_empty() {
                if c.self_closing {
                    s.push_str(&format!("<cell x=\"{}\" y=\"{}\" map=\"{}\"/>", c.x, c.y, esc(map)));
                } else {
                    s.push_str(&format!("<cell x=\"{}\" y=\"{}\" map=\"{}\"></cell>", c.x, c.y, esc(map)));
                }
            } else {
                s.push_str(&format!("<cell x=\"{}\" y=\"{}\" map=\"{}\">", c.x, c.y, esc(map)));
                for ch in &c.children {
                    match ch {
                        CellChild::Lot(l) => s.push_str(&format!("{nl}  <lot x=\"{}\" y=\"{}\" level=\"{}\" width=\"{}\" height=\"{}\" map=\"{}\"/>", l.x, l.y, l.level, l.width, l.height, esc(&l.map))),
                        CellChild::Object(o) => s.push_str(&format!("{nl}  <object name=\"{}\" group=\"{}\" type=\"{}\" x=\"{}\" y=\"{}\" level=\"{}\" width=\"{}\" height=\"{}\"/>", esc(&o.name), esc(&o.group), esc(&o.kind), o.x, o.y, o.level, o.width, o.height)),
                        CellChild::Raw(r) => s.push_str(r),
                    }
                }
                s.push_str(&format!("{nl} </cell>"));
            }
        }
        s.push_str(&self.closing);
        s
    }

    /// A new project: a `<world>` with a `GenerateLots` block (origin 70,0) and one empty cell per
    /// position, in WorldEd's format.
    pub fn new_project(name: &str, cells_x: u32, cells_y: u32) -> World {
        let head = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<world version=\"1.0\" width=\"{cells_x}\" height=\"{cells_y}\">");
        let raw = vec!["\n <GenerateLots>\n  <exportdir path=\"\"/>\n  <ZombieSpawnMap path=\"\"/>\n  <TileDefFolder path=\"\"/>\n  <worldOrigin origin=\"70,0\"/>\n  <numberOfThreads count=\"4\"/>\n </GenerateLots>".to_string()];
        let cells = (0..cells_y as i32).flat_map(|y| (0..cells_x as i32).map(move |x| Cell { x, y, map: Some(format!("tmx/{name}_{}_{}.tmx", 70 + x, y)), children: Vec::new(), lead: "\n ".to_string(), self_closing: true })).collect();
        World { width: cells_x, height: cells_y, origin: (70, 0), head, nl: "\n".to_string(), raw, cells, closing: "\n</world>\n".to_string() }
    }

    /// The cell at world position (x, y), if the project lists it.
    pub fn cell_at(&self, x: i32, y: i32) -> Option<usize> {
        self.cells.iter().position(|c| c.x == x && c.y == y)
    }
}

/// Where a cell TMX really is: the path in the project if it exists, else the same file name
/// beside the `.pzw` or in a `tmx` folder beside it. `None` when no file is found.
pub fn resolve_map(cell: &Cell, pzw_dir: &Path) -> Option<PathBuf> {
    let m = cell.map.as_deref()?;
    if m.is_empty() {
        return None;
    }
    let p = Path::new(m);
    // a relative path is relative to the project, never to the folder the program runs in
    let direct = if p.is_relative() { pzw_dir.join(p) } else { p.to_path_buf() };
    if direct.exists() {
        return Some(direct);
    }
    let name = p.file_name()?;
    [pzw_dir.join(name), pzw_dir.join("tmx").join(name)].into_iter().find(|c| c.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<world version=\"1.0\" width=\"2\" height=\"1\">\n <GenerateLots><worldOrigin origin=\"70,0\"/></GenerateLots>\n <cell x=\"0\" y=\"0\" map=\"Z:/gone/demo_0_0.tmx\"></cell>\n <cell x=\"1\" y=\"0\" map=\"\"></cell>\n</world>\n";

    #[test]
    fn parses_cells_lots_objects_and_keeps_settings_verbatim() {
        let w = parse_pzw_str(DEMO).unwrap();
        assert_eq!((w.width, w.height, w.origin), (2, 1, (70, 0)));
        assert_eq!(w.cells.len(), 2);
        assert_eq!(w.cells[0].map.as_deref(), Some("Z:/gone/demo_0_0.tmx"));
        assert_eq!(w.cells[1].map.as_deref(), Some(""));
        assert!(w.raw.iter().any(|r| r.contains("<GenerateLots>")));
        assert_eq!(w.to_xml(), DEMO, "round trip is byte exact");
    }

    #[test]
    fn real_worlded_project_round_trips_byte_for_byte() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../knoxlots/fixtures/real/real1/real1.pzw");
        let text = std::fs::read_to_string(&p).unwrap();
        let w = parse_pzw_str(&text).unwrap();
        let out = w.to_xml();
        if out != text {
            let i = out.bytes().zip(text.bytes()).position(|(a, b)| a != b).unwrap_or(out.len().min(text.len()));
            panic!("first diff at byte {i}: out={:?} text={:?}", &out[i.saturating_sub(40)..(i + 40).min(out.len())], &text[i.saturating_sub(40)..(i + 40).min(text.len())]);
        }
        let cell0 = &w.cells[0];
        let lots = cell0.children.iter().filter(|c| matches!(c, CellChild::Lot(_))).count();
        let objs = cell0.children.iter().filter(|c| matches!(c, CellChild::Object(_))).count();
        assert_eq!((lots, objs), (23, 29), "cell 0,0 of the real project");
        assert!(matches!(&cell0.children[0], CellChild::Lot(l) if l.map == "buildings/real1_0031.tbx" && l.width == 25));
        assert!(cell0.children.iter().any(|c| matches!(c, CellChild::Object(o) if o.group == "ParkingStall" && o.kind == "ParkingStall" && o.width == 3)));
    }

    #[test]
    fn adding_and_removing_a_lot_edits_only_that_cell() {
        let mut w = parse_pzw_str(DEMO).unwrap();
        let lot = Lot { x: 5, y: 6, level: 0, width: 10, height: 12, map: "buildings/house.tbx".into() };
        w.cells[0].children.push(CellChild::Lot(lot));
        let text = w.to_xml();
        assert!(text.contains("\n  <lot x=\"5\" y=\"6\" level=\"0\" width=\"10\" height=\"12\" map=\"buildings/house.tbx\"/>"));
        let mut w2 = parse_pzw_str(&text).unwrap();
        assert_eq!(w2.to_xml(), text);
        assert!(matches!(&w2.cells[0].children[0], CellChild::Lot(l) if l.x == 5 && l.map == "buildings/house.tbx"));
        w2.cells[0].children.clear();
        let text2 = w2.to_xml();
        assert!(text2.contains("<cell x=\"0\" y=\"0\" map=\"Z:/gone/demo_0_0.tmx\"></cell>"));
        assert!(!text2.contains("house.tbx"));
    }

    #[test]
    fn new_project_matches_worlded_format_and_resolves() {
        let w = World::new_project("demo", 2, 1);
        let text = w.to_xml();
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<world version=\"1.0\" width=\"2\" height=\"1\">\n"));
        assert!(text.contains("<worldOrigin origin=\"70,0\"/>"));
        assert!(text.contains("<cell x=\"1\" y=\"0\" map=\"tmx/demo_71_0.tmx\"/>"));
        let w2 = parse_pzw_str(&text).unwrap();
        assert_eq!(w2.cells.len(), 2);
        assert_eq!(w2.origin, (70, 0));
        assert_eq!(w2.to_xml(), text);
    }

    #[test]
    fn resolve_map_finds_beside_the_project() {
        let dir = std::env::temp_dir();
        let c = Cell { x: 0, y: 0, map: Some("Z:/elsewhere/pzw_found.tmx".to_string()), children: vec![], lead: String::new(), self_closing: false };
        assert!(resolve_map(&c, &dir).is_none());
        let empty = Cell { x: 0, y: 0, map: Some(String::new()), children: vec![], lead: String::new(), self_closing: false };
        assert!(resolve_map(&empty, &dir).is_none(), "an empty map attribute is no map, not the project folder");
        let f = dir.join("pzw_found.tmx");
        std::fs::write(&f, "<map/>").unwrap();
        assert_eq!(resolve_map(&c, &dir), Some(f));
        let _ = std::fs::remove_file(dir.join("pzw_found.tmx"));
    }
}
