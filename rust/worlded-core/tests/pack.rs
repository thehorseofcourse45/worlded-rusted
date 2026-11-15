use std::path::PathBuf;
use worlded_core::pack::Pack;

fn le(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}
fn s(v: &mut Vec<u8>, t: &str) {
    le(v, t.len() as u32);
    v.extend_from_slice(t.as_bytes());
}

/// One page (2x2 png: red, green / blue, white) holding tile "t_0" = whole image, "t_1" = right column.
fn synthetic() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(2, 2, |x, y| image::Rgba([[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]][(y * 2 + x) as usize]));
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let mut v = b"PZPK".to_vec();
    le(&mut v, 1);
    le(&mut v, 1);
    s(&mut v, "Page0");
    le(&mut v, 2);
    le(&mut v, 1);
    for (name, x, w) in [("t_0", 0, 2), ("t_1", 1, 1)] {
        s(&mut v, name);
        for n in [x, 0, w, 2, 0, 0, 2, 2] {
            le(&mut v, n);
        }
    }
    le(&mut v, png.len() as u32);
    v.extend_from_slice(&png);
    v
}

fn write(name: &str, bytes: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn round_trips_one_page_and_entries() {
    let p = Pack::open(&write("wc_ok.pack", &synthetic())).unwrap();
    let e = p.entry("t_1").unwrap();
    assert_eq!((e.page, e.x, e.y, e.w, e.h, e.fw, e.fh), (0, 1, 0, 1, 2, 2, 2));
    let (w, h, rgba) = p.page_rgba(0).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
    assert_eq!(&rgba[12..16], &[255, 255, 255, 255]);
    assert!(p.entry("nope").is_none());
}

#[test]
fn bad_page_index_is_error() {
    let p = Pack::open(&write("wc_ok2.pack", &synthetic())).unwrap();
    assert!(p.page_rgba(5).is_err());
}

#[test]
fn truncated_or_wrong_pack_is_error_not_panic() {
    let full = synthetic();
    for cut in [0, 3, 10, 25, 40, full.len() - 5] {
        let r = Pack::open(&write("wc_cut.pack", &full[..cut]));
        // a pack cut inside the png opens (index is intact) but the page read must fail
        if let Ok(p) = r {
            assert!(p.page_rgba(0).is_err());
        }
    }
    assert!(Pack::open(&write("wc_junk.pack", b"NOTAPACKFILEATALL")).is_err());
}

#[test]
fn reads_real_pack() {
    let Ok(dir) = std::env::var("PZ_INSTALL") else { return };
    let p = Pack::open(&PathBuf::from(dir).join("media/texturepacks/Tiles2x.floor.pack")).unwrap();
    let e = p.entry("blends_natural_01_0").unwrap();
    assert_eq!((e.w, e.h, e.fw, e.fh), (126, 64, 128, 256));
    let (w, h, rgba) = p.page_rgba(e.page).unwrap();
    assert_eq!(rgba.len(), (w * h * 4) as usize);
}

#[test]
fn opens_the_big_real_pack_quickly() {
    let Ok(dir) = std::env::var("PZ_INSTALL") else { return };
    let t = std::time::Instant::now();
    let p = Pack::open(&PathBuf::from(dir).join("media/texturepacks/Tiles2x.pack")).unwrap();
    assert!(p.names().count() > 1000);
    assert!(t.elapsed().as_secs() < 5, "took {:?}", t.elapsed());
}
