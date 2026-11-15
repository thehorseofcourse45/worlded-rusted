use base64::Engine;
use flate2::write::GzEncoder;
use std::io::Write;
use worlded_core::tmx::parse_bmp_images;

fn real() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/real_cell_a.tmx")).unwrap()
}

/// One `<bmp-image>` with the given colours and raw (already little-endian u32) pixel bytes.
fn picture(colors: &[&str], raw: &[u8]) -> String {
    let mut e = GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(raw).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(e.finish().unwrap());
    let cols: String = colors.iter().map(|c| format!("<color rgb=\"{c}\"/>")).collect();
    format!("<map width=\"1\" height=\"1\"><bmp-image index=\"0\" seed=\"3\">{cols}<pixels>\n   {b64}\n  </pixels></bmp-image></map>")
}

fn pixels(f: impl Fn(usize) -> u32) -> Vec<u8> {
    (0..90_000).flat_map(|i| f(i).to_le_bytes()).collect()
}

#[test]
fn real_fixture_has_two_pictures() {
    let imgs = parse_bmp_images(&real()).unwrap();
    assert_eq!(imgs.iter().map(|i| i.index).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(imgs.iter().map(|i| i.seed).collect::<Vec<_>>(), vec![1, 1]);
    assert_eq!(imgs.iter().map(|i| i.colors.len()).collect::<Vec<_>>(), vec![11, 10]);
    for i in &imgs {
        assert_eq!((i.width, i.height, i.pixels.len()), (300, 300, 90_000));
        assert!(i.pixels.iter().all(|&p| p as usize <= i.colors.len()));
    }
    // first pixel of the landscape picture is index 2: the second colour, 201 31 201
    assert_eq!(imgs[0].pixels[0], 2);
    assert_eq!(imgs[0].colors[1], [201, 31, 201]);
}

#[test]
fn file_without_pictures_gives_empty_list() {
    assert!(parse_bmp_images("<map width=\"1\" height=\"1\"/>").unwrap().is_empty());
}

#[test]
fn synthetic_picture_round_trips() {
    let imgs = parse_bmp_images(&picture(&["1 2 3", "4 5 6"], &pixels(|i| (i % 3) as u32))).unwrap();
    assert_eq!(imgs.len(), 1);
    assert_eq!((imgs[0].seed, imgs[0].colors.clone()), (3, vec![[1, 2, 3], [4, 5, 6]]));
    assert_eq!(&imgs[0].pixels[..4], &[0, 1, 2, 0]);
}

#[test]
fn damaged_gzip_wrong_size_and_bad_index_are_errors() {
    let garbage = "<map><bmp-image index=\"0\" seed=\"1\"><color rgb=\"1 2 3\"/><pixels>AAAAAAAAAA==</pixels></bmp-image></map>";
    assert!(parse_bmp_images(garbage).is_err());
    assert!(parse_bmp_images(&picture(&["1 2 3"], &[0u8; 100])).is_err(), "too few pixels");
    assert!(parse_bmp_images(&picture(&["1 2 3"], &pixels(|_| 0).repeat(2))).is_err(), "too many pixels");
    assert!(parse_bmp_images(&picture(&["1 2 3"], &pixels(|i| if i == 7 { 5 } else { 0 }))).is_err(), "index beyond the colours");
    assert!(parse_bmp_images("<map><bmp-image index=\"0\" seed=\"1\"><color rgb=\"1 2\"/></bmp-image></map>").is_err(), "bad colour");
    assert!(parse_bmp_images("not xml").is_err());
}
