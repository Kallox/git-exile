//! Lista de archivos con cambios sin commitear.

use eframe::egui::{self, Color32, Rect, RichText, ScrollArea, Ui, vec2};

use super::commit_details::{Trailing, file_row};
use super::icons::{self, Icon};
use super::theme;
use crate::git::{ChangeKind, StatusEntry, WorkingTreeStatus};

/// Ancho reservado a la derecha de cada fila para su botón.
const BUTTON_SPACE: f32 = 86.0;
/// Hueco del botón de descartar, a la izquierda del anterior.
const DISCARD_SPACE: f32 = 28.0;

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
    /// Pedir confirmación para descartar los cambios de un archivo.
    Discard {
        entry: StatusEntry,
        staged: bool,
    },
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
            ui.add_space(12.0);
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
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.label(RichText::new(title).font(theme::font_semibold(13.0)));
        ui.label(
            RichText::new(entries.len().to_string())
                .size(12.0)
                .color(theme::colors().text_muted),
        );
        if entries.is_empty() {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = match kind {
                Section::Unstaged => "Añadir todo",
                Section::Staged => "Quitar todo",
                Section::Conflicted => return,
            };
            if small_button(ui, text).clicked() {
                *action = Some(if staged {
                    StatusAction::UnstageAll
                } else {
                    StatusAction::StageAll
                });
            }
        });
    });
    if entries.is_empty() {
        ui.label(RichText::new("Nada").color(theme::colors().text_muted));
    }
    ui.spacing_mut().item_spacing.y = 2.0;
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
        let label = match &entry.orig_path {
            Some(orig) => format!("{orig} → {}", entry.path),
            None => entry.path.clone(),
        };
        let resp = file_row(
            ui,
            entry.kind.letter(),
            kind_color(entry.kind),
            &label,
            Trailing::Space(BUTTON_SPACE + DISCARD_SPACE),
            selected == Some(&file),
        )
        .on_hover_text("Doble clic para moverlo de lista");

        // Botón encima del hueco reservado a la derecha de la fila.
        let (text, hint) = match kind {
            Section::Staged => ("Quitar", "Sacar del próximo commit"),
            Section::Unstaged => ("Añadir", "Añadir al próximo commit"),
            Section::Conflicted => ("Resuelto", "Marcar el conflicto como resuelto"),
        };
        let rect = resp.rect;
        let button_rect = Rect::from_min_size(
            egui::pos2(rect.right() - BUTTON_SPACE + 4.0, rect.center().y - 12.0),
            vec2(BUTTON_SPACE - 14.0, 24.0),
        );
        let button = ui
            .put(
                button_rect,
                egui::Button::new(RichText::new(text).size(12.0)).corner_radius(6),
            )
            .on_hover_text(hint);

        // En un conflicto no hay un único estado al que volver.
        let discard = kind != Section::Conflicted
            && ui
                .put(
                    Rect::from_center_size(
                        egui::pos2(button_rect.left() - DISCARD_SPACE / 2.0, rect.center().y),
                        vec2(24.0, 24.0),
                    ),
                    |ui: &mut Ui| {
                        icons::icon_button(ui, Icon::Trash, 24.0, 14.0, theme::colors().text_muted)
                    },
                )
                .on_hover_text("Descartar los cambios…")
                .clicked();

        if discard {
            *action = Some(StatusAction::Discard {
                entry: entry.clone(),
                staged,
            });
        } else if button.clicked() || resp.double_clicked() {
            *action = Some(toggle());
        } else if resp.clicked() {
            *action = Some(StatusAction::Select(file.clone()));
        }
    }
}

fn small_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(12.0)).corner_radius(6))
}

pub fn kind_color(kind: ChangeKind) -> Color32 {
    match kind {
        ChangeKind::Added | ChangeKind::Untracked => theme::colors().green,
        ChangeKind::Deleted | ChangeKind::Conflicted => theme::colors().red,
        ChangeKind::Modified | ChangeKind::TypeChanged => theme::colors().yellow,
        ChangeKind::Renamed | ChangeKind::Copied => theme::colors().blue,
    }
}
