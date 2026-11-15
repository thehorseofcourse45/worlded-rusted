//! `.tmx` cell map (Tiled format): tilesets and tile layers. Object groups are ignored.
use crate::CoreError;
use base64::Engine;
use std::io::Read;
use std::path::Path;

pub struct Tmx {
    pub width: u32,
    pub height: u32,
    pub tilesets: Vec<TilesetRef>,
    pub layers: Vec<Layer>,
}

pub struct TilesetRef {
    pub first_gid: u32,
    pub name: String,
}

pub struct Layer {
    pub name: String,
    pub visible: bool,
    /// Row-major, width * height global tile ids; 0 is empty. Flip flags are removed.
    pub gids: Vec<u32>,
}

const FLAGS: u32 = 0xE000_0000;

pub fn parse_tmx(path: &Path) -> Result<Tmx, CoreError> {
    parse_tmx_str(&std::fs::read_to_string(path)?)
}

pub fn parse_tmx_str(text: &str) -> Result<Tmx, CoreError> {
    parse_inner(text, false).map(|(t, _)| t)
}

/// One layer's data as stored: flip flags kept, `readable` false when the encoding or
/// compression is unknown (lenient mode; the layer then shows as all empty).
pub(crate) struct RawLayer {
    pub raw: Vec<u32>,
    pub readable: bool,
}

pub(crate) fn parse_inner(text: &str, lenient: bool) -> Result<(Tmx, Vec<RawLayer>), CoreError> {
    let doc = roxmltree::Document::parse(text).map_err(|e| CoreError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "map" {
        return Err(CoreError::Parse("root element is not <map>".into()));
    }
    let dim = |n: roxmltree::Node, k: &str| n.attribute(k).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let (width, height) = (dim(root, "width"), dim(root, "height"));
    let size_ok = |k: &str| root.attribute(k).is_some_and(|v| v.parse::<u32>().is_ok());
    if !size_ok("width") || !size_ok("height") || (width as u64 * height as u64) > 4_000_000 {
        return Err(CoreError::Parse("map needs numeric width and height (at most 4 million tiles)".into()));
    }
    let tilesets = root
        .children()
        .filter(|n| n.has_tag_name("tileset"))
        .map(|n| TilesetRef { first_gid: dim(n, "firstgid"), name: n.attribute("name").unwrap_or("").to_string() })
        .collect();
    let mut layers = Vec::new();
    let mut raws = Vec::new();
    for n in root.children().filter(|n| n.has_tag_name("layer")) {
        let data = n.children().find(|c| c.has_tag_name("data")).ok_or_else(|| CoreError::Parse("layer without data".into()))?;
        let want = width as usize * height as usize;
        let (raw, readable) = match decode(data.text().unwrap_or(""), data.attribute("encoding"), data.attribute("compression"), want) {
            Ok(v) => (v, true),
            Err(CoreError::Parse(m)) if lenient && (m == "unknown compression" || m == "unsupported data encoding") => (vec![0; want], false),
            Err(e) => return Err(e),
        };
        if raw.len() != want {
            return Err(CoreError::Parse(format!("layer has {} tiles, map needs {want}", raw.len())));
        }
        let gids = raw.iter().map(|g| g & !FLAGS).collect();
        raws.push(RawLayer { raw, readable });
        layers.push(Layer {
            name: n.attribute("name").unwrap_or("").to_string(),
            visible: n.attribute("visible") != Some("0"),
            gids,
        });
    }
    Ok((Tmx { width, height, tilesets, layers }, raws))
}

pub(crate) fn decode(text: &str, encoding: Option<&str>, compression: Option<&str>, want: usize) -> Result<Vec<u32>, CoreError> {
    let bad = |s: &str| CoreError::Parse(s.to_string());
    match encoding {
        Some("csv") => text
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u32>().map_err(|_| bad("bad csv tile id")))
            .collect(),
        Some("base64") => {
            let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let raw = base64::engine::general_purpose::STANDARD.decode(clean).map_err(|_| bad("bad base64"))?;
            let mut bytes = Vec::new();
            match compression {
                Some("zlib") => flate2::read::ZlibDecoder::new(&raw[..]).take(want as u64 * 4 + 1).read_to_end(&mut bytes).map(|_| ()),
                Some("gzip") => flate2::read::GzDecoder::new(&raw[..]).take(want as u64 * 4 + 1).read_to_end(&mut bytes).map(|_| ()),
                None | Some("") => {
                    bytes = raw;
                    Ok(())
                }
                Some(_) => return Err(bad("unknown compression")),
            }
            .map_err(|_| bad("bad compressed data"))?;
            if bytes.len() % 4 != 0 {
                return Err(bad("tile data not a multiple of 4 bytes"));
            }
            Ok(bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
        }
        _ => Err(bad("unsupported data encoding")),
    }
}

/// `"<tileset>_<index>"` for a global id, or `None` for 0 or an id below every tileset.
pub fn tile_name(tmx: &Tmx, gid: u32) -> Option<String> {
    let gid = gid & !FLAGS;
    if gid == 0 {
        return None;
    }
    let ts = tmx.tilesets.iter().filter(|t| t.first_gid <= gid).max_by_key(|t| t.first_gid)?;
    Some(format!("{}_{}", ts.name, gid - ts.first_gid))
}

/// Inverse of `decode`: gids to the text body of a `<data>` element for the given
/// encoding (`csv` or `base64`) and compression (`zlib`, `gzip` or empty).
pub fn encode_data(gids: &[u32], encoding: &str, compression: &str) -> Result<String, CoreError> {
    use std::io::Write;
    let bad = |s: &str| CoreError::Parse(s.to_string());
    match encoding {
        "csv" => Ok(gids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")),
        "base64" => {
            let bytes: Vec<u8> = gids.iter().flat_map(|g| g.to_le_bytes()).collect();
            let packed = match compression {
                "zlib" => {
                    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    e.write_all(&bytes)?;
                    e.finish()?
                }
                "gzip" => {
                    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                    e.write_all(&bytes)?;
                    e.finish()?
                }
                "" => bytes,
                _ => return Err(bad("unknown compression")),
            };
            Ok(base64::engine::general_purpose::STANDARD.encode(packed))
        }
        _ => Err(bad("unsupported data encoding")),
    }
}
