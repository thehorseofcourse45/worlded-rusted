//! Tile names (`<tileset>_<index>`) to gids, adding a tileset entry when the file lacks one.
use crate::tmx::Tmx;
use crate::tmxdoc::TmxDoc;
use crate::CoreError;

/// Tile pixel size of a PZ 2x tileset image cell.
const TILE_W: u32 = 64;
const TILE_H: u32 = 128;
/// A new tileset starts this far past the highest existing `firstgid`, clear of any real tileset
/// (their images hold well under this many tiles). WorldEd reassigns gids from tileset names on load.
const GID_GAP: u32 = 1024;

/// `"blends_natural_01_024"` to `("blends_natural_01", 24)`: split at the last `_` that is
/// followed only by digits and preceded by something.
pub fn split_tile_name(tile: &str) -> Option<(&str, u32)> {
    let (set, idx) = tile.rsplit_once('_')?;
    if set.is_empty() || idx.is_empty() || !idx.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((set, idx.parse().ok()?))
}

/// Largest gid: the top three bits of a tile id are flip flags.
const MAX_GID: u32 = 0x1FFF_FFFF;
/// Most tiles a tileset added here may be addressed by.
const MAX_NEW_INDEX: u32 = 65_535;

fn bad(msg: String) -> CoreError {
    CoreError::Parse(msg)
}

fn parse_name(tile: &str) -> Result<(&str, u32), CoreError> {
    let (set, idx) = split_tile_name(tile).ok_or_else(|| bad(format!("tile name without a numeric suffix: {tile}")))?;
    if !set.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-')) {
        return Err(bad(format!("tileset name has characters a map file cannot hold: {set}")));
    }
    Ok((set, idx))
}

/// The gid of `tile` when the file already lists its tileset (`None` when it does not), without
/// changing the document. The index must lie inside the tileset: below the next tileset's
/// `firstgid`, or below `GID_GAP` for the last one.
pub fn lookup_gid(tmx: &Tmx, tile: &str) -> Result<Option<u32>, CoreError> {
    let (set, idx) = parse_name(tile)?;
    let Some(ts) = tmx.tilesets.iter().find(|t| t.name == set) else { return Ok(None) };
    let room = tmx.tilesets.iter().map(|t| t.first_gid).filter(|&g| g > ts.first_gid).min().map_or(GID_GAP, |n| n - ts.first_gid);
    if idx >= room {
        return Err(bad(format!("{tile}: tileset {set} holds only {room} tiles")));
    }
    ts.first_gid.checked_add(idx).filter(|g| *g <= MAX_GID).map(Some).ok_or_else(|| bad(format!("{tile}: tile id out of range")))
}

/// The gid of `tile`. A tileset the file does not list is appended in the form WorldEd writes,
/// its image beside the others, `columns` tiles wide and as many rows as the index needs.
pub fn gid_for(doc: &mut TmxDoc, tile: &str, columns: u32) -> Result<u32, CoreError> {
    if let Some(g) = lookup_gid(&doc.tmx, tile)? {
        return Ok(g);
    }
    let (set, idx) = parse_name(tile)?;
    if idx > MAX_NEW_INDEX {
        return Err(bad(format!("{tile}: index too large for a new tileset")));
    }
    let columns = columns.max(1);
    let rows = idx / columns + 1;
    let top = doc.tmx.tilesets.iter().map(|t| t.first_gid).max();
    let first_gid = top.map_or(Some(1), |t| t.checked_add(GID_GAP)).filter(|f| f.saturating_add(MAX_NEW_INDEX) <= MAX_GID).ok_or_else(|| bad("no gid space left for a new tileset".into()))?;
    let source = image_dir(doc) + set + ".png";
    let mut element = format!(
        " <tileset firstgid=\"{first_gid}\" name=\"{set}\" tilewidth=\"{TILE_W}\" tileheight=\"{TILE_H}\">\n  <image source=\"{source}\" width=\"{}\" height=\"{}\"/>\n </tileset>\n",
        columns * TILE_W,
        rows * TILE_H
    );
    if doc.text().contains("\r\n") {
        element = element.replace('\n', "\r\n");
    }
    doc.insert_tileset(set, first_gid, &element)?;
    Ok(first_gid + idx)
}

/// Folder part (with trailing slash) of the last tileset image source in the file.
fn image_dir(doc: &TmxDoc) -> String {
    let text = doc.text();
    text.rfind("<image source=\"")
        .map(|i| &text[i + 15..])
        .and_then(|r| r.split('"').next())
        .and_then(|src| src.rfind('/').map(|s| src[..=s].to_string()))
        .unwrap_or_default()
}
