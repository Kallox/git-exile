//! Confirmación antes de descartar los cambios de un archivo.

use eframe::egui::{self, RichText};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::git::{ChangeKind, StatusEntry};

pub struct DiscardDialog {
    pub entry: StatusEntry,
    pub staged: bool,
}

pub enum DiscardAction {
    Confirm,
    Cancel,
}

impl DiscardDialog {
    /// Qué le pasará al archivo si se confirma.
    fn description(&self) -> String {
        let what = match self.entry.kind {
            ChangeKind::Untracked | ChangeKind::Added | ChangeKind::Copied => {
                "Este archivo es nuevo: se borrará del disco."
            }
            ChangeKind::Deleted => "El archivo borrado se restaurará.",
            ChangeKind::Renamed => "El archivo volverá a su nombre original y a su contenido.",
            _ => "Se desharán los cambios de este archivo.",
        };
        if self.staged {
            format!("{what} Se incluyen sus cambios sin añadir, si los tiene.")
        } else {
            what.to_owned()
        }
    }

    pub fn show(&self, ctx: &egui::Context) -> Option<DiscardAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("discard_dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new("Descartar cambios")
                    .font(theme::font_semibold(16.0))
                    .color(theme::colors().text),
            );
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(theme::colors().bg_code)
                .stroke(egui::Stroke::new(1.0, theme::colors().border))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(&self.entry.path)
                            .font(theme::mono(12.0))
                            .color(theme::colors().text_soft),
                    );
                });
            ui.add_space(8.0);
            ui.label(self.description());
            ui.colored_label(theme::colors().red, "Esta acción no se puede deshacer.");
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Sin atajo de teclado: se confirma solo con un clic deliberado.
                if icons::button(ui, Icon::Trash, "Descartar", ButtonStyle::Primary, true).clicked()
                {
                    action = Some(DiscardAction::Confirm);
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(DiscardAction::Cancel);
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() {
            action = Some(DiscardAction::Cancel);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(kind: ChangeKind, staged: bool) -> DiscardDialog {
        DiscardDialog {
            entry: StatusEntry {
                path: "a.txt".into(),
                orig_path: None,
                kind,
            },
            staged,
        }
    }

    #[test]
    fn describes_what_happens_to_the_file() {
        let text = |kind, staged| dialog(kind, staged).description();
        assert!(text(ChangeKind::Untracked, false).contains("borrará"));
        assert!(text(ChangeKind::Added, true).contains("borrará"));
        assert!(text(ChangeKind::Deleted, false).contains("restaurará"));
        assert!(text(ChangeKind::Modified, false).contains("desharán"));
        assert!(!text(ChangeKind::Modified, false).contains("sin añadir"));
        assert!(text(ChangeKind::Modified, true).contains("sin añadir"));
    }
}
