//! Diálogo para crear, editar y borrar perfiles de identidad.

use eframe::egui::{self, RichText, TextEdit};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::git::{Profile, Service, SigningFormat};

#[derive(Default)]
pub struct ProfilesDialog {
    /// Perfil del formulario.
    draft: Profile,
    /// Posición del perfil que se edita; `None` si es uno nuevo.
    editing: Option<usize>,
    error: Option<String>,
}

pub enum ProfilesAction {
    /// Guardar `profile` en la posición `index`, o añadirlo si es `None`.
    Save {
        index: Option<usize>,
        profile: Profile,
    },
    Delete(usize),
    Close,
}

impl ProfilesDialog {
    pub fn show(&mut self, ctx: &egui::Context, profiles: &[Profile]) -> Option<ProfilesAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("profiles_dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.label(
                RichText::new("Perfiles de identidad")
                    .font(theme::font_semibold(16.0))
                    .color(theme::colors().text),
            );
            ui.add_space(4.0);
            muted(
                ui,
                "Nombre, correo y firma con que se crean los commits. Al elegir un perfil en un \
                 repositorio se guarda en su .git/config, así git usa lo mismo desde la terminal.",
            );
            ui.add_space(8.0);
            if let Some(a) = self.list(ui, profiles) {
                action = Some(a);
            }
            ui.separator();
            if let Some(a) = self.form(ui) {
                action = Some(a);
            }
        });
        if modal.should_close() && action.is_none() {
            action = Some(ProfilesAction::Close);
        }
        action
    }

    /// Perfiles guardados con sus botones de editar y borrar.
    fn list(&mut self, ui: &mut egui::Ui, profiles: &[Profile]) -> Option<ProfilesAction> {
        let mut action = None;
        if profiles.is_empty() {
            muted(ui, "Todavía no hay perfiles.");
        }
        for (i, profile) in profiles.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(profile.label()).color(theme::colors().text));
                    let mut details = Vec::new();
                    if profile.sign {
                        details.push(format!("firma {}", profile.format.label()));
                    }
                    if let Some(service) = profile.service {
                        details.push(format!("para {}", service.label()));
                    }
                    if !details.is_empty() {
                        muted(ui, &details.join(" · "));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icons::button(ui, Icon::Trash, "Borrar", ButtonStyle::Ghost, true).clicked()
                    {
                        action = Some(ProfilesAction::Delete(i));
                        if self.editing == Some(i) {
                            *self = Self::default();
                        }
                    }
                    let editing = self.editing == Some(i);
                    if icons::button(ui, Icon::Sliders, "Editar", ButtonStyle::Ghost, !editing)
                        .clicked()
                    {
                        self.draft = profile.clone();
                        self.editing = Some(i);
                        self.error = None;
                    }
                });
            });
        }
        // Al borrar uno anterior, el que se edita cambia de posición.
        if let (Some(ProfilesAction::Delete(deleted)), Some(editing)) = (&action, self.editing)
            && *deleted < editing
        {
            self.editing = Some(editing - 1);
        }
        action
    }

    fn form(&mut self, ui: &mut egui::Ui) -> Option<ProfilesAction> {
        let mut action = None;
        let title = if self.editing.is_some() {
            "Editar perfil"
        } else {
            "Nuevo perfil"
        };
        ui.label(RichText::new(title).font(theme::font_semibold(13.0)));
        let d = &mut self.draft;
        let mut changed = false;
        egui::Grid::new("profile_form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Nombre");
                changed |= field(ui, &mut d.name, "Nombre Apellido");
                ui.end_row();
                ui.label("Correo");
                changed |= field(ui, &mut d.email, "correo@ejemplo.com");
                ui.end_row();
                ui.label("Usar en");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.service, None, "Ninguno");
                    for service in Service::ALL {
                        ui.radio_value(&mut d.service, Some(service), service.label());
                    }
                })
                .response
                .on_hover_text(
                    "Si un repositorio está en este servicio y usa otra identidad, el editor \
                     de commits avisa y ofrece cambiar a este perfil",
                );
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut d.sign, "Firmar los commits");
                ui.end_row();
                if d.sign {
                    ui.label("Formato");
                    ui.horizontal(|ui| {
                        for format in SigningFormat::ALL {
                            ui.radio_value(&mut d.format, format, format.label());
                        }
                    });
                    ui.end_row();
                    ui.label("Clave");
                    let hint = match d.format {
                        SigningFormat::Ssh => "~/.ssh/id_ed25519.pub",
                        _ => "Id de la clave (vacío: la del correo)",
                    };
                    changed |= field(ui, &mut d.signing_key, hint);
                    ui.end_row();
                }
            });
        if changed {
            self.error = None;
        }
        muted(
            ui,
            "Para que GitHub o GitLab muestren los commits firmados como verificados, el \
             correo debe estar verificado en tu cuenta y la clave subida a ella.",
        );
        if let Some(error) = &self.error {
            ui.colored_label(theme::colors().red, error);
        }
        ui.add_space(4.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if self.editing.is_some() {
                "Guardar"
            } else {
                "Añadir perfil"
            };
            if icons::button(ui, Icon::Check, label, ButtonStyle::Primary, true).clicked() {
                match self.draft.problem() {
                    Some(problem) => self.error = Some(problem.to_owned()),
                    None => {
                        action = Some(ProfilesAction::Save {
                            index: self.editing,
                            profile: self.draft.clone(),
                        });
                        *self = Self::default();
                    }
                }
            }
            if icons::button(ui, Icon::Close, "Cerrar", ButtonStyle::Secondary, true).clicked() {
                action = Some(ProfilesAction::Close);
            }
            if self.editing.is_some() && ui.button("Cancelar edición").clicked() {
                *self = Self::default();
            }
        });
        action
    }
}

fn field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> bool {
    ui.add(
        TextEdit::singleline(value)
            .hint_text(hint)
            .margin(egui::vec2(10.0, 6.0))
            .desired_width(360.0),
    )
    .changed()
}

fn muted(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .color(theme::colors().text_muted),
    );
}
