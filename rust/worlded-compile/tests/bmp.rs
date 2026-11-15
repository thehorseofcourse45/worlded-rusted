mod common;
use std::path::PathBuf;
use worlded_compile::bmp::{bmp_info, read_rgb_rows};

fn le32(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

/// A BMP with the given pixels (row-major, top row first). bits 24 or 8 (grey-ramp-reversed palette).
fn make(name: &str, w: u32, h: u32, bits: u16, bottom_up: bool, px: &dyn Fn(u32, u32) -> [u8; 3]) -> PathBuf {
    let bpp = (bits / 8) as u32;
    let stride = (w * bpp + 3) / 4 * 4;
    let pal = if bits == 8 { 1024 } else { 0 };
    let mut v = b"BM".to_vec();
    le32(&mut v, 54 + pal + stride * h);
    le32(&mut v, 0);
    le32(&mut v, 54 + pal);
    le32(&mut v, 40);
    le32(&mut v, w);
    le32(&mut v, if bottom_up { h } else { (h as i32).wrapping_neg() as u32 });
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&bits.to_le_bytes());
    for n in [0, 0, 0, 0, if bits == 8 { 256 } else { 0 }, 0] {
        le32(&mut v, n);
    }
    if bits == 8 {
        for i in 0..256u32 {
            v.extend_from_slice(&[(255 - i) as u8, i as u8, (i / 2) as u8, 0]); // B, G, R order: B=255-i G=i R=i/2
        }
    }
    let rows: Vec<u32> = if bottom_up { (0..h).rev().collect() } else { (0..h).collect() };
    for y in rows {
        let mut line = Vec::new();
        for x in 0..w {
            let p = px(x, y);
            if bits == 24 {
                line.extend_from_slice(&[p[2], p[1], p[0]]);
            } else {
                line.push(p[0]);
            }
        }
        line.resize(stride as usize, 0);
        v.extend_from_slice(&line);
    }
    let path = std::env::temp_dir().join(format!("wcc_{name}.bmp"));
    std::fs::write(&path, v).unwrap();
    path
}

fn px(x: u32, y: u32) -> [u8; 3] {
    [(x * 40 + 1) as u8, (y * 70 + 2) as u8, (x * 3 + y * 5) as u8]
}

#[test]
fn reads_24bit_bottom_up_with_row_padding() {
    let p = make("bu", 5, 3, 24, true, &px); // 5*3=15 bytes per row -> padded to 16
    let s = read_rgb_rows(&p, 0, 3, None).unwrap();
    assert_eq!((s.rows, s.cols), (3, 5));
    assert_eq!(&s.rgb[0..3], &px(0, 0));
    assert_eq!(&s.rgb[(2 * 5 + 4) * 3..(2 * 5 + 4) * 3 + 3], &px(4, 2));
}

#[test]
fn top_down_bmp_matches_bottom_up() {
    let a = read_rgb_rows(&make("td", 5, 3, 24, false, &px), 0, 3, None).unwrap();
    let b = read_rgb_rows(&make("bu2", 5, 3, 24, true, &px), 0, 3, None).unwrap();
    assert_eq!(a.rgb, b.rgb);
}

#[test]
fn reads_8bit_palette() {
    let p = make("p8", 3, 2, 8, true, &|x, y| [(x + y * 3) as u8, 0, 0]);
    let s = read_rgb_rows(&p, 0, 2, None).unwrap();
    // pixel index i -> RGB (i/2, i, 255-i) given the palette written by make()
    assert_eq!(&s.rgb[3..6], &[0, 1, 254]);
    assert_eq!(&s.rgb[(1 * 3 + 2) * 3..(1 * 3 + 2) * 3 + 3], &[2, 5, 250]);
}

#[test]
fn rows_are_clamped_to_the_picture_and_width_limits_columns() {
    let p = make("clamp", 5, 3, 24, true, &px);
    let s = read_rgb_rows(&p, -4, 10, Some(2)).unwrap();
    assert_eq!((s.rows, s.cols), (3, 2));
    let s = read_rgb_rows(&p, 2, 9, Some(99)).unwrap();
    assert_eq!((s.rows, s.cols), (1, 5));
    let s = read_rgb_rows(&p, 5, 9, None).unwrap();
    assert_eq!(s.rows, 0);
}

#[test]
fn info_reports_size() {
    let i = bmp_info(&make("info", 5, 3, 24, true, &px)).unwrap();
    assert_eq!((i.width, i.height), (5, 3));
}

#[test]
fn truncated_and_non_bmp_are_errors() {
    let p = make("trunc", 5, 3, 24, true, &px);
    let full = std::fs::read(&p).unwrap();
    let cut = std::env::temp_dir().join("wcc_cut.bmp");
    std::fs::write(&cut, &full[..full.len() - 10]).unwrap();
    assert!(read_rgb_rows(&cut, 0, 3, None).is_err());
    std::fs::write(&cut, b"definitely not a bitmap, far too long to be a header at all....").unwrap();
    assert!(bmp_info(&cut).is_err());
    assert!(bmp_info(&std::env::temp_dir().join("wcc_missing.bmp")).is_err());
    // 32-bit and compressed bitmaps are refused with a message, not misread
    let mut bad = full.clone();
    bad[28] = 32;
    std::fs::write(&cut, &bad).unwrap();
    assert!(read_rgb_rows(&cut, 0, 3, None).is_err());
}

#[test]
fn matches_python_on_a_generated_bitmap() {
    let p = make("py", 37, 29, 24, true, &|x, y| [(x * 7 + y) as u8, (y * 11) as u8, (x ^ y) as u8]);
    let Some(want) = common::python(&["bmp", p.to_str().unwrap(), "5", "21", "30"]) else { return };
    let s = read_rgb_rows(&p, 5, 21, Some(30)).unwrap();
    assert_eq!(s.rgb, want);
}
