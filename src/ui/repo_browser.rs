//! Navegador de carpetas para buscar y abrir repositorios.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Align, Color32, Key, Layout, RichText, ScrollArea, TextEdit, Ui};

use crate::browser::{self, DirEntry};

/// Profundidad máxima de la búsqueda recursiva.
const SEARCH_DEPTH: usize = 5;
const MAX_RESULTS: usize = 1000;
const REPO_COLOR: Color32 = Color32::from_rgb(90, 180, 100);

pub enum BrowserAction {
    Open(PathBuf),
}

enum SearchEvent {
    Found(PathBuf),
    Done,
}

/// Búsqueda recursiva en marcha (o terminada) en un hilo aparte.
struct Search {
    root: PathBuf,
    rx: Receiver<SearchEvent>,
    cancel: Arc<AtomicBool>,
    results: Vec<PathBuf>,
    done: bool,
}

impl Search {
    fn start(root: PathBuf, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let thread_root = root.clone();
        std::thread::spawn(move || {
            let mut count = 0;
            browser::find_repos(&thread_root, SEARCH_DEPTH, &thread_cancel, &mut |path| {
                count += 1;
                let ok = tx.send(SearchEvent::Found(path)).is_ok();
                ctx.request_repaint();
                ok && count < MAX_RESULTS
            });
            let _ = tx.send(SearchEvent::Done);
            ctx.request_repaint();
        });
        Self {
            root,
            rx,
            cancel,
            results: Vec::new(),
            done: false,
        }
    }

    fn poll(&mut self) {
        for event in self.rx.try_iter() {
            match event {
                SearchEvent::Found(p) => self.results.push(p),
                SearchEvent::Done => self.done = true,
            }
        }
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub struct RepoBrowser {
    current: PathBuf,
    path_input: String,
    entries: Result<Vec<DirEntry>, String>,
    filter: String,
    show_hidden: bool,
    selected: Option<PathBuf>,
    search: Option<Search>,
}

impl RepoBrowser {
    pub fn new(start: PathBuf) -> Self {
        let mut browser = Self {
            current: PathBuf::new(),
            path_input: String::new(),
            entries: Ok(Vec::new()),
            filter: String::new(),
            show_hidden: false,
            selected: None,
            search: None,
        };
        browser.navigate(start);
        browser
    }

    pub fn current(&self) -> &Path {
        &self.current
    }

    fn navigate(&mut self, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        self.entries = browser::list_dirs(&path).map_err(|e| e.to_string());
        self.path_input = path.display().to_string();
        self.current = path;
        self.filter.clear();
        self.selected = None;
    }

    /// Repositorio que se abriría con el botón: el seleccionado o la carpeta actual.
    fn target(&self) -> Option<PathBuf> {
        let candidate = self.selected.as_deref().unwrap_or(&self.current);
        browser::repo_root_of(candidate)
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        recent: &[PathBuf],
        open: &[PathBuf],
    ) -> Option<BrowserAction> {
        let mut action = None;
        let mut navigate = None;
        if let Some(search) = &mut self.search {
            search.poll();
        }

        egui::TopBottomPanel::top("browser_nav").show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Inicio").on_hover_text("Carpeta personal").clicked() {
                    navigate = Some(browser::home_dir());
                }
                let parent = self.current.parent().map(Path::to_path_buf);
                if ui
                    .add_enabled(parent.is_some(), egui::Button::new("Subir"))
                    .on_hover_text("Carpeta superior")
                    .clicked()
                {
                    navigate = parent;
                }
                let resp = ui.add(
                    TextEdit::singleline(&mut self.path_input)
                        .hint_text("Ruta")
                        .desired_width(f32::INFINITY),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    navigate = Some(PathBuf::from(self.path_input.trim()));
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.filter)
                        .hint_text("Filtrar carpetas")
                        .desired_width(240.0),
                );
                ui.checkbox(&mut self.show_hidden, "Mostrar ocultas");
                ui.separator();
                let searching = self.search.as_ref().is_some_and(|s| !s.done);
                if searching {
                    if ui.button("Detener búsqueda").clicked() {
                        self.search = None;
                    }
                } else if ui
                    .button("Buscar repositorios aquí")
                    .on_hover_text(format!(
                        "Buscar repositorios git dentro de esta carpeta (hasta {SEARCH_DEPTH} niveles)"
                    ))
                    .clicked()
                {
                    self.search = Some(Search::start(self.current.clone(), ui.ctx().clone()));
                }
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("browser_actions").show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let target = self.target();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let button = ui.add_enabled(
                        target.is_some(),
                        egui::Button::new(RichText::new("Abrir repositorio").strong()),
                    );
                    if let Some(target) = &target {
                        if button.clicked() {
                            action = Some(BrowserAction::Open(target.clone()));
                        }
                        ui.label(target.display().to_string());
                    } else {
                        ui.weak("Selecciona una carpeta que sea un repositorio git.");
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("browser_side")
            .resizable(true)
            .default_width(280.0)
            .show_inside(ui, |ui| {
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    if !open.is_empty() {
                        ui.strong("Abiertos");
                        for path in open {
                            if ui.link(path.display().to_string()).clicked() {
                                action = Some(BrowserAction::Open(path.clone()));
                            }
                        }
                        ui.add_space(8.0);
                    }
                    ui.strong("Recientes");
                    if recent.is_empty() {
                        ui.weak("Ninguno todavía");
                    }
                    for path in recent {
                        let resp = ui.link(path.display().to_string());
                        if resp.clicked() {
                            action = Some(BrowserAction::Open(path.clone()));
                        }
                    }
                });
            });

        let mut close_search = false;
        let mut select = None;
        if let Some(search) = &self.search {
            egui::SidePanel::right("browser_search")
                .resizable(true)
                .default_width(360.0)
                .show_inside(ui, |ui| {
                    let mut close = false;
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Repositorios encontrados ({})",
                            search.results.len()
                        ));
                        if !search.done {
                            ui.spinner();
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            close = ui.small_button("Cerrar").clicked();
                        });
                    });
                    ui.weak(format!("en {}", search.root.display()));
                    ui.separator();
                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        for path in &search.results {
                            let label = path
                                .strip_prefix(&search.root)
                                .unwrap_or(path)
                                .display()
                                .to_string();
                            let resp = ui
                                .selectable_label(self.selected.as_ref() == Some(path), label)
                                .on_hover_text("Doble clic para abrir");
                            if resp.double_clicked() {
                                action = Some(BrowserAction::Open(path.clone()));
                            } else if resp.clicked() {
                                select = Some(path.clone());
                            }
                        }
                        if search.done && search.results.is_empty() {
                            ui.weak("No se encontró ningún repositorio.");
                        }
                    });
                    close_search = close;
                });
        }
        if close_search {
            self.search = None;
        }
        if select.is_some() {
            self.selected = select;
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(root) = browser::repo_root_of(&self.current) {
                let text = if root == self.current {
                    "Esta carpeta es un repositorio git.".to_owned()
                } else {
                    format!(
                        "Esta carpeta está dentro del repositorio {}.",
                        root.display()
                    )
                };
                ui.colored_label(REPO_COLOR, text);
                ui.separator();
            }
            match &self.entries {
                Err(e) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("No se puede leer la carpeta: {e}"),
                    );
                }
                Ok(entries) => {
                    let filter = self.filter.to_lowercase();
                    let visible: Vec<&DirEntry> = entries
                        .iter()
                        .filter(|e| self.show_hidden || !e.hidden)
                        .filter(|e| filter.is_empty() || e.name.to_lowercase().contains(&filter))
                        .collect();
                    if visible.is_empty() {
                        ui.weak("No hay subcarpetas.");
                    }
                    let row_height = ui.spacing().interact_size.y;
                    ScrollArea::vertical()
                        .id_salt(("browser_dirs", &self.current))
                        .auto_shrink(false)
                        .show_rows(ui, row_height, visible.len(), |ui, range| {
                            for entry in &visible[range] {
                                ui.horizontal(|ui| {
                                    let selected = self.selected.as_ref() == Some(&entry.path);
                                    let mut text = RichText::new(&entry.name);
                                    if entry.is_repo {
                                        text = text.strong();
                                    }
                                    let resp = ui
                                        .selectable_label(selected, text)
                                        .on_hover_text("Doble clic para entrar");
                                    if resp.double_clicked() {
                                        navigate = Some(entry.path.clone());
                                    } else if resp.clicked() {
                                        self.selected = Some(entry.path.clone());
                                    }
                                    if entry.is_repo {
                                        ui.label(
                                            RichText::new("repositorio git")
                                                .small()
                                                .color(REPO_COLOR),
                                        );
                                    }
                                });
                            }
                        });
                }
            }
        });

        if let Some(path) = navigate {
            self.navigate(path);
        }
        action
    }
}
