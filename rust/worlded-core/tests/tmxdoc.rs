use worlded_core::tmx::parse_tmx_str;
use worlded_core::tmxdoc::{encode_data, open};

fn real() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/real_cell_a.tmx")).unwrap()
}

/// A 2x2 map with one layer in the given data form.
fn tiny(encoding: &str, compression: &str, gids: &[u32]) -> String {
    let attrs = match (encoding, compression) {
        ("", _) => String::new(),
        (e, "") => format!(" encoding=\"{e}\""),
        (e, c) => format!(" encoding=\"{e}\" compression=\"{c}\""),
    };
    let body = encode_data(gids, encoding, compression).unwrap();
    format!(
        "<?xml version=\"1.0\"?>\n<map width=\"2\" height=\"2\">\n <tileset firstgid=\"1\" name=\"t\"/>\n <layer name=\"L\" level=\"0\" width=\"2\" height=\"2\">\n  <data{attrs}>\n   {body}\n  </data>\n </layer>\n</map>\n"
    )
}

#[test]
fn untouched_document_gives_back_the_same_text() {
    let text = real();
    let doc = open(text.clone()).unwrap();
    assert!(doc.dirty_layers().is_empty());
    assert_eq!(doc.to_text().unwrap(), text);
}

#[test]
fn edit_one_square_changes_only_that_layer_data() {
    let text = real();
    let mut doc = open(text.clone()).unwrap();
    doc.set_gid(0, 5, 7, 12);
    assert_eq!(doc.dirty_layers(), vec![0]);
    let out = doc.to_text().unwrap();
    // Common prefix and suffix cover everything but one <data> body.
    let pre = text.bytes().zip(out.bytes()).take_while(|(a, b)| a == b).count();
    let suf = text.bytes().rev().zip(out.bytes().rev()).take_while(|(a, b)| a == b).count();
    let (start, end) = (text.find("<data").unwrap(), text.find("</data>").unwrap());
    assert!(pre >= start && text.len() - suf <= end, "diff escapes the first <data>");
    let old = parse_tmx_str(&text).unwrap();
    let new = parse_tmx_str(&out).unwrap();
    for (l, (a, b)) in old.layers.iter().zip(&new.layers).enumerate() {
        for i in 0..a.gids.len() {
            let want = if l == 0 && i == 7 * 300 + 5 { 12 } else { a.gids[i] };
            assert_eq!(b.gids[i], want, "layer {l} square {i}");
        }
    }
}

#[test]
fn csv_base64_zlib_and_gzip_data_round_trip() {
    for (enc, comp) in [("csv", ""), ("base64", ""), ("base64", "zlib"), ("base64", "gzip")] {
        let mut doc = open(tiny(enc, comp, &[1, 2, 3, 4])).unwrap();
        doc.set_gid(0, 1, 1, 9);
        let out = doc.to_text().unwrap();
        let t = parse_tmx_str(&out).unwrap();
        assert_eq!(t.layers[0].gids, vec![1, 2, 3, 9], "{enc}/{comp}");
        assert!(out.contains(&format!("encoding=\"{enc}\"")), "{enc}/{comp} keeps its encoding");
    }
}

#[test]
fn flip_flags_survive_an_edit_elsewhere() {
    let flipped = 0x8000_0000 | 3;
    let mut doc = open(tiny("base64", "zlib", &[flipped, 2, 3, 4])).unwrap();
    assert_eq!(doc.tmx.layers[0].gids[0], 3, "display gid has flags masked");
    doc.set_gid(0, 1, 1, 9);
    let out = doc.to_text().unwrap();
    let again = open(out).unwrap();
    // Re-encode untouched copy to read raw values back.
    let raw = again.raw_gid(0, 0, 0);
    assert_eq!(raw, flipped);
}

#[test]
fn unknown_encoding_opens_but_refuses_to_save() {
    let text = tiny("base64", "zlib", &[1, 2, 3, 4]).replace("zlib", "zstd");
    let mut doc = open(text).unwrap();
    assert!(doc.read_only.is_some());
    doc.set_gid(0, 0, 0, 5);
    assert!(doc.to_text().is_err());
    assert!(doc.dirty_layers().is_empty(), "an unreadable layer cannot be edited");
}
