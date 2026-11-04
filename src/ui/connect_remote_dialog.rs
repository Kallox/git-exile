//! Diálogo para conectar un repositorio local a un remoto.

use eframe::egui::{self, Key, RichText, TextEdit};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;

#[derive(Default)]
pub struct ConnectRemoteDialog {
    url: String,
    pub error: Option<String>,
    /// Conexión en marcha.
    pub busy: bool,
    focus_requested: bool,
}

pub enum ConnectAction {
    Connect(String),
    Cancel,
}

impl ConnectRemoteDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> Option<ConnectAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("connect_remote_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(
                RichText::new("Conectar remoto")
                    .font(theme::font_semibold(16.0))
                    .color(theme::colors().text),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Crea un repositorio vacío en GitHub, GitLab u otro servidor y pega aquí su \
                     dirección. Se añadirá como origin; nada se sube hasta que pulses Push.",
                )
                .size(12.0)
                .color(theme::colors().text_muted),
            );
            ui.add_space(8.0);
            let edit = ui.add(
                TextEdit::singleline(&mut self.url)
                    .hint_text("git@github.com:usuario/repositorio.git")
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
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(theme::colors().red, error);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_connect = !self.url.trim().is_empty() && !self.busy;
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if icons::button(
                    ui,
                    Icon::Cloud,
                    "Conectar",
                    ButtonStyle::Primary,
                    can_connect,
                )
                .clicked()
                    || (can_connect && enter)
                {
                    action = Some(ConnectAction::Connect(self.url.clone()));
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(ConnectAction::Cancel);
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() && !self.busy {
            action = Some(ConnectAction::Cancel);
        }
        action
    }
}
