use std::path::PathBuf;

use eframe::egui;

use crate::git::{self, Commit, CommitDetails, FileDiff, Graph, Repo, WorkingTreeStatus};
use crate::ui::commit_details::{self, DetailsAction};
use crate::ui::commit_list::{CommitList, ListRow};
use crate::ui::diff_view;
use crate::ui::status_panel::{self, StatusAction, WtFile};
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
    Details {
        id: String,
        result: git::Result<CommitDetails>,
    },
    Status(git::Result<WorkingTreeStatus>),
    WorktreeDiff {
        file: WtFile,
        result: git::Result<FileDiff>,
    },
}

/// Lo que la interfaz pide hacer al terminar el frame.
enum Action {
    LoadMore,
    Select(ListRow),
    SelectId(String),
    SelectWorktreeFile(WtFile),
}

struct RepoState {
    repo: Repo,
    branch: Option<String>,
    commits: Vec<Commit>,
    graph: Graph,
    log_complete: bool,
    log_loading: bool,
    list: CommitList,
    selected: Option<ListRow>,

    details: Option<CommitDetails>,
    /// Commit cuyos detalles se están cargando.
    details_loading: Option<String>,
    selected_file: Option<usize>,

    status: Option<WorkingTreeStatus>,
    wt_file: Option<WtFile>,
    wt_diff: Option<FileDiff>,
}

impl RepoState {
    fn new(repo: Repo, branch: Option<String>) -> Self {
        Self {
            repo,
            branch,
            commits: Vec::new(),
            graph: Graph::default(),
            log_complete: false,
            log_loading: false,
            list: CommitList::default(),
            selected: None,
            details: None,
            details_loading: None,
            selected_file: None,
            status: None,
            wt_file: None,
            wt_diff: None,
        }
    }

    fn has_changes(&self) -> bool {
        self.status.as_ref().is_some_and(|s| !s.is_clean())
    }
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

    fn refresh_status(&mut self) {
        let Some(state) = &self.repo else { return };
        let repo = state.repo.clone();
        self.spawn(move || MessageKind::Status(git::load_status(&repo)));
    }

    fn select(&mut self, row: ListRow) {
        let Some(state) = &mut self.repo else { return };
        match row {
            ListRow::WorkingTree => {
                state.selected = Some(row);
                let file = state
                    .wt_file
                    .clone()
                    .or_else(|| state.status.as_ref().and_then(WtFile::first));
                if let Some(file) = file {
                    self.select_worktree_file(file);
                }
            }
            ListRow::Commit(index) => {
                if state.selected == Some(row) {
                    return;
                }
                let Some(commit) = state.commits.get(index) else {
                    return;
                };
                state.selected = Some(row);
                let id = commit.id.clone();
                let repo = state.repo.clone();
                state.details_loading = Some(id.clone());
                self.spawn(move || MessageKind::Details {
                    result: git::load_commit(&repo, &id),
                    id,
                });
            }
        }
    }

    fn select_worktree_file(&mut self, file: WtFile) {
        let Some(state) = &mut self.repo else { return };
        let Some(entry) = state.status.as_ref().and_then(|s| file.find(s)).cloned() else {
            state.wt_file = None;
            state.wt_diff = None;
            return;
        };
        if state.wt_file.as_ref() != Some(&file) {
            state.wt_diff = None;
        }
        state.wt_file = Some(file.clone());
        let repo = state.repo.clone();
        self.spawn(move || MessageKind::WorktreeDiff {
            result: git::diff_entry(&repo, &entry, file.staged),
            file,
        });
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::LoadMore => self.load_log_page(),
            Action::Select(row) => self.select(row),
            Action::SelectWorktreeFile(file) => self.select_worktree_file(file),
            Action::SelectId(id) => {
                let index = self
                    .repo
                    .as_ref()
                    .and_then(|s| s.commits.iter().position(|c| c.id == id));
                if let Some(i) = index {
                    if let Some(state) = &mut self.repo {
                        state.list.scroll_to(ListRow::Commit(i));
                    }
                    self.select(ListRow::Commit(i));
                }
            }
        }
    }

    fn handle_messages(&mut self) {
        let mut select_initial = false;
        let mut reload_wt = false;
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
                    self.repo = Some(RepoState::new(repo, branch));
                    self.error = None;
                    // El worker procesa en orden: el estado llega antes que el log.
                    self.refresh_status();
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
                            select_initial |= skip == 0;
                            state.graph.extend(&page);
                            state.commits.extend(page);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            state.log_complete = true;
                            self.error = Some(format!("Error leyendo el historial: {e}"));
                        }
                    }
                }
                MessageKind::Details { id, result } => {
                    let Some(state) = &mut self.repo else {
                        continue;
                    };
                    if state.details_loading.as_ref() != Some(&id) {
                        continue;
                    }
                    state.details_loading = None;
                    match result {
                        Ok(details) => {
                            state.selected_file = (!details.files.is_empty()).then_some(0);
                            state.details = Some(details);
                        }
                        Err(e) => {
                            state.details = None;
                            self.error = Some(format!("Error leyendo el commit: {e}"));
                        }
                    }
                }
                MessageKind::Status(result) => {
                    let Some(state) = &mut self.repo else {
                        continue;
                    };
                    match result {
                        Ok(status) => {
                            state.branch = status.branch.clone();
                            state.status = Some(status);
                            reload_wt = true;
                        }
                        Err(e) => self.error = Some(format!("Error leyendo el estado: {e}")),
                    }
                }
                MessageKind::WorktreeDiff { file, result } => {
                    let Some(state) = &mut self.repo else {
                        continue;
                    };
                    if state.wt_file.as_ref() != Some(&file) {
                        continue;
                    }
                    match result {
                        Ok(diff) => state.wt_diff = Some(diff),
                        Err(e) => {
                            state.wt_diff = None;
                            self.error = Some(format!("Error leyendo el diff: {e}"));
                        }
                    }
                }
            }
        }

        if select_initial && let Some(state) = &self.repo {
            let row = if state.has_changes() {
                ListRow::WorkingTree
            } else {
                ListRow::Commit(0)
            };
            self.select(row);
        }
        if reload_wt {
            self.after_status_change();
        }
    }

    /// Mantiene coherente la vista de cambios cuando cambia el estado.
    fn after_status_change(&mut self) {
        let Some(state) = &mut self.repo else { return };
        if state.selected != Some(ListRow::WorkingTree) {
            return;
        }
        if !state.has_changes() {
            state.wt_file = None;
            state.wt_diff = None;
            if !state.commits.is_empty() {
                self.select(ListRow::Commit(0));
            }
            return;
        }
        let status = state.status.as_ref().unwrap();
        let file = state
            .wt_file
            .clone()
            .filter(|f| f.find(status).is_some())
            .or_else(|| WtFile::first(status));
        if let Some(file) = file {
            self.select_worktree_file(file);
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
            if ui.button("Abrir…").clicked() {
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
                    Some(b) => format!("rama: {b}"),
                    None => "HEAD separado".to_owned(),
                })
                .on_hover_text(state.repo.root().display().to_string());
                if let Some(status) = &state.status
                    && let Some(upstream) = &status.upstream
                {
                    ui.weak(format!(
                        "{upstream} (+{} / -{})",
                        status.ahead, status.behind
                    ));
                }
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

fn details_panel(ui: &mut egui::Ui, state: &mut RepoState, actions: &mut Vec<Action>) {
    let Some(details) = &state.details else {
        ui.centered_and_justified(|ui| {
            if state.details_loading.is_some() {
                ui.spinner();
            } else {
                ui.weak("Selecciona un commit para ver sus cambios.");
            }
        });
        return;
    };
    egui::SidePanel::left("commit_info")
        .resizable(true)
        .default_width(ui.available_width() * 0.4)
        .min_width(200.0)
        .show_inside(ui, |ui| {
            if let Some(DetailsAction::SelectCommit(id)) =
                commit_details::show(ui, details, &mut state.selected_file)
            {
                actions.push(Action::SelectId(id));
            }
        });
    egui::CentralPanel::default().show_inside(ui, |ui| {
        match state.selected_file.and_then(|i| details.files.get(i)) {
            Some(file) => diff_view::show(ui, file),
            None => diff_view::placeholder(ui, "Selecciona un archivo para ver el diff."),
        }
    });
}

fn worktree_panel(ui: &mut egui::Ui, state: &mut RepoState, actions: &mut Vec<Action>) {
    let Some(status) = &state.status else {
        ui.spinner();
        return;
    };
    egui::SidePanel::left("worktree_files")
        .resizable(true)
        .default_width(ui.available_width() * 0.4)
        .min_width(200.0)
        .show_inside(ui, |ui| {
            if let Some(StatusAction::Select(file)) =
                status_panel::show(ui, status, state.wt_file.as_ref())
            {
                actions.push(Action::SelectWorktreeFile(file));
            }
        });
    egui::CentralPanel::default().show_inside(ui, |ui| match &state.wt_diff {
        Some(diff) => diff_view::show(ui, diff),
        None if state.wt_file.is_some() => {
            ui.centered_and_justified(|ui| ui.spinner());
        }
        None => diff_view::placeholder(ui, "Selecciona un archivo para ver el diff."),
    });
}

fn repo_view(ui: &mut egui::Ui, state: &mut RepoState, actions: &mut Vec<Action>) {
    // Si los cambios desaparecen, la fila deja de existir.
    if state.selected == Some(ListRow::WorkingTree) && !state.has_changes() {
        state.selected = None;
    }
    if state.selected.is_some() {
        egui::TopBottomPanel::bottom("details_panel")
            .resizable(true)
            .default_height(ui.available_height() * 0.55)
            .min_height(120.0)
            .show_inside(ui, |ui| match state.selected {
                Some(ListRow::WorkingTree) => worktree_panel(ui, state, actions),
                _ => details_panel(ui, state, actions),
            });
    }

    egui::CentralPanel::default().show_inside(ui, |ui| {
        let changes = state
            .status
            .as_ref()
            .filter(|s| !s.is_clean())
            .map(|s| s.change_count());
        if state.commits.is_empty() && changes.is_none() && !state.log_loading {
            ui.weak("Este repositorio todavía no tiene commits.");
        }
        let resp = state
            .list
            .show(ui, &state.commits, &state.graph, state.selected, changes);
        if let Some(row) = resp.selected {
            actions.push(Action::Select(row));
        }
        if resp.wants_more {
            actions.push(Action::LoadMore);
        }
    });
}

impl eframe::App for GitExileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            self.top_bar(ui);
            self.error_bar(ui);
        });

        let mut actions = Vec::new();
        let mut pick = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| match &mut self.repo {
                None => {
                    ui.centered_and_justified(|ui| {
                        pick = ui.button("Abrir un repositorio git…").clicked();
                    });
                }
                Some(state) => repo_view(ui, state, &mut actions),
            });
        if pick {
            self.pick_folder();
        }
        for action in actions {
            self.apply(action);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, RECENT_KEY, &self.recent);
    }
}
