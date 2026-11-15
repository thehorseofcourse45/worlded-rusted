//! Tiles for a building, from its .tbx (port of knoxbuild/buildtiles.py `level_layers` and friends).
//! What each part of a building becomes was worked out from WorldEd output; see the Python
//! docstrings and comments, which this keeps rule for rule.
use crate::buildroofs::{entry, upper_layers};
use crate::tbx::{attr_int, int, Attrs, Building, Entry, LotError, MAX_SIZE, R};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub type Stack = Vec<(String, String)>;
pub type Layers = BTreeMap<(i64, i64), Stack>;
type Tiles = BTreeMap<(i64, i64), Vec<String>>;

const SPECIAL_WINDOWS: [(i64, i64); 8] = [(40, 17), (42, 17), (48, 17), (50, 17), (41, 18), (43, 18), (49, 18), (51, 18)];

/// Which numbered Window variant of a wall holds this window (0 = the plain window wall).
pub fn window_variant(window_tile: &str) -> i64 {
    let digits = |s: &str| -> Option<i64> {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    if let Some(rest) = window_tile.strip_prefix("fixtures_windows_01_") {
        return digits(rest).and_then(|n| SPECIAL_WINDOWS.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)).unwrap_or(0);
    }
    for fam in ["white", "metal", "wood", "church"] {
        if let Some(rest) = window_tile.strip_prefix(&format!("fixtures_windows_{fam}_")) {
            return digits(rest).map_or(0, |n| n / 2 + 1);
        }
    }
    0
}

/// The three squares a flight of stairs stands on, from the lowest step's end.
fn stair_squares(o: &Attrs) -> R<Vec<(i64, i64)>> {
    let (x, y) = (attr_int(o, "x")?, attr_int(o, "y")?);
    Ok(if o.get("dir").map(String::as_str) == Some("N") { (0..3).map(|i| (x, y + 1 + i)).collect() } else { (0..3).map(|i| (x + 1 + i, y)).collect() })
}

/// `e.get(kind) or e.get(fallback, "")`
fn or_get<'a>(e: &'a Entry, kind: &str, fallback: &str) -> &'a str {
    match e.get(kind) {
        Some(s) if !s.is_empty() => s,
        _ => e.get_or_empty(fallback),
    }
}

fn truthy(e: &Entry, kind: &str) -> bool {
    e.get(kind).is_some_and(|s| !s.is_empty())
}

/// An entry named by attribute `key` of `o` (default entry 0 = none).
fn ent<'a>(b: &'a Building, o: &Attrs, key: &str) -> R<&'a Entry> {
    entry(b, o.get(key).map(String::as_str).unwrap_or("0"))
}

/// `o.get(key, attrs.get(default_key, 0))`
fn ent_or<'a>(b: &'a Building, o: &Attrs, key: &str, default_key: &str) -> R<&'a Entry> {
    entry(b, o.get(key).or_else(|| b.attrs.get(default_key)).map(String::as_str).unwrap_or("0"))
}

fn s(x: &str) -> String {
    x.to_string()
}

/// `{(x, y): [(layer, tile)]}`, bottom first, over the building and the ring of squares outside its
/// south and east sides, where its exterior walls stand.
pub fn level_layers(b: &Building, z: usize) -> R<Layers> {
    let floor = b.floors.get(z).ok_or_else(|| format!("no floor {z}"))?;
    let (w, h) = (b.width, b.height);
    let room = |x: i64, y: i64| -> i64 {
        if 0 <= x && x < w && 0 <= y && y < h {
            floor.rooms[y as usize][x as usize]
        } else {
            0
        }
    };
    let ext = ent(b, &b.attrs, "ExteriorWall")?;
    let trim = ent(b, &b.attrs, "ExteriorWallTrim")?;
    let grime = ent(b, &b.attrs, "GrimeWall")?;
    let ground = z == 0;

    let mut custom_walls: HashMap<(i64, i64, &str), &Attrs> = HashMap::new();
    let mut custom_ends: HashMap<(i64, i64), &Attrs> = HashMap::new();
    for o in &floor.objects {
        if o.get("type").map(String::as_str) == Some("wall") {
            let horizontal = o.get("dir").map_or("W", String::as_str) == "W";
            let length = match o.get("length") {
                Some(v) => int(v)?,
                None => 1,
            };
            if length.abs() > MAX_SIZE || attr_int(o, "x")?.abs() > 1_000_000 || attr_int(o, "y")?.abs() > 1_000_000 {
                return Err(format!("wall of length {length} at {},{} is out of range", attr_int(o, "x")?, attr_int(o, "y")?));
            }
            for i in 0..length {
                let (x, y) = (attr_int(o, "x")? + if horizontal { i } else { 0 }, attr_int(o, "y")? + if horizontal { 0 } else { i });
                custom_walls.insert((x, y, if horizontal { "North" } else { "West" }), o);
            }
            custom_ends.insert((attr_int(o, "x")? + if horizontal { length } else { 0 }, attr_int(o, "y")? + if horizontal { 0 } else { length }), o);
        }
    }
    let west = |x: i64, y: i64| custom_walls.contains_key(&(x, y, "West")) || room(x - 1, y) != room(x, y);
    let north = |x: i64, y: i64| custom_walls.contains_key(&(x, y, "North")) || room(x, y - 1) != room(x, y);

    let mut floors: Tiles = BTreeMap::new();
    let mut walls: Tiles = BTreeMap::new();
    let mut extra: Tiles = BTreeMap::new();
    let mut kind_at: HashMap<(i64, i64), &str> = HashMap::new();

    let wall_tiles = |x: i64, y: i64, kind: &str, fallback: &str, end: bool| -> R<Vec<String>> {
        let r = room(x, y);
        let direction = if kind.starts_with("North") { "North" } else { "West" };
        let mut custom = custom_walls.get(&(x, y, direction)).copied();
        if kind == "SouthEast" {
            custom = if end { custom_ends.get(&(x, y)).copied() } else { None };
        }
        if let Some(c) = custom {
            let e = ent(b, c, if r != 0 { "InteriorTile" } else { "Tile" })?;
            let t = ent(b, c, if r != 0 { "InteriorTrim" } else { "ExteriorTrim" })?;
            let mut v = vec![s(or_get(e, kind, fallback)), s(t.get_or_empty(if truthy(e, kind) { kind } else { fallback }))];
            if r == 0 && ground {
                v.push(s(grime.get_or_empty(kind)));
            }
            return Ok(v);
        }
        if r > 0 {
            let room_def = b.rooms.get((r - 1) as usize).ok_or_else(|| format!("room {r} is not defined"))?;
            let e = entry(b, room_def.get("InteriorWall").ok_or("room without InteriorWall")?)?;
            let mut tiles = vec![s(or_get(e, kind, fallback))];
            let trim_no = match room_def.get("InteriorWallTrim") {
                Some(v) => int(v)?,
                None => 0,
            };
            if trim_no != 0 {
                tiles.push(s(entry(b, &room_def["InteriorWallTrim"])?.get_or_empty(if truthy(e, kind) { kind } else { fallback })));
            }
            return Ok(tiles);
        }
        let mut v = vec![s(ext.get_or_empty(kind))];
        if ground {
            v.push(s(trim.get_or_empty(kind)));
            v.push(s(grime.get_or_empty(kind)));
        }
        Ok(v)
    };

    // Stairs: three stair tiles on this level, and a hole (no floor tile) over them on the level above.
    let mut holes: HashSet<(i64, i64)> = HashSet::new();
    if z >= 1 {
        for o in &b.floors[z - 1].objects {
            if o.get("type").map(String::as_str) == Some("stairs") {
                holes.extend(stair_squares(o)?);
            }
        }
    }

    let mut wall_furniture: BTreeMap<(i64, i64), Vec<(String, String)>> = BTreeMap::new();
    for o in &floor.objects {
        if o.get("type").map(String::as_str) != Some("furniture") {
            continue;
        }
        let idx = match o.get("FurnitureTiles") {
            Some(v) => int(v)?,
            None => -1,
        };
        if 0 <= idx && (idx as usize) < b.furniture.len() && b.furniture[idx as usize].layer == "Walls" {
            let orient = o.get("orient").map_or("N", String::as_str);
            for (dx, dy, tile) in b.furniture[idx as usize].orients.get(orient).map(Vec::as_slice).unwrap_or(&[]) {
                let pos = (attr_int(o, "x")? + dx, attr_int(o, "y")? + dy);
                wall_furniture.entry(pos).or_default().push((s(orient), tile.clone()));
            }
        }
    }

    for y in 0..=h {
        for x in 0..=w {
            let r = room(x, y);
            if r > 0 && !holes.contains(&(x, y)) {
                let room_def = b.rooms.get((r - 1) as usize).ok_or_else(|| format!("room {r} is not defined"))?;
                floors.insert((x, y), vec![s(entry(b, room_def.get("Floor").ok_or("room without Floor")?)?.get_or_empty("Floor"))]);
            }
            let (mut wl, mut nl) = (west(x, y), north(x, y));
            for (orient, _) in wall_furniture.get(&(x, y)).map(Vec::as_slice).unwrap_or(&[]) {
                if orient == "W" {
                    wl = false;
                } else if orient == "N" {
                    nl = false;
                }
            }
            let kind = if wl && nl {
                "NorthWest"
            } else if wl {
                "West"
            } else if nl {
                "North"
            } else {
                ""
            };
            if !kind.is_empty() {
                kind_at.insert((x, y), kind);
                if kind == "NorthWest" && (custom_walls.contains_key(&(x, y, "North")) || custom_walls.contains_key(&(x, y, "West"))) {
                    let cd = if custom_walls.contains_key(&(x, y, "North")) { "North" } else { "West" };
                    let custom = wall_tiles(x, y, cd, "", false)?;
                    let other = wall_tiles(x, y, if cd == "North" { "West" } else { "North" }, "", false)?;
                    let mut v = custom[..1].to_vec();
                    v.extend(other);
                    v.extend_from_slice(&custom[1..]);
                    walls.insert((x, y), v);
                } else {
                    walls.insert((x, y), wall_tiles(x, y, kind, "", false)?);
                }
            } else if !wall_furniture.contains_key(&(x, y)) && west(x, y - 1) && north(x - 1, y) {
                walls.insert((x, y), wall_tiles(x, y, "SouthEast", "", false)?);
            } else if custom_ends.contains_key(&(x, y)) {
                walls.insert((x, y), wall_tiles(x, y, "SouthEast", "", true)?);
            }
        }
    }
    for (pos, items) in &wall_furniture {
        let mut v = walls.get(pos).cloned().unwrap_or_default();
        v.extend(items.iter().map(|(_, t)| t.clone()));
        walls.insert(*pos, v);
    }

    let mut curtains: Tiles = BTreeMap::new();
    let mut curtains_before: Tiles = BTreeMap::new();
    let mut stair_tiles: Tiles = BTreeMap::new();

    for o in &floor.objects {
        if o.get("type").map(String::as_str) == Some("stairs") {
            let steps = ent_or(b, o, "Tile", "Stairs")?;
            let name = if o.get("dir").map(String::as_str) == Some("N") { "North" } else { "West" };
            for (i, pos) in stair_squares(o)?.into_iter().enumerate() {
                stair_tiles.entry(pos).or_default().push(s(steps.get_or_empty(&format!("{name}{}", 3 - i))));
            }
        }
    }

    // doors and windows; a repeated (type, x, y, dir) keeps its first position and its last value
    let mut openings: Vec<((String, String, String, String), &Attrs)> = Vec::new();
    for o in &floor.objects {
        let t = o.get("type").map(String::as_str);
        if t == Some("door") || t == Some("window") {
            let key = (s(t.unwrap()), o.get("x").cloned().ok_or("opening without x")?, o.get("y").cloned().ok_or("opening without y")?, o.get("dir").cloned().unwrap_or_else(|| s("N")));
            match openings.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = o,
                None => openings.push((key, o)),
            }
        }
    }
    let add = |extra: &mut Tiles, x: i64, y: i64, tile: &str| extra.entry((x, y)).or_default().push(s(tile));
    for (_, o) in &openings {
        let kind = o["type"].as_str();
        let (x, y) = (attr_int(o, "x")?, attr_int(o, "y")?);
        let d = o.get("dir").map_or("N", String::as_str);
        let dname = if d == "N" { "North" } else { "West" };
        if !kind_at.contains_key(&(x, y)) {
            continue; // nothing to put it in
        }
        let r = room(x, y);
        let preserve_corner = |tiles: Vec<String>| -> R<Vec<String>> {
            if kind_at[&(x, y)] != "NorthWest" {
                return Ok(tiles);
            }
            let other = wall_tiles(x, y, if d == "N" { "West" } else { "North" }, "", false)?;
            if r == 0 && tiles.len() == 3 && other.len() == 3 {
                let mut v = other[..2].to_vec();
                v.extend(tiles);
                v.extend_from_slice(&other[2..]);
                return Ok(v);
            }
            let mut v = other;
            v.extend(tiles);
            Ok(v)
        };

        if kind == "door" {
            walls.insert((x, y), preserve_corner(wall_tiles(x, y, &format!("{dname}Door"), "", false)?)?);
            add(&mut extra, x, y, ent_or(b, o, "FrameTile", "DoorFrame")?.get_or_empty(dname));
            add(&mut extra, x, y, ent_or(b, o, "Tile", "Door")?.get_or_empty(dname));
            continue;
        }
        let win = ent_or(b, o, "Tile", "Window")?.get_or_empty(dname);
        let v = window_variant(win);
        let variant = if v != 0 { format!("{dname}Window{v}") } else { format!("{dname}Window") };
        if r > 0 {
            walls.insert((x, y), preserve_corner(wall_tiles(x, y, &variant, &format!("{dname}Window"), false)?)?);
        } else {
            let kind = if truthy(ext, &variant) { variant.clone() } else { format!("{dname}Window") };
            let mut base = vec![s(ext.get_or_empty(&kind))];
            if ground {
                base.push(s(trim.get_or_empty(&kind)));
                base.push(s(grime.get_or_empty(&kind)));
            }
            walls.insert((x, y), preserve_corner(base)?);
        }
        if r == 0 {
            let sh = ent(b, o, "ShuttersTile")?;
            if !sh.is_empty() {
                let names = if d == "N" { ("NorthLeft", "NorthRight") } else { ("WestAbove", "WestBelow") };
                add(&mut extra, x, y, sh.get_or_empty(names.0));
                add(&mut extra, x, y, sh.get_or_empty(names.1));
            }
        }
        add(&mut extra, x, y, ent_or(b, o, "Tile", "Window")?.get_or_empty(dname));
        let cur = ent(b, o, "CurtainsTile")?;
        if !cur.is_empty() {
            if r > 0 {
                add(&mut curtains_before, x, y, cur.get_or_empty(dname));
            } else {
                // the inside square is the one before
                let (ix, iy) = if d == "N" { (x, y - 1) } else { (x - 1, y) };
                add(&mut curtains, ix, iy, cur.get_or_empty(if d == "N" { "South" } else { "East" }));
            }
        }
        if r == 0 {
            let sh = ent(b, o, "ShuttersTile")?;
            if !sh.is_empty() {
                if d == "N" {
                    if x > 0 && room(x - 1, y) == 0 {
                        add(&mut extra, x - 1, y, sh.get_or_empty("NorthLeft"));
                    }
                    if x < w && room(x + 1, y) == 0 {
                        add(&mut extra, x + 1, y, sh.get_or_empty("NorthRight"));
                    }
                } else {
                    if y > 0 && room(x, y - 1) == 0 {
                        add(&mut extra, x, y - 1, sh.get_or_empty("WestAbove"));
                    }
                    if y < h && room(x, y + 1) == 0 {
                        add(&mut extra, x, y + 1, sh.get_or_empty("WestBelow"));
                    }
                }
            }
        }
    }

    // Furniture goes onto the square in the order of BuildingEd's layers (see the Python source).
    let mut furn_floor: Tiles = BTreeMap::new();
    let mut furn: BTreeMap<(i64, i64), Vec<(f64, String)>> = BTreeMap::new();
    for o in &floor.objects {
        if o.get("type").map(String::as_str) != Some("furniture") {
            continue;
        }
        let idx = match o.get("FurnitureTiles") {
            Some(v) => int(v)?,
            None => -1,
        };
        if !(0 <= idx && (idx as usize) < b.furniture.len()) {
            continue;
        }
        let d = &b.furniture[idx as usize];
        if d.layer == "Walls" {
            continue;
        }
        let orient = o.get("orient").map_or("N", String::as_str);
        let mut rank: f64 = match d.layer.as_str() {
            "FloorFurniture" => -1.0,
            "Roof" => 3.0,
            _ => 0.0,
        };
        if d.layer == "WallFurniture" {
            rank = if orient == "N" || orient == "W" { 0.0 } else { 2.0 };
        } else if rank == 0.0 {
            rank = 1.0;
        }
        for (k, (dx, dy, name)) in d.orients.get(orient).map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
            let pos = (attr_int(o, "x")? + dx, attr_int(o, "y")? + dy);
            // Further tiles follow primary furniture tiles, before east/south curtains.
            let r = if rank == 1.0 && k > 0 { 1.5 } else { rank };
            if r < 0.0 {
                furn_floor.entry(pos).or_default().push(name.clone());
            } else {
                furn.entry(pos).or_default().push((r, name.clone()));
            }
        }
    }
    let pick_rank = |lo: f64, hi: f64| -> Tiles {
        furn.iter()
            .map(|(pos, items)| {
                let mut sel: Vec<&(f64, String)> = items.iter().filter(|i| lo <= i.0 && i.0 < hi).collect();
                sel.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()); // stable, like sorted(key=...)
                (*pos, sel.into_iter().map(|i| i.1.clone()).collect())
            })
            .collect()
    };
    let furn_before = pick_rank(0.0, 0.5);
    let furn_mid = pick_rank(1.0, 2.0);
    let furn_after = pick_rank(2.0, 2.5);
    let furn_roof = pick_rank(3.0, 3.5);

    let (upper_floor, upper) = upper_layers(b, z)?;
    let mut trim_names: BTreeSet<String> = trim.values().cloned().collect();
    for rd in &b.rooms {
        trim_names.extend(ent(b, rd, "InteriorWallTrim")?.values().cloned());
    }
    for o in custom_walls.values() {
        for key in ["InteriorTrim", "ExteriorTrim"] {
            trim_names.extend(ent(b, o, key)?.values().cloned());
        }
    }
    let grime_names: BTreeSet<String> = grime.values().cloned().collect();
    let mut north_trim: BTreeSet<String> = BTreeSet::new();
    for e in &b.entries {
        for (en, tile) in &e.0 {
            if trim_names.contains(tile) && (en == "SouthEast" || (en.starts_with("North") && en != "NorthWest")) {
                north_trim.insert(tile.clone());
            }
        }
    }
    let north_grime: BTreeSet<String> = grime.0.iter().filter(|(en, _)| en.starts_with("North") && en != "NorthWest").map(|(_, t)| t.clone()).collect();

    // bottom to top: floor, rugs, walls, wall furniture (north/west), door frames, doors, windows,
    // ordinary furniture, curtains, wall furniture (east/south), roof furniture
    let mut painted: Tiles = BTreeMap::new();
    for (_layer, grid) in &floor.tile_layers {
        for (y, row) in grid.iter().enumerate() {
            for (x, tile) in row.iter().enumerate() {
                if !tile.is_empty() {
                    painted.entry((x as i64, y as i64)).or_default().push(tile.clone());
                }
            }
        }
    }
    let groups: Vec<(&str, &Tiles)> = vec![
        ("floor", &floors),
        ("floor", &upper_floor),
        ("floor_furniture", &furn_floor),
        ("walls", &walls),
        ("wall_furniture_nw", &furn_before),
        ("painted", &painted),
        ("openings", &extra),
        ("curtains_nw", &curtains_before),
        ("stairs", &stair_tiles),
        ("furniture", &furn_mid),
        ("curtains_se", &curtains),
        ("wall_furniture_se", &furn_after),
        ("roof", &upper),
        ("roof_furniture", &furn_roof),
    ];
    let positions: BTreeSet<(i64, i64)> = groups.iter().flat_map(|(_, g)| g.keys().copied()).collect();
    let mut out = Layers::new();
    for pos in positions {
        let mut stack = Stack::new();
        for (layer, group) in &groups {
            for tile in group.get(&pos).map(Vec::as_slice).unwrap_or(&[]) {
                if tile.is_empty() {
                    continue;
                }
                let mut channel = s(layer);
                if *layer == "walls" {
                    if trim_names.contains(tile) {
                        channel = s(if north_trim.contains(tile) { "wall_trim_north" } else { "wall_trim_west" });
                    } else if grime_names.contains(tile) {
                        channel = s(if north_grime.contains(tile) { "wall_grime_north" } else { "wall_grime_west" });
                    }
                }
                stack.push((channel, tile.clone()));
            }
        }
        if !stack.is_empty() {
            out.insert(pos, stack);
        }
    }
    Ok(out)
}

/// Building tiles in drawing order.
pub fn level_tiles(b: &Building, z: usize) -> R<Tiles> {
    Ok(level_layers(b, z)?.into_iter().map(|(pos, items)| (pos, items.into_iter().map(|(_, t)| t).collect())).collect())
}

/// Compose (building, x, y, base level) placements in project order. Rooms replace earlier lot
/// squares. Outside rooms, only populated layers replace matching earlier layers; empty squares
/// preserve the underlying lot.
pub fn compose_lots(lots: &[(&Building, i64, i64, i64)]) -> Result<BTreeMap<(i64, i64, i64), Vec<String>>, LotError> {
    let mut world: BTreeMap<(i64, i64, i64), Stack> = BTreeMap::new();
    for (lot, (b, tx, ty, base)) in lots.iter().enumerate() {
        for (z, floor) in b.floors.iter().enumerate() {
            let layers = level_layers(b, z).map_err(|msg| LotError { lot, msg })?;
            let mut rooms: BTreeSet<(i64, i64)> = BTreeSet::new();
            for (y, row) in floor.rooms.iter().enumerate() {
                for (x, r) in row.iter().enumerate() {
                    if *r > 0 {
                        rooms.insert((x as i64, y as i64));
                    }
                }
            }
            let all: BTreeSet<(i64, i64)> = rooms.iter().copied().chain(layers.keys().copied()).collect();
            for (x, y) in all {
                let pos = (tx + x, ty + y, base + z as i64);
                let new = layers.get(&(x, y)).cloned().unwrap_or_default();
                if rooms.contains(&(x, y)) {
                    world.insert(pos, new);
                } else {
                    let channels: HashSet<&str> = new.iter().map(|(l, _)| l.as_str()).collect();
                    let mut old = world.get(&pos).cloned().unwrap_or_default();
                    if channels.contains("floor") {
                        old.clear(); // a floor tile (a flat roof is one) takes the square
                    }
                    let mut merged: Stack = old.into_iter().filter(|(l, _)| !channels.contains(l.as_str())).collect();
                    merged.extend(new.iter().cloned());
                    world.insert(pos, merged);
                }
            }
        }
    }
    Ok(world.into_iter().filter(|(_, items)| !items.is_empty()).map(|(pos, items)| (pos, items.into_iter().map(|(_, t)| t).collect())).collect())
}
