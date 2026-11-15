//! Tile names (`<tileset>_<index>`) to gids, adding a tileset entry when the file lacks one.
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

/// The gid of `tile`. A tileset the file does not list is appended in the form WorldEd writes,
/// its image beside the others, `columns` tiles wide and as many rows as the index needs.
pub fn gid_for(doc: &mut TmxDoc, tile: &str, columns: u32) -> Result<u32, CoreError> {
    let (set, idx) = split_tile_name(tile).ok_or_else(|| CoreError::Parse(format!("tile name without a numeric suffix: {tile}")))?;
    if let Some(ts) = doc.tmx.tilesets.iter().find(|t| t.name == set) {
        return Ok(ts.first_gid + idx);
    }
    let columns = columns.max(1);
    let rows = idx / columns + 1;
    let first_gid = doc.tmx.tilesets.iter().map(|t| t.first_gid).max().unwrap_or(0) + if doc.tmx.tilesets.is_empty() { 1 } else { GID_GAP };
    let source = image_dir(doc) + set + ".png";
    let element = format!(
        " <tileset firstgid=\"{first_gid}\" name=\"{set}\" tilewidth=\"{TILE_W}\" tileheight=\"{TILE_H}\">\n  <image source=\"{source}\" width=\"{}\" height=\"{}\"/>\n </tileset>\n",
        columns * TILE_W,
        rows * TILE_H
    );
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
