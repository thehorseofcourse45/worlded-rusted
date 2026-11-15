//! Cell view: one TMX drawn as isometric tiles, with layer toggles and a tile readout.
use crate::atlas::{Atlas, Sprite};
use crate::iso::{self, Rect};
use eframe::egui::{self, epaint, Color32, Mesh, Pos2, Sense, Shape};
use std::collections::HashMap;
use worlded_core::pack::PackSet;
use worlded_core::tmx::{tile_name, Tmx};

/// Below this zoom only the first visible layer is drawn (a 300x300 cell with several
/// layers would otherwise be hundreds of thousands of quads per frame).
const LOD_ZOOM: f32 = 0.15;

pub struct CellState {
    pub title: String,
    pub tmx: Tmx,
    pub visible: Vec<bool>,
    /// Indexed by gid: `None` = not looked up yet, `Some(None)` = no art (placeholder).
    sprites: Vec<Option<Option<Sprite>>>,
    zoom: f32,
    pan: egui::Vec2,
    pub hover: Option<(i32, i32)>,
}

/// Map pixel under a screen point, and the level-0 tile there.
pub fn screen_to_tile(p: Pos2, origin: Pos2, zoom: f32) -> (i32, i32) {
    iso::tile_at((p.x - origin.x) / zoom, (p.y - origin.y) / zoom)
}

impl CellState {
    pub fn new(title: String, tmx: Tmx) -> CellState {
        let visible = tmx.layers.iter().map(|l| l.visible).collect();
        CellState { title, tmx, visible, sprites: Vec::new(), zoom: 0.0, pan: egui::Vec2::ZERO, hover: None }
    }

    /// Forget looked-up art (after the Project Zomboid folder changed); keeps view and toggles.
    pub fn reset_art(&mut self) {
        self.sprites.clear();
    }

    pub fn layers_panel(&mut self, ui: &mut egui::Ui) {
        for (i, l) in self.tmx.layers.iter().enumerate() {
            ui.checkbox(&mut self.visible[i], &l.name);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, atlas: &mut Atlas, packs: Option<&PackSet>) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        if self.zoom == 0.0 {
            // start centred on the middle of the map
            self.zoom = 0.25;
            let (cx, cy) = iso::tile_top(self.tmx.width as i32 / 2, self.tmx.height as i32 / 2, 0);
            self.pan = egui::vec2(-cx * self.zoom, -cy * self.zoom);
        }
        let centre = rect.center();
        self.pan += resp.drag_delta();
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let old = self.zoom;
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.05, 4.0);
                if let Some(m) = resp.hover_pos() {
                    let k = self.zoom / old; // keep the point under the cursor fixed
                    self.pan = (m - centre) - ((m - centre) - self.pan) * k;
                }
            }
        }
        let origin = centre + self.pan;
        let zoom = self.zoom;
        self.hover = resp.hover_pos().map(|p| screen_to_tile(p, origin, zoom));
        let (x0, y0) = ((rect.min.x - origin.x) / zoom, (rect.min.y - origin.y) / zoom);
        let (x1, y1) = ((rect.max.x - origin.x) / zoom, (rect.max.y - origin.y) / zoom);
        let clip = Rect { x0, y0, x1, y1 };

        let tmx = &self.tmx;
        let sprites = &mut self.sprites;
        let ctx = ui.ctx().clone();
        let first_visible = self.visible.iter().position(|v| *v);
        let mut visible = self.visible.clone();
        if zoom < LOD_ZOOM {
            for (i, v) in visible.iter_mut().enumerate() {
                *v = Some(i) == first_visible;
            }
        }
        let mut meshes: HashMap<egui::TextureId, Mesh> = HashMap::new();
        let mut flat = Mesh::default();
        let to_screen = |x: f32, y: f32| Pos2::new(origin.x + x * zoom, origin.y + y * zoom);
        iso::visible_tiles(tmx, &visible, clip, |_, gid, _, _, fx, fy| {
            let g = gid as usize;
            if sprites.len() <= g {
                sprites.resize(g + 1, None);
            }
            let s = *sprites[g].get_or_insert_with(|| tile_name(tmx, gid).and_then(|n| atlas.sprite(&ctx, packs, &n)));
            match s {
                Some(s) => {
                    // frames larger than 128 x 256 (trees) are anchored bottom-centre on the tile
                    let ax = fx - (s.fw - iso::FRAME_W) / 2.0;
                    let ay = fy - (s.fh - iso::FRAME_H);
                    let r = egui::Rect::from_min_size(to_screen(ax + s.ox, ay + s.oy), egui::vec2(s.w, s.h) * zoom);
                    meshes.entry(s.tex).or_insert_with(|| Mesh::with_texture(s.tex)).add_rect_with_uv(r, s.uv, Color32::WHITE);
                }
                None => {
                    // placeholder: the tile diamond (bottom 64 rows of the frame)
                    let (cx, top) = (fx + iso::HALF_W, fy + iso::FRAME_H - 2.0 * iso::HALF_H);
                    let base = flat.vertices.len() as u32;
                    for (x, y) in [(cx, top), (cx + iso::HALF_W, top + iso::HALF_H), (cx, top + 2.0 * iso::HALF_H), (cx - iso::HALF_W, top + iso::HALF_H)] {
                        flat.vertices.push(epaint::Vertex { pos: to_screen(x, y), uv: epaint::WHITE_UV, color: Color32::from_gray(60) });
                    }
                    flat.add_triangle(base, base + 1, base + 2);
                    flat.add_triangle(base, base + 2, base + 3);
                }
            }
        });
        if !flat.is_empty() {
            painter.add(Shape::mesh(flat));
        }
        for (_, m) in meshes {
            painter.add(Shape::mesh(m));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_over_a_tile_centre_reports_that_tile() {
        // tile (3,2) at level 0: top vertex (64,160), centre (64,192); view origin at (100,50), zoom 0.5
        let (origin, zoom) = (Pos2::new(100.0, 50.0), 0.5);
        let centre = Pos2::new(origin.x + 64.0 * zoom, origin.y + 192.0 * zoom);
        assert_eq!(screen_to_tile(centre, origin, zoom), (3, 2));
    }
}
