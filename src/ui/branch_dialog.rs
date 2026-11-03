//! Diálogo para crear una rama nueva.

use eframe::egui::{self, Key, RichText, TextEdit};

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
            ui.heading("Nueva rama");
            ui.add_space(8.0);

            ui.label("Desde:");
            ui.label(RichText::new(&self.start_label).monospace());
            ui.add_space(8.0);

            ui.label("Nombre:");
            let edit = ui.add(
                TextEdit::singleline(&mut self.name)
                    .hint_text("p. ej. feature/nueva-funcion")
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
                ui.colored_label(ui.visuals().error_fg_color, error);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let can_create = !self.name.trim().is_empty() && !self.busy;
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if ui
                    .add_enabled(
                        can_create,
                        egui::Button::new(RichText::new("Crear").strong()),
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
                if ui.button("Cancelar").clicked() {
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
