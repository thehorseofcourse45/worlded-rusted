//! worlded-view: read-only viewer for WorldEd projects.
mod atlas;
mod cell_view;
mod iso;
mod settings;
mod world_view;

use eframe::egui;
use settings::Settings;
use std::path::PathBuf;
use worlded_core::{pack::PackSet, pzw::World, tmx::parse_tmx};

struct App {
    settings: Settings,
    packs: Option<PackSet>,
    pack_note: String,
    project: Option<(PathBuf, World)>,
    world_state: world_view::ViewState,
    cell: Option<cell_view::CellState>,
    atlas: atlas::Atlas,
    message: String,
}

impl App {
    fn new() -> App {
        let settings = Settings::load_from(&settings::default_path());
        let mut app = App {
            settings,
            packs: None,
            pack_note: String::new(),
            project: None,
            world_state: Default::default(),
            cell: None,
            atlas: Default::default(),
            message: String::new(),
        };
        app.load_packs();
        if let Some(p) = std::env::args_os().nth(1).map(PathBuf::from).or_else(|| app.settings.last_project.clone()) {
            app.open_project(p);
        }
        // optional second argument: open that cell straight away (index in the project cell list)
        if let Some(i) = std::env::args().nth(2).and_then(|a| a.parse::<usize>().ok()) {
            if app.project.as_ref().is_some_and(|(_, w)| i < w.cells.len()) {
                app.open_cell(i);
            }
        }
        app
    }

    fn save_settings(&self) {
        self.settings.save_to(&settings::default_path());
    }

    fn load_packs(&mut self) {
        self.atlas = Default::default();
        self.packs = None;
        self.pack_note = match self.settings.pack_dir() {
            None => "Project Zomboid folder not set: tiles are drawn as placeholders.".into(),
            Some(dir) => match PackSet::open_dir(&dir) {
                Ok((set, errs)) if set.tile_count() > 0 => {
                    self.packs = Some(set);
                    if errs.is_empty() { String::new() } else { format!("{} pack(s) could not be read", errs.len()) }
                }
                Ok(_) | Err(_) => format!("No tile packs found in {}: tiles are drawn as placeholders.", dir.display()),
            },
        };
        self.cell = self.cell.take().map(|c| cell_view::CellState::new(c.title, c.tmx));
    }

    fn open_project(&mut self, path: PathBuf) {
        match worlded_core::pzw::parse_pzw(&path) {
            Ok(w) => {
                self.message = format!("Opened {} ({} cells)", path.display(), w.cells.len());
                self.settings.last_project = Some(path.clone());
                self.save_settings();
                self.project = Some((path, w));
                self.cell = None;
                self.world_state = Default::default();
            }
            Err(e) => self.message = format!("Could not open {}: {e}", path.display()),
        }
    }

    fn open_cell(&mut self, i: usize) {
        let Some((pzw, world)) = &self.project else { return };
        let cell = &world.cells[i];
        let Some(path) = world_view::resolve_map(cell, pzw.parent().unwrap_or(std::path::Path::new("."))) else { return };
        match parse_tmx(&path) {
            Ok(t) => self.cell = Some(cell_view::CellState::new(format!("cell {},{}", cell.x, cell.y), t)),
            Err(e) => self.message = format!("Could not read {}: {e}", path.display()),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open project...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("WorldEd project", &["pzw"]).pick_file() {
                        self.open_project(p);
                    }
                }
                if ui.button("Set Project Zomboid folder...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.settings.pz_install = Some(p);
                        self.save_settings();
                        self.load_packs();
                    }
                }
                if self.cell.is_some() && ui.button("Back to world").clicked() {
                    self.cell = None;
                }
            });
            if !self.pack_note.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(220, 170, 60), &self.pack_note);
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let hover = self.cell.as_ref().and_then(|c| c.hover).map(|(x, y)| format!("   tile {x}, {y}")).unwrap_or_default();
            ui.label(format!("{}{hover}", self.message));
        });
        if let Some(c) = &mut self.cell {
            egui::SidePanel::left("layers").show(ctx, |ui| {
                ui.heading(&c.title);
                c.layers_panel(ui);
            });
        }
        let mut open = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(c) = &mut self.cell {
                c.show(ui, &mut self.atlas, self.packs.as_ref());
            } else if let Some((pzw, world)) = &self.project {
                open = world_view::show(ui, world, pzw.parent().unwrap_or(std::path::Path::new(".")), &mut self.world_state);
            } else {
                ui.label("Open a WorldEd project (.pzw) to begin.");
            }
        });
        if let Some(i) = open {
            self.open_cell(i);
        }
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native("worlded viewer", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(App::new()))))
}
