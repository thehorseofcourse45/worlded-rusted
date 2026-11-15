//! Cell view: one TMX drawn as isometric tiles, with layer toggles and a tile readout.
use crate::atlas::{Atlas, Sprite};
use crate::iso::{self, Rect};
use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke};
use std::collections::HashMap;
use worlded_core::pack::PackSet;
use worlded_core::tmx::{tile_name, Tmx};

pub struct CellState {
    pub title: String,
    pub tmx: Tmx,
    pub visible: Vec<bool>,
    sprites: HashMap<u32, Option<Sprite>>,
    zoom: f32,
    pan: egui::Vec2,
    pub hover: Option<(i32, i32)>,
}

impl CellState {
    pub fn new(title: String, tmx: Tmx) -> CellState {
        let visible = tmx.layers.iter().map(|l| l.visible).collect();
        CellState { title, tmx, visible, sprites: HashMap::new(), zoom: 0.0, pan: egui::Vec2::ZERO, hover: None }
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
                self.zoom = (self.zoom * (1.0 + scroll * 0.002)).clamp(0.05, 4.0);
                if let Some(m) = resp.hover_pos() {
                    let k = self.zoom / old; // keep the point under the cursor fixed
                    self.pan = (m - centre) - ((m - centre) - self.pan) * k;
                }
            }
        }
        let origin = centre + self.pan;
        let to_map = |p: Pos2| ((p.x - origin.x) / self.zoom, (p.y - origin.y) / self.zoom);
        self.hover = resp.hover_pos().map(|p| {
            let (x, y) = to_map(p);
            iso::tile_at(x, y + iso::HALF_H) // the pointer is over the diamond body, not its top vertex
        });
        let (x0, y0) = to_map(rect.min);
        let (x1, y1) = to_map(rect.max);
        let clip = Rect { x0, y0, x1, y1 };
        let tmx = &self.tmx;
        let sprites = &mut self.sprites;
        let zoom = self.zoom;
        let ctx = ui.ctx().clone();
        let to_screen = |x: f32, y: f32| Pos2::new(origin.x + x * zoom, origin.y + y * zoom);
        iso::visible_tiles(tmx, &self.visible, clip, |_, gid, _, _, fx, fy| {
            let s = sprites.entry(gid).or_insert_with(|| tile_name(tmx, gid).and_then(|n| atlas.sprite(&ctx, packs, &n))).clone();
            match s {
                Some(s) => {
                    let r = egui::Rect::from_min_size(to_screen(fx + s.ox, fy + s.oy), egui::vec2(s.w, s.h) * zoom);
                    painter.image(s.tex, r, s.uv, Color32::WHITE);
                }
                None => {
                    // placeholder: the tile diamond (frame bottom 64 rows)
                    let (cx, top) = (fx + iso::HALF_W, fy + iso::FRAME_H - 2.0 * iso::HALF_H);
                    let pts = vec![
                        to_screen(cx, top),
                        to_screen(cx + iso::HALF_W, top + iso::HALF_H),
                        to_screen(cx, top + 2.0 * iso::HALF_H),
                        to_screen(cx - iso::HALF_W, top + iso::HALF_H),
                    ];
                    painter.add(Shape::convex_polygon(pts, Color32::from_gray(60), Stroke::new(1.0_f32, Color32::from_gray(110))));
                }
            }
        });
    }
}
