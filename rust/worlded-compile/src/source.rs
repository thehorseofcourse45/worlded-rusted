//! Writing cell sources for `knoxlots` (port of knoxbuild/kcell.py; format in
//! rust/knoxlots/SOURCE_FORMAT.md).
use crate::header::Room;

pub const SIDE: usize = 256;
const NONE: u32 = 0xFFFF_FFFF;

fn put(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_name(out: &mut Vec<u8>, s: &str) -> Result<(), String> {
    let n = u16::try_from(s.len()).map_err(|_| format!("a name is {} bytes, the most is 65535", s.len()))?;
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

fn word(v: i64, what: &str) -> Result<u32, String> {
    u32::try_from(v).map_err(|_| format!("{what} {v} does not fit the cell source"))
}

/// The bytes of a `cell_<x>_<y>.kcell` file. `levels[z][y * 256 + x]` is the stack of that square,
/// bottom first, as 1-based indexes into `names` (empty = no tiles).
pub fn cell_source(cell_x: i64, cell_y: i64, names: &[String], levels: &[Vec<Vec<u32>>], rooms: &[Room], buildings: &[Vec<usize>]) -> Result<Vec<u8>, String> {
    let mut out = b"KCEL".to_vec();
    for v in [1, word(cell_x, "cell x")?, word(cell_y, "cell y")?, 0, levels.len() as u32] {
        put(&mut out, v);
    }
    put(&mut out, names.len() as u32);
    for n in names {
        put_name(&mut out, n)?;
    }
    put(&mut out, rooms.len() as u32);
    for r in rooms {
        put_name(&mut out, &r.name)?;
        put(&mut out, word(r.z, "room level")?);
        put(&mut out, r.rects.len() as u32);
        for &(x, y, w, h) in &r.rects {
            for v in [x, y, w, h] {
                put(&mut out, word(v, "room rectangle")?);
            }
        }
        put(&mut out, 0); // room objects: not written
    }
    put(&mut out, buildings.len() as u32);
    for b in buildings {
        put(&mut out, b.len() as u32);
        for &i in b {
            put(&mut out, i as u32);
        }
    }
    out.push(0); // no zombie bytes: the spawn map gives them
    // Level after level; one run of empty squares never crosses a level.
    for level in levels {
        assert_eq!(level.len(), SIDE * SIDE, "a level has 256 x 256 squares");
        let mut i = 0;
        while i < level.len() {
            let stack = &level[i];
            if !stack.is_empty() {
                put(&mut out, 1 + stack.len() as u32);
                put(&mut out, NONE);
                for &t in stack {
                    put(&mut out, t - 1);
                }
                i += 1;
            } else {
                let start = i;
                while i < level.len() && level[i].is_empty() {
                    i += 1;
                }
                put(&mut out, NONE);
                put(&mut out, (i - start) as u32);
            }
        }
    }
    Ok(out)
}
