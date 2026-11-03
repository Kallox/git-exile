//! Una pestaña con un repositorio abierto: historial, detalles y cambios.

use eframe::egui;

use crate::app::{Message, MessageKind};
use crate::git::{self, Commit, CommitDetails, FileDiff, Graph, Repo, WorkingTreeStatus};
use crate::ui::commit_details::{self, DetailsAction};
use crate::ui::commit_editor::{CommitEditor, EditorAction};
use crate::ui::commit_list::{CommitList, ListRow};
use crate::ui::diff_view;
use crate::ui::status_panel::{self, StatusAction, WtFile};
use crate::worker::Worker;

const LOG_PAGE: usize = 500;

/// Resultado de un trabajo en segundo plano de una pestaña.
pub enum RepoMessage {
    Log {
        skip: usize,
        result: git::Result<Vec<Commit>>,
    },
    Details {
        id: String,
        result: git::Result<CommitDetails>,
    },
    /// Estado del working tree y huella de las referencias.
    Status(git::Result<(WorkingTreeStatus, String)>),
    WorktreeDiff {
        file: WtFile,
        result: git::Result<FileDiff>,
    },
    /// Resultado de una operación que modifica el repositorio.
    Done(git::Result<()>),
    Committed(git::Result<String>),
    LastMessage(git::Result<String>),
    Ident(git::Result<String>),
}

/// Lanza trabajos en el worker etiquetados con la pestaña que los pidió.
pub struct Jobs<'a> {
    pub worker: &'a mut Worker<Message>,
    pub tab: u64,
}

impl Jobs<'_> {
    fn spawn(&mut self, job: impl FnOnce() -> RepoMessage + Send + 'static) {
        let tab = self.tab;
        self.worker.spawn(move || Message {
            tab,
            kind: MessageKind::Repo(job()),
        });
    }
}

/// Lo que la interfaz pide hacer al terminar el frame.
enum Action {
    LoadMore,
    Select(ListRow),
    SelectId(String),
    SelectWorktreeFile(WtFile),
    Stage(Vec<String>),
    Unstage(Vec<String>),
    StageAll,
    UnstageAll,
    Commit { message: String, amend: bool },
    LoadLastMessage,
    Refresh,
}

pub struct RepoTab {
    pub repo: Repo,
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
    editor: CommitEditor,
    /// Identidad con la que se harán los commits (`git var GIT_AUTHOR_IDENT`).
    author: Option<String>,
    /// Huella de ramas/tags/HEAD de la última lectura del estado.
    refs: Option<String>,
    /// Commit a volver a seleccionar cuando se recargue el historial.
    reselect: Option<String>,
    pub error: Option<String>,
}

impl RepoTab {
    /// Crea la pestaña y lanza la carga inicial.
    pub fn new(repo: Repo, branch: Option<String>, jobs: &mut Jobs) -> Self {
        let mut tab = Self {
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
            editor: CommitEditor::default(),
            author: None,
            refs: None,
            reselect: None,
            error: None,
        };
        // El worker procesa en orden: el estado llega antes que el log.
        tab.refresh_status(jobs);
        tab.load_log_page(jobs);
        let repo = tab.repo.clone();
        jobs.spawn(move || RepoMessage::Ident(git::author_ident(&repo)));
        tab
    }

    pub fn name(&self) -> String {
        self.repo.name()
    }

    fn has_changes(&self) -> bool {
        self.status.as_ref().is_some_and(|s| !s.is_clean())
    }

    fn load_log_page(&mut self, jobs: &mut Jobs) {
        if self.log_loading || self.log_complete {
            return;
        }
        self.log_loading = true;
        let repo = self.repo.clone();
        let skip = self.commits.len();
        jobs.spawn(move || RepoMessage::Log {
            skip,
            result: git::load_log(&repo, skip, LOG_PAGE),
        });
    }

    /// Vuelve a leer el historial desde el principio (p. ej. tras un commit).
    fn reload_log(&mut self, jobs: &mut Jobs) {
        self.graph = Graph::default();
        self.log_complete = false;
        self.log_loading = false;
        if let Some(ListRow::Commit(i)) = self.selected {
            self.reselect = self.commits.get(i).map(|c| c.id.clone());
            self.selected = None;
        }
        self.commits.clear();
        self.load_log_page(jobs);
    }

    /// Vuelve a leer el estado; si cambiaron las referencias también el historial.
    pub fn refresh_status(&mut self, jobs: &mut Jobs) {
        let repo = self.repo.clone();
        jobs.spawn(move || {
            RepoMessage::Status(
                git::load_status(&repo).and_then(|s| Ok((s, repo.refs_fingerprint()?))),
            )
        });
    }

    fn select(&mut self, row: ListRow, jobs: &mut Jobs) {
        match row {
            ListRow::WorkingTree => {
                self.selected = Some(row);
                let file = self
                    .wt_file
                    .clone()
                    .or_else(|| self.status.as_ref().and_then(WtFile::first));
                if let Some(file) = file {
                    self.select_worktree_file(file, jobs);
                }
            }
            ListRow::Commit(index) => {
                if self.selected == Some(row) {
                    return;
                }
                let Some(commit) = self.commits.get(index) else {
                    return;
                };
                self.selected = Some(row);
                let id = commit.id.clone();
                let repo = self.repo.clone();
                self.details_loading = Some(id.clone());
                jobs.spawn(move || RepoMessage::Details {
                    result: git::load_commit(&repo, &id),
                    id,
                });
            }
        }
    }

    fn select_worktree_file(&mut self, file: WtFile, jobs: &mut Jobs) {
        let Some(entry) = self.status.as_ref().and_then(|s| file.find(s)).cloned() else {
            self.wt_file = None;
            self.wt_diff = None;
            return;
        };
        if self.wt_file.as_ref() != Some(&file) {
            self.wt_diff = None;
        }
        self.wt_file = Some(file.clone());
        let repo = self.repo.clone();
        jobs.spawn(move || RepoMessage::WorktreeDiff {
            result: git::diff_entry(&repo, &entry, file.staged),
            file,
        });
    }

    /// Ejecuta una operación sobre el repo y después refresca el estado.
    fn run_op(
        &mut self,
        jobs: &mut Jobs,
        op: impl FnOnce(&Repo) -> git::Result<()> + Send + 'static,
    ) {
        let repo = self.repo.clone();
        jobs.spawn(move || RepoMessage::Done(op(&repo)));
        self.refresh_status(jobs);
    }

    /// Si el archivo seleccionado cambia de lista, la selección lo sigue.
    fn follow_selection(&mut self, paths: &[String], staged: bool) {
        if let Some(file) = &mut self.wt_file
            && paths.contains(&file.path)
        {
            file.staged = staged;
        }
    }

    fn apply(&mut self, action: Action, jobs: &mut Jobs) {
        match action {
            Action::LoadMore => self.load_log_page(jobs),
            Action::Select(row) => self.select(row, jobs),
            Action::SelectWorktreeFile(file) => self.select_worktree_file(file, jobs),
            Action::Stage(paths) => {
                self.follow_selection(&paths, true);
                self.run_op(jobs, move |repo| git::stage(repo, &paths));
            }
            Action::Unstage(paths) => {
                self.follow_selection(&paths, false);
                self.run_op(jobs, move |repo| git::unstage(repo, &paths));
            }
            Action::StageAll => self.run_op(jobs, git::stage_all),
            Action::UnstageAll => self.run_op(jobs, git::unstage_all),
            Action::Commit { message, amend } => {
                self.editor.committing = true;
                let repo = self.repo.clone();
                jobs.spawn(move || {
                    RepoMessage::Committed(git::create_commit(&repo, &message, amend))
                });
            }
            Action::Refresh => self.refresh_status(jobs),
            Action::LoadLastMessage => {
                let repo = self.repo.clone();
                jobs.spawn(move || RepoMessage::LastMessage(git::last_commit_message(&repo)));
            }
            Action::SelectId(id) => {
                if let Some(i) = self.commits.iter().position(|c| c.id == id) {
                    self.list.scroll_to(ListRow::Commit(i));
                    self.select(ListRow::Commit(i), jobs);
                }
            }
        }
    }

    pub fn handle(&mut self, msg: RepoMessage, jobs: &mut Jobs) {
        match msg {
            RepoMessage::Log { skip, result } => {
                self.log_loading = false;
                match result {
                    Ok(page) if skip == self.commits.len() => {
                        self.log_complete = page.len() < LOG_PAGE;
                        self.graph.extend(&page);
                        self.commits.extend(page);
                        if skip == 0 {
                            self.select_initial(jobs);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        self.log_complete = true;
                        self.error = Some(format!("Error leyendo el historial: {e}"));
                    }
                }
            }
            RepoMessage::Details { id, result } => {
                if self.details_loading.as_ref() != Some(&id) {
                    return;
                }
                self.details_loading = None;
                match result {
                    Ok(details) => {
                        self.selected_file = (!details.files.is_empty()).then_some(0);
                        self.details = Some(details);
                    }
                    Err(e) => {
                        self.details = None;
                        self.error = Some(format!("Error leyendo el commit: {e}"));
                    }
                }
            }
            RepoMessage::Status(result) => match result {
                Ok((status, refs)) => {
                    self.branch = status.branch.clone();
                    self.status = Some(status);
                    // Cambios hechos fuera (terminal, otro programa...).
                    let refs_changed = self.refs.as_ref().is_some_and(|r| r != &refs);
                    self.refs = Some(refs);
                    self.after_status_change(jobs);
                    if refs_changed {
                        self.reload_log(jobs);
                    }
                }
                Err(e) => self.error = Some(format!("Error leyendo el estado: {e}")),
            },
            RepoMessage::Committed(result) => {
                self.editor.committing = false;
                match result {
                    Ok(_) => {
                        self.editor.clear();
                        // El refresco del estado detecta el nuevo commit y recarga el log.
                        self.refresh_status(jobs);
                    }
                    Err(e) => self.error = Some(format!("No se pudo crear el commit: {e}")),
                }
            }
            RepoMessage::LastMessage(result) => match result {
                Ok(message) if self.editor.amend => self.editor.set_message(&message),
                Ok(_) => {}
                Err(e) => self.error = Some(e.to_string()),
            },
            // Sin user.name/user.email: git lo explicará al hacer commit.
            RepoMessage::Ident(result) => self.author = result.ok(),
            RepoMessage::Done(result) => {
                if let Err(e) = result {
                    self.error = Some(e.to_string());
                }
            }
            RepoMessage::WorktreeDiff { file, result } => {
                if self.wt_file.as_ref() != Some(&file) {
                    return;
                }
                match result {
                    Ok(diff) => self.wt_diff = Some(diff),
                    Err(e) => {
                        self.wt_diff = None;
                        self.error = Some(format!("Error leyendo el diff: {e}"));
                    }
                }
            }
        }
    }

    /// Selección tras cargar la primera página del historial.
    fn select_initial(&mut self, jobs: &mut Jobs) {
        let reselect = self
            .reselect
            .take()
            .and_then(|id| self.commits.iter().position(|c| c.id == id));
        let row = if let Some(i) = reselect {
            ListRow::Commit(i)
        } else if self.selected == Some(ListRow::WorkingTree) || self.has_changes() {
            ListRow::WorkingTree
        } else if self.commits.is_empty() {
            return;
        } else {
            ListRow::Commit(0)
        };
        // Tras recargar, los detalles del commit siguen siendo válidos si es el mismo.
        if let ListRow::Commit(i) = row
            && self.details.as_ref().map(|d| &d.id) == Some(&self.commits[i].id)
        {
            self.selected = Some(row);
            self.list.scroll_to(row);
        } else {
            self.select(row, jobs);
        }
    }

    /// Mantiene coherente la vista de cambios cuando cambia el estado.
    fn after_status_change(&mut self, jobs: &mut Jobs) {
        if self.selected != Some(ListRow::WorkingTree) {
            return;
        }
        if !self.has_changes() {
            self.wt_file = None;
            self.wt_diff = None;
            if !self.commits.is_empty() {
                self.select(ListRow::Commit(0), jobs);
            }
            return;
        }
        let status = self.status.as_ref().unwrap();
        let file = self
            .wt_file
            .clone()
            .filter(|f| f.find(status).is_some())
            .or_else(|| WtFile::first(status));
        if let Some(file) = file {
            self.select_worktree_file(file, jobs);
        }
    }

    /// Dibuja la pestaña completa.
    pub fn ui(&mut self, ui: &mut egui::Ui, jobs: &mut Jobs) {
        let mut actions = Vec::new();
        egui::TopBottomPanel::top("repo_toolbar").show_inside(ui, |ui| {
            self.toolbar(ui, &mut actions);
        });
        self.repo_view(ui, &mut actions);
        for action in actions {
            self.apply(action, jobs);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.strong(self.repo.name())
                .on_hover_text(self.repo.root().display().to_string());
            ui.label(match &self.branch {
                Some(b) => format!("rama: {b}"),
                None => "HEAD separado".to_owned(),
            });
            if let Some(status) = &self.status
                && let Some(upstream) = &status.upstream
            {
                ui.weak(format!(
                    "{upstream} (+{} / -{})",
                    status.ahead, status.behind
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Refrescar")
                    .on_hover_text("Volver a leer el repositorio (F5)")
                    .clicked()
                {
                    actions.push(Action::Refresh);
                }
            });
        });
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

    fn repo_view(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // Si los cambios desaparecen, la fila deja de existir.
        if self.selected == Some(ListRow::WorkingTree) && !self.has_changes() {
            self.selected = None;
        }
        if self.selected.is_some() {
            egui::TopBottomPanel::bottom("details_panel")
                .resizable(true)
                .default_height(ui.available_height() * 0.55)
                .min_height(120.0)
                .show_inside(ui, |ui| match self.selected {
                    Some(ListRow::WorkingTree) => self.worktree_panel(ui, actions),
                    _ => self.details_panel(ui, actions),
                });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let changes = self
                .status
                .as_ref()
                .filter(|s| !s.is_clean())
                .map(|s| s.change_count());
            if self.commits.is_empty() && changes.is_none() && !self.log_loading {
                ui.weak("Este repositorio todavía no tiene commits.");
            }
            let resp = self
                .list
                .show(ui, &self.commits, &self.graph, self.selected, changes);
            if let Some(row) = resp.selected {
                actions.push(Action::Select(row));
            }
            if resp.wants_more {
                actions.push(Action::LoadMore);
            }
        });
    }

    fn details_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(details) = &self.details else {
            ui.centered_and_justified(|ui| {
                if self.details_loading.is_some() {
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
                    commit_details::show(ui, details, &mut self.selected_file)
                {
                    actions.push(Action::SelectId(id));
                }
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            match self.selected_file.and_then(|i| details.files.get(i)) {
                Some(file) => diff_view::show(ui, file),
                None => diff_view::placeholder(ui, "Selecciona un archivo para ver el diff."),
            }
        });
    }

    fn worktree_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(status) = &self.status else {
            ui.spinner();
            return;
        };
        egui::SidePanel::left("worktree_files")
            .resizable(true)
            .default_width(ui.available_width() * 0.4)
            .min_width(200.0)
            .show_inside(ui, |ui| {
                egui::TopBottomPanel::bottom("commit_editor")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        let editor_action =
                            self.editor
                                .show(ui, status.staged.len(), self.author.as_deref());
                        match editor_action {
                            Some(EditorAction::Commit { message, amend }) => {
                                actions.push(Action::Commit { message, amend })
                            }
                            Some(EditorAction::LoadLastMessage) => {
                                actions.push(Action::LoadLastMessage)
                            }
                            None => {}
                        }
                    });
                if let Some(action) = status_panel::show(ui, status, self.wt_file.as_ref()) {
                    actions.push(match action {
                        StatusAction::Select(file) => Action::SelectWorktreeFile(file),
                        StatusAction::Stage(paths) => Action::Stage(paths),
                        StatusAction::Unstage(paths) => Action::Unstage(paths),
                        StatusAction::StageAll => Action::StageAll,
                        StatusAction::UnstageAll => Action::UnstageAll,
                    });
                }
            });
        egui::CentralPanel::default().show_inside(ui, |ui| match &self.wt_diff {
            Some(diff) => diff_view::show(ui, diff),
            None if self.wt_file.is_some() => {
                ui.centered_and_justified(|ui| ui.spinner());
            }
            None => diff_view::placeholder(ui, "Selecciona un archivo para ver el diff."),
        });
    }
}
