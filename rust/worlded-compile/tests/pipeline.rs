mod common;
use std::path::{Path, PathBuf};
use std::process::Command;
use worlded_compile::header::Room;
use worlded_compile::pipeline::{compile_map, read_lots, Options};
use worlded_compile::source::cell_source;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn python_available() -> bool {
    Command::new("python").args(["-c", "import numpy, PIL"]).output().map(|o| o.status.success()).unwrap_or(false)
}

fn python(args: &[&str]) {
    let out = Command::new("python").args(args).current_dir(common::repo()).output().unwrap();
    assert!(out.status.success(), "python {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

#[test]
fn cell_source_bytes_equal_python() {
    // levels: 2 levels, 3 tile names; a pattern with empty runs, 1 to 3 tile stacks and a run at the level end
    let names: Vec<String> = ["a_1", "b_22", "cc_3"].iter().map(|s| s.to_string()).collect();
    let mut levels: Vec<Vec<Vec<u32>>> = vec![vec![Vec::new(); 256 * 256]; 2];
    for z in 0..2 {
        for y in 0..256 {
            for x in 0..256 {
                let i = (x * 7 + y * 13 + z * 3) % 29;
                let mut st = Vec::new();
                for k in 0..3 {
                    let v = (x * 7 + y * 13 + k * 5 + z * 3) % 23;
                    if (1..=3).contains(&v) {
                        st.push(v as u32);
                    }
                }
                if i % 4 != 0 {
                    levels[z][y * 256 + x] = st;
                }
            }
        }
    }
    let rooms = vec![Room { name: "kitchen".into(), z: 0, rects: vec![(1, 2, 3, 4), (5, 6, 7, 8)] }, Room { name: "".into(), z: 1, rects: vec![(0, 0, 1, 1)] }];
    let buildings = vec![vec![0usize, 1], vec![]];
    let got = cell_source(83, 2, &names, &levels, &rooms, &buildings).unwrap();
    if !python_available() {
        return;
    }
    let script = tmp("wcc_kcell").join("k.py");
    std::fs::write(
        &script,
        r#"
import sys, numpy as np
sys.path.insert(0, sys.argv[1])
from knoxbuild.kcell import cell_source
names = ["a_1", "b_22", "cc_3"]
arrays = []
for z in range(2):
    lv = []
    for k in range(3):
        a = np.zeros((256, 256), np.int64)
        for y in range(256):
            for x in range(256):
                i = (x * 7 + y * 13 + z * 3) % 29
                v = (x * 7 + y * 13 + k * 5 + z * 3) % 23
                a[y, x] = v if (1 <= v <= 3 and i % 4 != 0) else 0
        lv.append(a)
    arrays.append(lv)
rooms = [("kitchen", 0, [(1, 2, 3, 4), (5, 6, 7, 8)], []), ("", 1, [(0, 0, 1, 1)], [])]
sys.stdout.buffer.write(cell_source(83, 2, names, arrays, rooms=rooms, buildings=[[0, 1], []]))
"#,
    )
    .unwrap();
    let out = Command::new("python").arg(&script).arg(common::repo()).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(got.len(), out.stdout.len());
    assert!(got == out.stdout, "cell source bytes differ from Python");
}

#[test]
fn read_lots_of_real1_keeps_project_order() {
    let lots = read_lots(&common::repo().join("rust/knoxlots/fixtures/real/real1/real1.pzw")).unwrap();
    assert!(lots.len() > 100);
    assert_eq!((lots[0].name.as_str(), lots[0].tx, lots[0].ty, lots[0].level), ("real1_0031", 181, 41, 0));
}

fn differential(size: &str, seed: &str, seams: bool, dir: &str) {
    if !python_available() {
        return;
    }
    let root = tmp(dir);
    let project = root.join("tm");
    let (py_src, rs_src, rs_lots) = (root.join("py_src"), root.join("rs_src"), root.join("rs_lots"));
    python(&["tools/make_test_map.py", project.to_str().unwrap(), "--size", size, "--seed", seed]);
    let mut args = vec!["tools/map_source.py", project.to_str().unwrap(), py_src.to_str().unwrap(), "--seed", seed];
    if seams {
        args.push("--worlded-seams");
    }
    python(&args);
    let opts = Options { seed: seed.parse().unwrap(), seams, workers: Some(2), keep_source: Some(rs_src.clone()), rules: common::repo().join("config/Rules.txt") };
    let n = compile_map(&project, &rs_lots, &opts, &|_, _| {}).unwrap();
    let want = files(&py_src);
    assert_eq!(files(&rs_src), want, "same files");
    assert_eq!(n, want.iter().filter(|f| f.ends_with(".kcell")).count());
    for f in &want {
        let norm = |mut v: Vec<u8>| {
            if f.ends_with(".txt") {
                v.retain(|c| *c != 13); // Python writes text files with CRLF on Windows
            }
            v
        };
        let (a, b) = (norm(std::fs::read(py_src.join(f)).unwrap()), norm(std::fs::read(rs_src.join(f)).unwrap()));
        assert_eq!(a.len(), b.len(), "{f}: size");
        assert!(a == b, "{f}: bytes differ from Python (size {size} seed {seed} seams {seams})");
    }
    assert!(want.len() >= 5, "several cells expected, got {want:?}");
    // end to end: lot files from the Python cell sources (knoxlots compile) equal the Rust lot files
    let manifest = knoxlots::compile::Manifest::parse(&std::fs::read_to_string(py_src.join("source.txt")).unwrap()).unwrap();
    for f in want.iter().filter(|f| f.ends_with(".kcell")) {
        let k = knoxlots::kcell::KCell::parse(&std::fs::read(py_src.join(f)).unwrap()).unwrap();
        let c = knoxlots::compile::compile_cell(&k, &manifest, None);
        let (x, y) = (k.cell_x, k.cell_y);
        assert!(std::fs::read(rs_lots.join(format!("{x}_{y}.lotheader"))).unwrap() == c.header, "{x}_{y}.lotheader");
        assert!(std::fs::read(rs_lots.join(format!("world_{x}_{y}.lotpack"))).unwrap() == c.pack, "world_{x}_{y}.lotpack");
        assert!(std::fs::read(rs_lots.join(format!("chunkdata_{x}_{y}.bin"))).unwrap() == c.chunkdata, "chunkdata_{x}_{y}.bin");
    }
}

#[test]
fn cell_sources_equal_python_with_seams() {
    differential("600", "1", true, "wcc_diff_a");
}

#[test]
fn cell_sources_equal_python_without_seams_other_seed_and_size() {
    differential("520", "5", false, "wcc_diff_b");
}

#[test]
fn missing_vegetation_picture_is_an_error_naming_it() {
    let root = tmp("wcc_noveg");
    let project = root.join("tm");
    std::fs::create_dir_all(&project).unwrap();
    if python_available() {
        python(&["tools/make_test_map.py", project.to_str().unwrap(), "--size", "300"]);
        std::fs::remove_file(project.join("tm_veg.bmp")).unwrap();
        let opts = Options { seed: 1, seams: false, workers: Some(1), keep_source: None, rules: common::repo().join("config/Rules.txt") };
        let e = compile_map(&project, &root.join("out"), &opts, &|_, _| {}).unwrap_err().to_string();
        assert!(e.contains("tm_veg.bmp"), "{e}");
    }
}

#[test]
fn missing_rules_file_is_an_error_naming_it() {
    if !python_available() {
        return;
    }
    let root = tmp("wcc_norules");
    let project = root.join("tm");
    python(&["tools/make_test_map.py", project.to_str().unwrap(), "--size", "300"]);
    let opts = Options { seed: 1, seams: false, workers: Some(1), keep_source: None, rules: root.join("none/Rules.txt") };
    let e = compile_map(&project, &root.join("out"), &opts, &|_, _| {}).unwrap_err().to_string();
    assert!(e.contains("Rules.txt"), "{e}");
}
