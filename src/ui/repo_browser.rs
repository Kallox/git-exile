//! Navegador de carpetas para buscar y abrir repositorios.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Align, Color32, Key, Layout, RichText, ScrollArea, TextEdit, Ui};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::browser::{self, DirEntry};

/// Profundidad máxima de la búsqueda recursiva.
const SEARCH_DEPTH: usize = 5;
const MAX_RESULTS: usize = 1000;
const REPO_COLOR: Color32 = theme::GREEN;

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

        egui::TopBottomPanel::top("browser_nav")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 12)))
            .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Abrir repositorio")
                        .font(theme::font_semibold(16.0))
                        .color(theme::TEXT),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::Home, "Inicio", ButtonStyle::Secondary, true)
                    .on_hover_text("Carpeta personal")
                    .clicked()
                {
                    navigate = Some(browser::home_dir());
                }
                let parent = self.current.parent().map(Path::to_path_buf);
                if icons::button(ui, Icon::ArrowUp, "Subir", ButtonStyle::Secondary, parent.is_some())
                    .on_hover_text("Carpeta superior")
                    .clicked()
                {
                    navigate = parent;
                }
                let resp = ui.add(
                    TextEdit::singleline(&mut self.path_input)
                        .hint_text("Ruta")
                        .font(theme::mono(12.0))
                        .margin(egui::vec2(10.0, 8.0))
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
                        .margin(egui::vec2(10.0, 8.0))
                        .desired_width(260.0),
                );
                ui.checkbox(&mut self.show_hidden, "Mostrar ocultas");
                ui.separator();
                let searching = self.search.as_ref().is_some_and(|s| !s.done);
                if searching {
                    if icons::button(ui, Icon::Close, "Detener búsqueda", ButtonStyle::Secondary, true)
                        .clicked()
                    {
                        self.search = None;
                    }
                } else if icons::button(
                    ui,
                    Icon::Search,
                    "Buscar repositorios aquí",
                    ButtonStyle::Secondary,
                    true,
                )
                    .on_hover_text(format!(
                        "Buscar repositorios git dentro de esta carpeta (hasta {SEARCH_DEPTH} niveles)"
                    ))
                    .clicked()
                {
                    self.search = Some(Search::start(self.current.clone(), ui.ctx().clone()));
                }
            });
        });

        egui::TopBottomPanel::bottom("browser_actions")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 12)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let target = self.target();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let button = icons::button(
                            ui,
                            Icon::Folder,
                            "Abrir repositorio",
                            ButtonStyle::Primary,
                            target.is_some(),
                        );
                        if let Some(target) = &target {
                            if button.clicked() {
                                action = Some(BrowserAction::Open(target.clone()));
                            }
                            ui.label(
                                RichText::new(target.display().to_string())
                                    .font(theme::mono(12.0))
                                    .color(theme::TEXT_SECONDARY),
                            );
                        } else {
                            ui.label(
                                RichText::new("Selecciona una carpeta que sea un repositorio git.")
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    });
                });
            });

        egui::SidePanel::left("browser_side")
            .resizable(true)
            .default_width(300.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 16)))
            .show_inside(ui, |ui| {
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    if !open.is_empty() {
                        section_title(ui, "Abiertos");
                        for path in open {
                            if repo_link(ui, path).clicked() {
                                action = Some(BrowserAction::Open(path.clone()));
                            }
                        }
                        ui.add_space(12.0);
                    }
                    section_title(ui, "Recientes");
                    if recent.is_empty() {
                        ui.label(RichText::new("Ninguno todavía").color(theme::TEXT_MUTED));
                    }
                    for path in recent {
                        if repo_link(ui, path).clicked() {
                            action = Some(BrowserAction::Open(path.clone()));
                        }
                    }
                });
            });

        let mut close_search = false;
        let mut select = None;
        if let Some(search) = &self.search {
            egui::SidePanel::right("browser_search")
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 16)))
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
                            let resp = folder_row(
                                ui,
                                &label,
                                true,
                                self.selected.as_ref() == Some(path),
                                false,
                            )
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

        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 16)))
            .show_inside(ui, |ui| {
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
                            .filter(|e| {
                                filter.is_empty() || e.name.to_lowercase().contains(&filter)
                            })
                            .collect();
                        if visible.is_empty() {
                            ui.weak("No hay subcarpetas.");
                        }
                        let row_height = 34.0;
                        ScrollArea::vertical()
                            .id_salt(("browser_dirs", &self.current))
                            .auto_shrink(false)
                            .show_rows(ui, row_height, visible.len(), |ui, range| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                for entry in &visible[range] {
                                    let selected = self.selected.as_ref() == Some(&entry.path);
                                    let resp = folder_row(
                                        ui,
                                        &entry.name,
                                        entry.is_repo,
                                        selected,
                                        entry.is_repo,
                                    )
                                    .on_hover_text("Doble clic para entrar");
                                    if resp.double_clicked() {
                                        navigate = Some(entry.path.clone());
                                    } else if resp.clicked() {
                                        self.selected = Some(entry.path.clone());
                                    }
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

fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .font(theme::font_semibold(11.0))
            .color(theme::TEXT_MUTED),
    );
}

/// Fila de 34 px con icono de carpeta, nombre y ruta atenuada.
fn repo_link(ui: &mut Ui, path: &Path) -> egui::Response {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    folder_row(ui, &name, true, false, false).on_hover_text(path.display().to_string())
}

/// Fila de carpeta. `repo` resalta las que son repositorios git.
fn folder_row(ui: &mut Ui, name: &str, repo: bool, selected: bool, badge: bool) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 7.0, theme::accent_soft());
    } else if resp.hovered() {
        painter.rect_filled(rect, 7.0, theme::BG_CARD);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 10.0 + 7.0, rect.center().y),
        egui::vec2(14.0, 14.0),
    );
    icons::paint(
        painter,
        icon_rect,
        Icon::Folder,
        if repo {
            theme::ACCENT
        } else {
            theme::TEXT_MUTED
        },
        2.0,
    );
    let font = if repo {
        theme::font_semibold(13.0)
    } else {
        theme::font(13.0)
    };
    let text = painter.text(
        egui::pos2(icon_rect.right() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        font,
        if repo { theme::TEXT } else { theme::TEXT_SOFT },
    );
    if badge {
        let galley = painter.layout_no_wrap(
            "repositorio git".to_owned(),
            theme::font_semibold(11.0),
            REPO_COLOR,
        );
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(text.right() + 10.0, rect.center().y - 10.0),
            egui::vec2(galley.size().x + 14.0, 20.0),
        );
        painter.rect(
            badge_rect,
            5.0,
            theme::alpha(REPO_COLOR, 0.14),
            egui::Stroke::new(1.0, theme::alpha(REPO_COLOR, 0.4)),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            egui::pos2(
                badge_rect.left() + 7.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            REPO_COLOR,
        );
    }
    resp
}
