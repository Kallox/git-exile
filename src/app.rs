use std::path::{Path, PathBuf};

use eframe::egui;

use crate::git::{self, Commit, Repo};
use crate::worker::Worker;

const RECENT_KEY: &str = "recent_repositories";
const MAX_RECENT: usize = 10;
const LOG_PAGE: usize = 500;

/// Resultado de un trabajo en segundo plano.
/// `epoch` identifica el repo abierto cuando se lanzó; si cambió, se descarta.
struct Message {
    epoch: u64,
    kind: MessageKind,
}

enum MessageKind {
    Opened(git::Result<(Repo, Option<String>)>, PathBuf),
    Log {
        skip: usize,
        result: git::Result<Vec<Commit>>,
    },
}

struct RepoState {
    repo: Repo,
    branch: Option<String>,
    commits: Vec<Commit>,
    log_complete: bool,
    log_loading: bool,
}

pub struct GitExileApp {
    worker: Worker<Message>,
    epoch: u64,
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
            worker: Worker::new(cc.egui_ctx.clone()),
            epoch: 0,
            repo: None,
            recent,
            error: None,
        };

        if let Err(e) = git::version() {
            app.error = Some(format!("No se encontró git en el PATH: {e}"));
        } else if let Some(path) = initial_path.or_else(|| app.recent.first().cloned()) {
            app.open(path);
        }
        app
    }

    fn spawn(&mut self, job: impl FnOnce() -> MessageKind + Send + 'static) {
        let epoch = self.epoch;
        self.worker.spawn(move || Message { epoch, kind: job() });
    }

    fn open(&mut self, path: PathBuf) {
        self.epoch += 1;
        self.spawn(move || {
            let result = Repo::open(&path).and_then(|repo| {
                let branch = repo.current_branch()?;
                Ok((repo, branch))
            });
            MessageKind::Opened(result, path)
        });
    }

    fn load_log_page(&mut self) {
        let Some(state) = &mut self.repo else { return };
        if state.log_loading || state.log_complete {
            return;
        }
        state.log_loading = true;
        let repo = state.repo.clone();
        let skip = state.commits.len();
        self.spawn(move || MessageKind::Log {
            skip,
            result: git::load_log(&repo, skip, LOG_PAGE),
        });
    }

    fn handle_messages(&mut self) {
        for msg in self.worker.poll() {
            if msg.epoch != self.epoch {
                continue;
            }
            match msg.kind {
                MessageKind::Opened(Ok((repo, branch)), _) => {
                    let root = repo.root().to_path_buf();
                    self.recent.retain(|p| p != &root);
                    self.recent.insert(0, root);
                    self.recent.truncate(MAX_RECENT);
                    self.repo = Some(RepoState {
                        repo,
                        branch,
                        commits: Vec::new(),
                        log_complete: false,
                        log_loading: false,
                    });
                    self.error = None;
                    self.load_log_page();
                }
                MessageKind::Opened(Err(e), path) => {
                    self.error = Some(format!("No se pudo abrir {}: {e}", path.display()));
                }
                MessageKind::Log { skip, result } => {
                    let Some(state) = &mut self.repo else {
                        continue;
                    };
                    state.log_loading = false;
                    match result {
                        Ok(page) if skip == state.commits.len() => {
                            state.log_complete = page.len() < LOG_PAGE;
                            state.commits.extend(page);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            state.log_complete = true;
                            self.error = Some(format!("Error leyendo el historial: {e}"));
                        }
                    }
                }
            }
        }
    }

    fn pick_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title("Abrir repositorio");
        if let Some(state) = &self.repo {
            dialog = dialog.set_directory(state.repo.root());
        }
        if let Some(path) = dialog.pick_folder() {
            self.open(path);
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
                self.open(path);
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

            if self.worker.is_busy() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spinner();
                });
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
        self.handle_messages();

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            self.top_bar(ui);
            self.error_bar(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| match &self.repo {
            None => {
                ui.centered_and_justified(|ui| {
                    if ui.button("Abrir un repositorio git…").clicked() {
                        self.pick_folder();
                    }
                });
            }
            Some(state) => {
                ui.label(format!("{} commits cargados", state.commits.len()));
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, RECENT_KEY, &self.recent);
    }
}
