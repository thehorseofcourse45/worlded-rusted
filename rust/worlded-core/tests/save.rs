use worlded_core::save::Saver;

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("worlded_save_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn save_writes_the_text_and_leaves_no_temp_file() {
    let d = dir("write");
    let f = d.join("a.tmx");
    Saver::new().save(&f, "hello").unwrap();
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello");
    assert!(!d.join("a.tmx.tmp").exists());
}

#[test]
fn first_save_makes_a_bak_of_the_old_file_second_does_not_overwrite_it() {
    let d = dir("bak");
    let f = d.join("a.tmx");
    std::fs::write(&f, "original").unwrap();
    let mut s = Saver::new();
    s.save(&f, "one").unwrap();
    s.save(&f, "two").unwrap();
    assert_eq!(std::fs::read_to_string(d.join("a.tmx.bak")).unwrap(), "original");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "two");
}

#[test]
fn failed_rename_leaves_the_old_file_untouched() {
    let d = dir("fail");
    let target = d.join("a.tmx");
    std::fs::create_dir(&target).unwrap(); // a directory cannot be replaced by a file
    std::fs::write(target.join("keep"), "x").unwrap();
    assert!(Saver::new().save(&target, "new").is_err());
    assert!(target.join("keep").exists());
    assert!(!d.join("a.tmx.tmp").exists());
}

#[test]
fn save_as_a_new_path_makes_no_bak() {
    let d = dir("new");
    let f = d.join("b.tmx");
    Saver::new().save(&f, "x").unwrap();
    assert!(!d.join("b.tmx.bak").exists());
}
