use base64::Engine;
use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;
use worlded_core::tmx::{parse_tmx_str, tile_name};

fn b64_zlib(ids: &[u32]) -> String {
    let raw: Vec<u8> = ids.iter().flat_map(|i| i.to_le_bytes()).collect();
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&raw).unwrap();
    base64::engine::general_purpose::STANDARD.encode(z.finish().unwrap())
}

fn sample() -> String {
    format!(
        r#"<map version="1.0" orientation="isometric" width="2" height="2" tilewidth="64" tileheight="32">
 <tileset firstgid="1" name="floors_burnt_01" tilewidth="64" tileheight="128"><image source="x.png" width="512" height="512"/></tileset>
 <tileset firstgid="65" name="walls_01" tilewidth="64" tileheight="128"/>
 <layer name="0_Floor" width="2" height="2"><data encoding="csv">1,2,
0,66</data></layer>
 <layer name="0_Walls" width="2" height="2" visible="0"><data encoding="base64" compression="zlib">{}</data></layer>
</map>"#,
        b64_zlib(&[0x8000_0000 | 65, 0, 0, 3])
    )
}

#[test]
fn gid_count_matches_dimensions_and_encodings_decode() {
    let t = parse_tmx_str(&sample()).unwrap();
    assert_eq!((t.width, t.height), (2, 2));
    assert_eq!(t.layers.len(), 2);
    assert_eq!(t.layers[0].gids, vec![1, 2, 0, 66]);
    assert!(t.layers[0].visible && !t.layers[1].visible);
    assert_eq!(t.layers[1].gids, vec![65, 0, 0, 3]); // flip flag masked off
}

#[test]
fn tile_name_resolves_gid_to_tileset_and_index() {
    let t = parse_tmx_str(&sample()).unwrap();
    assert_eq!(tile_name(&t, 1).as_deref(), Some("floors_burnt_01_0"));
    assert_eq!(tile_name(&t, 66).as_deref(), Some("walls_01_1"));
    assert_eq!(tile_name(&t, 0), None);
}

#[test]
fn empty_and_zero_size_maps_open() {
    let t = parse_tmx_str(r#"<map width="0" height="0"/>"#).unwrap();
    assert!(t.layers.is_empty());
    let t = parse_tmx_str(r#"<map width="1" height="1"><layer name="a" width="1" height="1"><data encoding="csv">0</data></layer></map>"#).unwrap();
    assert_eq!(t.layers[0].gids, vec![0]);
}

#[test]
fn damaged_tmx_is_error() {
    assert!(parse_tmx_str("<map").is_err());
    let bad = r#"<map width="2" height="2"><layer name="a"><data encoding="base64" compression="zlib">!!!</data></layer></map>"#;
    assert!(parse_tmx_str(bad).is_err());
}

#[test]
fn wrong_root_missing_size_and_zip_bombs_are_errors() {
    assert!(parse_tmx_str("<world width=\"1\" height=\"1\"/>").is_err());
    assert!(parse_tmx_str("<map/>").is_err());
    assert!(parse_tmx_str("<map width=\"abc\" height=\"1\"/>").is_err());
    // 1x1 map whose data inflates to 1 MB: must stop and fail, not allocate it all
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&vec![0u8; 1 << 20]).unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(z.finish().unwrap());
    let bomb = format!(r#"<map width="1" height="1"><layer name="a"><data encoding="base64" compression="zlib">{data}</data></layer></map>"#);
    assert!(parse_tmx_str(&bomb).is_err());
}
