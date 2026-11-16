//! BMP to TMX import: turn a world picture (one pixel per tile) into the `<bmp-image>` pictures
//! embedded in each cell's `.tmx`, the way WorldEd's BMPtoTMX does. Index 0 is the main picture,
//! index 1 the vegetation one (`<name>_veg.bmp` beside it).
use crate::bmp;
use std::path::{Path, PathBuf};
use worlded_core::pzw::{self, World};
use worlded_core::tmx;

/// One WorldEd cell in tiles.
pub const CELL: i64 = 300;

/// The `<bmp path x y width height/>` element of a project: the picture `path` covers the
/// `width` x `height` block of cells starting at cell (x, y).
pub struct BmpSpec {
    pub path: String,
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

pub fn bmp_element(world: &World) -> Option<BmpSpec> {
    let raw = world.raw.iter().find(|r| r.contains("<bmp "))?;
    let wrapped = format!("<w>{raw}</w>");
    let doc = roxmltree::Document::parse(&wrapped).ok()?;
    let n = doc.root_element().children().find(|n| n.has_tag_name("bmp"))?;
    let num = |k: &str| n.attribute(k).and_then(|v| v.parse().ok());
    Some(BmpSpec {
        path: n.attribute("path").unwrap_or("").to_string(),
        x: num("x").unwrap_or(0),
        y: num("y").unwrap_or(0),
        width: num("width").unwrap_or(0) as u32,
        height: num("height").unwrap_or(0) as u32,
    })
}

/// The 300x300 picture of one cell: RGB block at pixel (px, py) of `img`, colours in
/// first-appearance order, pixels 1-based (0 where the picture has no pixel there).
fn cell_picture(img: &Path, px: i64, py: i64) -> Result<(Vec<[u8; 3]>, Vec<u32>), String> {
    let strip = bmp::read_rgb_rows(img, py, py + CELL, None).map_err(|e| e.to_string())?;
    let x0 = px.max(0) as usize;
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let mut pixels = vec![0u32; (CELL * CELL) as usize];
    for row in 0..CELL as usize {
        if row >= strip.rows {
            break;
        }
        for col in 0..CELL as usize {
            let sx = x0 + col;
            if sx >= strip.cols {
                break;
            }
            let o = (row * strip.cols + sx) * 3;
            let rgb = [strip.rgb[o], strip.rgb[o + 1], strip.rgb[o + 2]];
            let idx = match colors.iter().position(|c| *c == rgb) {
                Some(i) => i as u32 + 1,
                None => {
                    colors.push(rgb);
                    colors.len() as u32
                }
            };
            pixels[row * CELL as usize + col] = idx;
        }
    }
    Ok((colors, pixels))
}

/// Replace the `<bmp-image>` elements of a cell map with `images` (inserted before `</map>` when
/// the map has none). Everything else in the file is kept verbatim.
pub fn set_bmp_images(tmx_text: &str, images: &[String]) -> Result<String, String> {
    let doc = roxmltree::Document::parse(tmx_text).map_err(|e| format!("not a map: {e}"))?;
    let mut cuts: Vec<std::ops::Range<usize>> = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("bmp-image"))
        .map(|n| {
            // include the whitespace in front of the element
            let mut s = n.range().start;
            while s > 0 && tmx_text.as_bytes()[s - 1].is_ascii_whitespace() {
                s -= 1;
            }
            s..n.range().end
        })
        .collect();
    cuts.sort_by_key(|r| r.start);
    let map_end = doc.root_element().range().end;
    let close = tmx_text[..map_end].rfind("</map>").ok_or("no </map> in the map file")?;
    let mut out = String::new();
    let mut pos = 0;
    for r in &cuts {
        out.push_str(&tmx_text[pos..r.start]);
        pos = r.end;
    }
    out.push_str(&tmx_text[pos..close]);
    for im in images {
        out.push_str("\n ");
        out.push_str(im);
    }
    out.push_str(&tmx_text[close..]);
    Ok(out)
}

/// Import `bmp_path` (plus its `_veg` sibling when `veg_path` is given or the file exists) into
/// the project: every cell of the `<bmp>` block gets its `<bmp-image>` pictures in its `.tmx`
/// (created empty when the file is missing), and the `<bmp>` element's path is set to the
/// picture's file name. Returns the number of cells written.
pub fn import_bmp(pzw_path: &Path, bmp_path: &Path, veg_path: Option<&Path>) -> Result<usize, String> {
    let text = std::fs::read_to_string(pzw_path).map_err(|e| format!("{}: {e}", pzw_path.display()))?;
    let mut world = pzw::parse_pzw_str(&text).map_err(|e| e.to_string())?;
    let spec = bmp_element(&world).ok_or("the project has no <bmp> element (New project adds one)")?;
    if spec.width == 0 || spec.height == 0 {
        return Err("the project's <bmp> element covers no cells".into());
    }
    let dir = pzw_path.parent().unwrap_or(Path::new("."));
    let veg: Option<PathBuf> = veg_path.map(Path::to_path_buf).or_else(|| {
        let stem = bmp_path.file_stem()?.to_string_lossy().into_owned();
        let cand = bmp_path.with_file_name(format!("{stem}_veg.bmp"));
        cand.exists().then_some(cand)
    });
    let bmp_name = bmp_path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = pzw_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "world".into());
    let mut written = 0;
    for cy in 0..spec.height as i32 {
        for cx in 0..spec.width as i32 {
            let px = (spec.x + cx as i64) * CELL;
            let py = (spec.y + cy as i64) * CELL;
            let (c0, p0) = cell_picture(bmp_path, px, py)?;
            let mut images = vec![tmx::bmp_image_xml(0, 1, &c0, &p0)];
            if let Some(v) = &veg {
                let (c1, p1) = cell_picture(v, px, py)?;
                images.push(tmx::bmp_image_xml(1, 1, &c1, &p1));
            }
            let ci = match world.cell_at(cx, cy) {
                Some(i) => i,
                None => {
                    world.cells.push(pzw::Cell { x: cx, y: cy, map: None, children: vec![], lead: "\n ".into(), self_closing: true });
                    world.cells.len() - 1
                }
            };
            let rel = world.cells[ci].map.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| format!("tmx/{stem}_{}_{}.tmx", world.origin.0 + cx, cy));
            let map_path = pzw::resolve_map(&world.cells[ci], dir).unwrap_or_else(|| dir.join(&rel));
            if world.cells[ci].map.as_deref().map_or(true, |m| m.is_empty()) {
                world.cells[ci].map = Some(rel.clone());
            }
            let old = std::fs::read_to_string(&map_path).unwrap_or_else(|_| tmx::empty_map_xml(300, 300));
            let new = set_bmp_images(&old, &images)?;
            if let Some(p) = map_path.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            std::fs::write(&map_path, new).map_err(|e| format!("{}: {e}", map_path.display()))?;
            written += 1;
        }
    }
    for r in world.raw.iter_mut() {
        if r.contains("<bmp ") {
            if let Some(i) = r.find("path=\"") {
                if let Some(j) = r[i + 6..].find('"') {
                    r.replace_range(i + 6..i + 6 + j, &bmp_name);
                }
            }
        }
    }
    std::fs::write(pzw_path, world.to_xml()).map_err(|e| format!("{}: {e}", pzw_path.display()))?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a 24-bit bottom-up BMP.
    fn write_bmp24(path: &Path, w: u32, h: u32, rgb: &[u8]) {
        let stride = (w as usize * 3 + 3) / 4 * 4;
        let mut f = Vec::new();
        let size = 54 + stride * h as usize;
        f.extend_from_slice(b"BM");
        f.extend_from_slice(&(size as u32).to_le_bytes());
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(&54u32.to_le_bytes());
        f.extend_from_slice(&40u32.to_le_bytes());
        f.extend_from_slice(&(w as i32).to_le_bytes());
        f.extend_from_slice(&(h as i32).to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&24u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&((stride * h as usize) as u32).to_le_bytes());
        f.extend_from_slice(&[0; 16]);
        for row in (0..h as usize).rev() {
            for col in 0..w as usize {
                let o = (row * w as usize + col) * 3;
                f.extend_from_slice(&[rgb[o + 2], rgb[o + 1], rgb[o]]);
            }
            f.extend(std::iter::repeat(0).take(stride - w as usize * 3));
        }
        std::fs::write(path, f).unwrap();
    }

    #[test]
    fn set_bmp_images_replaces_existing_and_inserts_before_map_close() {
        let old = "<map>\n <layer name=\"0_Floor\" width=\"1\" height=\"1\"><data encoding=\"csv\">0</data></layer>\n <bmp-image index=\"0\" seed=\"9\"><pixels>AAAA</pixels></bmp-image>\n</map>\n";
        let img = tmx::bmp_image_xml(0, 1, &[[1, 2, 3]], &vec![1u32; 300 * 300]);
        let out = set_bmp_images(old, &[img.clone()]).unwrap();
        assert_eq!(out.matches("<bmp-image").count(), 1, "old picture replaced");
        assert!(out.contains("<layer"));
        assert!(out.trim_end().ends_with("</map>"));
        assert!(out.contains(&img));
        // and it parses back
        let imgs = tmx::parse_bmp_images(&out).unwrap();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].seed, 1);
    }

    #[test]
    fn import_bmp_writes_pictures_into_every_cell_of_the_block() {
        let dir = std::env::temp_dir().join(format!("wc_bmpimport_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 2x1 cells -> a 600x300 picture: left half red, right half green; veg all blue
        let w = 600usize;
        let h = 300usize;
        let mut main = vec![0u8; w * h * 3];
        let mut veg = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 3;
                if x < 300 {
                    main[o] = 200;
                    main[o + 1] = 10;
                    main[o + 2] = 10;
                } else {
                    main[o + 1] = 20;
                    main[o + 2] = 220;
                }
                veg[o] = 1;
                veg[o + 1] = 2;
                veg[o + 2] = 3;
            }
        }
        let bmp = dir.join("demo.bmp");
        let vegp = dir.join("demo_veg.bmp");
        write_bmp24(&bmp, w as u32, h as u32, &main);
        write_bmp24(&vegp, w as u32, h as u32, &veg);
        // a 2x1 project (new_project writes a <bmp> block)
        let pzw = dir.join("demo.pzw");
        let world = World::new_project("demo", 2, 1);
        assert!(bmp_element(&world).is_some(), "new projects carry a <bmp> element");
        std::fs::write(&pzw, world.to_xml()).unwrap();
        let n = import_bmp(&pzw, &bmp, None).unwrap();
        assert_eq!(n, 2, "the _veg sibling is picked up by name");
        // project file: bmp path set, still round-trips
        let text = std::fs::read_to_string(&pzw).unwrap();
        assert!(text.contains("<bmp path=\"demo.bmp\""), "{text}");
        assert_eq!(pzw::parse_pzw_str(&text).unwrap().to_xml(), text);
        // each cell map has both pictures with the right colours
        for (cx, rgb) in [(0, [200u8, 10, 10]), (1, [0, 20, 220])] {
            let mp = dir.join(format!("tmx/demo_{}_{}.tmx", 70 + cx, 0));
            let t = std::fs::read_to_string(&mp).unwrap();
            let imgs = tmx::parse_bmp_images(&t).unwrap();
            assert_eq!(imgs.len(), 2, "main + vegetation");
            assert_eq!(imgs[0].index, 0);
            assert_eq!(imgs[0].colors, vec![rgb]);
            assert!(imgs[0].pixels.iter().all(|&p| p == 1));
            assert_eq!(imgs[1].colors, vec![[1u8, 2, 3]]);
            // layers still empty and parseable
            let tmx = tmx::parse_tmx_str(&t).unwrap();
            assert!(tmx.layers.iter().all(|l| l.gids.iter().all(|&g| g == 0)));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
