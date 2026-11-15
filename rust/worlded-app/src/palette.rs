//! Tile palette: tile names grouped by tileset, a search box and a grid of thumbnails.
use crate::atlas::Atlas;
use eframe::egui::{self, Color32, Sense};
use std::collections::BTreeMap;
use worlded_core::gids::split_tile_name;
use worlded_core::pack::PackSet;

/// Tileset prefix to its tiles as `(index, full name)`, sorted by index. Names without a
/// numeric suffix are left out.
pub fn group_tiles(names: &[&str]) -> BTreeMap<String, Vec<(u32, String)>> {
    let mut groups: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    for n in names {
        if let Some((set, idx)) = split_tile_name(n) {
            groups.entry(set.to_string()).or_default().push((idx, n.to_string()));
        }
    }
    groups.values_mut().for_each(|v| v.sort());
    groups
}

/// Group names containing `search`, ignoring case, in group order.
pub fn filter_groups<'a>(groups: &'a BTreeMap<String, Vec<(u32, String)>>, search: &str) -> Vec<&'a String> {
    let s = search.trim().to_lowercase();
    groups.keys().filter(|g| s.is_empty() || g.to_lowercase().contains(&s)).collect()
}

const CELL_W: f32 = 56.0;
const CELL_H: f32 = 84.0;

#[derive(Default)]
pub struct Palette {
    groups: BTreeMap<String, Vec<(u32, String)>>,
    search: String,
    open_group: Option<String>,
}

impl Palette {
    pub fn new(packs: Option<&PackSet>) -> Palette {
        let groups = packs.map(|p| group_tiles(&p.tile_names())).unwrap_or_default();
        Palette { groups, ..Default::default() }
    }

    /// Draws the palette; returns a tile name when the user picks one this frame.
    pub fn show(&mut self, ui: &mut egui::Ui, atlas: &mut Atlas, packs: Option<&PackSet>, selected: &str) -> Option<String> {
        if self.groups.is_empty() {
            ui.label("No tiles to browse: set the Project Zomboid folder.");
            return None;
        }
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("search tilesets"));
        let shown = filter_groups(&self.groups, &self.search);
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
        let mut next_group = None;
        egui::ScrollArea::vertical().id_salt("palette_groups").max_height(160.0).show_rows(ui, row_h, shown.len(), |ui, range| {
            for g in &shown[range] {
                let open = self.open_group.as_deref() == Some(g.as_str());
                if ui.selectable_label(open, g.as_str()).clicked() {
                    next_group = Some((*g).clone());
                }
            }
        });
        if next_group.is_some() {
            self.open_group = next_group;
        }
        ui.separator();
        let mut picked = None;
        if let Some(tiles) = self.open_group.as_ref().and_then(|g| self.groups.get(g)) {
            let cols = ((ui.available_width() / (CELL_W + ui.spacing().item_spacing.x)).floor() as usize).max(1);
            let rows = tiles.len().div_ceil(cols);
            let ctx = ui.ctx().clone();
            egui::ScrollArea::vertical().id_salt("palette_tiles").show_rows(ui, CELL_H + ui.spacing().item_spacing.y, rows, |ui, range| {
                for r in range {
                    ui.horizontal(|ui| {
                        for (_, name) in tiles.iter().skip(r * cols).take(cols) {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(CELL_W, CELL_H), Sense::click());
                            let p = ui.painter_at(rect);
                            if name == selected {
                                p.rect_filled(rect, 3.0, Color32::from_rgb(90, 80, 20));
                            } else if resp.hovered() {
                                p.rect_filled(rect, 3.0, Color32::from_gray(50));
                            }
                            // The frame (128 x 256 for ordinary tiles) scaled into the cell.
                            if let Some(s) = atlas.sprite(&ctx, packs, name) {
                                let k = (CELL_W / s.fw).min(CELL_H / s.fh);
                                let size = egui::vec2(s.w, s.h) * k;
                                let off = egui::vec2(s.ox, s.oy) * k + egui::vec2((CELL_W - s.fw * k) / 2.0, CELL_H - s.fh * k);
                                p.image(s.tex, egui::Rect::from_min_size(rect.min + off, size), s.uv, Color32::WHITE);
                            }
                            if resp.on_hover_text(name.as_str()).clicked() {
                                picked = Some(name.clone());
                            }
                        }
                    });
                }
            });
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_group_by_prefix_and_sort_numerically() {
        let g = group_tiles(&["a_10", "a_2", "b_1", "a_0"]);
        assert_eq!(g.keys().collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(g["a"].iter().map(|t| t.0).collect::<Vec<_>>(), vec![0, 2, 10]);
        assert_eq!(g["a"][2].1, "a_10");
    }

    #[test]
    fn search_filters_case_insensitively_and_keeps_group_order() {
        let g = group_tiles(&["Floors_01_0", "walls_01_0", "floors_02_3", "trees_1_1"]);
        assert_eq!(filter_groups(&g, "FLOOR"), vec!["Floors_01", "floors_02"]);
        assert_eq!(filter_groups(&g, "").len(), 4);
        assert!(filter_groups(&g, "zzz").is_empty());
    }

    #[test]
    fn names_without_numeric_suffix_are_ignored() {
        let g = group_tiles(&["plain", "x_y", "_5", "ok_3"]);
        assert_eq!(g.keys().collect::<Vec<_>>(), vec!["ok"]);
    }
}
