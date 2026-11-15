//! Isometric geometry of a 2x PZ cell map, in unscaled "map pixels".
//! A tile is a 128 x 64 diamond; its sprite frame is 128 x 256 with the diamond in the bottom
//! 64 rows; one level is 192 px high.
use worlded_core::tmx::Tmx;

pub const HALF_W: f32 = 64.0;
pub const HALF_H: f32 = 32.0;
pub const LEVEL_PX: f32 = 192.0;
pub const FRAME_W: f32 = 128.0;
pub const FRAME_H: f32 = 256.0;
/// Largest frames (jumbo trees, 896 x 1024) reach this far past a normal frame.
const MARGIN_X: f32 = 384.0;
const MARGIN_Y: f32 = 768.0;

/// Top vertex of the tile diamond.
pub fn tile_top(tx: i32, ty: i32, level: i32) -> (f32, f32) {
    ((tx - ty) as f32 * HALF_W, (tx + ty) as f32 * HALF_H - level as f32 * LEVEL_PX)
}

/// The level-0 tile under a map-pixel point.
pub fn tile_at(x: f32, y: f32) -> (i32, i32) {
    let (a, b) = (x / HALF_W, y / HALF_H);
    (((a + b) / 2.0).floor() as i32, ((b - a) / 2.0).floor() as i32)
}

/// Level of a layer named like `0_Floor` or `2_Walls`; 0 when the name has no number.
pub fn layer_level(name: &str) -> i32 {
    name.split('_').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Calls `f(layer, gid, tx, ty, frame_x, frame_y)` for every drawn tile of every visible layer
/// whose sprite frame touches `clip`, in painting order. `frame_*` is the frame top-left.
pub fn visible_tiles(tmx: &Tmx, visible: &[bool], clip: Rect, mut f: impl FnMut(usize, u32, i32, i32, f32, f32)) {
    let w = tmx.width as usize;
    for (li, layer) in tmx.layers.iter().enumerate() {
        if !visible.get(li).copied().unwrap_or(layer.visible) {
            continue;
        }
        let level = layer_level(&layer.name);
        for (i, &gid) in layer.gids.iter().enumerate() {
            if gid == 0 {
                continue;
            }
            let (tx, ty) = ((i % w) as i32, (i / w) as i32);
            let (sx, sy) = tile_top(tx, ty, level);
            let (fx, fy) = (sx - HALF_W, sy - (FRAME_H - 2.0 * HALF_H));
            if fx - MARGIN_X > clip.x1 || fx + FRAME_W + MARGIN_X < clip.x0 || fy - MARGIN_Y > clip.y1 || fy + FRAME_H + MARGIN_Y < clip.y0 {
                continue;
            }
            f(li, gid, tx, ty, fx, fy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worlded_core::tmx::parse_tmx_str;

    #[test]
    fn tile_at_inverts_tile_top() {
        for (tx, ty) in [(0, 0), (3, 2), (299, 0), (10, 250)] {
            let (x, y) = tile_top(tx, ty, 0);
            assert_eq!(tile_at(x, y + HALF_H), (tx, ty), "centre of ({tx},{ty})");
        }
    }

    #[test]
    fn levels_come_from_layer_names() {
        assert_eq!((layer_level("0_Floor"), layer_level("3_Walls"), layer_level("Floor")), (0, 3, 0));
        assert_eq!(tile_top(0, 0, 1).1, -LEVEL_PX);
    }

    fn tmx() -> Tmx {
        parse_tmx_str(
            r#"<map width="2" height="2"><tileset firstgid="1" name="t"/>
          <layer name="0_Floor"><data encoding="csv">1,1,1,1</data></layer>
          <layer name="1_Walls" visible="0"><data encoding="csv">0,0,0,1</data></layer></map>"#,
        )
        .unwrap()
    }

    #[test]
    fn hidden_layers_and_empty_tiles_are_skipped_and_clip_culls() {
        let t = tmx();
        let all = Rect { x0: -1e4, y0: -1e4, x1: 1e4, y1: 1e4 };
        let mut n = 0;
        visible_tiles(&t, &[true, false], all, |_, _, _, _, _, _| n += 1);
        assert_eq!(n, 4);
        let mut n = 0;
        visible_tiles(&t, &[true, true], all, |_, _, _, _, _, _| n += 1);
        assert_eq!(n, 5);
        let mut n = 0;
        visible_tiles(&t, &[true, false], Rect { x0: 5000.0, y0: 0.0, x1: 6000.0, y1: 100.0 }, |_, _, _, _, _, _| n += 1);
        assert_eq!(n, 0); // far to the east: culled
        let mut n = 0;
        visible_tiles(&t, &[true, false], Rect { x0: 0.0, y0: 0.0, x1: 10.0, y1: 10.0 }, |_, _, _, _, _, _| n += 1);
        assert_eq!(n, 4); // a window on the map: all four tiles (their frames reach it)
    }

    #[test]
    fn empty_map_draws_nothing() {
        let t = parse_tmx_str(r#"<map width="0" height="0"/>"#).unwrap();
        visible_tiles(&t, &[], Rect { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 }, |_, _, _, _, _, _| panic!());
    }
}
