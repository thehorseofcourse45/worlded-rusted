//! World view: the grid of cells. Pure geometry here; painting in `show`.
use worlded_core::pzw::{Cell, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const CELL_PX: f32 = 100.0; // on-screen size of one cell at zoom 1

pub fn cell_rect(cell: &Cell, zoom: f32) -> R {
    let s = CELL_PX * zoom;
    R { x: cell.x as f32 * s, y: cell.y as f32 * s, w: s, h: s }
}

pub use worlded_core::pzw::resolve_map;

#[derive(Default)]
pub struct ViewState {
    pub zoom: f32,
    pub pan: (f32, f32),
}

/// Paints the world; returns the index of a cell that was double-clicked.
/// `maps[i]` is the resolved TMX of cell i (see `resolve_map`), looked up once when the project opens.
pub fn show(ui: &mut eframe::egui::Ui, world: &World, maps: &[Option<std::path::PathBuf>], st: &mut ViewState) -> Option<usize> {
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
    let origin = resp.rect.min + egui::vec2(st.pan.0 + 20.0, st.pan.1 + 20.0);
    let mut opened = None;
    for (i, cell) in world.cells.iter().enumerate() {
        let r = cell_rect(cell, st.zoom);
        let rect = egui::Rect::from_min_size(Pos2::new(origin.x + r.x, origin.y + r.y), egui::vec2(r.w, r.h));
        if !resp.rect.intersects(rect) {
            continue;
        }
        let found = maps.get(i).is_some_and(|m| m.is_some());
        let fill = if found { Color32::from_rgb(60, 90, 60) } else { Color32::from_rgb(70, 50, 50) };
        painter.rect(rect.shrink(1.0), 2.0, fill, Stroke::new(1.0_f32, Color32::GRAY));
        let label = if found { format!("{},{}", cell.x, cell.y) } else { format!("{},{}\nmissing .tmx", cell.x, cell.y) };
        if rect.width() >= 40.0 {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(14.0), Color32::WHITE);
        }
        if found && resp.double_clicked() && resp.interact_pointer_pos().is_some_and(|p| rect.contains(p)) {
            opened = Some(i);
        }
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(x: i32, y: i32, map: Option<&str>) -> Cell {
        Cell { x, y, map: map.map(Into::into) }
    }

    #[test]
    fn cell_rects_tile_without_gaps() {
        let (a, b, c) = (cell_rect(&cell(0, 0, None), 2.0), cell_rect(&cell(1, 0, None), 2.0), cell_rect(&cell(0, 1, None), 2.0));
        assert_eq!(a.x + a.w, b.x);
        assert_eq!(a.y + a.h, c.y);
        assert_eq!(a.w, CELL_PX * 2.0);
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
