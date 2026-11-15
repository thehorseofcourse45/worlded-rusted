//! The room and building tables of a lot header, from the buildings placed in a cell
//! (port of knoxbuild/buildheader.py; see its docstring for the rules and what is not done).
use crate::tbx::{Building, LotError, R};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const CELL: i64 = 256;
pub const ORIGIN_X: i64 = 21000; // the world's west edge in lot-file tiles (70 x 300)

pub type Rect = (i64, i64, i64, i64);

#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    pub name: String,
    pub z: i64,
    pub rects: Vec<Rect>,
}

/// Rectangles (x, y, w, h) covering exactly `squares`: take the first free square in row order,
/// extend right as far as the room goes, then down while the row below has a run of exactly that
/// width starting at the same column.
pub fn room_rects(squares: &BTreeSet<(i64, i64)>) -> Vec<Rect> {
    let mut free: HashSet<(i64, i64)> = squares.iter().copied().collect();
    let mut order: Vec<(i64, i64)> = squares.iter().map(|&(x, y)| (y, x)).collect();
    order.sort();
    let mut out = Vec::new();
    for (y, x) in order {
        if !free.contains(&(x, y)) {
            continue;
        }
        let mut w = 1;
        while free.contains(&(x + w, y)) {
            w += 1;
        }
        let mut h = 1;
        loop {
            let row = y + h;
            if !(0..w).all(|i| free.contains(&(x + i, row))) {
                break;
            }
            // the run in that row must be exactly this wide: no free square either side
            if free.contains(&(x - 1, row)) || free.contains(&(x + w, row)) {
                break;
            }
            h += 1;
        }
        for yy in y..y + h {
            for xx in x..x + w {
                free.remove(&(xx, yy));
            }
        }
        out.push((x, y, w, h));
    }
    out
}

fn room_name(b: &Building, rid: i64) -> R<String> {
    Ok(b.rooms.get((rid - 1) as usize).ok_or_else(|| format!("room {rid} is not defined"))?.get("Name").cloned().unwrap_or_default())
}

/// Room entries of a building placed at world tile (tx, ty), in world coordinates.
pub fn building_rooms(b: &Building, tx: i64, ty: i64, base: i64) -> R<Vec<Room>> {
    let mut rooms = Vec::new();
    for (z, floor) in b.floors.iter().enumerate() {
        let mut by_id: BTreeMap<i64, BTreeSet<(i64, i64)>> = BTreeMap::new();
        for (y, row) in floor.rooms.iter().enumerate() {
            for (x, &v) in row.iter().enumerate() {
                if v > 0 {
                    by_id.entry(v).or_default().insert((x as i64, y as i64));
                }
            }
        }
        for (rid, sq) in by_id {
            let rects = room_rects(&sq).into_iter().map(|(x, y, w, h)| (tx + x, ty + y, w, h)).collect();
            rooms.push(Room { name: room_name(b, rid)?, z: base + z as i64, rects });
        }
    }
    Ok(rooms)
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Room entries for buildings placed in project order, as `cell_tables` wants them. `lots` is
/// `[(building, tx, ty, base level)]`. One item per building group, at the north-west corner of its
/// first part.
pub fn placed_rooms(lots: &[(&Building, i64, i64, i64)]) -> Result<Vec<(i64, i64, Vec<Room>)>, LotError> {
    let mut order: Vec<usize> = (0..lots.len()).collect();
    order.sort_by_key(|&i| (lots[i].2.div_euclid(300), lots[i].1.div_euclid(300), i));
    let rank: HashMap<usize, usize> = order.iter().enumerate().map(|(r, &i)| (i, r)).collect();
    let mut owner: HashMap<(i64, i64, i64), (usize, i64)> = HashMap::new();
    for &i in &order {
        let (b, tx, ty, base) = lots[i];
        for (z, floor) in b.floors.iter().enumerate() {
            for (y, row) in floor.rooms.iter().enumerate() {
                for (x, &v) in row.iter().enumerate() {
                    if v > 0 {
                        owner.insert((tx + x as i64, ty + y as i64, base + z as i64), (i, v));
                    }
                }
            }
        }
    }
    let mut squares: HashMap<(usize, i64, i64), BTreeSet<(i64, i64)>> = HashMap::new();
    for (&(x, y, z), &(i, rid)) in &owner {
        squares.entry((i, z, rid)).or_default().insert((x, y));
    }
    // parts: (lot, level, room number) with squares left
    let mut parts: Vec<(usize, i64, i64)> = squares.keys().copied().collect();
    parts.sort_by_key(|k| (rank[&k.0], k.1, k.2));
    let mut at: HashMap<(i64, i64, i64), (usize, i64, i64)> = HashMap::new();
    for k in &parts {
        for &(x, y) in &squares[k] {
            at.insert((x, y, k.1), *k);
        }
    }
    let mut lot_parent: Vec<usize> = (0..lots.len()).collect();
    for k in &parts {
        for &(x, y) in &squares[k] {
            for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                // touching includes corners
                if let Some(n) = at.get(&(x + dx, y + dy, k.1)) {
                    if n.0 != k.0 {
                        let a = find(&mut lot_parent, n.0);
                        let b = find(&mut lot_parent, k.0);
                        lot_parent[a] = b;
                    }
                }
            }
        }
    }
    // a building group: the lots that touch; its corner is that of its first part
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for &i in &order {
        let root = find(&mut lot_parent, i);
        match groups.iter_mut().find(|(r, _)| *r == root) {
            Some(g) => g.1.push(i),
            None => groups.push((root, vec![i])),
        }
    }
    let mut out = Vec::new();
    for (_, members) in groups {
        let first = members[0];
        let mut entries: Vec<Room> = Vec::new();
        for k in &parts {
            if !members.contains(&k.0) {
                continue;
            }
            // parts of one room number in one lot stay one entry each (nothing joins them)
            entries.push(Room { name: room_name(lots[k.0].0, k.2).map_err(|msg| LotError { lot: k.0, msg })?, z: k.1, rects: room_rects(&squares[k]) });
        }
        out.push((lots[first].1, lots[first].2, entries));
    }
    Ok(out)
}

/// (rooms, buildings) of one cell. A building is listed in the cell that holds its north-west
/// corner, with every rectangle whole, even where it runs past the cell's edge.
pub fn cell_tables(cell_x: i64, cell_y: i64, placed: &[(i64, i64, Vec<Room>)]) -> (Vec<Room>, Vec<Vec<usize>>) {
    let (x0, y0) = (cell_x * CELL - ORIGIN_X, cell_y * CELL);
    let mut rooms: Vec<Room> = Vec::new();
    let mut buildings: Vec<Vec<usize>> = Vec::new();
    let mut sorted: Vec<&(i64, i64, Vec<Room>)> = placed.iter().collect();
    sorted.sort_by_key(|p| (p.1.div_euclid(300), p.0.div_euclid(300)));
    for (tx, ty, entries) in sorted {
        if entries.is_empty() {
            continue;
        }
        let all: Vec<&Rect> = entries.iter().flat_map(|e| e.rects.iter()).collect();
        if all.is_empty() {
            continue;
        }
        let tx = (*tx).min(all.iter().map(|r| r.0).min().unwrap());
        let ty = (*ty).min(all.iter().map(|r| r.1).min().unwrap());
        if !(x0 <= tx && tx < x0 + CELL && y0 <= ty && ty < y0 + CELL) {
            continue;
        }
        buildings.push((rooms.len()..rooms.len() + entries.len()).collect());
        for e in entries {
            rooms.push(Room { name: e.name.clone(), z: e.z, rects: e.rects.iter().map(|&(x, y, w, h)| (x - x0, y - y0, w, h)).collect() });
        }
    }
    (rooms, buildings)
}
