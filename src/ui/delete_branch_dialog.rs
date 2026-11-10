//! Confirmación antes de borrar una rama local.

use eframe::egui::{self, RichText};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;

pub struct DeleteBranchDialog {
    pub name: String,
    /// Commits que solo están en esta rama; `None` mientras se calculan.
    pub unmerged: Option<usize>,
    pub error: Option<String>,
    /// Borrado en marcha.
    pub busy: bool,
}

pub enum DeleteBranchAction {
    Confirm,
    Cancel,
}

impl DeleteBranchDialog {
    pub fn new(name: String) -> Self {
        Self {
            name,
            unmerged: None,
            error: None,
            busy: false,
        }
    }

    /// Qué pasará con los commits de la rama si se confirma.
    fn description(&self) -> Option<String> {
        match self.unmerged? {
            0 => Some(
                "Todos sus commits están en otras ramas: no se pierde nada. \
                 La rama remota, si existe, no se toca."
                    .to_owned(),
            ),
            1 => Some("1 commit solo está en esta rama y se perderá.".to_owned()),
            n => Some(format!(
                "{n} commits solo están en esta rama y se perderán."
            )),
        }
    }

    pub fn show(&self, ctx: &egui::Context) -> Option<DeleteBranchAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("delete_branch_dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new("Borrar rama local")
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
                        RichText::new(&self.name)
                            .font(theme::mono(12.0))
                            .color(theme::colors().text_soft),
                    );
                });
            ui.add_space(8.0);
            match self.description() {
                Some(text) => {
                    ui.label(text);
                }
                None if self.error.is_none() => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Comprobando sus commits…");
                    });
                }
                None => {}
            }
            if self.unmerged.is_some_and(|n| n > 0) {
                ui.colored_label(theme::colors().red, "Esta acción no se puede deshacer.");
            }
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(theme::colors().red, error);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Sin atajo de teclado: se confirma solo con un clic deliberado.
                let ready = self.unmerged.is_some() && !self.busy;
                if icons::button(ui, Icon::Trash, "Borrar", ButtonStyle::Primary, ready).clicked() {
                    action = Some(DeleteBranchAction::Confirm);
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(DeleteBranchAction::Cancel);
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() && !self.busy {
            action = Some(DeleteBranchAction::Cancel);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_what_is_lost() {
        let text = |unmerged| {
            DeleteBranchDialog {
                unmerged,
                ..DeleteBranchDialog::new("x".into())
            }
            .description()
        };
        assert_eq!(text(None), None);
        assert!(text(Some(0)).unwrap().contains("no se pierde nada"));
        assert!(text(Some(1)).unwrap().starts_with("1 commit "));
        assert!(text(Some(3)).unwrap().contains("3 commits"));
    }
}
