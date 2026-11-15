use worlded_core::gids::{gid_for, split_tile_name};
use worlded_core::tmx::{parse_tmx_str, tile_name};
use worlded_core::tmxdoc::open;

const MAP: &str = "<?xml version=\"1.0\"?>
<map version=\"2.0\" orientation=\"levelisometric\" width=\"2\" height=\"2\" tilewidth=\"64\" tileheight=\"32\">
 <tileset firstgid=\"1\" name=\"a_01\" tilewidth=\"64\" tileheight=\"128\">
  <image source=\"../Tiles/a_01.png\" width=\"512\" height=\"512\"/>
 </tileset>
 <tileset firstgid=\"33\" name=\"b_02\" tilewidth=\"64\" tileheight=\"128\">
  <image source=\"../Tiles/b_02.png\" width=\"512\" height=\"512\"/>
 </tileset>
 <layer name=\"L\" level=\"0\" width=\"2\" height=\"2\">
  <data encoding=\"csv\">0,0,0,0</data>
 </layer>
</map>
";

#[test]
fn existing_tileset_gives_firstgid_plus_index() {
    let mut d = open(MAP.to_string()).unwrap();
    assert_eq!(gid_for(&mut d, "a_01_3", 8).unwrap(), 4);
    assert_eq!(gid_for(&mut d, "b_02_0", 8).unwrap(), 33);
    assert_eq!(d.to_text().unwrap(), MAP, "nothing added for known tilesets");
}

#[test]
fn new_tileset_is_appended_with_next_firstgid_and_saved_text_reparses() {
    let mut d = open(MAP.to_string()).unwrap();
    let gid = gid_for(&mut d, "c_03_5", 8).unwrap();
    assert!(gid > 33 + 31, "after the last tileset, clear of its tiles: {gid}");
    assert_eq!(gid_for(&mut d, "c_03_5", 8).unwrap(), gid, "second call finds it");
    assert!(d.set_gid(0, 0, 0, gid));
    let text = d.to_text().unwrap();
    assert!(text.contains("../Tiles/c_03.png"), "image beside the others");
    let t = parse_tmx_str(&text).unwrap();
    assert_eq!(t.tilesets.len(), 3);
    assert_eq!(tile_name(&t, t.layers[0].gids[0]).unwrap(), "c_03_5");
    // The new element sits before the layers and the old ones are untouched.
    assert!(text.find("c_03").unwrap() < text.find("<layer").unwrap());
    assert!(text.contains(" <tileset firstgid=\"33\" name=\"b_02\""));
}

#[test]
fn names_without_a_numeric_suffix_are_errors() {
    let mut d = open(MAP.to_string()).unwrap();
    assert!(gid_for(&mut d, "plain", 8).is_err());
    assert!(gid_for(&mut d, "a_01_x", 8).is_err());
    assert!(gid_for(&mut d, "_4", 8).is_err());
    assert_eq!(split_tile_name("blends_natural_01_024"), Some(("blends_natural_01", 24)));
    assert_eq!(split_tile_name("e_holly_1_5"), Some(("e_holly_1", 5)));
    assert_eq!(split_tile_name("nope"), None);
}

#[test]
fn tileset_columns_from_the_pack_set_the_new_entry_size() {
    let mut d = open(MAP.to_string()).unwrap();
    gid_for(&mut d, "n_20", 8).unwrap(); // 21 tiles in 8 columns: 3 rows
    let text = d.to_text().unwrap();
    assert!(text.contains("name=\"n\"") || text.contains("name=\"n\" "), "tileset named n");
    assert!(text.contains("width=\"512\" height=\"384\""), "8 columns x 3 rows of 64x128");
}
