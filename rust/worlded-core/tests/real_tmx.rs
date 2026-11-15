use worlded_core::tmx::parse_tmx;
use std::path::Path;

#[test]
fn parses_real_worlded_cell() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/real_cell_a.tmx");
    let t = parse_tmx(&p).expect("real tmx parses");
    println!("{:?} layers={}", (t.width, t.height), t.layers.len());
    assert_eq!(t.layers.len(), 15);
}
