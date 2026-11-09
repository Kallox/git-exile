//! Editor del mensaje de commit.

use eframe::egui::{self, Key, Modifiers, RichText, TextEdit, Ui};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::git::{Identity, Profile};

const SUMMARY_SOFT_LIMIT: usize = 50;
const SUMMARY_HARD_LIMIT: usize = 72;

#[derive(Default)]
pub struct CommitEditor {
    pub summary: String,
    pub body: String,
    pub amend: bool,
    /// Con amend, poner como autor la identidad actual.
    pub reset_author: bool,
    /// Hay un commit en marcha: se desactiva el botón.
    pub committing: bool,
}

pub enum EditorAction {
    Commit {
        message: String,
        amend: bool,
        reset_author: bool,
    },
    /// Se activó "amend" con el editor vacío: rellenar con el último mensaje.
    LoadLastMessage,
    /// Aplicar al repositorio el perfil con esta posición.
    UseProfile(usize),
    /// Quitar la identidad del repositorio para usar la global.
    UseGlobalIdentity,
    ManageProfiles,
}

/// Lo que el editor necesita saber del repositorio para dibujarse.
pub struct EditorContext<'a> {
    /// Número de archivos preparados.
    pub staged: usize,
    /// `None` mientras se lee.
    pub identity: Option<&'a Identity>,
    pub profiles: &'a [Profile],
    /// Perfil asociado a los remotos del repositorio, con su posición en `profiles`.
    pub suggested: Option<(usize, &'a Profile)>,
    /// Aviso de que se aplicó un perfil al abrir el repositorio.
    pub notice: Option<&'a str>,
}

impl CommitEditor {
    pub fn clear(&mut self) {
        self.summary.clear();
        self.body.clear();
        self.amend = false;
        self.reset_author = false;
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

    pub fn show(&mut self, ui: &mut Ui, cx: EditorContext) -> Option<EditorAction> {
        let mut action = None;
        // Se consume antes de dibujar los campos para que no inserte un salto de línea.
        let shortcut = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));

        egui::Frame::new()
            .fill(theme::colors().bg_card)
            .stroke(egui::Stroke::new(1.0, theme::colors().border))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(16, 14))
            .show(ui, |ui| {
                action = self.form(ui, &cx, shortcut);
            });
        action
    }

    fn form(&mut self, ui: &mut Ui, cx: &EditorContext, shortcut: bool) -> Option<EditorAction> {
        let staged = cx.staged;
        let mut action = None;
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Nuevo commit").font(theme::font_semibold(13.0)));
            if let Some(chosen) = identity_header(ui, cx) {
                action = Some(chosen);
            }
        });
        if let Some(warning) = identity_warning(ui, cx) {
            action = Some(warning);
        }
        if let Some(notice) = cx.notice {
            ui.label(
                RichText::new(notice)
                    .size(12.0)
                    .color(theme::colors().text_muted),
            );
        }

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
                        .margin(egui::vec2(10.0, 7.0))
                        .desired_width(f32::INFINITY),
                );
            });
        });
        ui.add(
            TextEdit::multiline(&mut self.body)
                .hint_text("Descripción (opcional)")
                .margin(egui::vec2(10.0, 7.0))
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
            if self.amend {
                ui.checkbox(&mut self.reset_author, "Actualizar autor")
                    .on_hover_text(
                        "Poner como autor la identidad actual. Sin esto, el amend conserva \
                         el autor original del commit.",
                    );
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
                let button =
                    icons::button(ui, Icon::Check, label, ButtonStyle::Primary, can_commit)
                        .on_hover_text(&hint);
                if can_commit && (button.clicked() || shortcut) {
                    action = Some(EditorAction::Commit {
                        message: self.message(),
                        amend: self.amend,
                        reset_author: self.amend && self.reset_author,
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

/// "como Nombre <correo>", de dónde sale y el menú para cambiarla.
fn identity_header(ui: &mut Ui, cx: &EditorContext) -> Option<EditorAction> {
    let identity = cx.identity?;
    let muted = theme::colors().text_muted;
    match &identity.ident {
        Some(ident) => {
            let mut hint = format!("Identidad tomada de: {}", identity.scope_label());
            if let Some(origin) = &identity.origin {
                hint.push_str(&format!("\n{origin}"));
            }
            hint.push_str(&match &identity.signing {
                Some(signing) => format!("\nFirma: {signing}"),
                None => "\nLos commits no se firman".to_owned(),
            });
            ui.label(
                RichText::new(format!("como {ident}"))
                    .size(12.0)
                    .color(muted),
            )
            .on_hover_text(hint);
            let scope = if identity.signing.is_some() {
                format!("{} · firmado", identity.scope_label())
            } else {
                identity.scope_label().to_owned()
            };
            ui.label(
                RichText::new(scope)
                    .size(11.0)
                    .color(theme::colors().text_secondary)
                    .background_color(theme::colors().bg_input),
            );
        }
        None => {
            ui.label(
                RichText::new("sin identidad: falta user.name o user.email")
                    .size(12.0)
                    .color(ui.visuals().warn_fg_color),
            );
        }
    }

    let mut action = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let button = ui
            .add(egui::Button::new(RichText::new("Cambiar ▾").size(12.0)).frame(false))
            .on_hover_text("Elegir con qué identidad se crean los commits en este repositorio");
        egui::Popup::menu(&button).show(|ui| {
            ui.set_min_width(300.0);
            ui.label(
                RichText::new("PERFILES")
                    .font(theme::font_semibold(11.0))
                    .color(muted),
            );
            if cx.profiles.is_empty() {
                ui.weak("Todavía no hay perfiles");
            }
            for (i, profile) in cx.profiles.iter().enumerate() {
                let current = identity.local && profile.is_active(identity);
                let mut text = profile.label();
                if profile.sign {
                    text.push_str(&format!(" · {}", profile.format.label()));
                }
                if ui
                    .selectable_label(current, text)
                    .on_hover_text("Se guarda en .git/config: también lo usa git desde la terminal")
                    .clicked()
                {
                    action = Some(EditorAction::UseProfile(i));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .add_enabled(
                    identity.local,
                    egui::Button::new("Usar la identidad global"),
                )
                .on_hover_text(
                    "Quita nombre, correo y firma de .git/config para volver a la \
                     configuración global (y sus reglas includeIf). A este repositorio \
                     no se le volverá a aplicar un perfil automáticamente.",
                )
                .on_disabled_hover_text("Este repositorio ya usa la identidad global")
                .clicked()
            {
                action = Some(EditorAction::UseGlobalIdentity);
                ui.close();
            }
            if ui.button("Administrar perfiles…").clicked() {
                action = Some(EditorAction::ManageProfiles);
                ui.close();
            }
        });
    });
    action
}

/// Aviso si los remotos piden un perfil distinto de la identidad actual.
fn identity_warning(ui: &mut Ui, cx: &EditorContext) -> Option<EditorAction> {
    let (index, profile) = cx.suggested?;
    let identity = cx.identity?;
    // Con la identidad global elegida a propósito no se insiste.
    if profile.is_active(identity) || !identity.auto_profile {
        return None;
    }
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "Los remotos de este repositorio corresponden al perfil {}",
                profile.label()
            ))
            .size(12.0)
            .color(ui.visuals().warn_fg_color),
        );
        if ui.small_button("Usar este perfil").clicked() {
            action = Some(EditorAction::UseProfile(index));
        }
    });
    action
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
