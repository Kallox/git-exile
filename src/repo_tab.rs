//! Una pestaña con un repositorio abierto: historial, detalles y cambios.

use eframe::egui;

use crate::app::{Message, MessageKind};
use crate::git::{self, Commit, CommitDetails, FileDiff, Graph, Repo, WorkingTreeStatus};
use crate::ui::branch_dialog::{DialogAction, NewBranchDialog};
use crate::ui::commit_details::{self, DetailsAction};
use crate::ui::commit_editor::{CommitEditor, EditorAction};
use crate::ui::commit_list::{CommitList, ContextAction, ListRow};
use crate::ui::diff_view;
use crate::ui::icons::{self, ButtonStyle, Icon};
use crate::ui::status_panel::{self, StatusAction, WtFile};
use crate::ui::theme;
use crate::worker::Worker;

const LOG_PAGE: usize = 500;
/// Ancho fijo del buscador: el contador y las flechas van dentro y no mueven nada.
const SEARCH_WIDTH: f32 = 340.0;

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
    BranchCreated(git::Result<()>),
    Branches(git::Result<Vec<git::Branch>>),
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
    Commit {
        message: String,
        amend: bool,
    },
    LoadLastMessage,
    Refresh,
    /// Abrir el diálogo de nueva rama desde un commit (o HEAD si es `None`).
    NewBranch(Option<usize>),
    CreateBranch {
        name: String,
        start: String,
        checkout: bool,
    },
    CloseDialog,
    /// Cambiar a una rama local por nombre.
    SwitchTo(String),
    /// Mover HEAD a un commit (por hash), sin rama.
    CheckoutCommit(String),
    /// Ir al resultado de búsqueda siguiente (`forward`) o al anterior.
    StepMatch {
        forward: bool,
    },
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
    new_branch: Option<NewBranchDialog>,
    branches: Vec<git::Branch>,
    branch_filter: String,
    /// Texto del buscador de commits.
    search: String,
    /// Resultado de la última búsqueda: (texto, commits revisados, índices).
    search_cache: Option<(String, usize, Vec<usize>)>,
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
            new_branch: None,
            branches: Vec::new(),
            branch_filter: String::new(),
            search: String::new(),
            search_cache: None,
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

    fn selected_commit(&self) -> Option<usize> {
        match self.selected {
            Some(ListRow::Commit(i)) => Some(i),
            _ => None,
        }
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
        self.search_cache = None;
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
        let repo = self.repo.clone();
        jobs.spawn(move || RepoMessage::Branches(git::list_branches(&repo)));
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
            Action::NewBranch(index) => self.open_new_branch(index),
            Action::CreateBranch {
                name,
                start,
                checkout,
            } => {
                if let Some(dialog) = &mut self.new_branch {
                    dialog.busy = true;
                }
                let repo = self.repo.clone();
                jobs.spawn(move || {
                    RepoMessage::BranchCreated(git::create_branch(&repo, &name, &start, checkout))
                });
            }
            Action::CloseDialog => self.new_branch = None,
            Action::SwitchTo(name) => {
                self.run_op(jobs, move |repo| {
                    let branches = git::list_branches(repo)?;
                    match branches.iter().find(|b| b.name == name) {
                        Some(b) => git::switch_branch(repo, b, &branches),
                        None => repo.git(&["switch", &name]).map(drop),
                    }
                });
            }
            Action::CheckoutCommit(id) => {
                self.run_op(jobs, move |repo| git::checkout_commit(repo, &id));
            }
            Action::StepMatch { forward } => {
                let current = self.selected_commit();
                let target = self
                    .search_results()
                    .and_then(|found| step_match(found, current, forward));
                if let Some(i) = target {
                    self.list.scroll_to(ListRow::Commit(i));
                    self.select(ListRow::Commit(i), jobs);
                }
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
            RepoMessage::BranchCreated(result) => match result {
                Ok(()) => {
                    self.new_branch = None;
                    // Las referencias cambian: se recarga el historial.
                    self.refresh_status(jobs);
                }
                Err(e) => {
                    if let Some(dialog) = &mut self.new_branch {
                        dialog.busy = false;
                        dialog.error = Some(e.to_string());
                    } else {
                        self.error = Some(e.to_string());
                    }
                }
            },
            RepoMessage::Branches(result) => match result {
                Ok(branches) => self.branches = branches,
                Err(e) => self.error = Some(format!("Error leyendo las ramas: {e}")),
            },
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

    /// Abre el diálogo de nueva rama. Sin commit indicado se usa HEAD.
    fn open_new_branch(&mut self, index: Option<usize>) {
        let describe = |c: &Commit| format!("{}  {}", c.short_id, c.summary);
        let (start, label) = match index.and_then(|i| self.commits.get(i)) {
            Some(c) => (c.id.clone(), describe(c)),
            None => {
                let head = self.commits.iter().find(|c| {
                    c.refs.iter().any(|r| {
                        matches!(
                            r.kind,
                            git::RefKind::Head | git::RefKind::Branch { current: true }
                        )
                    })
                });
                let label = match head {
                    Some(c) => format!("HEAD ({})", describe(c)),
                    None => "HEAD".to_owned(),
                };
                ("HEAD".to_owned(), label)
            }
        };
        self.new_branch = Some(NewBranchDialog::new(start, label));
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
        egui::TopBottomPanel::top("repo_toolbar")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 12)))
            .show_inside(ui, |ui| {
                self.toolbar(ui, &mut actions);
            });
        self.error_bar(ui);
        self.repo_view(ui, &mut actions);
        if let Some(dialog) = &mut self.new_branch {
            match dialog.show(ui.ctx()) {
                Some(DialogAction::Create {
                    name,
                    start,
                    checkout,
                }) => actions.push(Action::CreateBranch {
                    name,
                    start,
                    checkout,
                }),
                Some(DialogAction::Cancel) => actions.push(Action::CloseDialog),
                None => {}
            }
        }
        for action in actions {
            self.apply(action, jobs);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.label(
                egui::RichText::new(self.repo.name())
                    .font(theme::font_semibold(16.0))
                    .color(theme::TEXT),
            )
            .on_hover_text(self.repo.root().display().to_string());
            self.branch_menu(ui, actions);
            if let Some(status) = &self.status
                && let Some(upstream) = &status.upstream
            {
                upstream_chip(ui, upstream, status.ahead, status.behind);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let selected_commit = self.selected_commit();
                let hint = match selected_commit.and_then(|i| self.commits.get(i)) {
                    Some(c) => format!("Crear una rama en el commit {}", c.short_id),
                    None => "Crear una rama en HEAD (el último commit)".to_owned(),
                };
                if icons::button(
                    ui,
                    Icon::Plus,
                    "Nueva rama",
                    ButtonStyle::Primary,
                    !self.commits.is_empty(),
                )
                .on_hover_text(hint)
                .clicked()
                {
                    actions.push(Action::NewBranch(selected_commit));
                }
                if icons::button(ui, Icon::Refresh, "Refrescar", ButtonStyle::Secondary, true)
                    .on_hover_text("Volver a leer el repositorio (F5)")
                    .clicked()
                {
                    actions.push(Action::Refresh);
                }
                let current = self.selected_commit();
                let log_complete = self.log_complete;
                let scanned = self.commits.len();
                let results = self.search_results().map(|found| SearchResults {
                    text: results_text(found, current),
                    any: !found.is_empty(),
                    scanned: (!log_complete).then_some(scanned),
                });
                if let Some(forward) = search_box(ui, &mut self.search, results.as_ref()) {
                    actions.push(Action::StepMatch { forward });
                }
            });
        });
    }

    /// Índices de los commits que coinciden con la búsqueda, o `None` sin búsqueda.
    fn search_results(&mut self) -> Option<&[usize]> {
        let query = self.search.trim().to_lowercase();
        if query.is_empty() {
            self.search_cache = None;
            return None;
        }
        let stale = match &self.search_cache {
            Some((q, n, _)) => *q != query || *n != self.commits.len(),
            None => true,
        };
        if stale {
            let found = search_commits(&self.commits, &query);
            self.search_cache = Some((query, self.commits.len(), found));
        }
        self.search_cache.as_ref().map(|(_, _, v)| v.as_slice())
    }

    fn error_bar(&mut self, ui: &mut egui::Ui) {
        let Some(error) = &self.error else { return };
        let mut close = false;
        egui::TopBottomPanel::top("repo_error")
            .frame(
                egui::Frame::new()
                    .fill(theme::alpha(theme::RED, 0.10))
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(theme::RED, error);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = icons::icon_button(ui, Icon::Close, 24.0, 12.0, theme::TEXT_MUTED)
                            .on_hover_text("Cerrar")
                            .clicked();
                    });
                });
            });
        if close {
            self.error = None;
        }
    }

    /// Menú para cambiar de rama, con filtro por nombre.
    fn branch_menu(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let button = branch_button(ui, self.branch.as_deref()).on_hover_text(
            "Cambiar de rama. Los cambios sin commitear se mantienen si no hay conflicto.",
        );
        let resp = egui::Popup::menu(&button).show(|ui| {
            ui.set_min_width(280.0);
            let filter = ui.add(
                egui::TextEdit::singleline(&mut self.branch_filter)
                    .hint_text("Filtrar ramas")
                    .desired_width(f32::INFINITY),
            );
            filter.request_focus();
            let needle = self.branch_filter.to_lowercase();
            let matches =
                |b: &&git::Branch| needle.is_empty() || b.name.to_lowercase().contains(&needle);
            let local: Vec<&git::Branch> = self
                .branches
                .iter()
                .filter(|b| !b.remote)
                .filter(matches)
                .collect();
            let remote: Vec<&git::Branch> = self
                .branches
                .iter()
                .filter(|b| b.remote)
                .filter(matches)
                .collect();

            // Enter cambia a la primera rama que coincide.
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter && let Some(first) = local.iter().chain(&remote).find(|b| !b.current) {
                actions.push(Action::SwitchTo(first.name.clone()));
                ui.close();
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for (title, list) in [("Locales", &local), ("Remotas", &remote)] {
                        if list.is_empty() {
                            continue;
                        }
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(title.to_uppercase())
                                .font(theme::font_semibold(11.0))
                                .color(theme::TEXT_MUTED),
                        );
                        for branch in list.iter() {
                            let resp = branch_row(ui, branch);
                            let resp = match &branch.upstream {
                                Some(up) => resp.on_hover_text(format!("Sigue a {up}")),
                                None => resp,
                            };
                            if resp.clicked() {
                                actions.push(Action::SwitchTo(branch.name.clone()));
                                ui.close();
                            }
                        }
                    }
                    if local.is_empty() && remote.is_empty() {
                        ui.weak("Ninguna rama coincide.");
                    }
                });
        });
        // Al cerrar el menú se limpia el filtro.
        if resp.is_none() {
            self.branch_filter.clear();
        }
    }

    fn repo_view(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // Si los cambios desaparecen, la fila deja de existir.
        if self.selected == Some(ListRow::WorkingTree) && !self.has_changes() {
            self.selected = None;
        }
        if self.selected.is_some() {
            egui::TopBottomPanel::bottom("details_panel")
                .frame(egui::Frame::new())
                .resizable(true)
                .default_height(ui.available_height() * 0.55)
                .min_height(120.0)
                .show_inside(ui, |ui| match self.selected {
                    Some(ListRow::WorkingTree) => self.worktree_panel(ui, actions),
                    _ => self.details_panel(ui, actions),
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show_inside(ui, |ui| {
                let searching = !self.search.trim().is_empty();
                let changes = self
                    .status
                    .as_ref()
                    .filter(|s| !s.is_clean())
                    .map(|s| s.change_count());
                if self.commits.is_empty() && changes.is_none() && !self.log_loading && !searching {
                    ui.weak("Este repositorio todavía no tiene commits.");
                }
                let found = self.search_results().map(<[usize]>::to_vec);
                // La lista ya no se acorta: hay que pedir el resto del historial.
                if found.is_some() && !self.log_complete {
                    actions.push(Action::LoadMore);
                }
                let resp = self.list.show(
                    ui,
                    &self.commits,
                    &self.graph,
                    self.selected,
                    changes,
                    found.as_deref(),
                );
                if let Some(row) = resp.selected {
                    actions.push(Action::Select(row));
                }
                if resp.wants_more {
                    actions.push(Action::LoadMore);
                }
                match resp.context {
                    Some(ContextAction::CreateBranch(i)) => {
                        actions.push(Action::NewBranch(Some(i)))
                    }
                    Some(ContextAction::SwitchBranch(name)) => actions.push(Action::SwitchTo(name)),
                    Some(ContextAction::Checkout(i)) => {
                        if let Some(commit) = self.commits.get(i) {
                            actions.push(Action::CheckoutCommit(commit.id.clone()));
                        }
                    }
                    None => {}
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
            .default_width(ui.available_width() * 0.45)
            .min_width(280.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)))
            .show_inside(ui, |ui| {
                if let Some(DetailsAction::SelectCommit(id)) =
                    commit_details::show(ui, details, &mut self.selected_file)
                {
                    actions.push(Action::SelectId(id));
                }
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)))
            .show_inside(ui, |ui| {
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
            .default_width(ui.available_width() * 0.45)
            .min_width(280.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)))
            .show_inside(ui, |ui| {
                egui::TopBottomPanel::bottom("commit_editor")
                    .frame(egui::Frame::new().inner_margin(egui::Margin {
                        top: 12,
                        ..Default::default()
                    }))
                    .show_separator_line(false)
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
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)))
            .show_inside(ui, |ui| match &self.wt_diff {
                Some(diff) => diff_view::show(ui, diff),
                None if self.wt_file.is_some() => {
                    ui.centered_and_justified(|ui| ui.spinner());
                }
                None => diff_view::placeholder(ui, "Selecciona un archivo para ver el diff."),
            });
    }
}

/// Botón de rama con fondo de acento, como en el diseño.
fn branch_button(ui: &mut egui::Ui, branch: Option<&str>) -> egui::Response {
    let painter = ui.painter();
    let label = painter.layout_no_wrap(
        if branch.is_some() {
            "Rama"
        } else {
            "HEAD separado"
        }
        .to_owned(),
        theme::font(13.0),
        theme::TEXT_SECONDARY,
    );
    let name = painter.layout_no_wrap(
        branch.unwrap_or_default().to_owned(),
        theme::font_semibold(13.0),
        theme::TEXT,
    );
    let gap = 8.0;
    let name_width = if branch.is_some() {
        name.size().x + gap
    } else {
        0.0
    };
    let width = 12.0 + 14.0 + gap + label.size().x + gap + name_width + 12.0 + 12.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 34.0), egui::Sense::click());
    let painter = ui.painter();
    let fill = if resp.hovered() {
        theme::alpha(theme::ACCENT, 0.2)
    } else {
        theme::accent_soft()
    };
    painter.rect(
        rect,
        8.0,
        fill,
        egui::Stroke::new(1.0, theme::accent_line()),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 12.0;
    let cy = rect.center().y;
    icons::paint(
        painter,
        egui::Rect::from_min_size(egui::pos2(x, cy - 7.0), egui::vec2(14.0, 14.0)),
        Icon::Branch,
        theme::ACCENT,
        2.0,
    );
    x += 14.0 + gap;
    let h = label.size().y;
    let w = label.size().x;
    painter.galley(egui::pos2(x, cy - h / 2.0), label, theme::TEXT_SECONDARY);
    x += w + gap;
    if branch.is_some() {
        let w = name.size().x;
        painter.galley(egui::pos2(x, cy - h / 2.0), name, theme::TEXT);
        x += w + gap;
    }
    icons::paint(
        painter,
        egui::Rect::from_min_size(egui::pos2(x, cy - 6.0), egui::vec2(12.0, 12.0)),
        Icon::ChevronDown,
        theme::TEXT_MUTED,
        2.2,
    );
    resp
}

/// Rama remota que se sigue, con commits por subir y por bajar.
fn upstream_chip(ui: &mut egui::Ui, upstream: &str, ahead: u32, behind: u32) {
    egui::Frame::new()
        .fill(theme::BG_INPUT)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_height(34.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icons::show(ui, Icon::Cloud, 14.0, theme::TEXT_SECONDARY);
                let mono = |text: String, color| {
                    egui::RichText::new(text)
                        .font(theme::mono(12.0))
                        .color(color)
                };
                ui.label(mono(upstream.to_owned(), theme::TEXT_SECONDARY));
                ui.label(mono(format!("↑{ahead}"), theme::GREEN))
                    .on_hover_text("Commits por subir");
                ui.label(mono(format!("↓{behind}"), theme::TEXT_SECONDARY))
                    .on_hover_text("Commits por bajar");
                let state = match (ahead, behind) {
                    (0, 0) => "Sincronizado".to_owned(),
                    (a, 0) => format!("{a} por subir"),
                    (0, b) => format!("{b} por bajar"),
                    (a, b) => format!("{a} por subir, {b} por bajar"),
                };
                ui.label(
                    egui::RichText::new(state)
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });
}

/// Commits cuyo mensaje, autor, correo o hash contienen `query` (en minúsculas).
fn search_commits(commits: &[Commit], query: &str) -> Vec<usize> {
    commits
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.id.starts_with(query)
                || c.summary.to_lowercase().contains(query)
                || c.author_name.to_lowercase().contains(query)
                || c.author_email.to_lowercase().contains(query)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Resultado siguiente o anterior a `current` en `found` (índices ordenados).
/// Al llegar a un extremo da la vuelta; `current` no tiene por qué ser un resultado.
fn step_match(found: &[usize], current: Option<usize>, forward: bool) -> Option<usize> {
    if forward {
        let next = current.map_or(0, |c| found.partition_point(|&i| i <= c));
        found.get(next).or(found.first()).copied()
    } else {
        let end = current.map_or(found.len(), |c| found.partition_point(|&i| i < c));
        end.checked_sub(1)
            .and_then(|i| found.get(i))
            .or(found.last())
            .copied()
    }
}

/// Texto del contador: la posición si el commit `current` es un resultado.
fn results_text(found: &[usize], current: Option<usize>) -> String {
    let total = found.len();
    match (current.and_then(|c| found.binary_search(&c).ok()), total) {
        (Some(pos), _) => format!("{}/{total}", pos + 1),
        (None, 0) => "Sin resultados".to_owned(),
        (None, 1) => "1 resultado".to_owned(),
        (None, _) => format!("{total} resultados"),
    }
}

/// Lo que el buscador muestra de la búsqueda en curso.
struct SearchResults {
    text: String,
    any: bool,
    /// Commits revisados, si todavía falta historial por cargar.
    scanned: Option<usize>,
}

/// Campo de búsqueda de commits, con contador y navegación integrados.
/// Ctrl+F lo enfoca, Escape lo vacía y Enter (o las flechas) devuelve el sentido
/// en que moverse por los resultados (`true` hacia abajo).
fn search_box(
    ui: &mut egui::Ui,
    text: &mut String,
    results: Option<&SearchResults>,
) -> Option<bool> {
    let id = egui::Id::new("commit_search");
    let mut step = None;
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
        ui.memory_mut(|m| m.request_focus(id));
    }
    egui::Frame::new()
        .fill(theme::BG_INPUT)
        .stroke(egui::Stroke::new(1.0, theme::BORDER_STRONG))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_width(SEARCH_WIDTH);
            ui.set_height(32.0);
            // El padre es de derecha a izquierda: los controles van primero.
            if let Some(results) = results {
                search_controls(ui, text, results, id, &mut step);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                icons::show(ui, Icon::Search, 14.0, theme::TEXT_MUTED);
                let edit = ui.add(
                    egui::TextEdit::singleline(text)
                        .id(id)
                        .frame(false)
                        .hint_text("Buscar commit, autor o hash…")
                        .desired_width(f32::INFINITY)
                        // Enter navega por los resultados sin perder el foco.
                        .return_key(None),
                );
                if edit.has_focus() {
                    ui.input(|i| {
                        if i.key_pressed(egui::Key::Enter) {
                            step = Some(!i.modifiers.shift);
                        }
                    });
                }
                if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    text.clear();
                    edit.surrender_focus();
                }
                edit.on_hover_text(
                    "Buscar en mensajes, autores y hashes (Ctrl+F). \
                     Enter y Mayús+Enter recorren los resultados.",
                );
            });
        });
    step
}

/// Contador, flechas y botón de limpiar, de derecha a izquierda.
fn search_controls(
    ui: &mut egui::Ui,
    text: &mut String,
    results: &SearchResults,
    id: egui::Id,
    step: &mut Option<bool>,
) {
    ui.spacing_mut().item_spacing.x = 2.0;
    if icons::icon_button(ui, Icon::Close, 24.0, 12.0, theme::TEXT_MUTED)
        .on_hover_text("Limpiar la búsqueda (Escape)")
        .clicked()
    {
        text.clear();
        ui.memory_mut(|m| m.request_focus(id));
    }
    ui.add_enabled_ui(results.any, |ui| {
        let mut arrow = |icon, hint: &str, forward: bool| {
            let clicked = icons::icon_button(ui, icon, 24.0, 14.0, theme::TEXT_SECONDARY)
                .on_hover_text(hint)
                .clicked();
            if clicked {
                *step = Some(forward);
                // El foco sigue en el campo para poder continuar con Enter.
                ui.memory_mut(|m| m.request_focus(id));
            }
        };
        // De derecha a izquierda: la de abajo se coloca primero.
        arrow(Icon::ChevronDown, "Resultado siguiente (Enter)", true);
        arrow(Icon::ChevronUp, "Resultado anterior (Mayús+Enter)", false);
    });
    ui.add_space(6.0);
    let dots = if results.scanned.is_some() { "…" } else { "" };
    let label = ui.add(
        egui::Label::new(
            egui::RichText::new(format!("{}{dots}", results.text))
                .size(12.0)
                .color(theme::TEXT_MUTED),
        )
        .selectable(false),
    );
    if let Some(scanned) = results.scanned {
        label.on_hover_text(format!("Buscando en el historial, {scanned} revisados…"));
    }
    ui.add_space(6.0);
}

/// Fila del menú de ramas: icono, nombre y marca de la actual.
fn branch_row(ui: &mut egui::Ui, branch: &git::Branch) -> egui::Response {
    let sense = if branch.current {
        egui::Sense::hover()
    } else {
        egui::Sense::click()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), sense);
    let painter = ui.painter();
    if branch.current {
        painter.rect_filled(rect, 6.0, theme::accent_soft());
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, theme::BG_ACTIVE);
    }
    let icon = if branch.remote {
        Icon::Cloud
    } else {
        Icon::BranchSmall
    };
    let (icon_color, font, text_color) = if branch.current {
        (
            theme::ACCENT,
            theme::font_semibold(13.0),
            theme::TEXT_STRONG,
        )
    } else {
        (theme::TEXT_MUTED, theme::font(13.0), theme::TEXT)
    };
    icons::paint(
        painter,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 14.0, rect.center().y),
            egui::vec2(12.0, 12.0),
        ),
        icon,
        icon_color,
        2.2,
    );
    painter.text(
        egui::pos2(rect.left() + 30.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &branch.name,
        font,
        text_color,
    );
    if branch.current {
        icons::paint(
            painter,
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 14.0, rect.center().y),
                egui::vec2(12.0, 12.0),
            ),
            Icon::Check,
            theme::ACCENT,
            2.2,
        );
        resp.on_hover_text("Rama actual")
    } else {
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, summary: &str, author: &str) -> Commit {
        Commit {
            id: id.into(),
            short_id: id[..7].into(),
            parents: Vec::new(),
            author_name: author.into(),
            author_email: format!("{}@example.com", author.to_lowercase()),
            time: 0,
            refs: Vec::new(),
            summary: summary.into(),
        }
    }

    #[test]
    fn steps_through_matches_wrapping_around() {
        let found = [2, 5, 9];
        assert_eq!(step_match(&found, None, true), Some(2));
        assert_eq!(step_match(&found, None, false), Some(9));
        assert_eq!(step_match(&found, Some(2), true), Some(5));
        assert_eq!(step_match(&found, Some(5), false), Some(2));
        // Da la vuelta en los extremos.
        assert_eq!(step_match(&found, Some(9), true), Some(2));
        assert_eq!(step_match(&found, Some(2), false), Some(9));
        // Desde un commit que no es resultado, el más cercano en ese sentido.
        assert_eq!(step_match(&found, Some(6), true), Some(9));
        assert_eq!(step_match(&found, Some(6), false), Some(5));
        assert_eq!(step_match(&[], Some(1), true), None);
        assert_eq!(step_match(&[], None, false), None);
    }

    #[test]
    fn counts_results_with_position_of_selected_commit() {
        let found = [2, 5, 9];
        assert_eq!(results_text(&found, Some(5)), "2/3");
        assert_eq!(results_text(&found, Some(3)), "3 resultados");
        assert_eq!(results_text(&found, None), "3 resultados");
        assert_eq!(results_text(&[4], None), "1 resultado");
        assert_eq!(results_text(&[], None), "Sin resultados");
    }

    #[test]
    fn searches_message_author_email_and_hash() {
        let commits = [
            commit("abc1234def", "feat: Add login", "Ana"),
            commit("bcd2345efa", "fix: typo", "Luis"),
            commit("cde3456fab", "docs: readme", "Ana Pérez"),
        ];
        assert_eq!(search_commits(&commits, "login"), [0]);
        assert_eq!(search_commits(&commits, "ana"), [0, 2]);
        assert_eq!(search_commits(&commits, "luis@"), [1]);
        assert_eq!(search_commits(&commits, "cde34"), [2]);
        // El hash solo coincide por el principio.
        assert!(search_commits(&commits, "def").is_empty());
    }
}
