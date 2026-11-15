//! worlded-view: viewer and tile editor for WorldEd projects.
mod atlas;
mod cell_view;
mod document;
mod iso;
mod palette;
mod settings;
mod tools;
mod world_view;

use eframe::egui;
use settings::Settings;
use std::cell::OnceCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use worlded_compile::blends::{load_blends, Blend};
use worlded_compile::pipeline;
use worlded_compile::rules::{load_rules, Ruleset};
use worlded_compile::tmxground::Generator;
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
    palette: palette::Palette,
    message: String,
    /// A close or "back to world" waiting for an answer about unsaved changes.
    confirm: Option<Confirm>,
    allow_quit: bool,
    last_title: String,
    /// Rules/Blends for ground generation, loaded once (None = no config found).
    rules: OnceCell<Option<(Ruleset, Vec<Blend>)>>,
    /// A lot export running on another thread.
    export: Option<Export>,
}

/// A lot export running on another thread.
struct Export {
    out: PathBuf,
    progress: Arc<Mutex<(usize, usize)>>,
    result: Arc<Mutex<Option<Result<usize, String>>>>,
    started: std::time::Instant,
}

fn load_config() -> Option<(Ruleset, Vec<Blend>)> {
    let rules = pipeline::find_rules().ok()?;
    let blends = rules.with_file_name("Blends.txt");
    match (load_rules(&rules), load_blends(&blends)) {
        (Ok(r), Ok(b)) => Some((r, b)),
        _ => None,
    }
}

#[derive(Clone, PartialEq)]
enum Confirm {
    Back,
    Quit,
    /// Open this project once the unsaved cell is dealt with.
    Open(PathBuf),
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
            palette: Default::default(),
            message: String::new(),
            confirm: None,
            allow_quit: false,
            last_title: String::new(),
            rules: OnceCell::new(),
            export: None,
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
        self.palette = palette::Palette::new(self.packs.as_ref());
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
                let mut c = cell_view::CellState::new(format!("cell {},{}", cell.x, cell.y), d);
                c.cell_xy = (cell.x.max(0) as u32, cell.y.max(0) as u32);
                self.cell = Some(c);
            }
            Err(e) => self.message = format!("Could not read {}: {e}", path.display()),
        }
    }

    /// Compile the open project into lot files beside it, on another thread.
    fn start_export(&mut self) {
        let Some((pzw, _)) = &self.project else { return };
        if let Some(c) = &mut self.cell {
            c.save();
            if c.doc.is_dirty() {
                self.message = "Could not save the open cell; export cancelled.".into();
                return;
            }
        }
        let Ok(rules_path) = pipeline::find_rules() else {
            self.message = "Rules.txt/Blends.txt not found: put a config folder next to the program (or set KNOXLOTS_CONFIG).".into();
            return;
        };
        let pzw = pzw.clone();
        let out = pzw.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(std::path::Path::new(".")).join("lots");
        let progress: Arc<Mutex<(usize, usize)>> = Arc::new(Mutex::new((0, 0)));
        let result: Arc<Mutex<Option<Result<usize, String>>>> = Arc::new(Mutex::new(None));
        let (pr, rs) = (progress.clone(), result.clone());
        let out_t = out.clone();
        std::thread::spawn(move || {
            let opts = pipeline::Options { seed: 1, seams: false, workers: None, keep_source: None, rules: rules_path };
            let r = pipeline::compile_map(&pzw, &out_t, &opts, &|d, t| *pr.lock().unwrap() = (d, t)).map_err(|e| e.to_string());
            *rs.lock().unwrap() = Some(r);
        });
        self.export = Some(Export { out, progress, result, started: std::time::Instant::now() });
        self.message = "Compiling lots...".into();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.handle_keys_and_close(ctx);
        if let Some(e) = self.export.take() {
            let done = e.result.lock().unwrap().take();
            match done {
                Some(Ok(n)) => self.message = format!("{n} cells compiled into {} in {:.1} s", e.out.display(), e.started.elapsed().as_secs_f32()),
                Some(Err(err)) => self.message = format!("Export failed: {err}"),
                None => {
                    let (d, t) = *e.progress.lock().unwrap();
                    self.message = format!("Compiling lots... row {d} of {t}");
                    self.export = Some(e);
                    ctx.request_repaint();
                }
            }
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open project...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("WorldEd project", &["pzw"]).pick_file() {
                        if self.cell.as_ref().is_some_and(|c| c.doc.is_dirty()) {
                            self.confirm = Some(Confirm::Open(p));
                        } else {
                            self.open_project(p);
                        }
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
                if self.project.is_some() && ui.add_enabled(self.export.is_none(), egui::Button::new("Export lots")).clicked() {
                    self.start_export();
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
        if let Some(c) = &mut self.cell {
            if c.doc.read_only.is_none() {
                egui::SidePanel::right("palette").default_width(260.0).show(ctx, |ui| {
                    if let Some(name) = self.palette.show(ui, &mut self.atlas, self.packs.as_ref(), &c.selected_tile) {
                        c.selected_tile = name;
                    }
                });
            }
        }
        let mut open = None;
        let rules = self.rules.get_or_init(load_config);
        let gen = rules.as_ref().map(|(r, b)| Generator { rules: r, blends: b });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(c) = &mut self.cell {
                c.show(ui, &mut self.atlas, self.packs.as_ref(), gen.as_ref());
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
        if let Some(what) = self.confirm.clone() {
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
                        Confirm::Open(p) => self.open_project(p),
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
