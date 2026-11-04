use eframe::egui::{
    self, Align2, Color32, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2,
};

use super::commit_list::avatar;
use super::icons::{self, Icon};
use super::theme;
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
            ui.spacing_mut().item_spacing.y = 6.0;
            let is_merge = details.parents.len() > 1;
            if is_merge {
                pill(ui, Icon::Merge, "Merge");
            }
            let (summary, body) = details
                .message
                .split_once('\n')
                .unwrap_or((&details.message, ""));
            ui.add(
                egui::Label::new(
                    RichText::new(summary)
                        .font(theme::font_semibold(18.0))
                        .color(theme::colors().text),
                )
                .wrap(),
            );
            let body = body.trim();
            if !body.is_empty() {
                ui.label(RichText::new(body).color(theme::colors().text_secondary));
            }
            ui.add_space(12.0);

            if let Some(a) = meta_card(ui, details) {
                action = Some(a);
            }
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let count = match details.files.len() {
                    1 => "1 archivo cambiado".to_owned(),
                    n => format!("{n} archivos cambiados"),
                };
                ui.label(RichText::new(count).font(theme::font_semibold(13.0)));
                if is_merge {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Respecto al primer padre")
                                .size(12.0)
                                .color(theme::colors().text_muted),
                        );
                    });
                }
            });
            if details.truncated {
                ui.colored_label(
                    theme::colors().yellow,
                    "El diff es demasiado grande y se ha cortado.",
                );
            }
            file_list(ui, &details.files, selected_file);
        });
    action
}

/// Etiqueta redondeada con icono sobre fondo de acento.
fn pill(ui: &mut Ui, icon: Icon, text: &str) {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme::font_semibold(11.0),
        theme::colors().accent,
    );
    let size = vec2(8.0 + 11.0 + 5.0 + galley.size().x + 8.0, 20.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 10.0, theme::accent_soft());
    icons::paint(
        painter,
        Rect::from_center_size(
            pos2(rect.left() + 8.0 + 5.5, rect.center().y),
            vec2(11.0, 11.0),
        ),
        icon,
        theme::colors().accent,
        2.4,
    );
    painter.galley(
        pos2(
            rect.left() + 8.0 + 11.0 + 5.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        theme::colors().accent,
    );
}

/// Tarjeta con autor, fecha, hash y padres.
fn meta_card(ui: &mut Ui, details: &CommitDetails) -> Option<DetailsAction> {
    let mut action = None;
    egui::Frame::new()
        .fill(theme::colors().bg_card)
        .stroke(Stroke::new(1.0, theme::colors().border))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("commit_meta")
                .num_columns(2)
                .min_col_width(72.0)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    let label = |ui: &mut Ui, text: &str| {
                        ui.label(
                            RichText::new(text)
                                .size(12.0)
                                .color(theme::colors().text_muted),
                        );
                    };

                    label(ui, "Autor");
                    person(ui, &details.author);
                    ui.end_row();

                    if details.committer.name != details.author.name
                        || details.committer.email != details.author.email
                    {
                        label(ui, "Committer");
                        person(ui, &details.committer);
                        ui.end_row();
                    }

                    label(ui, "Fecha");
                    let mut date = super::format_date_long(details.author.time);
                    if details.committer.time != details.author.time {
                        date.push_str(&format!(
                            "  (commit: {})",
                            super::format_date_long(details.committer.time)
                        ));
                    }
                    ui.label(RichText::new(date).color(theme::colors().text_soft));
                    ui.end_row();

                    label(ui, "Commit");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&details.id)
                                .font(theme::mono(12.0))
                                .color(theme::colors().text_soft),
                        );
                        if copy_button(ui).on_hover_text("Copiar hash").clicked() {
                            ui.ctx().copy_text(details.id.clone());
                        }
                    });
                    ui.end_row();

                    if !details.parents.is_empty() {
                        label(
                            ui,
                            if details.parents.len() > 1 {
                                "Padres"
                            } else {
                                "Padre"
                            },
                        );
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            for parent in &details.parents {
                                let short = &parent[..parent.len().min(7)];
                                if ui
                                    .link(RichText::new(short).font(theme::mono(12.0)))
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
        });
    action
}

fn person(ui: &mut Ui, p: &Person) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        avatar(ui.painter(), rect.center(), 11.0, &p.name);
        ui.label(RichText::new(&p.name).color(theme::colors().text));
        ui.label(RichText::new(&p.email).color(theme::colors().text_muted));
    });
}

/// Botón cuadrado de 28 px con el icono de copiar.
fn copy_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        6.0,
        if resp.hovered() {
            theme::colors().bg_active
        } else {
            theme::colors().bg_button
        },
        Stroke::new(1.0, theme::colors().border_button),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        painter,
        Rect::from_center_size(rect.center(), vec2(13.0, 13.0)),
        Icon::Copy,
        theme::colors().text_secondary,
        2.0,
    );
    resp
}

/// Lista seleccionable de archivos con su estado y número de líneas.
pub fn file_list(ui: &mut Ui, files: &[FileDiff], selected: &mut Option<usize>) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for (i, file) in files.iter().enumerate() {
        let path = match (&file.old_path, &file.new_path) {
            (Some(old), Some(new)) if old != new => format!("{old} → {new}"),
            _ => file.path().to_owned(),
        };
        let trailing = if file.binary {
            Trailing::Binary
        } else {
            let (added, removed) = file.stats();
            Trailing::Stats(added, removed)
        };
        if file_row(
            ui,
            file.status.letter(),
            status_color(file.status),
            &path,
            trailing,
            *selected == Some(i),
        )
        .clicked()
        {
            *selected = Some(i);
        }
    }
}

/// Lo que se muestra a la derecha de una fila de archivo.
#[derive(Clone, Copy)]
pub enum Trailing {
    /// Líneas añadidas y borradas.
    Stats(usize, usize),
    Binary,
    /// Hueco libre de ese ancho (para botones encima).
    Space(f32),
}

/// Fila de archivo de 34 px: letra de estado, ruta (carpeta atenuada) y +/−.
pub fn file_row(
    ui: &mut Ui,
    letter: char,
    color: Color32,
    path: &str,
    trailing: Trailing,
    selected: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 7.0, theme::accent_soft());
    } else if resp.hovered() {
        painter.rect_filled(rect, 7.0, theme::colors().bg_card);
    }
    let y = rect.center().y;
    let badge = Rect::from_min_size(pos2(rect.left() + 10.0, y - 9.0), vec2(18.0, 18.0));
    paint_badge(painter, badge, letter, color);

    // Números a la derecha.
    let mut right = rect.right() - 10.0;
    match trailing {
        Trailing::Stats(added, removed) => {
            let r = painter.text(
                pos2(right, y),
                Align2::RIGHT_CENTER,
                format!("−{removed}"),
                theme::mono(12.0),
                theme::colors().red,
            );
            right = r.left() - 8.0;
            let r = painter.text(
                pos2(right, y),
                Align2::RIGHT_CENTER,
                format!("+{added}"),
                theme::mono(12.0),
                theme::colors().green,
            );
            right = r.left() - 10.0;
        }
        Trailing::Space(width) => right -= width,
        Trailing::Binary => {
            let r = painter.text(
                pos2(right, y),
                Align2::RIGHT_CENTER,
                "binario",
                theme::font(12.0),
                theme::colors().text_muted,
            );
            right = r.left() - 10.0;
        }
    }

    // Ruta: carpeta atenuada y nombre normal.
    let (dir, name) = match path.rfind('/') {
        Some(i) if !path.contains(" → ") => path.split_at(i + 1),
        _ => ("", path),
    };
    let clip = Rect::from_x_y_ranges(badge.right() + 10.0..=right, rect.y_range());
    let painter = painter.with_clip_rect(clip);
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(theme::mono(12.0), color);
    job.append(dir, 0.0, format(theme::colors().text_muted));
    job.append(name, 0.0, format(theme::colors().text));
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    painter.galley(
        pos2(clip.left(), y - galley.size().y / 2.0),
        galley,
        theme::colors().text,
    );
    resp.on_hover_text(path)
}

/// Cuadrado de 18 px con la letra de estado sobre fondo suave.
pub fn paint_badge(painter: &egui::Painter, rect: Rect, letter: char, color: Color32) {
    painter.rect_filled(rect, 4.0, theme::alpha(color, 0.14));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        theme::mono(10.0),
        color,
    );
}

pub fn status_color(status: FileStatus) -> Color32 {
    match status {
        FileStatus::Added => theme::colors().green,
        FileStatus::Deleted => theme::colors().red,
        FileStatus::Modified => theme::colors().yellow,
        FileStatus::Renamed | FileStatus::Copied => theme::colors().blue,
    }
}
