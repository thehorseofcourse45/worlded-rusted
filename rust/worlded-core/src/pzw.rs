//! `.pzw` world project (XML): size, origin and the cells with their `.tmx` paths.
use crate::CoreError;
use std::path::{Path, PathBuf};

pub struct World {
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    pub cells: Vec<Cell>,
}

pub struct Cell {
    pub x: i32,
    pub y: i32,
    /// Path of the cell's `.tmx` as written in the project; `None` when absent or empty.
    pub map: Option<PathBuf>,
}

pub fn parse_pzw(path: &Path) -> Result<World, CoreError> {
    let text = std::fs::read_to_string(path)?;
    let doc = roxmltree::Document::parse(&text).map_err(|e| CoreError::Parse(e.to_string()))?;
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
    let cells = root
        .children()
        .filter(|n| n.has_tag_name("cell"))
        .map(|n| Cell {
            x: num(n, "x").unwrap_or(0) as i32,
            y: num(n, "y").unwrap_or(0) as i32,
            map: n.attribute("map").filter(|m| !m.is_empty()).map(PathBuf::from),
        })
        .collect();
    Ok(World {
        width: num(root, "width").unwrap_or(0) as u32,
        height: num(root, "height").unwrap_or(0) as u32,
        origin,
        cells,
    })
}

/// Where a cell TMX really is: the path in the project if it exists, else the same file name
/// beside the `.pzw` or in a `tmx` folder beside it. `None` when no file is found.
pub fn resolve_map(cell: &Cell, pzw_dir: &Path) -> Option<PathBuf> {
    let p = cell.map.as_ref()?;
    if p.exists() {
        return Some(p.clone());
    }
    let name = p.file_name()?;
    [pzw_dir.join(name), pzw_dir.join("tmx").join(name)].into_iter().find(|c| c.exists())
}
