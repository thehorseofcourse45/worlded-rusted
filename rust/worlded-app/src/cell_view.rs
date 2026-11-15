//! Cell view: one TMX drawn as isometric tiles, with layer toggles, a tile readout and editing.
use crate::atlas::{Atlas, Sprite};
use crate::document::Document;
use crate::iso::{self, Rect};
use crate::tools::{line, Tool};
use eframe::egui::{self, epaint, Color32, Mesh, Pos2, Sense, Shape};
use std::collections::HashMap;
use worlded_core::gids::gid_for;
use worlded_core::pack::PackSet;
use worlded_core::tmx::{tile_name, Tmx};
use worlded_core::tools as core_tools;

/// Below this zoom only the first visible layer is drawn (a 300x300 cell with several
/// layers would otherwise be hundreds of thousands of quads per frame).
const LOD_ZOOM: f32 = 0.15;
/// Columns assumed for a tileset the file does not list yet (PZ 2x sheets are 512 px wide).
const NEW_TILESET_COLUMNS: u32 = 8;
/// Most squares of a brush or rectangle preview that are drawn.
const PREVIEW_MAX: usize = 400;

pub struct CellState {
    pub title: String,
    pub doc: Document,
    pub visible: Vec<bool>,
    pub locked: Vec<bool>,
    pub active_layer: usize,
    pub tool: Tool,
    pub brush: u32,
    /// Tile name (`<tileset>_<index>`) the paint and rectangle tools lay down.
    pub selected_tile: String,
    pub status: String,
    /// Indexed by gid: `None` = not looked up yet, `Some(None)` = no art (placeholder).
    sprites: Vec<Option<Option<Sprite>>>,
    zoom: f32,
    pan: egui::Vec2,
    pub hover: Option<(i32, i32)>,
    stroke: Option<Stroke>,
}

struct Stroke {
    start: (i32, i32),
    last: (i32, i32),
}

/// The tile under a screen point on a layer of the given level (higher levels sit higher on screen).
pub fn tile_at_level(p: Pos2, origin: Pos2, zoom: f32, level: i32) -> (i32, i32) {
    iso::tile_at((p.x - origin.x) / zoom, (p.y - origin.y) / zoom + level as f32 * iso::LEVEL_PX)
}

impl CellState {
    pub fn new(title: String, doc: Document) -> CellState {
        let n = doc.doc.tmx.layers.len();
        let visible = doc.doc.tmx.layers.iter().map(|l| l.visible).collect();
        CellState {
            title,
            doc,
            visible,
            locked: vec![false; n],
            active_layer: 0,
            tool: Tool::Paint,
            brush: 1,
            selected_tile: String::new(),
            status: String::new(),
            sprites: Vec::new(),
            zoom: 0.0,
            pan: egui::Vec2::ZERO,
            hover: None,
            stroke: None,
        }
    }

    pub fn tmx(&self) -> &Tmx {
        &self.doc.doc.tmx
    }

    /// Forget looked-up art (after the Project Zomboid folder changed); keeps view and toggles.
    pub fn reset_art(&mut self) {
        self.sprites.clear();
    }

    pub fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let editable = self.doc.read_only.is_none();
        for i in 0..self.doc.doc.tmx.layers.len() {
            ui.horizontal(|ui| {
                if editable {
                    ui.radio_value(&mut self.active_layer, i, "");
                }
                ui.checkbox(&mut self.visible[i], &self.doc.doc.tmx.layers[i].name);
                if editable {
                    ui.checkbox(&mut self.locked[i], "lock");
                }
            });
        }
    }

    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        if self.doc.read_only.is_some() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (t, label) in Tool::ALL {
                ui.selectable_value(&mut self.tool, t, label);
            }
            ui.add(egui::DragValue::new(&mut self.brush).range(1..=9).prefix("brush "));
            ui.label("tile");
            ui.add(egui::TextEdit::singleline(&mut self.selected_tile).desired_width(220.0).hint_text("tileset_index"));
            if ui.button("Undo").clicked() {
                self.undo();
            }
            if ui.button("Redo").clicked() {
                self.redo();
            }
            if ui.button("Save").clicked() {
                self.save();
            }
        });
    }

    pub fn undo(&mut self) {
        self.stroke = None;
        if !self.doc.history.undo(&mut self.doc.doc) {
            self.status = "nothing to undo".into();
        }
    }

    pub fn redo(&mut self) {
        if !self.doc.history.redo(&mut self.doc.doc) {
            self.status = "nothing to redo".into();
        }
    }

    pub fn save(&mut self) {
        self.status = match self.doc.save() {
            Ok(()) => format!("saved {}", self.doc.path.display()),
            Err(e) => format!("save failed: {e}"),
        };
    }

    pub fn save_as(&mut self, path: &std::path::Path) {
        self.status = match self.doc.save_as(path) {
            Ok(()) => format!("saved {}", path.display()),
            Err(e) => format!("save failed: {e}"),
        };
    }

    fn tile_gid(&mut self) -> Result<u32, String> {
        let name = self.selected_tile.trim().to_string();
        if name.is_empty() {
            return Err("no tile selected: type a name or use the eyedropper".into());
        }
        gid_for(&mut self.doc.doc, &name, NEW_TILESET_COLUMNS).map_err(|e| e.to_string())
    }

    /// Run a tool on the active layer. Refusals (read-only, hidden, locked, no tile) land in the
    /// status line; nothing changes then.
    fn edit_with(&mut self, needs_tile: bool, make: impl FnOnce(&Tmx, usize, u32) -> Option<worlded_core::edit::Edit>) {
        let l = self.active_layer;
        let (vis, lock) = (self.visible.get(l).copied().unwrap_or(false), self.locked.get(l).copied().unwrap_or(false));
        if let Err(e) = self.doc.check(l, vis, lock) {
            self.status = e;
            return;
        }
        let gid = if needs_tile {
            match self.tile_gid() {
                Ok(g) => g,
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        } else {
            0
        };
        match make(&self.doc.doc.tmx, l, gid) {
            Some(edit) => {
                self.status.clear();
                let _ = self.doc.apply(l, vis, lock, edit);
            }
            None => self.status = "fill region too large".into(),
        }
    }

    fn paint_at(&mut self, (x, y): (i32, i32)) {
        let brush = self.brush;
        if self.tool == Tool::Erase {
            self.edit_with(false, |t, l, _| Some(core_tools::erase(t, l, x, y, brush)));
        } else {
            self.edit_with(true, |t, l, g| Some(core_tools::paint(t, l, x, y, brush, g)));
        }
    }

    fn press(&mut self, at: (i32, i32)) {
        match self.tool {
            Tool::Paint | Tool::Erase => {
                self.doc.history.begin_group();
                self.stroke = Some(Stroke { start: at, last: at });
                self.paint_at(at);
            }
            Tool::RectFill => self.stroke = Some(Stroke { start: at, last: at }),
            Tool::Bucket => {
                let limit = (self.tmx().width * self.tmx().height) as usize;
                self.edit_with(true, |t, l, g| core_tools::bucket_fill(t, l, at.0, at.1, g, limit));
            }
            Tool::Pick => {
                let name = core_tools::pick(self.tmx(), self.active_layer, at.0, at.1).and_then(|g| tile_name(self.tmx(), g));
                match name {
                    Some(n) => {
                        self.status = format!("picked {n}");
                        self.selected_tile = n;
                    }
                    None => self.status = "nothing to pick there".into(),
                }
            }
        }
    }

    fn drag_to(&mut self, at: (i32, i32)) {
        let Some(s) = &mut self.stroke else { return };
        let from = s.last;
        s.last = at;
        if matches!(self.tool, Tool::Paint | Tool::Erase) {
            for p in line(from, at) {
                self.paint_at(p);
            }
        }
    }

    fn release(&mut self) {
        let Some(s) = self.stroke.take() else { return };
        match self.tool {
            Tool::Paint | Tool::Erase => self.doc.history.end_group(),
            Tool::RectFill => self.edit_with(true, |t, l, g| Some(core_tools::rect_fill(t, l, s.start, s.last, g))),
            _ => {}
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, atlas: &mut Atlas, packs: Option<&PackSet>) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        if self.zoom == 0.0 {
            // start centred on the middle of the map
            self.zoom = 0.25;
            let (cx, cy) = iso::tile_top(self.tmx().width as i32 / 2, self.tmx().height as i32 / 2, 0);
            self.pan = egui::vec2(-cx * self.zoom, -cy * self.zoom);
        }
        let centre = rect.center();
        let editing = self.doc.read_only.is_none();
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        // Middle button, or space + drag, pans; a read-only file pans with the primary button as before.
        if resp.dragged_by(egui::PointerButton::Middle) || ((!editing || space) && resp.dragged_by(egui::PointerButton::Primary)) {
            self.pan += resp.drag_delta();
        }
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
        let level = self.tmx().layers.get(self.active_layer).map_or(0, |l| l.level);
        self.hover = resp.hover_pos().map(|p| tile_at_level(p, origin, zoom, level));

        if editing && !space {
            let (pressed, down, released) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released()));
            match (self.hover, pressed && resp.hovered()) {
                (Some(at), true) => self.press(at),
                (Some(at), false) if down && self.stroke.is_some() => self.drag_to(at),
                _ => {}
            }
            if released {
                self.release();
            }
        }

        let (x0, y0) = ((rect.min.x - origin.x) / zoom, (rect.min.y - origin.y) / zoom);
        let (x1, y1) = ((rect.max.x - origin.x) / zoom, (rect.max.y - origin.y) / zoom);
        let clip = Rect { x0, y0, x1, y1 };

        let tmx = &self.doc.doc.tmx;
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

        if editing {
            // Preview of what the next click would touch: the brush squares, or the rectangle being dragged.
            let squares = match (&self.stroke, self.hover) {
                (Some(s), _) if self.tool == Tool::RectFill => Some(core_tools::rect_fill(tmx, self.active_layer, s.start, s.last, u32::MAX)),
                (None, Some((x, y))) if matches!(self.tool, Tool::Paint | Tool::Erase) => Some(core_tools::paint(tmx, self.active_layer, x, y, self.brush, u32::MAX)),
                (None, Some((x, y))) => Some(core_tools::paint(tmx, self.active_layer, x, y, 1, u32::MAX)),
                _ => None,
            };
            for c in squares.iter().flat_map(|e| e.0.iter()).take(PREVIEW_MAX) {
                let (tx, ty) = iso::tile_top(c.x as i32, c.y as i32, level);
                let pts = vec![
                    to_screen(tx, ty),
                    to_screen(tx + iso::HALF_W, ty + iso::HALF_H),
                    to_screen(tx, ty + 2.0 * iso::HALF_H),
                    to_screen(tx - iso::HALF_W, ty + iso::HALF_H),
                ];
                painter.add(Shape::convex_polygon(pts, Color32::from_rgba_unmultiplied(255, 220, 80, 70), egui::Stroke::new(1.0_f32, Color32::from_rgb(255, 220, 80))));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_at_level_inverts_tile_top_for_every_level() {
        let (origin, zoom) = (Pos2::new(100.0, 50.0), 0.5_f32);
        for level in 0..=3 {
            for (tx, ty) in [(0, 0), (3, 2), (299, 0), (10, 250)] {
                let (x, y) = iso::tile_top(tx, ty, level);
                // centre of the diamond, as a screen point
                let p = Pos2::new(origin.x + x * zoom, origin.y + (y + iso::HALF_H) * zoom);
                assert_eq!(tile_at_level(p, origin, zoom, level), (tx, ty), "level {level} tile ({tx},{ty})");
            }
        }
    }

    fn cell() -> CellState {
        let text = "<map width=\"4\" height=\"4\"><tileset firstgid=\"1\" name=\"t\"/><layer name=\"A\" level=\"0\" width=\"4\" height=\"4\"><data encoding=\"csv\">0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0</data></layer></map>";
        CellState::new("c".into(), Document::from_text(std::path::Path::new("c.tmx"), text.into()).unwrap())
    }

    #[test]
    fn paint_drag_is_one_undo_step_and_fill_and_pick_work() {
        let mut c = cell();
        c.selected_tile = "t_2".into();
        c.press((0, 0));
        c.drag_to((3, 0));
        c.release();
        assert_eq!(c.tmx().layers[0].gids[..4], [3, 3, 3, 3]);
        c.undo();
        assert!(c.tmx().layers[0].gids.iter().all(|g| *g == 0), "whole drag undone at once");
        c.redo();
        c.tool = Tool::Bucket;
        c.selected_tile = "t_0".into();
        c.press((0, 3)); // fills the empty region below the painted row with gid 1
        assert_eq!(c.tmx().layers[0].gids[4..].iter().filter(|g| **g == 1).count(), 12);
        c.tool = Tool::Pick;
        c.selected_tile.clear();
        c.press((0, 0));
        assert_eq!(c.selected_tile, "t_2");
    }

    #[test]
    fn locked_hidden_and_missing_tile_say_why_and_change_nothing() {
        let mut c = cell();
        c.press((1, 1));
        assert!(c.status.contains("no tile"), "{}", c.status);
        c.release();
        c.selected_tile = "t_1".into();
        c.locked[0] = true;
        c.press((1, 1));
        assert!(c.status.contains("locked"), "{}", c.status);
        c.release();
        c.locked[0] = false;
        c.visible[0] = false;
        c.press((1, 1));
        assert!(c.status.contains("hidden"), "{}", c.status);
        c.release();
        c.visible[0] = true;
        c.press((-5, 9)); // off the map: clipped, nothing changes, no panic
        c.release();
        assert!(c.tmx().layers[0].gids.iter().all(|g| *g == 0));
        assert!(!c.doc.is_dirty());
    }
}
