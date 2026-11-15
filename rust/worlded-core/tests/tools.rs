use worlded_core::edit::Edit;
use worlded_core::tmx::{Layer, Tmx};
use worlded_core::tools::{bucket_fill, erase, paint, pick, rect_fill};

fn map(w: u32, h: u32, gids: Vec<u32>) -> Tmx {
    Tmx { width: w, height: h, tilesets: vec![], layers: vec![Layer { name: "L".into(), visible: true, gids }] }
}

fn empty(w: u32, h: u32) -> Tmx {
    map(w, h, vec![0; (w * h) as usize])
}

fn squares(e: &Edit) -> Vec<(u32, u32)> {
    let mut v: Vec<_> = e.0.iter().map(|c| (c.x, c.y)).collect();
    v.sort();
    v
}

#[test]
fn brush_sizes_cover_expected_squares() {
    let t = empty(10, 10);
    assert_eq!(squares(&paint(&t, 0, 4, 4, 1, 9)), vec![(4, 4)]);
    assert_eq!(squares(&paint(&t, 0, 4, 4, 2, 9)), vec![(4, 4), (4, 5), (5, 4), (5, 5)]);
    assert_eq!(paint(&t, 0, 4, 4, 5, 9).0.len(), 25);
    assert_eq!(squares(&paint(&t, 0, 4, 4, 5, 9))[0], (2, 2));
    // Clipped at the corner: 3x3 brush at (0,0) covers 2x2.
    assert_eq!(squares(&paint(&t, 0, 0, 0, 3, 9)), vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
}

#[test]
fn paint_outside_the_map_gives_empty_edit() {
    let t = empty(10, 10);
    assert!(paint(&t, 0, -5, 3, 1, 9).0.is_empty());
    assert!(paint(&t, 0, 10, 3, 1, 9).0.is_empty());
    assert!(paint(&t, 7, 1, 1, 1, 9).0.is_empty(), "no such layer");
}

#[test]
fn paint_and_erase_skip_unchanged_squares() {
    let mut t = empty(3, 3);
    t.layers[0].gids[4] = 9;
    assert!(paint(&t, 0, 1, 1, 1, 9).0.is_empty());
    let e = erase(&t, 0, 1, 1, 3);
    assert_eq!(e.0.len(), 1);
    assert_eq!(e.0[0].new, 0);
}

#[test]
fn rect_fill_with_swapped_corners_equals_ordered() {
    let t = empty(10, 10);
    let a = rect_fill(&t, 0, (2, 3), (5, 6), 4);
    let b = rect_fill(&t, 0, (5, 6), (2, 3), 4);
    assert_eq!(squares(&a), squares(&b));
    assert_eq!(a.0.len(), 16);
    // Clipped to the map.
    assert_eq!(rect_fill(&t, 0, (8, 8), (20, 20), 4).0.len(), 4);
}

#[test]
fn bucket_fill_stops_at_boundaries_and_other_gids() {
    // 0 0 1
    // 0 1 0
    // 1 0 0
    let t = map(3, 3, vec![0, 0, 1, 0, 1, 0, 1, 0, 0]);
    let e = bucket_fill(&t, 0, 0, 0, 5, 100).unwrap();
    assert_eq!(squares(&e), vec![(0, 0), (0, 1), (1, 0)]);
    let e = bucket_fill(&t, 0, 2, 2, 5, 100).unwrap();
    assert_eq!(squares(&e), vec![(1, 2), (2, 1), (2, 2)]);
}

#[test]
fn bucket_fill_over_the_limit_is_none() {
    let t = empty(10, 10);
    assert!(bucket_fill(&t, 0, 5, 5, 3, 99).is_none());
    assert_eq!(bucket_fill(&t, 0, 5, 5, 3, 100).unwrap().0.len(), 100);
}

#[test]
fn bucket_fill_with_same_gid_is_empty() {
    let t = empty(4, 4);
    assert!(bucket_fill(&t, 0, 1, 1, 0, 100).unwrap().0.is_empty());
    assert!(bucket_fill(&t, 0, -1, 1, 3, 100).unwrap().0.is_empty());
}

#[test]
fn pick_returns_none_for_empty_or_outside() {
    let mut t = empty(3, 3);
    t.layers[0].gids[4] = 9;
    assert_eq!(pick(&t, 0, 1, 1), Some(9));
    assert_eq!(pick(&t, 0, 0, 0), None);
    assert_eq!(pick(&t, 0, 3, 1), None);
    assert_eq!(pick(&t, 0, -1, 1), None);
}
