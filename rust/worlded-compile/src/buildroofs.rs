//! Roof and ceiling tiles of a building (port of knoxbuild/buildroofs.py).
use crate::tbx::{attr_int, Attrs, Building, Entry, R};
use std::collections::{BTreeMap, BTreeSet};

pub type Squares = BTreeMap<(i64, i64), Vec<String>>;

static EMPTY: Entry = Entry(Vec::new());

/// Tile entry number `number` (1-based, as written in attributes); an empty entry when out of range.
pub fn entry<'a>(b: &'a Building, number: &str) -> R<&'a Entry> {
    let n = crate::tbx::int(number)?;
    Ok(if 1 <= n && n <= b.entries.len() as i64 { &b.entries[(n - 1) as usize] } else { &EMPTY })
}

/// `o.get(key, attrs.get(default_key, 0))` as an entry number.
fn pick_entry<'a>(b: &'a Building, o: &Attrs, key: &str, default_key: &str) -> R<&'a Entry> {
    let n = o.get(key).or_else(|| b.attrs.get(default_key)).map(String::as_str).unwrap_or("0");
    entry(b, n)
}

fn depth_info(d: &str) -> Option<(i64, bool)> {
    Some(match d {
        "Point5" => (0, true),
        "One" => (1, false),
        "OnePoint5" => (1, true),
        "Two" => (2, false),
        "TwoPoint5" => (2, true),
        "Three" => (3, false),
        _ => return None,
    })
}

fn pt5(d: &str) -> &'static str {
    match d {
        "Point5" => "Pt5",
        "OnePoint5" => "OnePt5",
        "TwoPoint5" => "TwoPt5",
        _ => "",
    }
}

/// `{(x, y): [cap, slope]}` for a pitched roof object, over its rectangle and the squares just
/// outside it where its caps stand.
pub fn roof_squares(b: &Building, o: &Attrs) -> R<Squares> {
    let kind = o.get("RoofType").map(String::as_str).unwrap_or("");
    let mut depth = o.get("Depth").cloned().unwrap_or_default();
    let (x0, y0, mut w, mut h) = (attr_int(o, "x")?, attr_int(o, "y")?, attr_int(o, "width")?, attr_int(o, "height")?);
    let slopes = pick_entry(b, o, "SlopeTiles", "RoofSlope")?;
    let caps = pick_entry(b, o, "CapTiles", "RoofCap")?;
    let capped = |d: &str| o.get(&format!("capped{d}")).map_or(false, |v| v == "true");
    if depth == "Three" && (kind == "PeakWE" || kind == "PeakNS") {
        // A roof too narrow for three rows each side takes the depth that fits.
        let across = if kind == "PeakWE" { h } else { w };
        depth = match across {
            1 => "Point5",
            2 => "One",
            3 => "OnePoint5",
            4 => "Two",
            5 => "TwoPoint5",
            _ => "Three",
        }
        .to_string();
    }
    let mut slope_tiles: BTreeMap<(i64, i64), String> = BTreeMap::new();
    let mut cap_tiles: BTreeMap<(i64, i64), String> = BTreeMap::new();
    let mut slope = |x: i64, y: i64, e: String| {
        slope_tiles.insert((x, y), slopes.get_or_empty(&e).to_string());
    };
    let mut cap = |x: i64, y: i64, e: String| {
        cap_tiles.insert((x, y), caps.get_or_empty(&e).to_string());
    };

    if (kind == "PeakWE" || kind == "PeakNS") && depth_info(&depth).is_some() {
        let (k, ridge) = depth_info(&depth).unwrap();
        let p = pt5(&depth);
        if kind == "PeakWE" {
            for i in 1..=k {
                for x in x0..x0 + w {
                    slope(x, y0 + h - i, format!("SlopeS{i}"));
                }
            }
            if ridge {
                for x in x0..x0 + w {
                    slope(x, y0 + h - k - 1, format!("Slope{p}S"));
                }
            }
            for (side, x) in [("W", x0), ("E", x0 + w)] {
                if !capped(side) {
                    continue;
                }
                for i in 1..=k {
                    cap(x, y0 + i - 1, format!("CapFallE{i}"));
                    cap(x, y0 + h - i, format!("CapRiseE{i}"));
                }
                if ridge {
                    cap(x, y0 + k, format!("Peak{p}E"));
                }
                if depth == "Three" {
                    for y in y0 + k..y0 + h - k {
                        cap(x, y, "CapGapE3".to_string());
                    }
                }
            }
        } else {
            for i in 1..=k {
                for y in y0..y0 + h {
                    slope(x0 + w - i, y, format!("SlopeE{i}"));
                }
            }
            if ridge {
                for y in y0..y0 + h {
                    slope(x0 + w - k - 1, y, format!("Slope{p}E"));
                }
            }
            for (side, y) in [("N", y0), ("S", y0 + h)] {
                if !capped(side) {
                    continue;
                }
                for i in 1..=k {
                    cap(x0 + i - 1, y, format!("CapRiseS{i}"));
                    cap(x0 + w - i, y, format!("CapFallS{i}"));
                }
                if ridge {
                    cap(x0 + k, y, format!("Peak{p}S"));
                }
                if depth == "Three" {
                    for x in x0 + k..x0 + w - k {
                        cap(x, y, "CapGapS3".to_string());
                    }
                }
            }
        }
    } else if kind == "Peak30WE" {
        // WorldEd limits the cross-section to eleven squares, anchored at the north edge.
        h = h.min(11);
        let k = (h - 1).div_euclid(2);
        for x in x0..x0 + w {
            for i in 1..=k {
                slope(x, y0 + i - 1, format!("Slope30N{i}"));
                slope(x, y0 + h - i, format!("Slope30S{i}"));
            }
            slope(x, y0 + k, format!("Peak30NS{}", k + 1));
        }
        for (side, x) in [("W", x0), ("E", x0 + w)] {
            if capped(side) {
                for i in 1..=k {
                    cap(x, y0 + i - 1, format!("CapSlope30FallE{i}"));
                    cap(x, y0 + h - i, format!("CapSlope30RiseE{i}"));
                }
                cap(x, y0 + k, format!("CapPeak30E{}", k + 1));
            }
        }
    } else if kind == "Peak30NS" {
        // WorldEd limits the cross-section to eleven squares, anchored at the west edge.
        w = w.min(11);
        let k = (w - 1).div_euclid(2);
        for y in y0..y0 + h {
            for i in 1..=k {
                slope(x0 + i - 1, y, format!("Slope30W{i}"));
                slope(x0 + w - i, y, format!("Slope30E{i}"));
            }
            slope(x0 + k, y, format!("Peak30WE{}", k + 1));
        }
        for (side, y) in [("N", y0), ("S", y0 + h)] {
            if capped(side) {
                for i in 1..=k {
                    cap(x0 + i - 1, y, format!("CapSlope30RiseS{i}"));
                    cap(x0 + w - i, y, format!("CapSlope30FallS{i}"));
                }
                cap(x0 + k, y, format!("CapPeak30S{}", k + 1));
            }
        }
    } else if kind == "Peak30Quad" && w.max(h).rem_euclid(2) == 1 {
        // A rectangle is cut from the square of its longer side, anchored at the north-west.
        let side = w.max(h);
        let k = (side - 1).div_euclid(2);
        for dy in 0..h {
            for dx in 0..w {
                let r = dx.min(dy).min(side - 1 - dx).min(side - 1 - dy) + 1;
                let e = if r == k + 1 {
                    format!("Peak30Quad{}", k + 1)
                } else {
                    let (top, bottom, left, right) = (dy == r - 1, dy == side - r, dx == r - 1, dx == side - r);
                    if top && left {
                        format!("OuterSlope30NW{r}")
                    } else if top && right {
                        format!("OuterSlope30NE{r}")
                    } else if bottom && left {
                        format!("OuterSlope30SW{r}")
                    } else if bottom && right {
                        format!("OuterSlope30SE{r}")
                    } else if top {
                        format!("Slope30N{r}")
                    } else if bottom {
                        format!("Slope30S{r}")
                    } else if left {
                        format!("Slope30W{r}")
                    } else {
                        format!("Slope30E{r}")
                    }
                };
                slope(x0 + dx, y0 + dy, e);
            }
        }
    }
    let positions: BTreeSet<(i64, i64)> = cap_tiles.keys().chain(slope_tiles.keys()).copied().collect();
    let mut out = Squares::new();
    for pos in positions {
        let tiles: Vec<String> = [cap_tiles.get(&pos), slope_tiles.get(&pos)].into_iter().flatten().filter(|t| !t.is_empty()).cloned().collect();
        out.insert(pos, tiles);
    }
    Ok(out)
}

fn rect(o: &Attrs) -> R<(i64, i64, i64, i64)> {
    Ok((attr_int(o, "x")?, attr_int(o, "y")?, attr_int(o, "width")?, attr_int(o, "height")?))
}

/// `tops.get("West1") or next(iter(tops.values()), "")`
fn top_tile(tops: &Entry, first: &str) -> String {
    match tops.get(first) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => tops.values().next().cloned().unwrap_or_default(),
    }
}

pub fn flat_squares(b: &Building, o: &Attrs) -> R<Squares> {
    let (x0, y0, w, h) = rect(o)?;
    let tile = top_tile(pick_entry(b, o, "TopTiles", "RoofTop")?, "West1");
    let mut out = Squares::new();
    if !tile.is_empty() {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                out.insert((x, y), vec![tile.clone()]);
            }
        }
    }
    Ok(out)
}

/// The flat band between the slopes of a depth Three peaked roof more than six squares across.
pub fn band_squares(b: &Building, o: &Attrs) -> R<Squares> {
    let kind = o.get("RoofType").map(String::as_str).unwrap_or("");
    let mut out = Squares::new();
    if o.get("Depth").map(String::as_str) != Some("Three") || !(kind == "PeakWE" || kind == "PeakNS") {
        return Ok(out);
    }
    let (x0, y0, w, h) = rect(o)?;
    let tile = top_tile(pick_entry(b, o, "TopTiles", "RoofTop")?, if kind == "PeakWE" { "West1" } else { "North1" });
    if tile.is_empty() {
        return Ok(out);
    }
    if kind == "PeakWE" {
        for y in y0 + 3..y0 + h - 3 {
            for x in x0..x0 + w {
                out.insert((x, y), vec![tile.clone()]);
            }
        }
    } else {
        for y in y0..y0 + h {
            for x in x0 + 3..x0 + w - 3 {
                out.insert((x, y), vec![tile.clone()]);
            }
        }
    }
    Ok(out)
}

/// What goes on level `z` because of the storey below and the roofs: (ceilings and flat roofs,
/// pitched roofs).
pub fn upper_layers(b: &Building, z: usize) -> R<(Squares, Squares)> {
    let mut out = Squares::new();
    if z >= 1 {
        let below = &b.floors[z - 1];
        let here = b.floors.get(z).map(|f| &f.rooms);
        for y in 0..b.height as usize {
            for x in 0..b.width as usize {
                let r = below.rooms[y][x];
                if r > 0 && here.map_or(true, |h| h[y][x] == 0) {
                    let room = b.rooms.get((r - 1) as usize).ok_or_else(|| format!("room {r} is not defined"))?;
                    let ceiling = entry(b, room.get("Ceiling").map(String::as_str).unwrap_or("0"))?.get_or_empty("Ceiling");
                    if !ceiling.is_empty() {
                        out.insert((x as i64, y as i64), vec![ceiling.to_string()]);
                    }
                }
            }
        }
        for o in &below.objects {
            if o.get("type").map(String::as_str) == Some("roof") {
                if o.get("RoofType").map(String::as_str) == Some("FlatTop") {
                    out.extend(flat_squares(b, o)?);
                } else {
                    out.extend(band_squares(b, o)?);
                }
            }
        }
    }
    let mut pitched = Squares::new();
    if let Some(f) = b.floors.get(z) {
        for o in &f.objects {
            if o.get("type").map(String::as_str) == Some("roof") && o.get("RoofType").map(String::as_str) != Some("FlatTop") {
                for (pos, tiles) in roof_squares(b, o)? {
                    pitched.entry(pos).or_default().extend(tiles);
                }
            }
        }
    }
    Ok((out, pitched))
}

/// Combined upper layers, for callers that only need the roof and ceiling tiles.
pub fn upper_level(b: &Building, z: usize) -> R<Squares> {
    let (mut out, pitched) = upper_layers(b, z)?;
    for (pos, tiles) in pitched {
        out.entry(pos).or_default().extend(tiles);
    }
    Ok(out)
}
