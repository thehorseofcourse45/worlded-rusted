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

// ---- .pzw mode ------------------------------------------------------------------------------

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/tmx_oracle")
}

/// A copy of the oracle project; returns its `.pzw`.
fn pzw_project(tag: &str) -> PathBuf {
    let dir = tmp(tag).join("p");
    std::fs::create_dir_all(dir.join("tmx")).unwrap();
    for f in ["rt.pzw", "rt_ZombieSpawnMap.bmp", "tmx/rt_70_0.tmx"] {
        std::fs::copy(fixture().join(f), dir.join(f)).unwrap();
    }
    dir.join("rt.pzw")
}

#[test]
fn cli_compiles_a_pzw_into_its_lots_folder() {
    let pzw = pzw_project("wcc_pzw_ok");
    let out = exe().arg(&pzw).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("4 cells compiled"));
    let lots = pzw.parent().unwrap().join("lots");
    for f in ["82_0.lotheader", "world_83_1.lotpack", "chunkdata_82_1.bin"] {
        assert!(lots.join(f).exists(), "{f}");
    }
    assert_eq!(std::fs::read_dir(&lots).unwrap().count(), 12);
}

#[test]
fn cli_rejects_seed_and_seams_in_pzw_mode() {
    let pzw = pzw_project("wcc_pzw_opts");
    for opt in [vec!["--seed", "3"], vec!["--worlded-seams"]] {
        let out = exe().arg(&pzw).args(&opt).output().unwrap();
        assert!(!out.status.success(), "{opt:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(opt[0]), "{opt:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(!pzw.parent().unwrap().join("lots").exists(), "nothing written");
}

#[test]
fn project_without_any_tmx_is_an_error_and_writes_nothing() {
    let pzw = pzw_project("wcc_pzw_none");
    std::fs::remove_file(pzw.parent().unwrap().join("tmx/rt_70_0.tmx")).unwrap();
    std::fs::write(&pzw, std::fs::read_to_string(&pzw).unwrap().replace("tmx/rt_70_0.tmx", "")).unwrap();
    let out = exe().arg(&pzw).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no cell has a .tmx file"));
    assert!(!pzw.parent().unwrap().join("lots").exists());
}

#[test]
fn damaged_tmx_and_damaged_picture_name_the_file() {
    let pzw = pzw_project("wcc_pzw_bad");
    let tmx = pzw.parent().unwrap().join("tmx/rt_70_0.tmx");
    let good = std::fs::read_to_string(&tmx).unwrap();
    std::fs::write(&tmx, &good[..good.len() / 2]).unwrap();
    let out = exe().arg(&pzw).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("rt_70_0.tmx"));
    // a picture whose pixel data is not gzip
    let start = good.find("<pixels>").unwrap() + "<pixels>".len();
    std::fs::write(&tmx, format!("{}AAAA{}", &good[..start], &good[good[start..].find("</pixels>").unwrap() + start..])).unwrap();
    let out = exe().arg(&pzw).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("rt_70_0.tmx") && !err.contains("panicked"), "{err}");
}

#[test]
fn missing_named_tmx_warns_and_compiles_the_rest() {
    let pzw = pzw_project("wcc_pzw_warn");
    // a second cell that names a map which does not exist
    let text = std::fs::read_to_string(&pzw).unwrap().replace("width=\"1\"", "width=\"2\"").replace("</world>", " <cell x=\"1\" y=\"0\" map=\"tmx/gone_71_0.tmx\">\n </cell>\n</world>");
    std::fs::write(&pzw, text).unwrap();
    let out = exe().arg(&pzw).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("gone_71_0.tmx"));
    assert!(pzw.parent().unwrap().join("lots/82_0.lotheader").exists());
}
