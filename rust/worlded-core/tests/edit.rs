use worlded_core::edit::{Change, Edit, History};
use worlded_core::tmxdoc::{encode_data, open, TmxDoc};

fn doc() -> TmxDoc {
    let body = encode_data(&[0; 9], "csv", "").unwrap();
    open(format!(
        "<map width=\"3\" height=\"3\">\n <layer name=\"L\" level=\"0\" width=\"3\" height=\"3\">\n  <data encoding=\"csv\">{body}</data>\n </layer>\n</map>\n"
    ))
    .unwrap()
}

fn set(x: u32, y: u32, new: u32) -> Edit {
    Edit(vec![Change { layer: 0, x, y, old: 0, new }])
}

fn gids(d: &TmxDoc) -> Vec<u32> {
    d.tmx.layers[0].gids.clone()
}

#[test]
fn undo_restores_exactly_and_redo_reapplies() {
    let (mut d, mut h) = (doc(), History::default());
    h.apply(&mut d, set(1, 1, 7));
    h.apply(&mut d, set(2, 2, 8));
    assert_eq!(gids(&d)[4], 7);
    assert!(h.undo(&mut d));
    assert_eq!(gids(&d), vec![0, 0, 0, 0, 7, 0, 0, 0, 0]);
    assert!(h.undo(&mut d));
    assert_eq!(gids(&d), vec![0; 9]);
    assert!(!h.undo(&mut d));
    assert!(h.redo(&mut d) && h.redo(&mut d));
    assert_eq!(gids(&d)[8], 8);
    assert!(!h.redo(&mut d));
}

#[test]
fn a_group_is_one_undo_step() {
    let (mut d, mut h) = (doc(), History::default());
    h.begin_group();
    h.apply(&mut d, set(0, 0, 1));
    h.apply(&mut d, set(1, 0, 1));
    h.apply(&mut d, set(2, 0, 1));
    h.end_group();
    assert!(h.undo(&mut d));
    assert_eq!(gids(&d), vec![0; 9]);
    assert!(!h.undo(&mut d));
}

#[test]
fn new_edit_clears_redo() {
    let (mut d, mut h) = (doc(), History::default());
    h.apply(&mut d, set(0, 0, 1));
    h.undo(&mut d);
    h.apply(&mut d, set(1, 1, 2));
    assert!(!h.redo(&mut d));
}

#[test]
fn clean_state_tracks_save_and_undo() {
    let (mut d, mut h) = (doc(), History::default());
    assert!(h.is_clean());
    h.apply(&mut d, set(0, 0, 1));
    assert!(!h.is_clean());
    h.mark_saved();
    assert!(h.is_clean());
    h.apply(&mut d, set(1, 1, 2));
    assert!(!h.is_clean());
    h.undo(&mut d);
    assert!(h.is_clean(), "back at the saved state");
    h.undo(&mut d);
    assert!(!h.is_clean(), "undo past the save point is dirty");
    h.redo(&mut d);
    assert!(h.is_clean());
    // An edit made after undoing below the save point can never be clean again by undo.
    h.undo(&mut d);
    h.apply(&mut d, set(2, 2, 3));
    h.undo(&mut d);
    assert!(!h.is_clean());
}

#[test]
fn noop_changes_are_dropped() {
    let (mut d, mut h) = (doc(), History::default());
    h.apply(&mut d, set(0, 0, 0));
    assert!(h.is_clean() && !h.undo(&mut d));
    h.apply(&mut d, set(0, 0, 5));
    h.apply(&mut d, set(0, 0, 5));
    h.undo(&mut d);
    assert_eq!(gids(&d)[0], 0);
    assert!(!h.undo(&mut d), "the repeated same-value edit made no step");
}
