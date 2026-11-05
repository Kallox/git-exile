//! Aviso de versión nueva y progreso de la actualización.

use eframe::egui;

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::update::{self, State, Updater};

/// `busy`: hay operaciones en curso que un reinicio interrumpiría.
pub fn show(ctx: &egui::Context, updater: &mut Updater, busy: bool) {
    let visible = match updater.state() {
        State::Idle | State::Checking { manual: false } => false,
        State::Installed { .. } => !updater.is_hidden(),
        _ => true,
    };
    if !visible {
        return;
    }

    let failed = matches!(updater.state(), State::Failed { .. });
    let c = theme::colors();
    let tint = if failed { c.red } else { c.accent };
    egui::TopBottomPanel::top("update_bar")
        .frame(
            egui::Frame::new()
                .fill(theme::alpha(tint, 0.10))
                .inner_margin(egui::Margin::symmetric(20, 8)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| content(ui, updater, busy));
        });
}

fn content(ui: &mut egui::Ui, updater: &mut Updater, busy: bool) {
    let c = theme::colors();
    let mut install = false;
    let mut restart = false;
    let mut dismiss = false;

    let (text, color, link) = match updater.state() {
        State::Idle | State::Checking { manual: false } => return,
        State::Checking { manual: true } => {
            ui.spinner();
            ("Buscando actualizaciones…".to_owned(), c.text, None)
        }
        State::UpToDate => (
            format!(
                "Ya tienes la última versión (v{}).",
                update::CURRENT_VERSION
            ),
            c.text,
            None,
        ),
        State::Available { version } => (
            format!(
                "Hay una versión nueva: v{version} (tienes v{}).",
                update::CURRENT_VERSION
            ),
            c.text,
            Some(("Ver novedades", update::release_url(version))),
        ),
        State::Installing { version } => {
            ui.spinner();
            (
                format!("Descargando e instalando v{version}…"),
                c.text,
                None,
            )
        }
        State::Installed { version, .. } => (
            format!("v{version} instalada. Reinicia la aplicación para usarla."),
            c.text,
            None,
        ),
        State::Failed { message, version } => (
            message.clone(),
            c.red,
            version
                .as_deref()
                .map(|v| ("Descargar desde GitHub", update::release_url(v))),
        ),
    };
    ui.colored_label(color, text);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if !matches!(updater.state(), State::Installing { .. }) {
            dismiss = icons::icon_button(ui, Icon::Close, 24.0, 12.0, c.text_muted)
                .on_hover_text("Cerrar")
                .clicked();
        }
        match updater.state() {
            State::Available { .. } => {
                install =
                    icons::button(ui, Icon::ArrowUp, "Actualizar", ButtonStyle::Primary, true)
                        .clicked();
            }
            State::Installed { .. } => {
                restart =
                    icons::button(ui, Icon::Refresh, "Reiniciar", ButtonStyle::Primary, !busy)
                        .on_disabled_hover_text("Espera a que terminen las operaciones en curso")
                        .clicked();
            }
            _ => {}
        }
        if let Some((label, url)) = link {
            ui.hyperlink_to(label, url);
        }
    });

    if install {
        updater.install();
    } else if restart {
        updater.restart(ui.ctx());
    } else if dismiss {
        updater.dismiss();
    }
}
