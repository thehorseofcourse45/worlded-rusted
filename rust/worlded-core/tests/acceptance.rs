//! Editing end to end. The WorldEd test is slow (about three minutes: WorldEd loads its whole
//! tileset catalogue twice) and needs the WorldEd command line, Python with Pillow and numpy, so
//! it runs only when `WORLDED_ACCEPTANCE=1` is set; set `PZWORLDED_CLI` to use another exe.
use std::path::{Path, PathBuf};
use std::process::Command;
use worlded_core::edit::{Change, Edit, History};
use worlded_core::gids::gid_for;
use worlded_core::save::Saver;
use worlded_core::tmx::parse_tmx_str;
use worlded_core::tmxdoc::open;

const DEFAULT_EXE: &str = r"C:\KnoxMap\vendor\PZMappingTools\bin\PZWorldEd_cli.exe";

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools")
}

fn set(layer: usize, x: u32, y: u32, new: u32) -> Edit {
    Edit(vec![Change { layer, x, y, old: 0, new }])
}

#[test]
fn edit_then_reopen_changes_only_the_edited_squares() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/real_cell_a.tmx");
    let text = std::fs::read_to_string(&path).unwrap();
    let before = parse_tmx_str(&text).unwrap();
    let mut doc = open(text).unwrap();
    let mut h = History::default();
    let g1 = gid_for(&mut doc, "Fire_01_3", 8).unwrap();
    let g2 = gid_for(&mut doc, "Smoke_01_5", 8).unwrap();
    h.apply(&mut doc, set(0, 10, 20, g1));
    h.apply(&mut doc, set(14, 299, 299, g2));

    let dir = std::env::temp_dir().join(format!("worlded_acc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("cell.tmx");
    Saver::new().save(&out, &doc.to_text().unwrap()).unwrap();
    let after = parse_tmx_str(&std::fs::read_to_string(&out).unwrap()).unwrap();

    assert_eq!(after.tilesets.len(), before.tilesets.len());
    for (l, (a, b)) in before.layers.iter().zip(&after.layers).enumerate() {
        for i in 0..a.gids.len() {
            let want = match (l, i) {
                (0, i) if i == 20 * 300 + 10 => g1,
                (14, i) if i == 299 * 300 + 299 => g2,
                _ => a.gids[i],
            };
            assert_eq!(b.gids[i], want, "layer {l} square {i}");
        }
    }
}

#[test]
fn saved_edit_loads_in_worlded() {
    let exe = std::env::var("PZWORLDED_CLI").unwrap_or_else(|_| DEFAULT_EXE.to_string());
    if std::env::var("WORLDED_ACCEPTANCE").is_err() || !Path::new(&exe).exists() || Command::new("python").arg("--version").output().is_err() {
        eprintln!("skipped: set WORLDED_ACCEPTANCE=1 with the WorldEd CLI and Python available");
        return;
    }
    let dir = std::env::temp_dir().join(format!("worlded_acc_cli_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let run = |script: &str, arg: &Path| {
        let out = Command::new("python").arg(tools().join(script)).arg(arg).args(["--exe", &exe]).output().unwrap();
        (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
    };
    let (ok, log) = run("make_real_tmx.py", &dir);
    assert!(ok, "make_real_tmx failed:\n{log}");
    let project = dir.join("rt");
    let tmx_path = std::fs::read_dir(project.join("tmx")).unwrap().next().unwrap().unwrap().path();

    let mut doc = open(std::fs::read_to_string(&tmx_path).unwrap()).unwrap();
    let mut h = History::default();
    let g1 = gid_for(&mut doc, "Fire_01_3", 8).unwrap();
    let g2 = gid_for(&mut doc, "Smoke_01_5", 8).unwrap();
    for x in 100..110 {
        h.apply(&mut doc, set(0, x, 100, g1));
        h.apply(&mut doc, set(14, x, 101, g2));
    }
    Saver::new().save(&tmx_path, &doc.to_text().unwrap()).unwrap();

    let (ok, log) = run("worlded_lots.py", &project);
    assert!(ok, "WorldEd rejected the saved map:\n{log}");
}
