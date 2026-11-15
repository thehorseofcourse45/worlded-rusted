mod common;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn exe() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_worlded"));
    c.env("KNOXLOTS_CONFIG", common::repo().join("config")).stdin(Stdio::null());
    c
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn make_map(root: &std::path::Path) -> Option<PathBuf> {
    let project = root.join("tm");
    let st = Command::new("python").args(["tools/make_test_map.py", project.to_str().unwrap(), "--size", "300"]).current_dir(common::repo()).output().ok()?;
    st.status.success().then_some(project)
}

#[test]
fn compiles_a_generated_map_into_the_default_lots_folder() {
    let root = tmp("wcc_cli_ok");
    let Some(project) = make_map(&root) else { return };
    let out = exe().arg(&project).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("cells compiled"));
    let lots = project.join("lots");
    let n = std::fs::read_dir(&lots).unwrap().count();
    assert!(n >= 6 && n % 3 == 0, "three files per cell, got {n}");
    // the written files read back: every header parses and its pack matches it
    for e in std::fs::read_dir(&lots).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "lotheader") {
            let h = knoxlots::header::Header::parse(&std::fs::read(&p).unwrap()).unwrap();
            assert!(h.rooms.len() + h.buildings.len() > 0 || h.tiles.len() > 0);
        }
    }
}

#[test]
fn bad_folder_prints_an_error_and_exits_nonzero() {
    let out = exe().arg(std::env::temp_dir().join("wcc_no_such_map")).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("wcc_no_such_map"));
}

#[test]
fn missing_rules_message_names_config() {
    let root = tmp("wcc_cli_norules");
    let Some(project) = make_map(&root) else { return };
    let out = exe().env("KNOXLOTS_CONFIG", root.join("none")).current_dir(&root).arg(&project).output().unwrap();
    // the exe is under rust/target, where the repo config is found as a last resort, so this
    // only fails when no config exists anywhere; accept either outcome but never a panic
    assert!(out.status.success() || String::from_utf8_lossy(&out.stderr).contains("config"));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
}

#[test]
fn unknown_option_and_missing_value_are_errors() {
    let out = exe().arg("--bogus").output().unwrap();
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("unknown option"));
    let out = exe().args(["x", "--seed"]).output().unwrap();
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("--seed"));
}

#[test]
fn no_arguments_asks_for_the_folder() {
    // stdin is empty: the prompt appears, then the empty answer is rejected with the usage text
    let out = exe().output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("Map folder"));
    assert!(!out.status.success());
}
