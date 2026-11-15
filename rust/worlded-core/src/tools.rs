//! Editing tools as pure functions: each reads a layer and returns the `Edit` it would make.
//! Squares that would not change are left out; nothing outside the map is ever touched.
use crate::edit::{Change, Edit};
use crate::tmx::Tmx;

fn at(tmx: &Tmx, layer: usize, x: i32, y: i32) -> Option<u32> {
    if x < 0 || y < 0 || x as u32 >= tmx.width || y as u32 >= tmx.height {
        return None;
    }
    tmx.layers.get(layer)?.gids.get(y as usize * tmx.width as usize + x as usize).copied()
}

fn set(tmx: &Tmx, layer: usize, x: i32, y: i32, gid: u32, out: &mut Vec<Change>) {
    if let Some(old) = at(tmx, layer, x, y) {
        if old != gid {
            out.push(Change { layer, x: x as u32, y: y as u32, old, new: gid });
        }
    }
}

/// Square brush of `size`: odd sizes centre on (x, y); even sizes start at (x, y) and extend
/// right and down.
pub fn paint(tmx: &Tmx, layer: usize, x: i32, y: i32, size: u32, gid: u32) -> Edit {
    let s = size.max(1) as i32;
    let (x0, y0) = if s % 2 == 1 { (x - s / 2, y - s / 2) } else { (x, y) };
    let mut out = Vec::new();
    for yy in y0..y0 + s {
        for xx in x0..x0 + s {
            set(tmx, layer, xx, yy, gid, &mut out);
        }
    }
    Edit(out)
}

pub fn erase(tmx: &Tmx, layer: usize, x: i32, y: i32, size: u32) -> Edit {
    paint(tmx, layer, x, y, size, 0)
}

/// Filled rectangle between two corners given in any order, clipped to the map.
pub fn rect_fill(tmx: &Tmx, layer: usize, a: (i32, i32), b: (i32, i32), gid: u32) -> Edit {
    let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(tmx.width as i32 - 1));
    let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(tmx.height as i32 - 1));
    let mut out = Vec::new();
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            set(tmx, layer, xx, yy, gid, &mut out);
        }
    }
    Edit(out)
}

/// 4-connected fill of the region sharing the clicked square's gid. `None` when the region
/// is larger than `limit` squares; an empty edit when the click is outside the map or the
/// region already has `gid`.
pub fn bucket_fill(tmx: &Tmx, layer: usize, x: i32, y: i32, gid: u32, limit: usize) -> Option<Edit> {
    let Some(target) = at(tmx, layer, x, y) else { return Some(Edit::default()) };
    if target == gid {
        return Some(Edit::default());
    }
    let w = tmx.width as usize;
    let mut seen = vec![false; w * tmx.height as usize];
    let mut stack = vec![(x, y)];
    seen[y as usize * w + x as usize] = true;
    let mut out = Vec::new();
    while let Some((cx, cy)) = stack.pop() {
        if out.len() >= limit {
            return None;
        }
        out.push(Change { layer, x: cx as u32, y: cy as u32, old: target, new: gid });
        for (nx, ny) in [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)] {
            if at(tmx, layer, nx, ny) == Some(target) {
                let i = ny as usize * w + nx as usize;
                if !seen[i] {
                    seen[i] = true;
                    stack.push((nx, ny));
                }
            }
        }
    }
    Some(Edit(out))
}

/// The gid under (x, y), `None` when empty or outside the map.
pub fn pick(tmx: &Tmx, layer: usize, x: i32, y: i32) -> Option<u32> {
    at(tmx, layer, x, y).filter(|&g| g != 0)
}
