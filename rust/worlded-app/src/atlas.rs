//! Tile art: turns a tile name into a texture and uv rectangle, decoding each pack page once.
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use worlded_core::pack::PackSet;

#[derive(Clone)]
pub struct Sprite {
    pub tex: egui::TextureId,
    pub uv: egui::Rect,
    /// Trimmed size and its offset inside the full 128 x 256 frame.
    pub w: f32,
    pub h: f32,
    pub ox: f32,
    pub oy: f32,
}

#[derive(Default)]
pub struct Atlas {
    pages: HashMap<(usize, usize), Option<(TextureHandle, u32, u32)>>,
    sprites: HashMap<String, Option<Sprite>>,
    warned: HashSet<String>,
    /// Page decodes done so far (each page is decoded at most once).
    pub decoded: usize,
}

impl Atlas {
    /// `None` when there is no art for the name (unknown tile, no packs, damaged page).
    pub fn sprite(&mut self, ctx: &egui::Context, packs: Option<&PackSet>, name: &str) -> Option<Sprite> {
        if let Some(s) = self.sprites.get(name) {
            return s.clone();
        }
        let s = packs.and_then(|p| self.build(ctx, p, name));
        if s.is_none() && self.warned.insert(name.to_string()) {
            eprintln!("no art for tile {name}");
        }
        self.sprites.insert(name.to_string(), s.clone());
        s
    }

    fn build(&mut self, ctx: &egui::Context, packs: &PackSet, name: &str) -> Option<Sprite> {
        let (pack, e) = packs.lookup(name)?;
        let key = (pack, e.page);
        if !self.pages.contains_key(&key) {
            self.decoded += 1;
            let page = packs.page_rgba(pack, e.page).ok().map(|(w, h, rgba)| {
                let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                (ctx.load_texture(format!("pack{pack}_{}", e.page), img, TextureOptions::NEAREST), w, h)
            });
            self.pages.insert(key, page);
        }
        let (handle, pw, ph) = self.pages.get(&key)?.as_ref()?;
        let (pw, ph) = (*pw as f32, *ph as f32);
        let uv = egui::Rect::from_min_max(
            egui::pos2(e.x as f32 / pw, e.y as f32 / ph),
            egui::pos2((e.x + e.w) as f32 / pw, (e.y + e.h) as f32 / ph),
        );
        Some(Sprite { tex: handle.id(), uv, w: e.w as f32, h: e.h as f32, ox: e.ox as f32, oy: e.oy as f32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &mut Vec<u8>, n: u32) {
        v.extend_from_slice(&n.to_le_bytes());
    }

    fn pack_dir() -> std::path::PathBuf {
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let mut v = b"PZPK".to_vec();
        for n in [1u32, 1] {
            le(&mut v, n);
        }
        le(&mut v, 2);
        v.extend_from_slice(b"P0");
        le(&mut v, 2);
        le(&mut v, 1);
        for (name, x) in [("a_0", 0u32), ("a_1", 2)] {
            le(&mut v, name.len() as u32);
            v.extend_from_slice(name.as_bytes());
            for n in [x, 0, 2, 4, 0, 192, 128, 256] {
                le(&mut v, n);
            }
        }
        le(&mut v, png.len() as u32);
        v.extend_from_slice(&png);
        let dir = std::env::temp_dir().join("wa_atlas_packs");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("p.pack"), v).unwrap();
        dir
    }

    #[test]
    fn atlas_decodes_each_page_once_and_unknown_is_none() {
        let (set, _) = PackSet::open_dir(&pack_dir()).unwrap();
        let ctx = egui::Context::default();
        let mut atlas = Atlas::default();
        let a = atlas.sprite(&ctx, Some(&set), "a_0").unwrap();
        let b = atlas.sprite(&ctx, Some(&set), "a_1").unwrap();
        assert_eq!(atlas.decoded, 1); // both tiles share one page
        assert_eq!((a.w, a.h, a.oy), (2.0, 4.0, 192.0));
        assert_eq!(b.uv.min.x, 0.5);
        assert!(atlas.sprite(&ctx, Some(&set), "nope").is_none());
        assert!(atlas.sprite(&ctx, None, "a_0").is_some()); // cached from before
        assert!(Atlas::default().sprite(&ctx, None, "a_0").is_none()); // no packs, placeholder
    }
}
