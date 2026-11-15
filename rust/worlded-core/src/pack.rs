//! Project Zomboid `.pack` texture packs (see FORMATS.md). Only the index is read on open;
//! page images are decoded on demand.
use crate::CoreError;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Entry {
    pub page: usize,
    /// Trimmed rectangle inside the page image.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Where the trimmed rectangle sits inside the full frame.
    pub ox: u32,
    pub oy: u32,
    /// Full frame size.
    pub fw: u32,
    pub fh: u32,
}

pub struct Pack {
    path: PathBuf,
    pages: Vec<(u64, u32)>, // png offset, png length
    entries: HashMap<String, Entry>,
}

struct Reader<'a> {
    f: &'a mut std::io::BufReader<std::fs::File>,
    pos: u64,
    len: u64,
}

impl Reader<'_> {
    fn bytes(&mut self, n: u64) -> Result<Vec<u8>, CoreError> {
        if self.pos + n > self.len {
            return Err(CoreError::Parse("pack ends early".into()));
        }
        let mut b = vec![0u8; n as usize];
        self.f.read_exact(&mut b)?;
        self.pos += n;
        Ok(b)
    }
    fn u32(&mut self) -> Result<u32, CoreError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Result<String, CoreError> {
        let n = self.u32()?;
        if n > 4096 {
            return Err(CoreError::Parse("name too long".into()));
        }
        Ok(String::from_utf8_lossy(&self.bytes(n as u64)?).into_owned())
    }
}

impl Pack {
    pub fn open(path: &Path) -> Result<Pack, CoreError> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        let mut f = std::io::BufReader::new(file);
        let r = &mut Reader { f: &mut f, pos: 0, len };
        if r.bytes(4)? != b"PZPK" {
            return Err(CoreError::Parse("not a PZPK pack".into()));
        }
        let _version = r.u32()?;
        let page_count = r.u32()?;
        let (mut pages, mut entries) = (Vec::new(), HashMap::new());
        for page in 0..page_count as usize {
            let _name = r.string()?;
            let count = r.u32()?;
            let _flag = r.u32()?;
            for _ in 0..count {
                let name = r.string()?;
                let mut v = [0u32; 8];
                for slot in v.iter_mut() {
                    *slot = r.u32()?;
                }
                entries.insert(name, Entry { page, x: v[0], y: v[1], w: v[2], h: v[3], ox: v[4], oy: v[5], fw: v[6], fh: v[7] });
            }
            let png_len = r.u32()?;
            pages.push((r.pos, png_len));
            if r.pos + png_len as u64 > len {
                // keep the index we have; reading this page will fail
                break;
            }
            r.f.seek_relative(png_len as i64)?;
            r.pos += png_len as u64;
        }
        Ok(Pack { path: path.to_path_buf(), pages, entries })
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Decode one page image to RGBA8: (width, height, pixels).
    pub fn page_rgba(&self, page: usize) -> Result<(u32, u32, Vec<u8>), CoreError> {
        let &(off, n) = self.pages.get(page).ok_or_else(|| CoreError::Parse("no such page".into()))?;
        let mut f = std::fs::File::open(&self.path)?;
        if off + n as u64 > f.metadata()?.len() {
            return Err(CoreError::Parse("pack ends early".into()));
        }
        f.seek(SeekFrom::Start(off))?;
        let mut png = vec![0u8; n as usize];
        f.read_exact(&mut png)?;
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|e| CoreError::Parse(e.to_string()))?
            .to_rgba8();
        Ok((img.width(), img.height(), img.into_raw()))
    }
}

/// Every `*.pack` in a folder, searched by tile name (the first pack that has the name wins).
pub struct PackSet {
    packs: Vec<Pack>,
    index: HashMap<String, usize>,
}

impl PackSet {
    /// Packs that fail to open are skipped; the error list says which.
    pub fn open_dir(dir: &Path) -> Result<(PackSet, Vec<String>), CoreError> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "pack"))
            .collect();
        files.sort();
        let (mut packs, mut index, mut errors) = (Vec::new(), HashMap::new(), Vec::new());
        for f in files {
            match Pack::open(&f) {
                Ok(p) => {
                    for n in p.names() {
                        index.entry(n.clone()).or_insert(packs.len());
                    }
                    packs.push(p);
                }
                Err(e) => errors.push(format!("{}: {e}", f.display())),
            }
        }
        Ok((PackSet { packs, index }, errors))
    }

    /// (pack number, entry) for a tile name.
    pub fn lookup(&self, name: &str) -> Option<(usize, &Entry)> {
        let i = *self.index.get(name)?;
        self.packs[i].entry(name).map(|e| (i, e))
    }

    pub fn page_rgba(&self, pack: usize, page: usize) -> Result<(u32, u32, Vec<u8>), CoreError> {
        self.packs.get(pack).ok_or_else(|| CoreError::Parse("no such pack".into()))?.page_rgba(page)
    }

    /// Every tile name once, in no particular order.
    pub fn tile_names(&self) -> Vec<&str> {
        self.index.keys().map(String::as_str).collect()
    }

    pub fn tile_count(&self) -> usize {
        self.index.len()
    }
}
