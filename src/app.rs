use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers};

use crate::browser;
use crate::git::{self, NewRepo, Repo};
use crate::repo_tab::{Jobs, RepoMessage, RepoTab};
use crate::ui::icons::{self, ButtonStyle, Icon};
use crate::ui::repo_browser::{BrowserAction, RepoBrowser};
use crate::ui::theme;
use crate::worker::Worker;

const RECENT_KEY: &str = "recent_repositories";
const OPEN_TABS_KEY: &str = "open_tabs";
const ACTIVE_TAB_KEY: &str = "active_tab";
const THEME_KEY: &str = "theme";
const MAX_RECENT: usize = 10;

/// Resultado de un trabajo en segundo plano, etiquetado con su pestaña.
/// Si la pestaña ya no existe, se descarta.
pub struct Message {
    pub tab: u64,
    pub kind: MessageKind,
}

/// Repositorio listo para mostrarse, con su rama actual.
type OpenedRepo = (Repo, Option<String>);

pub enum MessageKind {
    Opened(git::Result<OpenedRepo>),
    /// Terminó la creación de la pestaña `TabContent::Creating`.
    Created(git::Result<OpenedRepo>),
    Repo(RepoMessage),
}

enum TabContent {
    /// Buscador de repositorios.
    Browser(Box<RepoBrowser>),
    /// Abriendo el repositorio de esta ruta.
    Loading(PathBuf),
    /// Creando o clonando un repositorio desde el buscador en la carpeta `start`.
    Creating {
        new: NewRepo,
        start: PathBuf,
    },
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
            TabContent::Loading(_) | TabContent::Creating { .. } => {
                let path = self.path();
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            }
            TabContent::Repo(repo) => repo.name(),
        }
    }

    fn path(&self) -> &Path {
        match &self.content {
            TabContent::Browser(b) => b.current(),
            TabContent::Loading(path) => path,
            TabContent::Creating { new, .. } => new.path(),
            TabContent::Repo(repo) => repo.repo.root(),
        }
    }
}

pub struct GitExileApp {
    worker: Worker<Message>,
    /// Hilo aparte para fetch, pull y push.
    network: Worker<Message>,
    /// Hilo aparte para clonar: tarda mucho y no debe retrasar los fetch de las pestañas.
    clones: Worker<Message>,
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
        theme::apply(&cc.egui_ctx);
        let saved_theme = storage
            .and_then(|s| eframe::get_value::<String>(s, THEME_KEY))
            .and_then(|id| theme::Theme::from_id(&id));
        if let Some(saved) = saved_theme {
            theme::set(&cc.egui_ctx, saved);
        }
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
            network: Worker::new(cc.egui_ctx.clone()),
            clones: Worker::new(cc.egui_ctx.clone()),
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
            network: &mut self.network,
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

    /// Crea `new` y lo abre reemplazando la pestaña `index` (el buscador desde el que se pidió).
    fn create_in(&mut self, index: usize, new: NewRepo) {
        let tab = &mut self.tabs[index];
        let id = tab.id;
        let start = tab.path().to_path_buf();
        tab.content = TabContent::Creating {
            new: new.clone(),
            start,
        };
        let worker = match new {
            NewRepo::Init { .. } => &mut self.worker,
            NewRepo::Clone { .. } => &mut self.clones,
        };
        worker.spawn(move || Message {
            tab: id,
            kind: MessageKind::Created(new.create().and_then(|repo| {
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
                network: &mut self.network,
                tab: tab.id,
            };
            repo.refresh_status(&mut jobs);
        }
    }

    fn handle_messages(&mut self) {
        let mut done = self.worker.poll();
        done.extend(self.network.poll());
        done.extend(self.clones.poll());
        for msg in done {
            let Some(index) = self.tabs.iter().position(|t| t.id == msg.tab) else {
                continue;
            };
            match msg.kind {
                MessageKind::Opened(Ok((repo, branch)))
                | MessageKind::Created(Ok((repo, branch))) => {
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
                MessageKind::Created(Err(e)) => {
                    if let TabContent::Creating { new, start } = &self.tabs[index].content {
                        let browser =
                            RepoBrowser::with_failed(start.clone(), new.clone(), e.to_string());
                        self.tabs[index].content = TabContent::Browser(Box::new(browser));
                    }
                }
                MessageKind::Repo(repo_msg) => {
                    let tab = &mut self.tabs[index];
                    if let TabContent::Repo(repo) = &mut tab.content {
                        let mut jobs = Jobs {
                            worker: &mut self.worker,
                            network: &mut self.network,
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

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            egui::ScrollArea::horizontal()
                .max_width(ui.available_width() - 300.0)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let action = tab_widget(ui, tab, i == self.active);
                            match action {
                                Some(TabWidgetAction::Activate) => activate = Some(i),
                                Some(TabWidgetAction::Close) => close = Some(i),
                                None => {}
                            }
                        }
                        if icons::icon_button(
                            ui,
                            Icon::Plus,
                            32.0,
                            14.0,
                            theme::colors().text_muted,
                        )
                        .on_hover_text("Abrir otro repositorio (Ctrl+T)")
                        .clicked()
                        {
                            new_tab = true;
                        }
                    });
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                options_menu(ui);
                let recent = icons::button(ui, Icon::Clock, "Recientes", ButtonStyle::Ghost, true);
                egui::Popup::menu(&recent).show(|ui| {
                    ui.set_min_width(320.0);
                    if self.recent.is_empty() {
                        ui.weak("Sin repositorios recientes");
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

    fn error_bar(&mut self, ctx: &egui::Context) {
        let Some(error) = &self.error else { return };
        let mut close = false;
        egui::TopBottomPanel::top("error_bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::alpha(theme::colors().red, 0.10))
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(theme::colors().red, error);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = icons::icon_button(
                            ui,
                            Icon::Close,
                            24.0,
                            12.0,
                            theme::colors().text_muted,
                        )
                        .on_hover_text("Cerrar")
                        .clicked();
                    });
                });
            });
        if close {
            self.error = None;
        }
    }
}

/// Botón de opciones con su desplegable.
fn options_menu(ui: &mut egui::Ui) {
    let button = icons::button(ui, Icon::Sliders, "Opciones", ButtonStyle::Ghost, true);
    // Sigue abierto al elegir, para poder comparar los temas.
    egui::Popup::menu(&button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(220.0);
            ui.label(
                egui::RichText::new("TEMA")
                    .font(theme::font_semibold(11.0))
                    .color(theme::colors().text_muted),
            );
            for option in theme::Theme::ALL {
                if theme_row(ui, option).clicked() {
                    theme::set(ui.ctx(), option);
                }
            }
        });
}

/// Fila del selector de tema: muestra de colores, nombre y marca del activo.
fn theme_row(ui: &mut egui::Ui, option: theme::Theme) -> egui::Response {
    let active = option == theme::current();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    let painter = ui.painter();
    let c = theme::colors();
    if active {
        painter.rect_filled(rect, 6.0, theme::accent_soft());
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, c.bg_active);
    }
    let y = rect.center().y;
    let swatch = option.palette();
    painter.circle(
        egui::pos2(rect.left() + 16.0, y),
        7.0,
        swatch.bg,
        egui::Stroke::new(1.0, c.border_badge),
    );
    painter.circle_filled(egui::pos2(rect.left() + 16.0, y), 3.5, swatch.accent);
    painter.text(
        egui::pos2(rect.left() + 32.0, y),
        egui::Align2::LEFT_CENTER,
        option.name(),
        theme::font(13.0),
        c.text,
    );
    if active {
        icons::paint(
            painter,
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 14.0, y),
                egui::vec2(12.0, 12.0),
            ),
            Icon::Check,
            c.accent,
            2.2,
        );
    }
    resp
}

enum TabWidgetAction {
    Activate,
    Close,
}

/// Una pestaña: icono, título y botón de cerrar.
fn tab_widget(ui: &mut egui::Ui, tab: &Tab, active: bool) -> Option<TabWidgetAction> {
    let title = tab.title();
    let font = theme::font(13.0);
    let galley = ui.painter().layout_no_wrap(
        title,
        font,
        if active {
            theme::colors().text
        } else {
            theme::colors().text_secondary
        },
    );
    let width = 12.0 + 14.0 + 8.0 + galley.size().x + 8.0 + 24.0 + 6.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click());
    let resp = resp.on_hover_text(tab.path().display().to_string());
    let painter = ui.painter();
    if active {
        painter.rect(
            rect,
            8.0,
            theme::colors().bg_button,
            egui::Stroke::new(1.0, theme::colors().border_strong),
            egui::StrokeKind::Inside,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, 8.0, theme::colors().bg_input);
    }

    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 12.0 + 7.0, rect.center().y),
        egui::vec2(14.0, 14.0),
    );
    match &tab.content {
        TabContent::Loading(_) | TabContent::Creating { .. } => {
            egui::Spinner::new()
                .size(12.0)
                .color(theme::colors().accent)
                .paint_at(ui, icon_rect);
        }
        TabContent::Browser(_) => {
            icons::paint(
                painter,
                icon_rect,
                Icon::Search,
                theme::colors().accent,
                2.0,
            );
        }
        TabContent::Repo(_) => {
            let color = if active {
                theme::colors().accent
            } else {
                theme::colors().text_muted
            };
            icons::paint(painter, icon_rect, Icon::Folder, color, 2.0);
        }
    }
    let text_pos = egui::pos2(
        icon_rect.right() + 8.0,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, theme::colors().text);

    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 6.0 - 12.0, rect.center().y),
        egui::vec2(24.0, 24.0),
    );
    let close = ui
        .interact(close_rect, resp.id.with("close"), egui::Sense::click())
        .on_hover_text("Cerrar pestaña (Ctrl+W)");
    if close.hovered() {
        ui.painter()
            .rect_filled(close_rect, 6.0, theme::colors().bg_active);
    }
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(close_rect.center(), egui::vec2(12.0, 12.0)),
        Icon::Close,
        if close.hovered() {
            theme::colors().text
        } else {
            theme::colors().text_muted
        },
        2.2,
    );

    if close.clicked() || resp.middle_clicked() {
        Some(TabWidgetAction::Close)
    } else if resp.clicked() {
        Some(TabWidgetAction::Activate)
    } else {
        None
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

        egui::TopBottomPanel::top("tab_bar")
            .exact_height(44.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::colors().bg_tabs)
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| self.tab_bar(ui));
        self.error_bar(ctx);

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
                    TabContent::Creating { new, .. } => {
                        let path = new.path().display();
                        let text = match new {
                            NewRepo::Init { .. } => format!("Creando repositorio en {path}…"),
                            NewRepo::Clone { .. } => format!(
                                "Clonando en {path}…\nPuede tardar; mientras tanto puedes usar \
                                 otras pestañas. Si cierras esta, la descarga sigue y no se abrirá."
                            ),
                        };
                        ui.centered_and_justified(|ui| ui.label(text));
                    }
                    TabContent::Repo(repo) => {
                        let mut jobs = Jobs {
                            worker: &mut self.worker,
                            network: &mut self.network,
                            tab: tab.id,
                        };
                        // Cada pestaña guarda su propio scroll y estado de widgets.
                        ui.push_id(tab.id, |ui| repo.ui(ui, &mut jobs));
                    }
                }
            });
        match browser_action {
            Some(BrowserAction::Open(path)) => self.open_in(self.active, path),
            Some(BrowserAction::Create(new)) => self.create_in(self.active, new),
            None => {}
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Los buscadores no se guardan: solo los repositorios. Tampoco lo que aún se
        // está creando, que al volver a abrir podría no existir.
        let repos: Vec<(usize, PathBuf)> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !matches!(
                    t.content,
                    TabContent::Browser(_) | TabContent::Creating { .. }
                )
            })
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
        eframe::set_value(storage, THEME_KEY, &theme::current().id());
    }
}
