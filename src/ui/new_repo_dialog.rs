//! Diálogo para clonar un repositorio o crear uno nuevo.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, RichText, TextEdit};

use super::icons::{self, ButtonStyle, Icon};
use super::theme;
use crate::browser;
use crate::git::{self, NewRepo};

pub struct NewRepoDialog {
    cloning: bool,
    url: String,
    path: String,
    /// Carpeta en la que se clona mientras el destino se deduzca de la dirección.
    parent: PathBuf,
    /// El usuario escribió el destino: ya no se deduce de la dirección.
    path_edited: bool,
    pub error: Option<String>,
    focus_requested: bool,
}

pub enum NewRepoAction {
    Create(NewRepo),
    Cancel,
}

impl NewRepoDialog {
    pub fn init(path: &Path) -> Self {
        Self::new(false, String::new(), path.display().to_string(), path, None)
    }

    pub fn clone_into(parent: &Path) -> Self {
        Self::new(
            true,
            String::new(),
            parent.display().to_string(),
            parent,
            None,
        )
    }

    /// Reabre el diálogo tras un fallo para corregir los datos sin escribirlos de nuevo.
    pub fn failed(new: NewRepo, parent: &Path, error: String) -> Self {
        let path = new.path().display().to_string();
        let mut dialog = match new {
            NewRepo::Init { .. } => Self::new(false, String::new(), path, parent, Some(error)),
            NewRepo::Clone { url, .. } => Self::new(true, url, path, parent, Some(error)),
        };
        dialog.path_edited = true;
        dialog
    }

    fn new(cloning: bool, url: String, path: String, parent: &Path, error: Option<String>) -> Self {
        Self {
            cloning,
            url,
            path,
            parent: parent.to_path_buf(),
            path_edited: false,
            error,
            focus_requested: false,
        }
    }

    fn request(&self) -> NewRepo {
        let path = PathBuf::from(self.path.trim());
        if self.cloning {
            NewRepo::Clone {
                url: self.url.trim().to_owned(),
                path,
            }
        } else {
            NewRepo::Init { path }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<NewRepoAction> {
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::new("new_repo_dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            let (title, help) = if self.cloning {
                (
                    "Clonar repositorio",
                    "Descarga una copia con su historial. La aplicación usa tus credenciales de \
                     git y no puede pedir contraseñas.",
                )
            } else {
                (
                    "Nuevo repositorio",
                    "Crea un repositorio vacío. Si la carpeta no existe se creará; si tiene \
                     archivos se conservan, sin añadir.",
                )
            };
            ui.label(
                RichText::new(title)
                    .font(theme::font_semibold(16.0))
                    .color(theme::colors().text),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(help)
                    .size(12.0)
                    .color(theme::colors().text_muted),
            );
            ui.add_space(8.0);

            let label = |ui: &mut egui::Ui, text: &str| {
                ui.label(
                    RichText::new(text)
                        .size(12.0)
                        .color(theme::colors().text_muted),
                );
            };
            let mut enter = false;
            let mut first = None;
            if self.cloning {
                label(ui, "Dirección");
                let edit = ui.add(
                    TextEdit::singleline(&mut self.url)
                        .hint_text("https://github.com/usuario/repositorio.git")
                        .margin(egui::vec2(10.0, 8.0))
                        .desired_width(f32::INFINITY),
                );
                if edit.changed() {
                    self.error = None;
                    if !self.path_edited {
                        let name = git::repo_name_from_url(&self.url);
                        let path = name.map_or(self.parent.clone(), |n| self.parent.join(n));
                        self.path = path.display().to_string();
                    }
                }
                enter |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                first = Some(edit);
                ui.add_space(6.0);
            }
            label(
                ui,
                if self.cloning {
                    "Carpeta de destino"
                } else {
                    "Carpeta"
                },
            );
            let edit = ui.add(
                TextEdit::singleline(&mut self.path)
                    .font(theme::mono(12.0))
                    .margin(egui::vec2(10.0, 8.0))
                    .desired_width(f32::INFINITY),
            );
            if edit.changed() {
                self.error = None;
                self.path_edited = true;
            }
            enter |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if !self.focus_requested {
                first.as_ref().unwrap_or(&edit).request_focus();
                self.focus_requested = true;
            }

            let path = Path::new(self.path.trim());
            // Con `path` ya repositorio se rechaza al crear; aquí solo se avisa del anidado.
            if let Some(root) = browser::repo_root_of(path).filter(|root| root != path) {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Quedará anidado dentro del repositorio {}.",
                        root.display()
                    ))
                    .size(12.0)
                    .color(theme::colors().text_muted),
                );
            }
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.colored_label(theme::colors().red, error);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready =
                    !self.path.trim().is_empty() && (!self.cloning || !self.url.trim().is_empty());
                let (icon, text) = if self.cloning {
                    (Icon::Cloud, "Clonar")
                } else {
                    (Icon::Plus, "Crear repositorio")
                };
                if icons::button(ui, icon, text, ButtonStyle::Primary, ready).clicked()
                    || (ready && enter)
                {
                    action = Some(NewRepoAction::Create(self.request()));
                }
                if icons::button(ui, Icon::Close, "Cancelar", ButtonStyle::Secondary, true)
                    .clicked()
                {
                    action = Some(NewRepoAction::Cancel);
                }
            });
        });
        // Escape o clic fuera del diálogo.
        if modal.should_close() && action.is_none() {
            action = Some(NewRepoAction::Cancel);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, RawInput};

    /// Ejecuta el diálogo un frame recibiendo `events`.
    fn frame(
        ctx: &egui::Context,
        dialog: &mut NewRepoDialog,
        events: Vec<Event>,
    ) -> Option<NewRepoAction> {
        let mut action = None;
        let input = RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| action = dialog.show(ctx));
        action
    }

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        ctx
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn typing_a_url_derives_the_destination_and_enter_clones() {
        let ctx = context();
        let mut dialog = NewRepoDialog::clone_into(Path::new("/tmp/repos"));
        frame(&ctx, &mut dialog, Vec::new());
        // Se responde al foco pedido en el frame anterior.
        frame(&ctx, &mut dialog, Vec::new());
        let url = "git@github.com:usuario/proyecto.git";
        frame(&ctx, &mut dialog, vec![Event::Text(url.to_owned())]);
        // En Windows `join` usa `\`: se construye igual que en el diálogo.
        let expected = Path::new("/tmp/repos").join("proyecto");
        assert_eq!(dialog.path, expected.display().to_string());

        let action = frame(&ctx, &mut dialog, vec![key(Key::Enter)]);
        match action {
            Some(NewRepoAction::Create(new)) => assert_eq!(
                new,
                NewRepo::Clone {
                    url: url.to_owned(),
                    path: expected,
                }
            ),
            _ => panic!("Enter debería lanzar la clonación"),
        }
    }

    #[test]
    fn enter_does_nothing_without_a_url() {
        let ctx = context();
        let mut dialog = NewRepoDialog::clone_into(Path::new("/tmp/repos"));
        frame(&ctx, &mut dialog, Vec::new());
        frame(&ctx, &mut dialog, Vec::new());
        assert!(frame(&ctx, &mut dialog, vec![key(Key::Enter)]).is_none());
    }

    #[test]
    fn escape_cancels() {
        let ctx = context();
        let mut dialog = NewRepoDialog::init(Path::new("/tmp/repos"));
        frame(&ctx, &mut dialog, Vec::new());
        let action = frame(&ctx, &mut dialog, vec![key(Key::Escape)]);
        assert!(matches!(action, Some(NewRepoAction::Cancel)));
    }

    #[test]
    fn a_failed_clone_keeps_what_was_typed() {
        let new = NewRepo::Clone {
            url: "https://ejemplo.com/r.git".to_owned(),
            path: PathBuf::from("/otro/sitio"),
        };
        let mut dialog = NewRepoDialog::failed(new.clone(), Path::new("/tmp"), "fallo".to_owned());
        assert_eq!(dialog.request(), new);
        assert_eq!(dialog.error.as_deref(), Some("fallo"));

        // Corregir la dirección no pisa el destino elegido.
        let ctx = context();
        frame(&ctx, &mut dialog, Vec::new());
        frame(&ctx, &mut dialog, Vec::new());
        frame(&ctx, &mut dialog, vec![Event::Text("x".to_owned())]);
        assert_eq!(dialog.path, "/otro/sitio");
        assert_eq!(dialog.error, None);
    }
}
