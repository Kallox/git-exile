//! Confirmación antes de resetear la rama actual a un commit, eligiendo el
//! modo: soft (conservar los cambios) o hard (descartarlos).

use eframe::egui::{self, RichText};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::git::{ResetMode, ResetPreview};

pub struct ResetDialog {
    /// Hash completo del commit destino.
    pub target: String,
    /// Hash corto y asunto del commit destino, para mostrarlo.
    pub target_label: String,
    pub mode: ResetMode,
    /// Archivos con cambios sin commit (sin contar los nuevos sin seguimiento).
    pub changes: usize,
    /// `None` mientras se calcula.
    pub preview: Option<ResetPreview>,
    pub error: Option<String>,
    /// Reset en marcha.
    pub busy: bool,
}

pub enum ResetAction {
    Confirm,
    Cancel,
}

fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".to_owned()
    } else {
        format!("{n} commits")
    }
}

impl ResetDialog {
    pub fn new(target: String, target_label: String, changes: usize) -> Self {
        Self {
            target,
            target_label,
            // Por defecto el modo que no pierde nada.
            mode: ResetMode::Soft,
            changes,
            preview: None,
            error: None,
            busy: false,
        }
    }

    /// Qué pasará con los commits y los cambios si se confirma.
    fn description(&self) -> Option<Vec<String>> {
        let p = self.preview.as_ref()?;
        let mut lines = Vec::new();
        lines.push(match p.leaving {
            0 => format!("\"{}\" pasará a apuntar a este commit.", p.branch),
            n => format!("{} saldrán de \"{}\".", commits(n), p.branch),
        });
        match self.mode {
            ResetMode::Soft => {
                if p.leaving > 0 {
                    lines.push(
                        "Sus cambios quedarán añadidos, listos para un nuevo commit.".to_owned(),
                    );
                }
                lines.push("Tus cambios sin commit se conservan.".to_owned());
            }
            ResetMode::Hard => {
                match p.lost {
                    0 => {}
                    1 => {
                        lines.push("1 commit no está en ninguna otra rama y se perderá.".to_owned())
                    }
                    n => lines.push(format!(
                        "{n} commits no están en ninguna otra rama y se perderán."
                    )),
                }
                lines.push(match self.changes {
                    0 => "No hay cambios sin commit que perder.".to_owned(),
                    1 => "Se descartarán los cambios sin commit de 1 archivo.".to_owned(),
                    n => format!("Se descartarán los cambios sin commit de {n} archivos."),
                });
                lines.push("Los archivos nuevos sin seguimiento no se tocan.".to_owned());
            }
        }
        Some(lines)
    }

    /// El reset hard pierde algo que no se puede recuperar desde la aplicación.
    fn destructive(&self) -> bool {
        self.mode == ResetMode::Hard
            && (self.changes > 0 || self.preview.as_ref().is_some_and(|p| p.lost > 0))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<ResetAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("reset_dialog")).show(ctx, |ui| {
            ui.set_width(440.0);
            let title = match &self.preview {
                Some(p) => format!("Reset de {}", p.branch),
                None => "Reset de la rama actual".to_owned(),
            };
            ui.label(
                RichText::new(title)
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
                        RichText::new(&self.target_label)
                            .font(theme::mono(12.0))
                            .color(theme::colors().text_soft),
                    );
                });
            ui.add_space(8.0);
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.radio_value(
                    &mut self.mode,
                    ResetMode::Soft,
                    "Soft: mantener todos los cambios",
                );
                ui.radio_value(
                    &mut self.mode,
                    ResetMode::Hard,
                    "Hard: descartar todos los cambios",
                );
            });
            ui.add_space(8.0);
            match self.description() {
                Some(lines) => {
                    for line in lines {
                        ui.label(line);
                    }
                }
                None if self.error.is_none() => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Comprobando los commits…");
                    });
                }
                None => {}
            }
            if self.destructive() {
                ui.colored_label(theme::colors().red, "Esta acción no se puede deshacer.");
            }
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(theme::colors().red, error);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Sin atajo de teclado: se confirma solo con un clic deliberado.
                let ready = self.preview.is_some() && !self.busy;
                let (icon, label) = match self.mode {
                    ResetMode::Soft => (Icon::Refresh, "Reset soft"),
                    ResetMode::Hard => (Icon::Trash, "Reset hard"),
                };
                if icons::button(ui, icon, label, ButtonStyle::Primary, ready).clicked() {
                    action = Some(ResetAction::Confirm);
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(ResetAction::Cancel);
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() && !self.busy {
            action = Some(ResetAction::Cancel);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(mode: ResetMode, changes: usize, leaving: usize, lost: usize) -> ResetDialog {
        ResetDialog {
            mode,
            preview: Some(ResetPreview {
                branch: "main".into(),
                leaving,
                lost,
            }),
            ..ResetDialog::new("abc".into(), "abc first".into(), changes)
        }
    }

    #[test]
    fn soft_never_loses_anything() {
        let d = dialog(ResetMode::Soft, 3, 2, 2);
        assert!(!d.destructive());
        let text = d.description().unwrap().join(" ");
        assert!(text.contains("2 commits saldrán"));
        assert!(text.contains("se conservan"));
    }

    #[test]
    fn hard_warns_about_lost_commits_and_changes() {
        assert!(!dialog(ResetMode::Hard, 0, 2, 0).destructive());
        assert!(dialog(ResetMode::Hard, 1, 0, 0).destructive());
        let d = dialog(ResetMode::Hard, 2, 3, 1);
        assert!(d.destructive());
        let text = d.description().unwrap().join(" ");
        assert!(text.contains("1 commit no está en"));
        assert!(text.contains("de 2 archivos"));
    }

    #[test]
    fn no_description_until_preview() {
        let d = ResetDialog::new("abc".into(), "abc".into(), 0);
        assert!(d.description().is_none());
    }
}
