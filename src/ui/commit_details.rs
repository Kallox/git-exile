use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};

use crate::git::{CommitDetails, FileDiff, FileStatus, Person};

pub enum DetailsAction {
    /// Ir a otro commit (p. ej. un padre).
    SelectCommit(String),
}

/// Metadatos del commit y lista de archivos cambiados.
pub fn show(
    ui: &mut Ui,
    details: &CommitDetails,
    selected_file: &mut Option<usize>,
) -> Option<DetailsAction> {
    let mut action = None;
    ScrollArea::vertical()
        .id_salt("commit_details")
        .auto_shrink(false)
        .show(ui, |ui| {
            let (summary, body) = details
                .message
                .split_once('\n')
                .unwrap_or((&details.message, ""));
            ui.label(RichText::new(summary).strong().size(15.0));
            let body = body.trim();
            if !body.is_empty() {
                ui.add_space(4.0);
                ui.label(body);
            }
            ui.add_space(8.0);

            egui::Grid::new("commit_meta")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.weak("Autor");
                    person(ui, &details.author);
                    ui.end_row();

                    if details.committer != details.author {
                        ui.weak("Committer");
                        person(ui, &details.committer);
                        ui.end_row();
                    }

                    ui.weak("Commit");
                    ui.horizontal(|ui| {
                        ui.monospace(&details.id);
                        if ui.small_button("Copiar").clicked() {
                            ui.ctx().copy_text(details.id.clone());
                        }
                    });
                    ui.end_row();

                    if !details.parents.is_empty() {
                        ui.weak(if details.parents.len() > 1 {
                            "Padres"
                        } else {
                            "Padre"
                        });
                        ui.horizontal(|ui| {
                            for parent in &details.parents {
                                let short = &parent[..parent.len().min(8)];
                                if ui
                                    .link(RichText::new(short).monospace())
                                    .on_hover_text("Ir a este commit")
                                    .clicked()
                                {
                                    action = Some(DetailsAction::SelectCommit(parent.clone()));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.label(format!("{} archivos cambiados", details.files.len()));
            if details.parents.len() > 1 {
                ui.weak("Merge: cambios respecto al primer padre.");
            }
            if details.truncated {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "El diff es demasiado grande y se ha cortado.",
                );
            }
            file_list(ui, &details.files, selected_file);
        });
    action
}

fn person(ui: &mut Ui, p: &Person) {
    ui.horizontal(|ui| {
        ui.label(&p.name);
        ui.weak(format!("<{}>", p.email));
        ui.weak(super::format_time(p.time));
    });
}

/// Lista seleccionable de archivos con su estado y número de líneas.
pub fn file_list(ui: &mut Ui, files: &[FileDiff], selected: &mut Option<usize>) {
    for (i, file) in files.iter().enumerate() {
        ui.horizontal(|ui| {
            status_badge(ui, file.status);
            let label = match (&file.old_path, &file.new_path) {
                (Some(old), Some(new)) if old != new => format!("{old} → {new}"),
                _ => file.path().to_owned(),
            };
            if ui.selectable_label(*selected == Some(i), label).clicked() {
                *selected = Some(i);
            }
            let (added, removed) = file.stats();
            if file.binary {
                ui.weak("binario");
            } else {
                ui.colored_label(ADDED, format!("+{added}"));
                ui.colored_label(REMOVED, format!("-{removed}"));
            }
        });
    }
}

pub const ADDED: Color32 = Color32::from_rgb(90, 180, 100);
pub const REMOVED: Color32 = Color32::from_rgb(220, 90, 90);

pub fn status_badge(ui: &mut Ui, status: FileStatus) {
    let color = match status {
        FileStatus::Added => ADDED,
        FileStatus::Deleted => REMOVED,
        FileStatus::Modified => Color32::from_rgb(220, 170, 60),
        FileStatus::Renamed | FileStatus::Copied => Color32::from_rgb(100, 150, 220),
    };
    ui.label(
        RichText::new(status.letter().to_string())
            .monospace()
            .strong()
            .color(color),
    );
}
