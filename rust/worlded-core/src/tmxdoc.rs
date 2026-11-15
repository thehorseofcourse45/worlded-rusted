//! Lossless `.tmx` document: the original text plus the byte span of each layer's `<data>` body.
//! Saving splices re-encoded data of edited layers into the original text, so everything else
//! (tilesets, WorldEd's bmp settings, whitespace) comes back byte for byte.
use crate::tmx::{self, RawLayer, Tmx};
use crate::CoreError;
use std::ops::Range;

pub use crate::tmx::encode_data;

struct Span {
    range: Range<usize>,
    encoding: String,
    compression: String,
}

pub struct TmxDoc {
    /// Display model; gids have flip flags removed. Edit through `set_gid`.
    pub tmx: Tmx,
    /// Why the document cannot be saved safely (unknown encoding, odd `<data>` layout), if so.
    pub read_only: Option<String>,
    text: String,
    raws: Vec<RawLayer>,
    spans: Vec<Option<Span>>,
    dirty: Vec<bool>,
}

pub fn open(text: String) -> Result<TmxDoc, CoreError> {
    let (tmx, raws) = tmx::parse_inner(&text, true)?;
    let xml = roxmltree::Document::parse(&text).map_err(|e| CoreError::Parse(e.to_string()))?;
    let mut spans = Vec::new();
    let mut read_only = None;
    for (i, n) in xml.root_element().children().filter(|n| n.has_tag_name("layer")).enumerate() {
        let data = n.children().find(|c| c.has_tag_name("data"));
        let body = data.and_then(|d| {
            let mut kids = d.children();
            match (kids.next(), kids.next()) {
                (Some(t), None) if t.is_text() => Some((d, t.range())),
                _ => None,
            }
        });
        match body {
            Some((d, range)) if raws[i].readable => spans.push(Some(Span {
                range,
                encoding: d.attribute("encoding").unwrap_or("").to_string(),
                compression: d.attribute("compression").unwrap_or("").to_string(),
            })),
            Some(_) => {
                read_only.get_or_insert(format!("layer {i} uses an unknown data encoding"));
                spans.push(None);
            }
            None => {
                read_only.get_or_insert(format!("layer {i} has an unexpected <data> layout"));
                spans.push(None);
            }
        }
    }
    let dirty = vec![false; spans.len()];
    Ok(TmxDoc { tmx, read_only, text, raws, spans, dirty })
}

impl TmxDoc {
    fn index(&self, layer: usize, x: u32, y: u32) -> Option<usize> {
        (x < self.tmx.width && y < self.tmx.height && self.spans.get(layer)?.is_some())
            .then(|| (y * self.tmx.width + x) as usize)
    }

    /// Set a square to a gid (flip flags of the new value kept). Returns false, changing
    /// nothing, when the square is outside the map or the layer cannot be saved.
    pub fn set_gid(&mut self, layer: usize, x: u32, y: u32, gid: u32) -> bool {
        let Some(i) = self.index(layer, x, y) else { return false };
        if self.raws[layer].raw[i] != gid {
            self.raws[layer].raw[i] = gid;
            self.tmx.layers[layer].gids[i] = gid & 0x1FFF_FFFF;
            self.dirty[layer] = true;
        }
        true
    }

    /// The gid as stored, flip flags included; 0 outside the map.
    pub fn raw_gid(&self, layer: usize, x: u32, y: u32) -> u32 {
        if x < self.tmx.width && y < self.tmx.height {
            self.raws.get(layer).map_or(0, |r| r.raw[(y * self.tmx.width + x) as usize])
        } else {
            0
        }
    }

    pub fn dirty_layers(&self) -> Vec<usize> {
        (0..self.dirty.len()).filter(|&i| self.dirty[i]).collect()
    }

    /// The original text with every edited layer's `<data>` body re-encoded in its own
    /// encoding and compression. Refuses when the document is read-only.
    pub fn to_text(&self) -> Result<String, CoreError> {
        if let Some(why) = &self.read_only {
            return Err(CoreError::Parse(format!("cannot save: {why}")));
        }
        let mut out = self.text.clone();
        // Back to front so earlier spans keep their offsets.
        for i in self.dirty_layers().into_iter().rev() {
            let span = self.spans[i].as_ref().expect("dirty layers have spans");
            let old = &self.text[span.range.clone()];
            let lead = &old[..old.len() - old.trim_start().len()];
            let trail = &old[old.trim_end().len()..];
            let body = encode_data(&self.raws[i].raw, &span.encoding, &span.compression)?;
            out.replace_range(span.range.clone(), &format!("{lead}{body}{trail}"));
        }
        Ok(out)
    }
}
