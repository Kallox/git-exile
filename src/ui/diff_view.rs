//! Visor de diffs con números de línea y colores.

use eframe::egui::{self, Align2, Color32, FontId, RichText, ScrollArea, Sense, Ui, pos2, vec2};

use crate::git::{FileDiff, LineKind};

const FONT_SIZE: f32 = 12.5;
const TAB: &str = "    ";
/// Ancho reservado para cada columna de números de línea (en caracteres).
const NUMBER_CHARS: usize = 5;

enum Row<'a> {
    Header(&'a str),
    Line(&'a crate::git::DiffLine),
}

pub fn show(ui: &mut Ui, file: &FileDiff) {
    ui.horizontal(|ui| {
        super::commit_details::status_badge(ui, file.status);
        ui.strong(file.path());
    });
    ui.separator();

    if file.binary {
        ui.weak("Archivo binario: no se muestra el contenido.");
        return;
    }
    if file.hunks.is_empty() {
        ui.weak(match file.status {
            crate::git::FileStatus::Renamed => "Renombrado sin cambios en el contenido.",
            _ => "Sin cambios en el contenido (solo permisos o archivo vacío).",
        });
        return;
    }

    let rows: Vec<Row> = file
        .hunks
        .iter()
        .flat_map(|h| std::iter::once(Row::Header(&h.header)).chain(h.lines.iter().map(Row::Line)))
        .collect();

    let font = FontId::monospace(FONT_SIZE);
    let (char_width, row_height) =
        ui.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font) + 2.0));
    let longest = rows
        .iter()
        .map(|r| match r {
            Row::Header(h) => h.chars().count(),
            Row::Line(l) => l.text.chars().count() + l.text.matches('\t').count() * 3,
        })
        .max()
        .unwrap_or(0);
    let gutter = char_width * (NUMBER_CHARS * 2 + 5) as f32;
    let content_width = (gutter + char_width * (longest + 2) as f32).max(ui.available_width());

    let dark = ui.visuals().dark_mode;
    let added_bg = if dark {
        Color32::from_rgba_unmultiplied(60, 160, 80, 45)
    } else {
        Color32::from_rgba_unmultiplied(60, 180, 80, 50)
    };
    let removed_bg = if dark {
        Color32::from_rgba_unmultiplied(200, 60, 60, 45)
    } else {
        Color32::from_rgba_unmultiplied(220, 60, 60, 45)
    };
    let header_bg = ui.visuals().faint_bg_color;
    let text_color = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();

    ScrollArea::both()
        .id_salt(("diff", file.path()))
        .auto_shrink(false)
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(content_width, row_height), Sense::hover());
                let painter = ui.painter_at(rect);
                let y = rect.center().y;
                let text_x = rect.left() + gutter;
                match row {
                    Row::Header(h) => {
                        painter.rect_filled(rect, 0.0, header_bg);
                        painter.text(pos2(text_x, y), Align2::LEFT_CENTER, h, font.clone(), weak);
                    }
                    Row::Line(line) => {
                        let (bg, sign) = match line.kind {
                            LineKind::Added => (Some(added_bg), "+"),
                            LineKind::Removed => (Some(removed_bg), "-"),
                            LineKind::Context => (None, " "),
                            LineKind::NoNewline => (None, ""),
                        };
                        if let Some(bg) = bg {
                            painter.rect_filled(rect, 0.0, bg);
                        }
                        let number = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
                        let num_w = char_width * NUMBER_CHARS as f32;
                        painter.text(
                            pos2(rect.left() + num_w, y),
                            Align2::RIGHT_CENTER,
                            number(line.old_no),
                            font.clone(),
                            weak,
                        );
                        painter.text(
                            pos2(rect.left() + num_w * 2.0 + char_width, y),
                            Align2::RIGHT_CENTER,
                            number(line.new_no),
                            font.clone(),
                            weak,
                        );
                        let color = if line.kind == LineKind::NoNewline {
                            weak
                        } else {
                            text_color
                        };
                        painter.text(
                            pos2(text_x - char_width * 1.5, y),
                            Align2::LEFT_CENTER,
                            sign,
                            font.clone(),
                            color,
                        );
                        painter.text(
                            pos2(text_x, y),
                            Align2::LEFT_CENTER,
                            line.text.replace('\t', TAB),
                            font.clone(),
                            color,
                        );
                    }
                }
                // Separador entre los números de línea y el código.
                let sep_x = text_x - char_width * 2.0;
                painter.vline(
                    sep_x,
                    rect.y_range(),
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
            }
        });
}

/// Mensaje centrado para cuando no hay archivo que mostrar.
pub fn placeholder(ui: &mut Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).weak());
    });
}
