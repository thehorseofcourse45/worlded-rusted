use std::path::{Path, PathBuf};
use worlded_core::pzw::parse_pzw;

fn real1() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../knoxlots/fixtures/real/real1/real1.pzw")
}

#[test]
fn real1_has_four_cells_and_origin_70_0() {
    let w = parse_pzw(&real1()).unwrap();
    assert_eq!((w.width, w.height), (2, 2));
    assert_eq!(w.origin, (70, 0));
    assert_eq!(w.cells.len(), 4);
    assert!(w.cells.iter().any(|c| (c.x, c.y) == (0, 0) && c.map.is_some()));
}

#[test]
fn missing_map_path_is_none() {
    let dir = std::env::temp_dir().join("wc_pzw_test");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.pzw");
    std::fs::write(&p, r#"<world version="1.0" width="1" height="1"><cell x="0" y="0" map=""/><cell x="1" y="0"/></world>"#).unwrap();
    let w = parse_pzw(&p).unwrap();
    assert_eq!(w.cells.len(), 2);
    assert!(w.cells.iter().all(|c| c.map.is_none()));
}

#[test]
fn damaged_xml_is_error_not_panic() {
    let dir = std::env::temp_dir().join("wc_pzw_test");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("bad.pzw");
    std::fs::write(&p, "<world width=\"2\"><cell").unwrap();
    assert!(parse_pzw(&p).is_err());
    assert!(parse_pzw(&dir.join("nope.pzw")).is_err());
}
