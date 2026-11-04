//! Diálogo para crear una rama nueva.

use eframe::egui::{self, Key, RichText, TextEdit};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;

pub struct NewBranchDialog {
    pub name: String,
    /// Commit de partida: un hash o `HEAD`.
    pub start: String,
    /// Descripción del commit de partida para mostrar.
    pub start_label: String,
    pub checkout: bool,
    pub error: Option<String>,
    /// Creación en marcha.
    pub busy: bool,
    focus_requested: bool,
}

pub enum DialogAction {
    Create {
        name: String,
        start: String,
        checkout: bool,
    },
    Cancel,
}

impl NewBranchDialog {
    pub fn new(start: String, start_label: String) -> Self {
        Self {
            name: String::new(),
            start,
            start_label,
            checkout: true,
            error: None,
            busy: false,
            focus_requested: false,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<DialogAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("new_branch_dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new("Nueva rama")
                    .font(theme::font_semibold(16.0))
                    .color(theme::colors().text),
            );
            ui.add_space(8.0);

            let label = |ui: &mut egui::Ui, text: &str| {
                ui.label(
                    RichText::new(text)
                        .size(12.0)
                        .color(theme::colors().text_muted),
                );
            };
            label(ui, "Desde");
            egui::Frame::new()
                .fill(theme::colors().bg_code)
                .stroke(egui::Stroke::new(1.0, theme::colors().border))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(&self.start_label)
                            .font(theme::mono(12.0))
                            .color(theme::colors().text_soft),
                    );
                });
            ui.add_space(8.0);

            label(ui, "Nombre");
            let edit = ui.add(
                TextEdit::singleline(&mut self.name)
                    .hint_text("p. ej. feature/nueva-funcion")
                    .margin(egui::vec2(10.0, 8.0))
                    .desired_width(f32::INFINITY),
            );
            if !self.focus_requested {
                edit.request_focus();
                self.focus_requested = true;
            }
            if edit.changed() {
                self.error = None;
            }
            ui.checkbox(&mut self.checkout, "Cambiar a la nueva rama");

            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(theme::colors().red, error);
            }

            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_create = !self.name.trim().is_empty() && !self.busy;
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if icons::button(
                    ui,
                    Icon::Branch,
                    "Crear rama",
                    ButtonStyle::Primary,
                    can_create,
                )
                .clicked()
                    || (can_create && enter)
                {
                    action = Some(DialogAction::Create {
                        name: self.name.trim().to_owned(),
                        start: self.start.clone(),
                        checkout: self.checkout,
                    });
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(DialogAction::Cancel);
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() && !self.busy {
            action = Some(DialogAction::Cancel);
        }
        action
    }
}
