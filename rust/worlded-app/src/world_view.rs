//! World view: the grid of cells with their lots (buildings) and objects. Pure geometry here;
//! painting and mouse handling in `show`.
use worlded_core::pzw::{Cell, CellChild, Lot, Obj, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const CELL_PX: f32 = 100.0; // on-screen size of one cell at zoom 1
pub const CELL_TILES: i32 = 300; // a WorldEd cell is 300x300 tiles

pub fn cell_rect(cell: &Cell, zoom: f32) -> R {
    let s = CELL_PX * zoom;
    R { x: cell.x as f32 * s, y: cell.y as f32 * s, w: s, h: s }
}

/// Lot rectangle in the same coordinate space as `cell_rect` (lot x/y are cell-local tiles).
pub fn lot_rect(cell: &Cell, lot: &Lot, zoom: f32) -> R {
    let s = CELL_PX * zoom / CELL_TILES as f32;
    let base = cell_rect(cell, zoom);
    R { x: base.x + lot.x as f32 * s, y: base.y + lot.y as f32 * s, w: lot.width.max(1) as f32 * s, h: lot.height.max(1) as f32 * s }
}

/// Center of an object, same coordinate space.
pub fn obj_pos(cell: &Cell, o: &Obj, zoom: f32) -> (f32, f32) {
    let s = CELL_PX * zoom / CELL_TILES as f32;
    let base = cell_rect(cell, zoom);
    (base.x + (o.x as f32 + o.width as f32 / 2.0) * s, base.y + (o.y as f32 + o.height as f32 / 2.0) * s)
}

pub use worlded_core::pzw::resolve_map;

#[derive(Default)]
pub struct ViewState {
    pub zoom: f32,
    pub pan: (f32, f32),
}

/// What the user did on the world map.
pub enum WorldAction {
    OpenCell(usize),
    /// Place the selected building at cell-local tile (x, y).
    PlaceLot { cell: usize, x: i64, y: i64 },
    /// Delete lot `idx` of cell `cell`.
    RemoveLot { cell: usize, idx: usize },
}

/// The building being placed: (file name, width, height in tiles).
pub type Placing = Option<(String, i64, i64)>;

/// World-tile under a screen point, and the cell-local coords, if inside a listed cell.
pub fn tile_at(world: &World, origin: (f32, f32), zoom: f32, p: (f32, f32)) -> Option<(usize, i64, i64)> {
    let s = CELL_PX * zoom;
    let tx = (p.0 - origin.0) / s * CELL_TILES as f32;
    let ty = (p.1 - origin.1) / s * CELL_TILES as f32;
    if tx < 0.0 || ty < 0.0 {
        return None;
    }
    let (tx, ty) = (tx as i32, ty as i32);
    let cell = world.cell_at(tx / CELL_TILES, ty / CELL_TILES)?;
    Some((cell, (tx % CELL_TILES) as i64, (ty % CELL_TILES) as i64))
}

/// The lot under a screen point, if any.
pub fn lot_at(world: &World, origin: (f32, f32), zoom: f32, p: (f32, f32)) -> Option<(usize, usize)> {
    for (ci, cell) in world.cells.iter().enumerate() {
        let base = cell_rect(cell, zoom);
        let cell_screen = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::new(origin.0 + base.x, origin.1 + base.y),
            eframe::egui::vec2(base.w, base.h),
        );
        let pp = eframe::egui::Pos2::new(p.0, p.1);
        if !cell_screen.contains(pp) {
            continue;
        }
        for (li, ch) in cell.children.iter().enumerate() {
            if let CellChild::Lot(l) = ch {
                let r = lot_rect(cell, l, zoom);
                let rect = eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::new(origin.0 + r.x, origin.1 + r.y),
                    eframe::egui::vec2(r.w, r.h),
                );
                if rect.contains(pp) {
                    return Some((ci, li));
                }
            }
        }
    }
    None
}

/// Paints the world; returns what the user did (open a cell, place/remove a lot).
/// `maps[i]` is the resolved TMX of cell i (see `resolve_map`), looked up once when the project opens.
pub fn show(ui: &mut eframe::egui::Ui, world: &World, maps: &[Option<std::path::PathBuf>], st: &mut ViewState, placing: &mut Placing) -> Option<WorldAction> {
    use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
    if st.zoom == 0.0 {
        st.zoom = 1.0;
    }
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    st.pan.0 += resp.drag_delta().x;
    st.pan.1 += resp.drag_delta().y;
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            st.zoom = (st.zoom * (1.0 + scroll * 0.002)).clamp(0.1, 20.0);
        }
    }
    let origin = (resp.rect.min.x + st.pan.0 + 20.0, resp.rect.min.y + st.pan.1 + 20.0);
    let mut action = None;
    if let Some(p) = resp.interact_pointer_pos() {
        let pp = (p.x, p.y);
        if placing.is_some() && resp.clicked() {
            if let Some((cell, x, y)) = tile_at(world, origin, st.zoom, pp) {
                action = Some(WorldAction::PlaceLot { cell, x, y });
            }
        } else if resp.secondary_clicked() {
            if let Some((cell, idx)) = lot_at(world, origin, st.zoom, pp) {
                action = Some(WorldAction::RemoveLot { cell, idx });
            }
        }
    }
    for (i, cell) in world.cells.iter().enumerate() {
        let r = cell_rect(cell, st.zoom);
        let rect = egui::Rect::from_min_size(Pos2::new(origin.0 + r.x, origin.1 + r.y), egui::vec2(r.w, r.h));
        if !resp.rect.intersects(rect) {
            continue;
        }
        let found = maps.get(i).is_some_and(|m| m.is_some());
        let fill = if found { Color32::from_rgb(60, 90, 60) } else { Color32::from_rgb(70, 50, 50) };
        painter.rect(rect.shrink(1.0), 2.0, fill, Stroke::new(1.0_f32, Color32::GRAY));
        // lots (buildings) on this cell
        for ch in &cell.children {
            if let CellChild::Lot(l) = ch {
                let lr = lot_rect(cell, l, st.zoom);
                let lrect = egui::Rect::from_min_size(Pos2::new(origin.0 + lr.x, origin.1 + lr.y), egui::vec2(lr.w, lr.h));
                painter.rect(lrect, 1.0, Color32::from_rgba_premultiplied(80, 130, 220, 90), Stroke::new(1.0_f32, Color32::from_rgb(140, 180, 255)));
                if lrect.width() >= 30.0 {
                    let stem = std::path::Path::new(&l.map).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| l.map.clone());
                    painter.text(lrect.center(), egui::Align2::CENTER_CENTER, stem, egui::FontId::proportional(10.0), Color32::from_rgb(200, 220, 255));
                }
            } else if let CellChild::Object(o) = ch {
                let (x, y) = obj_pos(cell, o, st.zoom);
                painter.circle(Pos2::new(origin.0 + x, origin.1 + y), (CELL_PX * st.zoom / CELL_TILES as f32 * 2.0).max(2.5), Color32::from_rgb(230, 200, 90), Stroke::new(1.0_f32, Color32::BLACK));
            }
        }
        let label = if found { format!("{},{}", cell.x, cell.y) } else { format!("{},{}\nmissing .tmx", cell.x, cell.y) };
        if rect.width() >= 40.0 {
            painter.text(rect.min + egui::vec2(4.0, 2.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(12.0), Color32::WHITE);
        }
        if action.is_none() && placing.is_none() && resp.double_clicked() && resp.interact_pointer_pos().is_some_and(|p| rect.contains(p)) {
            action = Some(WorldAction::OpenCell(i));
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use worlded_core::pzw::Lot;

    fn cell(x: i32, y: i32, map: Option<&str>) -> Cell {
        Cell { x, y, map: map.map(str::to_string), children: vec![], lead: String::new(), self_closing: false }
    }

    fn lot(x: i64, y: i64, w: i64, h: i64) -> Lot {
        Lot { x, y, level: 0, width: w, height: h, map: "buildings/house.tbx".into() }
    }

    #[test]
    fn cell_rects_tile_without_gaps() {
        let (a, b, c) = (cell_rect(&cell(0, 0, None), 2.0), cell_rect(&cell(1, 0, None), 2.0), cell_rect(&cell(0, 1, None), 2.0));
        assert_eq!(a.x + a.w, b.x);
        assert_eq!(a.y + a.h, c.y);
        assert_eq!(a.w, CELL_PX * 2.0);
    }

    #[test]
    fn lot_rect_is_inside_its_cell() {
        let c = cell(1, 0, None);
        let r = lot_rect(&c, &lot(150, 30, 25, 29), 1.0);
        let base = cell_rect(&c, 1.0);
        assert!(r.x >= base.x && r.x + r.w <= base.x + base.w);
        assert!(r.y >= base.y && r.y + r.h <= base.y + base.h);
        assert!((r.x - base.x - 150.0 * CELL_PX / CELL_TILES as f32).abs() < 1e-4);
    }

    #[test]
    fn tile_at_maps_screen_to_cell_local_tiles() {
        let mut w = World {
            width: 2,
            height: 1,
            origin: (70, 0),
            head: String::new(),
            nl: "\n".into(),
            raw: vec![],
            cells: vec![cell(0, 0, None), cell(1, 0, None)],
            closing: String::new(),
        };
        w.cells[1].children.push(CellChild::Lot(lot(10, 20, 5, 5)));
        let zoom = 2.0;
        let s = CELL_PX * zoom;
        // center of cell (1,0) is tile (450, 150) -> local (150, 150)
        let (ci, x, y) = tile_at(&w, (20.0, 20.0), zoom, (20.0 + 1.5 * s, 20.0 + 0.5 * s)).unwrap();
        assert_eq!((ci, x, y), (1, 150, 150));
        assert!(tile_at(&w, (20.0, 20.0), zoom, (5.0, 5.0)).is_none(), "before the grid");
        assert!(tile_at(&w, (20.0, 20.0), zoom, (20.0 + 5.0 * s, 20.0)).is_none(), "beyond the listed cells");
        // lot at local (10,20)-(15,25): its center in screen coords
        let lr = lot_rect(&w.cells[1], match &w.cells[1].children[0] { CellChild::Lot(l) => l, _ => unreachable!() }, zoom);
        let hit = lot_at(&w, (20.0, 20.0), zoom, (20.0 + lr.x + lr.w / 2.0, 20.0 + lr.y + lr.h / 2.0));
        assert_eq!(hit, Some((1, 0)));
        assert!(lot_at(&w, (20.0, 20.0), zoom, (20.0 + 0.5 * s, 20.0 + 0.5 * s)).is_none(), "empty part of cell 1,0");
    }

    #[test]
    fn missing_map_cell_is_flagged() {
        let dir = std::env::temp_dir();
        assert!(resolve_map(&cell(0, 0, None), &dir).is_none());
        assert!(resolve_map(&cell(0, 0, Some("Z:/no/such/file.tmx")), &dir).is_none());
        let f = dir.join("wa_found.tmx");
        std::fs::write(&f, "<map/>").unwrap();
        assert_eq!(resolve_map(&cell(0, 0, Some("Z:/elsewhere/wa_found.tmx")), &dir), Some(f)); // found beside the project
    }
}
