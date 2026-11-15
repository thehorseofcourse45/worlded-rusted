//! Uncompressed 24-bit and 8-bit BMP strips (port of knoxbuild/bitmaps.py `read_rgb_rows`).
//! Only the rows asked for are read, so a 16,500 x 16,500 picture is never held whole.
use crate::CompileError;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BmpInfo {
    pub width: u32,
    pub height: u32,
    bits: u16,
    bottom_up: bool,
    offset: u64,
    palette_at: u64,
}

pub struct Strip {
    pub rows: usize,
    pub cols: usize,
    /// rows x cols x 3, RGB.
    pub rgb: Vec<u8>,
}

fn bad(p: &Path, m: &str) -> CompileError {
    CompileError::Parse(p.to_path_buf(), m.to_string())
}

pub fn bmp_info(path: &Path) -> Result<BmpInfo, CompileError> {
    let io = |e| CompileError::Io(path.to_path_buf(), e);
    let mut f = std::fs::File::open(path).map_err(io)?;
    let mut h = [0u8; 54];
    f.read_exact(&mut h).map_err(|_| bad(path, "not a BMP file (too short)"))?;
    if &h[0..2] != b"BM" {
        return Err(bad(path, "not a BMP file"));
    }
    let u32at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
    let i32at = |i: usize| u32at(i) as i32;
    let (offset, size, width, height) = (u32at(10), u32at(14), i32at(18), i32at(22));
    let (planes, bits, compression) = (u16::from_le_bytes([h[26], h[27]]), u16::from_le_bytes([h[28], h[29]]), u32at(30));
    if size < 40 || compression != 0 || planes != 1 || width <= 0 || height == 0 {
        return Err(bad(path, "unsupported BMP (needs an uncompressed Windows bitmap)"));
    }
    Ok(BmpInfo { width: width as u32, height: height.unsigned_abs(), bits, bottom_up: height > 0, offset: offset as u64, palette_at: 14 + size as u64 })
}

/// Rows `top..bottom` (clamped to the picture) and at most `width` columns, as RGB.
pub fn read_rgb_rows(path: &Path, top: i64, bottom: i64, width: Option<u32>) -> Result<Strip, CompileError> {
    let info = bmp_info(path)?;
    if info.bits != 24 && info.bits != 8 {
        return Err(bad(path, &format!("{}-bit BMP not supported; save it as 24-bit", info.bits)));
    }
    let io = |e| CompileError::Io(path.to_path_buf(), e);
    let bottom = bottom.min(info.height as i64);
    let top = top.max(0).min(bottom);
    let rows = (bottom - top) as usize;
    let cols = width.map_or(info.width, |w| w.min(info.width)) as usize;
    let bpp = (info.bits / 8) as u64;
    let stride = (info.width as u64 * bpp + 3) / 4 * 4;
    let mut f = std::fs::File::open(path).map_err(io)?;
    if f.metadata().map_err(io)?.len() < info.offset + stride * info.height as u64 {
        return Err(bad(path, "BMP file is shorter than its header says"));
    }
    let mut palette = [[0u8; 3]; 256];
    if info.bits == 8 {
        f.seek(SeekFrom::Start(info.palette_at)).map_err(io)?;
        let mut raw = [0u8; 1024];
        f.read_exact(&mut raw).map_err(|_| bad(path, "BMP palette missing"))?;
        for (i, p) in palette.iter_mut().enumerate() {
            *p = [raw[i * 4 + 2], raw[i * 4 + 1], raw[i * 4]];
        }
    }
    let mut rgb = Vec::with_capacity(rows * cols * 3);
    let mut line = vec![0u8; (cols as u64 * bpp) as usize];
    for y in top..bottom {
        let file_row = if info.bottom_up { info.height as i64 - 1 - y } else { y } as u64;
        f.seek(SeekFrom::Start(info.offset + file_row * stride)).map_err(io)?;
        f.read_exact(&mut line).map_err(io)?;
        if info.bits == 24 {
            for px in line.chunks_exact(3) {
                rgb.extend_from_slice(&[px[2], px[1], px[0]]);
            }
        } else {
            for &i in &line {
                rgb.extend_from_slice(&palette[i as usize]);
            }
        }
    }
    Ok(Strip { rows, cols, rgb })
}
