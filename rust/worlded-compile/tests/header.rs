mod common;
use std::collections::BTreeSet;
use std::path::PathBuf;
use worlded_compile::header::{cell_tables, placed_rooms, room_rects, Room};
use worlded_compile::tbx::{parse_tbx, Building};

fn sq(v: &[(i64, i64)]) -> BTreeSet<(i64, i64)> {
    v.iter().copied().collect()
}

#[test]
fn rectangles_cover_exactly_the_squares() {
    assert_eq!(room_rects(&sq(&[(0, 0), (1, 0), (0, 1), (1, 1)])), vec![(0, 0, 2, 2)]);
    // an L shape: the scan takes the first row, then what is left
    let l = sq(&[(0, 0), (1, 0), (2, 0), (0, 1)]);
    let rects = room_rects(&l);
    assert_eq!(rects, vec![(0, 0, 3, 1), (0, 1, 1, 1)]);
    let mut covered = BTreeSet::new();
    for (x, y, w, h) in rects {
        for yy in y..y + h {
            for xx in x..x + w {
                assert!(covered.insert((xx, yy)), "no square twice");
            }
        }
    }
    assert_eq!(covered, l);
    assert!(room_rects(&BTreeSet::new()).is_empty());
}

fn real1_lots() -> Vec<(PathBuf, i64, i64, i64)> {
    let dir = common::repo().join("rust/knoxlots/fixtures/real/real1");
    let text = std::fs::read_to_string(dir.join("real1.pzw")).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut lots = Vec::new();
    for cell in doc.root_element().children().filter(|n| n.has_tag_name("cell")) {
        let (cx, cy) = (cell.attribute("x").unwrap().parse::<i64>().unwrap(), cell.attribute("y").unwrap().parse::<i64>().unwrap());
        for l in cell.children().filter(|n| n.has_tag_name("lot")) {
            let f = |k: &str| l.attribute(k).unwrap().parse::<i64>().unwrap();
            lots.push((dir.join(l.attribute("map").unwrap()), cx * 300 + f("x"), cy * 300 + f("y"), f("level")));
        }
    }
    lots
}

fn rooms_text(rooms: &[Room]) -> String {
    rooms.iter().map(|r| format!("R|{}|{}|{}\n", r.name, r.z, r.rects.iter().map(|q| format!("{} {} {} {}", q.0, q.1, q.2, q.3)).collect::<Vec<_>>().join(","))).collect()
}

#[test]
fn placed_rooms_and_all_cell_tables_equal_python_on_real1() {
    let lots = real1_lots();
    let cells: Vec<(i64, i64)> = (82..=84).flat_map(|x| (0..=2).map(move |y| (x, y))).collect();
    let cell_arg = cells.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(";");
    let nums: Vec<String> = lots.iter().flat_map(|(p, x, y, l)| [p.to_string_lossy().to_string(), x.to_string(), y.to_string(), l.to_string()]).collect();
    let mut a = vec!["header", cell_arg.as_str()];
    a.extend(nums.iter().map(String::as_str));
    let Some(want) = common::python(&a) else { return };

    let buildings: Vec<Building> = lots.iter().map(|(p, ..)| parse_tbx(&std::fs::read_to_string(p).unwrap()).unwrap()).collect();
    let refs: Vec<(&Building, i64, i64, i64)> = buildings.iter().zip(&lots).map(|(b, (_, x, y, l))| (b, *x, *y, *l)).collect();
    let placed = placed_rooms(&refs).unwrap();
    let mut got = String::new();
    for (tx, ty, entries) in &placed {
        got += &format!("P|{tx}|{ty}\n{}", rooms_text(entries));
    }
    for (cx, cy) in &cells {
        let (rooms, bs) = cell_tables(*cx, *cy, &placed);
        got += &format!("C|{cx}|{cy}\n{}", rooms_text(&rooms));
        for b in bs {
            got += &format!("B|{}\n", b.iter().map(usize::to_string).collect::<Vec<_>>().join(","));
        }
    }
    let want = String::from_utf8(want).unwrap();
    assert!(want.lines().count() > 200, "the oracle output should be large");
    assert_eq!(got, want);
}
