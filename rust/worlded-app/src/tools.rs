//! Which editing tool is active, and the pointer path helper a paint drag needs.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Paint,
    Erase,
    RectFill,
    Bucket,
    Pick,
}

impl Tool {
    pub const ALL: [(Tool, &'static str); 5] =
        [(Tool::Paint, "Paint"), (Tool::Erase, "Erase"), (Tool::RectFill, "Rectangle"), (Tool::Bucket, "Fill"), (Tool::Pick, "Eyedropper")];
}

/// Squares on the straight path from `a` (excluded) to `b` (included), so a fast drag leaves
/// no gaps.
pub fn line(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut out = Vec::new();
    while (x, y) != b {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        out.push((x, y));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_has_no_gaps_and_ends_at_b() {
        let l = line((0, 0), (5, 2));
        assert_eq!(*l.last().unwrap(), (5, 2));
        let mut prev = (0, 0);
        for p in l {
            assert!((p.0 - prev.0).abs() <= 1 && (p.1 - prev.1).abs() <= 1);
            prev = p;
        }
        assert!(line((3, 3), (3, 3)).is_empty());
        assert_eq!(line((2, 2), (0, 2)), vec![(1, 2), (0, 2)]);
    }
}
