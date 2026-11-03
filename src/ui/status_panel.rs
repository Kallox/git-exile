//! Lista de archivos con cambios sin commitear.

use eframe::egui::{Align, Color32, Layout, RichText, ScrollArea, Ui};

use super::commit_details::{ADDED, REMOVED};
use crate::git::{ChangeKind, StatusEntry, WorkingTreeStatus};

/// Archivo seleccionado en la vista de cambios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtFile {
    pub path: String,
    /// `true` si es la versión del índice (staged).
    pub staged: bool,
}

impl WtFile {
    pub fn find<'a>(&self, status: &'a WorkingTreeStatus) -> Option<&'a StatusEntry> {
        let list = if self.staged {
            &status.staged
        } else {
            &status.unstaged
        };
        list.iter()
            .chain(&status.conflicted)
            .find(|e| e.path == self.path)
    }

    /// Primer archivo de la lista, empezando por los no añadidos.
    pub fn first(status: &WorkingTreeStatus) -> Option<Self> {
        let entry = |e: &StatusEntry, staged| Self {
            path: e.path.clone(),
            staged,
        };
        status
            .conflicted
            .first()
            .map(|e| entry(e, false))
            .or_else(|| status.unstaged.first().map(|e| entry(e, false)))
            .or_else(|| status.staged.first().map(|e| entry(e, true)))
    }
}

pub enum StatusAction {
    Select(WtFile),
    /// Añadir al índice. Las rutas incluyen la original en los renombrados.
    Stage(Vec<String>),
    Unstage(Vec<String>),
    StageAll,
    UnstageAll,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Conflicted,
    Unstaged,
    Staged,
}

pub fn show(
    ui: &mut Ui,
    status: &WorkingTreeStatus,
    selected: Option<&WtFile>,
) -> Option<StatusAction> {
    let mut action = None;
    ScrollArea::vertical()
        .id_salt("status_panel")
        .auto_shrink(false)
        .show(ui, |ui| {
            if !status.conflicted.is_empty() {
                section(
                    ui,
                    Section::Conflicted,
                    &status.conflicted,
                    selected,
                    &mut action,
                );
            }
            section(
                ui,
                Section::Unstaged,
                &status.unstaged,
                selected,
                &mut action,
            );
            section(ui, Section::Staged, &status.staged, selected, &mut action);
        });
    action
}

fn section(
    ui: &mut Ui,
    kind: Section,
    entries: &[StatusEntry],
    selected: Option<&WtFile>,
    action: &mut Option<StatusAction>,
) {
    let staged = kind == Section::Staged;
    let title = match kind {
        Section::Conflicted => "Conflictos",
        Section::Unstaged => "Sin añadir",
        Section::Staged => "Preparados para commit",
    };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.strong(format!("{title} ({})", entries.len()));
        if entries.is_empty() {
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match kind {
            Section::Unstaged => {
                if ui.small_button("Añadir todo").clicked() {
                    *action = Some(StatusAction::StageAll);
                }
            }
            Section::Staged => {
                if ui.small_button("Quitar todo").clicked() {
                    *action = Some(StatusAction::UnstageAll);
                }
            }
            Section::Conflicted => {}
        });
    });
    if entries.is_empty() {
        ui.weak("Nada");
    }
    for entry in entries {
        let file = WtFile {
            path: entry.path.clone(),
            staged,
        };
        let paths = || {
            let mut paths = vec![entry.path.clone()];
            paths.extend(entry.orig_path.clone());
            paths
        };
        let toggle = || {
            if staged {
                StatusAction::Unstage(paths())
            } else {
                StatusAction::Stage(paths())
            }
        };
        ui.horizontal(|ui| {
            kind_badge(ui, entry.kind);
            let label = match &entry.orig_path {
                Some(orig) => format!("{orig} → {}", entry.path),
                None => entry.path.clone(),
            };
            let resp = ui.selectable_label(selected == Some(&file), label);
            if resp.double_clicked() {
                *action = Some(toggle());
            } else if resp.clicked() {
                *action = Some(StatusAction::Select(file.clone()));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (text, hint) = match kind {
                    Section::Staged => ("Quitar", "Sacar del próximo commit"),
                    Section::Unstaged => ("Añadir", "Añadir al próximo commit"),
                    Section::Conflicted => ("Resuelto", "Marcar el conflicto como resuelto"),
                };
                if ui.small_button(text).on_hover_text(hint).clicked() {
                    *action = Some(toggle());
                }
            });
        });
    }
}

pub fn kind_badge(ui: &mut Ui, kind: ChangeKind) {
    let color = match kind {
        ChangeKind::Added | ChangeKind::Untracked => ADDED,
        ChangeKind::Deleted | ChangeKind::Conflicted => REMOVED,
        ChangeKind::Modified | ChangeKind::TypeChanged => Color32::from_rgb(220, 170, 60),
        ChangeKind::Renamed | ChangeKind::Copied => Color32::from_rgb(100, 150, 220),
    };
    ui.label(
        RichText::new(kind.letter().to_string())
            .monospace()
            .strong()
            .color(color),
    );
}
