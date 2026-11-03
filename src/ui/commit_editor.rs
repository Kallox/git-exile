//! Editor del mensaje de commit.

use eframe::egui::{self, Key, Modifiers, RichText, TextEdit, Ui};

const SUMMARY_SOFT_LIMIT: usize = 50;
const SUMMARY_HARD_LIMIT: usize = 72;

#[derive(Default)]
pub struct CommitEditor {
    pub summary: String,
    pub body: String,
    pub amend: bool,
    /// Hay un commit en marcha: se desactiva el botón.
    pub committing: bool,
}

pub enum EditorAction {
    Commit {
        message: String,
        amend: bool,
    },
    /// Se activó "amend" con el editor vacío: rellenar con el último mensaje.
    LoadLastMessage,
}

impl CommitEditor {
    pub fn clear(&mut self) {
        self.summary.clear();
        self.body.clear();
        self.amend = false;
    }

    /// Rellena el editor con un mensaje completo (resumen + cuerpo).
    pub fn set_message(&mut self, message: &str) {
        let (summary, body) = message.split_once('\n').unwrap_or((message, ""));
        self.summary = summary.trim().to_owned();
        self.body = body.trim().to_owned();
    }

    fn message(&self) -> String {
        let summary = self.summary.trim();
        let body = self.body.trim_end();
        if body.trim().is_empty() {
            format!("{summary}\n")
        } else {
            format!("{summary}\n\n{body}\n")
        }
    }

    /// `staged` es el número de archivos preparados; `author` la identidad de git.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        staged: usize,
        author: Option<&str>,
    ) -> Option<EditorAction> {
        let mut action = None;
        // Se consume antes de dibujar los campos para que no inserte un salto de línea.
        let shortcut = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));

        ui.horizontal(|ui| {
            ui.strong("Nuevo commit");
            if let Some(author) = author {
                ui.weak(format!("como {author}"));
            }
        });

        ui.horizontal(|ui| {
            let count = self.summary.chars().count();
            let color = if count > SUMMARY_HARD_LIMIT {
                ui.visuals().error_fg_color
            } else if count > SUMMARY_SOFT_LIMIT {
                ui.visuals().warn_fg_color
            } else {
                ui.visuals().weak_text_color()
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(count.to_string()).color(color).monospace())
                    .on_hover_text(
                        "Se recomienda que el resumen no pase de 50 caracteres (72 como máximo).",
                    );
                ui.add(
                    TextEdit::singleline(&mut self.summary)
                        .hint_text("Resumen (obligatorio)")
                        .desired_width(f32::INFINITY),
                );
            });
        });
        ui.add(
            TextEdit::multiline(&mut self.body)
                .hint_text("Descripción (opcional)")
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );

        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.amend, "Amend")
                .on_hover_text("Modificar el último commit en lugar de crear uno nuevo")
                .changed()
                && self.amend
                && self.summary.trim().is_empty()
                && self.body.trim().is_empty()
            {
                action = Some(EditorAction::LoadLastMessage);
            }

            let has_message = !self.summary.trim().is_empty();
            let can_commit = has_message && (staged > 0 || self.amend) && !self.committing;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.amend { "Amend" } else { "Commit" };
                let hint = if !has_message {
                    "Escribe un resumen".to_owned()
                } else if staged == 0 && !self.amend {
                    "No hay cambios preparados".to_owned()
                } else {
                    format!("{label} de {staged} archivos (Ctrl+Enter)")
                };
                let button = ui
                    .add_enabled(can_commit, egui::Button::new(RichText::new(label).strong()))
                    .on_hover_text(&hint)
                    .on_disabled_hover_text(&hint);
                if can_commit && (button.clicked() || shortcut) {
                    action = Some(EditorAction::Commit {
                        message: self.message(),
                        amend: self.amend,
                    });
                }
                if self.committing {
                    ui.spinner();
                }
            });
        });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_message_from_fields() {
        let mut e = CommitEditor {
            summary: "  Resumen ".into(),
            ..Default::default()
        };
        assert_eq!(e.message(), "Resumen\n");
        e.body = "Línea 1\nLínea 2\n\n".into();
        assert_eq!(e.message(), "Resumen\n\nLínea 1\nLínea 2\n");
    }

    #[test]
    fn splits_existing_message() {
        let mut e = CommitEditor::default();
        e.set_message("Título\n\nCuerpo\nmás");
        assert_eq!(e.summary, "Título");
        assert_eq!(e.body, "Cuerpo\nmás");
    }
}
