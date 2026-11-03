use std::path::{Path, PathBuf};

use eframe::egui;

use crate::git::{self, Repo};

const RECENT_KEY: &str = "recent_repositories";
const MAX_RECENT: usize = 10;

struct RepoState {
    repo: Repo,
    branch: Option<String>,
}

pub struct GitExileApp {
    repo: Option<RepoState>,
    recent: Vec<PathBuf>,
    error: Option<String>,
}

impl GitExileApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let recent: Vec<PathBuf> = cc
            .storage
            .and_then(|s| eframe::get_value(s, RECENT_KEY))
            .unwrap_or_default();

        let mut app = Self {
            repo: None,
            recent,
            error: None,
        };

        if let Err(e) = git::version() {
            app.error = Some(format!("No se encontró git en el PATH: {e}"));
        } else if let Some(path) = initial_path.or_else(|| app.recent.first().cloned()) {
            app.open(&path);
        }
        app
    }

    fn open(&mut self, path: &Path) {
        match Repo::open(path) {
            Ok(repo) => {
                let root = repo.root().to_path_buf();
                self.recent.retain(|p| p != &root);
                self.recent.insert(0, root);
                self.recent.truncate(MAX_RECENT);
                let branch = repo.current_branch().unwrap_or_default();
                self.repo = Some(RepoState { repo, branch });
                self.error = None;
            }
            Err(e) => self.error = Some(format!("No se pudo abrir {}: {e}", path.display())),
        }
    }

    fn pick_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title("Abrir repositorio");
        if let Some(state) = &self.repo {
            dialog = dialog.set_directory(state.repo.root());
        }
        if let Some(path) = dialog.pick_folder() {
            self.open(&path);
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("📂 Abrir…").clicked() {
                self.pick_folder();
            }

            let mut to_open = None;
            ui.menu_button("Recientes", |ui| {
                if self.recent.is_empty() {
                    ui.label("Sin repositorios recientes");
                }
                for path in &self.recent {
                    if ui.button(path.display().to_string()).clicked() {
                        to_open = Some(path.clone());
                        ui.close();
                    }
                }
            });
            if let Some(path) = to_open {
                self.open(&path);
            }

            if let Some(state) = &self.repo {
                ui.separator();
                ui.strong(state.repo.name());
                ui.label(match &state.branch {
                    Some(b) => format!("⎇ {b}"),
                    None => "HEAD separado".to_owned(),
                })
                .on_hover_text(state.repo.root().display().to_string());
            }
        });
    }

    fn error_bar(&mut self, ui: &mut egui::Ui) {
        let mut close = false;
        if let Some(error) = &self.error {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                close = ui.small_button("✖").clicked();
            });
        }
        if close {
            self.error = None;
        }
    }
}

impl eframe::App for GitExileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            self.top_bar(ui);
            self.error_bar(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.repo.is_none() {
                ui.centered_and_justified(|ui| {
                    if ui.button("Abrir un repositorio git…").clicked() {
                        self.pick_folder();
                    }
                });
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, RECENT_KEY, &self.recent);
    }
}
