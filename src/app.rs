use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers};

use crate::browser;
use crate::git::{self, Repo};
use crate::repo_tab::{Jobs, RepoMessage, RepoTab};
use crate::ui::repo_browser::{BrowserAction, RepoBrowser};
use crate::worker::Worker;

const RECENT_KEY: &str = "recent_repositories";
const OPEN_TABS_KEY: &str = "open_tabs";
const ACTIVE_TAB_KEY: &str = "active_tab";
const MAX_RECENT: usize = 10;

/// Resultado de un trabajo en segundo plano, etiquetado con su pestaña.
/// Si la pestaña ya no existe, se descarta.
pub struct Message {
    pub tab: u64,
    pub kind: MessageKind,
}

pub enum MessageKind {
    Opened(git::Result<(Repo, Option<String>)>),
    Repo(RepoMessage),
}

enum TabContent {
    /// Buscador de repositorios.
    Browser(Box<RepoBrowser>),
    /// Abriendo el repositorio de esta ruta.
    Loading(PathBuf),
    Repo(Box<RepoTab>),
}

struct Tab {
    id: u64,
    content: TabContent,
}

impl Tab {
    fn title(&self) -> String {
        match &self.content {
            TabContent::Browser(_) => "Abrir repositorio".to_owned(),
            TabContent::Loading(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            TabContent::Repo(repo) => repo.name(),
        }
    }

    fn path(&self) -> &Path {
        match &self.content {
            TabContent::Browser(b) => b.current(),
            TabContent::Loading(path) => path,
            TabContent::Repo(repo) => repo.repo.root(),
        }
    }
}

pub struct GitExileApp {
    worker: Worker<Message>,
    /// Si la ventana tenía el foco en el frame anterior.
    focused: bool,
    next_id: u64,
    tabs: Vec<Tab>,
    active: usize,
    recent: Vec<PathBuf>,
    error: Option<String>,
}

impl GitExileApp {
    pub fn new(cc: &eframe::CreationContext<'_>, paths: Vec<PathBuf>) -> Self {
        let storage = cc.storage;
        let recent: Vec<PathBuf> = storage
            .and_then(|s| eframe::get_value(s, RECENT_KEY))
            .unwrap_or_default();
        let open_tabs: Vec<PathBuf> = storage
            .and_then(|s| eframe::get_value(s, OPEN_TABS_KEY))
            .unwrap_or_default();
        let active: usize = storage
            .and_then(|s| eframe::get_value(s, ACTIVE_TAB_KEY))
            .unwrap_or_default();

        let mut app = Self {
            worker: Worker::new(cc.egui_ctx.clone()),
            focused: true,
            next_id: 0,
            tabs: Vec::new(),
            active: 0,
            recent,
            error: None,
        };

        if let Err(e) = git::version() {
            app.error = Some(format!("No se encontró git en el PATH: {e}"));
            return app;
        }
        for path in open_tabs {
            app.open(path);
        }
        app.active = active.min(app.tabs.len().saturating_sub(1));
        for path in paths {
            app.open(path);
        }
        app
    }

    fn jobs(&mut self, tab: u64) -> Jobs<'_> {
        Jobs {
            worker: &mut self.worker,
            tab,
        }
    }

    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn repo_tab_index(&self, path: &Path) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| !matches!(t.content, TabContent::Browser(_)) && same_path(t.path(), path))
    }

    /// Abre `path` en una pestaña nueva, o activa la que ya lo tenga abierto.
    fn open(&mut self, path: PathBuf) {
        if let Some(i) = self.repo_tab_index(&path) {
            self.activate(i);
            return;
        }
        let id = self.new_id();
        self.tabs.push(Tab {
            id,
            content: TabContent::Loading(path.clone()),
        });
        self.active = self.tabs.len() - 1;
        self.spawn_open(id, path);
    }

    /// Abre `path` reemplazando la pestaña `index` (el buscador desde el que se abrió).
    fn open_in(&mut self, index: usize, path: PathBuf) {
        if let Some(existing) = self.repo_tab_index(&path) {
            self.close(index);
            let existing = self.repo_tab_index(&path).unwrap_or(existing);
            self.activate(existing);
            self.active = existing;
            return;
        }
        let id = self.tabs[index].id;
        self.tabs[index].content = TabContent::Loading(path.clone());
        self.spawn_open(id, path);
    }

    fn spawn_open(&mut self, tab: u64, path: PathBuf) {
        self.worker.spawn(move || Message {
            tab,
            kind: MessageKind::Opened(Repo::open(&path).and_then(|repo| {
                let branch = repo.current_branch()?;
                Ok((repo, branch))
            })),
        });
    }

    /// Abre una pestaña con el buscador de repositorios.
    fn new_browser_tab(&mut self) {
        // Empezar junto al repositorio activo, o en la carpeta personal.
        let start = self
            .tabs
            .get(self.active)
            .filter(|t| matches!(t.content, TabContent::Repo(_)))
            .and_then(|t| t.path().parent().map(Path::to_path_buf))
            .unwrap_or_else(browser::home_dir);
        let id = self.new_id();
        self.tabs.push(Tab {
            id,
            content: TabContent::Browser(Box::new(RepoBrowser::new(start))),
        });
        self.active = self.tabs.len() - 1;
    }

    fn activate(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.active = index;
        // Puede haber cambios desde la última vez que se miró.
        self.refresh_active();
    }

    fn close(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
    }

    fn refresh_active(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let TabContent::Repo(repo) = &mut tab.content {
            let mut jobs = Jobs {
                worker: &mut self.worker,
                tab: tab.id,
            };
            repo.refresh_status(&mut jobs);
        }
    }

    fn handle_messages(&mut self) {
        for msg in self.worker.poll() {
            let Some(index) = self.tabs.iter().position(|t| t.id == msg.tab) else {
                continue;
            };
            match msg.kind {
                MessageKind::Opened(Ok((repo, branch))) => {
                    let root = repo.root().to_path_buf();
                    self.recent.retain(|p| p != &root);
                    self.recent.insert(0, root.clone());
                    self.recent.truncate(MAX_RECENT);

                    // Se abrió desde una subcarpeta de un repo que ya estaba abierto.
                    let existing = self.tabs.iter().position(|t| {
                        t.id != msg.tab
                            && matches!(&t.content, TabContent::Repo(r) if same_path(r.repo.root(), &root))
                    });
                    if let Some(existing) = existing {
                        let was_active = self.active == index;
                        self.close(index);
                        if was_active {
                            let existing = self
                                .tabs
                                .iter()
                                .position(|t| same_path(t.path(), &root))
                                .unwrap_or(existing);
                            self.activate(existing);
                        }
                        continue;
                    }
                    let mut jobs = self.jobs(msg.tab);
                    let repo = RepoTab::new(repo, branch, &mut jobs);
                    self.tabs[index].content = TabContent::Repo(Box::new(repo));
                }
                MessageKind::Opened(Err(e)) => {
                    let path = self.tabs[index].path().to_path_buf();
                    self.error = Some(format!("No se pudo abrir {}: {e}", path.display()));
                    // Volver al buscador en esa carpeta para poder elegir otra.
                    let start = if path.is_dir() {
                        path
                    } else {
                        browser::home_dir()
                    };
                    self.tabs[index].content =
                        TabContent::Browser(Box::new(RepoBrowser::new(start)));
                }
                MessageKind::Repo(repo_msg) => {
                    let tab = &mut self.tabs[index];
                    if let TabContent::Repo(repo) = &mut tab.content {
                        let mut jobs = Jobs {
                            worker: &mut self.worker,
                            tab: tab.id,
                        };
                        repo.handle(repo_msg, &mut jobs);
                    }
                }
            }
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::T)) {
            self.new_browser_tab();
        }
        let (close, next, prev) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::W),
                i.consume_key(Modifiers::COMMAND, Key::Tab),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab),
            )
        });
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        if close {
            self.close(self.active);
        } else if next {
            self.activate((self.active + 1) % n);
        } else if prev {
            self.activate((self.active + n - 1) % n);
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        let mut open = None;
        let mut new_tab = false;

        ui.horizontal(|ui| {
            egui::ScrollArea::horizontal()
                .max_width(ui.available_width() - 220.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let active = i == self.active;
                            let frame = egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(6, 2))
                                .corner_radius(4)
                                .fill(if active {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.visuals().faint_bg_color
                                });
                            frame.show(ui, |ui| {
                                let mut title = egui::RichText::new(tab.title());
                                if active {
                                    title = title.color(ui.visuals().selection.stroke.color);
                                }
                                if matches!(tab.content, TabContent::Loading(_)) {
                                    ui.spinner();
                                }
                                let resp = ui
                                    .add(egui::Label::new(title).sense(egui::Sense::click()))
                                    .on_hover_text(tab.path().display().to_string());
                                if resp.clicked() {
                                    activate = Some(i);
                                }
                                if resp.middle_clicked() {
                                    close = Some(i);
                                }
                                if ui
                                    .small_button("x")
                                    .on_hover_text("Cerrar pestaña (Ctrl+W)")
                                    .clicked()
                                {
                                    close = Some(i);
                                }
                            });
                        }
                        if ui
                            .button("+")
                            .on_hover_text("Abrir otro repositorio (Ctrl+T)")
                            .clicked()
                        {
                            new_tab = true;
                        }
                    });
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("Recientes", |ui| {
                    if self.recent.is_empty() {
                        ui.label("Sin repositorios recientes");
                    }
                    for path in &self.recent {
                        if ui.button(path.display().to_string()).clicked() {
                            open = Some(path.clone());
                            ui.close();
                        }
                    }
                });
                if self.worker.is_busy() {
                    ui.spinner();
                }
            });
        });

        if let Some(i) = close {
            self.close(i);
        } else if let Some(i) = activate {
            self.activate(i);
        }
        if let Some(path) = open {
            self.open(path);
        }
        if new_tab {
            self.new_browser_tab();
        }
    }

    fn error_bar(&mut self, ui: &mut egui::Ui) {
        let mut close = false;
        if let Some(error) = &self.error {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                close = ui.small_button("Cerrar").clicked();
            });
        }
        if close {
            self.error = None;
        }
    }
}

/// Compara rutas resolviendo enlaces simbólicos cuando es posible.
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

impl eframe::App for GitExileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();
        self.keyboard(ctx);
        // Sin pestañas se muestra el buscador.
        if self.tabs.is_empty() {
            self.new_browser_tab();
        }

        // Refrescar con F5 o al volver a la ventana (puede haber cambios hechos fuera).
        let focused = ctx.input(|i| i.focused);
        let regained_focus = focused && !self.focused;
        self.focused = focused;
        if regained_focus || ctx.input(|i| i.key_pressed(Key::F5)) {
            self.refresh_active();
        }

        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            self.tab_bar(ui);
            self.error_bar(ui);
        });

        let mut browser_action = None;
        let recent = &self.recent;
        let open: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter(|t| matches!(t.content, TabContent::Repo(_)))
            .map(|t| t.path().to_path_buf())
            .collect();
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return;
                };
                match &mut tab.content {
                    TabContent::Browser(b) => {
                        browser_action = ui.push_id(tab.id, |ui| b.show(ui, recent, &open)).inner;
                    }
                    TabContent::Loading(path) => {
                        ui.centered_and_justified(|ui| {
                            ui.label(format!("Abriendo {}…", path.display()));
                        });
                    }
                    TabContent::Repo(repo) => {
                        let mut jobs = Jobs {
                            worker: &mut self.worker,
                            tab: tab.id,
                        };
                        // Cada pestaña guarda su propio scroll y estado de widgets.
                        ui.push_id(tab.id, |ui| repo.ui(ui, &mut jobs));
                    }
                }
            });
        if let Some(BrowserAction::Open(path)) = browser_action {
            self.open_in(self.active, path);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Los buscadores no se guardan: solo los repositorios.
        let repos: Vec<(usize, PathBuf)> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.content, TabContent::Browser(_)))
            .map(|(i, t)| (i, t.path().to_path_buf()))
            .collect();
        let active = repos
            .iter()
            .position(|(i, _)| *i == self.active)
            .unwrap_or(0);
        let open: Vec<PathBuf> = repos.into_iter().map(|(_, p)| p).collect();
        eframe::set_value(storage, RECENT_KEY, &self.recent);
        eframe::set_value(storage, OPEN_TABS_KEY, &open);
        eframe::set_value(storage, ACTIVE_TAB_KEY, &active);
    }
}
