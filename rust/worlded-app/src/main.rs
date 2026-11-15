//! worlded-view: viewer and tile editor for WorldEd projects.
mod atlas;
mod cell_view;
mod document;
mod iso;
mod settings;
mod tools;
mod world_view;

use eframe::egui;
use settings::Settings;
use std::path::PathBuf;
use worlded_core::{pack::PackSet, pzw::World};

struct App {
    settings: Settings,
    packs: Option<PackSet>,
    pack_note: String,
    project: Option<(PathBuf, World)>,
    /// Resolved .tmx of each cell of the open project (None = missing).
    maps: Vec<Option<PathBuf>>,
    world_state: world_view::ViewState,
    cell: Option<cell_view::CellState>,
    atlas: atlas::Atlas,
    message: String,
    /// A close or "back to world" waiting for an answer about unsaved changes.
    confirm: Option<Confirm>,
    allow_quit: bool,
    last_title: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    Back,
    Quit,
}

impl App {
    fn new() -> App {
        let settings = Settings::load_from(&settings::default_path());
        let mut app = App {
            settings,
            packs: None,
            pack_note: String::new(),
            project: None,
            maps: Vec::new(),
            world_state: Default::default(),
            cell: None,
            atlas: Default::default(),
            message: String::new(),
            confirm: None,
            allow_quit: false,
            last_title: String::new(),
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
        if let Some(c) = &mut self.cell {
            c.reset_art();
        }
    }

    fn open_project(&mut self, path: PathBuf) {
        match worlded_core::pzw::parse_pzw(&path) {
            Ok(w) => {
                self.message = format!("Opened {} ({} cells)", path.display(), w.cells.len());
                self.settings.last_project = Some(path.clone());
                self.save_settings();
                let dir = path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
                self.maps = w.cells.iter().map(|c| world_view::resolve_map(c, &dir)).collect();
                self.project = Some((path, w));
                self.cell = None;
                self.world_state = Default::default();
            }
            Err(e) => self.message = format!("Could not open {}: {e}", path.display()),
        }
    }

    fn open_cell(&mut self, i: usize) {
        let Some((_, world)) = &self.project else { return };
        let cell = &world.cells[i];
        let Some(path) = self.maps.get(i).cloned().flatten() else {
            self.message = format!("Cell {},{} has no .tmx file", cell.x, cell.y);
            return;
        };
        match document::Document::open(&path) {
            Ok(d) => {
                self.message = format!("Opened cell {},{}", cell.x, cell.y);
                self.cell = Some(cell_view::CellState::new(format!("cell {},{}", cell.x, cell.y), d));
            }
            Err(e) => self.message = format!("Could not read {}: {e}", path.display()),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.handle_keys_and_close(ctx);
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
                    if self.cell.as_ref().is_some_and(|c| c.doc.is_dirty()) {
                        self.confirm = Some(Confirm::Back);
                    } else {
                        self.cell = None;
                    }
                }
            });
            if let Some(c) = &mut self.cell {
                c.toolbar(ui);
                if let Some(why) = &c.doc.read_only {
                    ui.colored_label(egui::Color32::from_rgb(220, 120, 60), format!("Read-only: {why}"));
                }
            }
            if !self.pack_note.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(220, 170, 60), &self.pack_note);
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let hover = self.cell.as_ref().and_then(|c| c.hover).map(|(x, y)| format!("   tile {x}, {y}")).unwrap_or_default();
            let status = self.cell.as_ref().map(|c| c.status.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| self.message.clone());
            ui.label(format!("{status}{hover}"));
        });
        if let Some(c) = &mut self.cell {
            egui::SidePanel::left("layers").show(ctx, |ui| {
                ui.heading(format!("{}{}", c.title, if c.doc.is_dirty() { " *" } else { "" }));
                c.layers_panel(ui);
            });
        }
        let mut open = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(c) = &mut self.cell {
                c.show(ui, &mut self.atlas, self.packs.as_ref());
            } else if let Some((_, world)) = &self.project {
                open = world_view::show(ui, world, &self.maps, &mut self.world_state);
            } else {
                ui.label("Open a WorldEd project (.pzw) to begin.");
            }
        });
        if let Some(i) = open {
            self.open_cell(i);
        }
    }
}

impl App {
    fn handle_keys_and_close(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let dirty = self.cell.as_ref().is_some_and(|c| c.doc.is_dirty());
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }
        if let Some(c) = &mut self.cell {
            if !ctx.wants_keyboard_input() {
                let cs = Modifiers::COMMAND | Modifiers::SHIFT;
                let (save_as, save, redo_shift, redo, undo) = ctx.input_mut(|i| {
                    (
                        i.consume_key(cs, Key::S),
                        i.consume_key(Modifiers::COMMAND, Key::S),
                        i.consume_key(cs, Key::Z),
                        i.consume_key(Modifiers::COMMAND, Key::Y),
                        i.consume_key(Modifiers::COMMAND, Key::Z),
                    )
                });
                if c.doc.read_only.is_none() {
                    if save_as {
                        if let Some(p) = rfd::FileDialog::new().add_filter("TMX map", &["tmx"]).save_file() {
                            c.save_as(&p);
                        }
                    } else if save {
                        c.save();
                    } else if redo_shift || redo {
                        c.redo();
                    } else if undo {
                        c.undo();
                    }
                }
            }
            let title = format!("worlded viewer - {}{}", c.title, if c.doc.is_dirty() { " *" } else { "" });
            if title != self.last_title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.last_title = title;
            }
        }
        if let Some(what) = self.confirm {
            let name = self.cell.as_ref().map(|c| c.title.clone()).unwrap_or_default();
            let mut choice = None;
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(format!("{name} has unsaved changes."));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm = None;
                    }
                });
            });
            if let Some(save) = choice {
                let mut ok = true;
                if save {
                    if let Some(c) = &mut self.cell {
                        c.save();
                        ok = !c.doc.is_dirty();
                    }
                }
                if ok {
                    self.confirm = None;
                    match what {
                        Confirm::Back => self.cell = None,
                        Confirm::Quit => {
                            self.allow_quit = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native("worlded viewer", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(App::new()))))
}
