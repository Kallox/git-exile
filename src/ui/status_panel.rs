//! Lista de archivos con cambios sin commitear.

use eframe::egui::{Color32, RichText, ScrollArea, Ui};

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
                    "Conflictos",
                    &status.conflicted,
                    false,
                    selected,
                    &mut action,
                );
            }
            section(
                ui,
                "Sin añadir",
                &status.unstaged,
                false,
                selected,
                &mut action,
            );
            section(
                ui,
                "Preparados para commit",
                &status.staged,
                true,
                selected,
                &mut action,
            );
        });
    action
}

fn section(
    ui: &mut Ui,
    title: &str,
    entries: &[StatusEntry],
    staged: bool,
    selected: Option<&WtFile>,
    action: &mut Option<StatusAction>,
) {
    ui.add_space(4.0);
    ui.strong(format!("{title} ({})", entries.len()));
    if entries.is_empty() {
        ui.weak("Nada");
    }
    for entry in entries {
        let file = WtFile {
            path: entry.path.clone(),
            staged,
        };
        ui.horizontal(|ui| {
            kind_badge(ui, entry.kind);
            let label = match &entry.orig_path {
                Some(orig) => format!("{orig} → {}", entry.path),
                None => entry.path.clone(),
            };
            if ui
                .selectable_label(selected == Some(&file), label)
                .clicked()
            {
                *action = Some(StatusAction::Select(file.clone()));
            }
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
