//! Visor de diffs con vista unificada o dividida (lado a lado).

use eframe::egui::{
    self, Align2, Color32, Painter, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2,
};

use super::commit_details::{paint_badge, status_color};
use super::theme;
use crate::git::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};

const FONT_SIZE: f32 = 12.5;
const LINE_HEIGHT: f32 = 24.0;
const NUMBER_WIDTH: f32 = 44.0;
const SIGN_WIDTH: f32 = 22.0;
const TAB: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    #[default]
    Unified,
    Split,
}

/// Fila de la vista unificada.
enum Row<'a> {
    Header(&'a str),
    Line(&'a DiffLine),
}

/// Fila de la vista dividida: línea antigua a la izquierda y nueva a la derecha.
enum SplitRow<'a> {
    Header(&'a str),
    Pair(Option<&'a DiffLine>, Option<&'a DiffLine>),
}

/// Colores de una línea según su tipo.
struct LineStyle {
    bg: Option<Color32>,
    text: Color32,
    number: Color32,
    sign: &'static str,
    sign_color: Color32,
}

fn line_style(kind: LineKind) -> LineStyle {
    let text = |color| theme::lighten(color, 0.4);
    let number = |color: Color32| color.lerp_to_gamma(theme::colors().bg, 0.2);
    match kind {
        LineKind::Added => LineStyle {
            bg: Some(theme::alpha(theme::colors().green, 0.10)),
            text: text(theme::colors().green),
            number: number(theme::colors().green),
            sign: "+",
            sign_color: theme::colors().green,
        },
        LineKind::Removed => LineStyle {
            bg: Some(theme::alpha(theme::colors().red, 0.10)),
            text: text(theme::colors().red),
            number: number(theme::colors().red),
            sign: "−",
            sign_color: theme::colors().red,
        },
        LineKind::Context => LineStyle {
            bg: None,
            text: theme::colors().text_soft,
            number: theme::colors().text_faint,
            sign: "",
            sign_color: theme::colors().text_faint,
        },
        LineKind::NoNewline => LineStyle {
            bg: None,
            text: theme::colors().text_faint,
            number: theme::colors().text_faint,
            sign: "",
            sign_color: theme::colors().text_faint,
        },
    }
}

type Pair<'a> = (Option<&'a DiffLine>, Option<&'a DiffLine>);

/// Empareja el bloque de líneas borradas con el de añadidas y vacía ambos.
fn flush<'a>(
    rows: &mut Vec<Pair<'a>>,
    removed: &mut Vec<&'a DiffLine>,
    added: &mut Vec<&'a DiffLine>,
) {
    let n = removed.len().max(added.len());
    for i in 0..n {
        rows.push((removed.get(i).copied(), added.get(i).copied()));
    }
    removed.clear();
    added.clear();
}

/// Agrupa las líneas de un hunk en pares antigua/nueva para la vista dividida.
/// Cada bloque de líneas borradas se empareja con el bloque añadido que le sigue.
fn split_rows(hunk: &Hunk) -> Vec<Pair<'_>> {
    let mut rows = Vec::new();
    let mut removed: Vec<&DiffLine> = Vec::new();
    let mut added: Vec<&DiffLine> = Vec::new();
    for line in &hunk.lines {
        match line.kind {
            LineKind::Removed => {
                if !added.is_empty() {
                    flush(&mut rows, &mut removed, &mut added);
                }
                removed.push(line);
            }
            LineKind::Added => added.push(line),
            LineKind::Context | LineKind::NoNewline => {
                flush(&mut rows, &mut removed, &mut added);
                rows.push((Some(line), Some(line)));
            }
        }
    }
    flush(&mut rows, &mut removed, &mut added);
    rows
}

pub fn show(ui: &mut Ui, file: &FileDiff) {
    let mode_id = egui::Id::new("diff_mode");
    let mut mode: DiffMode = ui.data(|d| d.get_temp(mode_id)).unwrap_or_default();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        paint_badge(
            ui.painter(),
            rect,
            file.status.letter(),
            status_color(file.status),
        );
        let path = match (&file.old_path, &file.new_path) {
            (Some(old), Some(new)) if old != new => format!("{old} → {new}"),
            _ => file.path().to_owned(),
        };
        ui.label(
            RichText::new(path)
                .font(theme::mono(13.0))
                .color(theme::colors().text),
        );
        if !file.binary {
            let (added, removed) = file.stats();
            ui.label(
                RichText::new(format!("+{added}"))
                    .font(theme::mono(12.0))
                    .color(theme::colors().green),
            );
            ui.label(
                RichText::new(format!("−{removed}"))
                    .font(theme::mono(12.0))
                    .color(theme::colors().red),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            segmented(ui, &mut mode);
        });
    });
    ui.data_mut(|d| d.insert_temp(mode_id, mode));
    ui.add_space(12.0);

    if file.binary {
        placeholder(ui, "Archivo binario: no se muestra el contenido.");
        return;
    }
    if file.hunks.is_empty() {
        placeholder(
            ui,
            match file.status {
                FileStatus::Renamed => "Renombrado sin cambios en el contenido.",
                _ => "Sin cambios en el contenido (solo permisos o archivo vacío).",
            },
        );
        return;
    }

    egui::Frame::new()
        .fill(theme::colors().bg_code)
        .stroke(Stroke::new(1.0, theme::colors().border))
        .corner_radius(10)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            match mode {
                DiffMode::Unified => unified(ui, file),
                DiffMode::Split => split(ui, file),
            }
        });
}

/// Selector Unificado / Dividido.
fn segmented(ui: &mut Ui, mode: &mut DiffMode) {
    egui::Frame::new()
        .fill(theme::colors().bg_input)
        .stroke(Stroke::new(1.0, theme::colors().border_strong))
        .corner_radius(8)
        .inner_margin(3)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // El padre es de derecha a izquierda: se dibujan en orden inverso.
            for (value, label) in [
                (DiffMode::Split, "Dividido"),
                (DiffMode::Unified, "Unificado"),
            ] {
                let active = *mode == value;
                let galley = ui.painter().layout_no_wrap(
                    label.to_owned(),
                    theme::font(12.0),
                    if active {
                        theme::colors().text
                    } else {
                        theme::colors().text_secondary
                    },
                );
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(galley.size().x + 20.0, 26.0), Sense::click());
                if active {
                    ui.painter()
                        .rect_filled(rect, 6.0, theme::colors().bg_active);
                } else if resp.hovered() {
                    ui.painter()
                        .rect_filled(rect, 6.0, theme::colors().bg_button);
                }
                ui.painter().galley(
                    rect.center() - galley.size() / 2.0,
                    galley,
                    theme::colors().text,
                );
                if resp.clicked() {
                    *mode = value;
                }
            }
        });
}

fn char_width(ui: &Ui) -> f32 {
    ui.fonts_mut(|f| f.glyph_width(&theme::mono(FONT_SIZE), 'M'))
}

fn display_len(text: &str) -> usize {
    text.chars().count() + text.matches('\t').count() * (TAB.len() - 1)
}

fn unified(ui: &mut Ui, file: &FileDiff) {
    let rows: Vec<Row> = file
        .hunks
        .iter()
        .flat_map(|h| std::iter::once(Row::Header(&h.header)).chain(h.lines.iter().map(Row::Line)))
        .collect();
    let cw = char_width(ui);
    let gutter = NUMBER_WIDTH * 2.0 + SIGN_WIDTH;
    let longest = rows
        .iter()
        .map(|r| match r {
            Row::Header(h) => h.chars().count(),
            Row::Line(l) => display_len(&l.text),
        })
        .max()
        .unwrap_or(0);
    let content_width = (gutter + cw * (longest + 2) as f32).max(ui.available_width());

    ScrollArea::both()
        .id_salt(("diff_unified", file.path()))
        .auto_shrink(false)
        .show_rows(ui, LINE_HEIGHT, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(content_width, LINE_HEIGHT), Sense::hover());
                let painter = ui.painter_at(rect);
                match row {
                    Row::Header(h) => paint_header(&painter, rect, h),
                    Row::Line(line) => {
                        let style = line_style(line.kind);
                        if let Some(bg) = style.bg {
                            painter.rect_filled(rect, 0.0, bg);
                        }
                        let y = rect.center().y;
                        number(
                            &painter,
                            rect.left() + NUMBER_WIDTH,
                            y,
                            line.old_no,
                            style.number,
                        );
                        number(
                            &painter,
                            rect.left() + NUMBER_WIDTH * 2.0,
                            y,
                            line.new_no,
                            style.number,
                        );
                        paint_code(&painter, rect.left() + NUMBER_WIDTH * 2.0, y, line, &style);
                    }
                }
            }
        });
}

fn split(ui: &mut Ui, file: &FileDiff) {
    let rows: Vec<SplitRow> = file
        .hunks
        .iter()
        .flat_map(|h| {
            std::iter::once(SplitRow::Header(&h.header)).chain(
                split_rows(h)
                    .into_iter()
                    .map(|(old, new)| SplitRow::Pair(old, new)),
            )
        })
        .collect();

    ScrollArea::vertical()
        .id_salt(("diff_split", file.path()))
        .auto_shrink(false)
        .show_rows(ui, LINE_HEIGHT, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let width = ui.available_width();
            for row in &rows[range] {
                let (rect, _) = ui.allocate_exact_size(vec2(width, LINE_HEIGHT), Sense::hover());
                match row {
                    SplitRow::Header(h) => paint_header(&ui.painter_at(rect), rect, h),
                    SplitRow::Pair(old, new) => {
                        let half = rect.width() / 2.0;
                        let left = Rect::from_min_size(rect.min, vec2(half, rect.height()));
                        let right = Rect::from_min_size(
                            rect.min + vec2(half, 0.0),
                            vec2(half, rect.height()),
                        );
                        paint_side(&ui.painter_at(left), left, *old, true);
                        paint_side(&ui.painter_at(right), right, *new, false);
                        ui.painter().vline(
                            right.left(),
                            rect.y_range(),
                            Stroke::new(1.0, theme::colors().border),
                        );
                    }
                }
            }
        });
}

/// Una mitad de la vista dividida. `old` indica el lado izquierdo.
fn paint_side(painter: &Painter, rect: Rect, line: Option<&DiffLine>, old: bool) {
    let Some(line) = line else {
        // Hueco: no hay línea equivalente en este lado.
        painter.rect_filled(rect, 0.0, theme::alpha(theme::colors().bg_card, 0.6));
        return;
    };
    // En el lado izquierdo una línea añadida no existe, y viceversa.
    let style = line_style(line.kind);
    if let Some(bg) = style.bg {
        painter.rect_filled(rect, 0.0, bg);
    }
    let y = rect.center().y;
    let no = if old { line.old_no } else { line.new_no };
    number(painter, rect.left() + NUMBER_WIDTH, y, no, style.number);
    paint_code(painter, rect.left() + NUMBER_WIDTH, y, line, &style);
}

fn paint_header(painter: &Painter, rect: Rect, header: &str) {
    painter.rect_filled(rect, 0.0, theme::accent_soft());
    painter.text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        header,
        theme::mono(FONT_SIZE),
        theme::colors().accent,
    );
}

/// Número de línea alineado a la derecha terminando en `right`.
fn number(painter: &Painter, right: f32, y: f32, n: Option<u32>, color: Color32) {
    if let Some(n) = n {
        painter.text(
            pos2(right - 8.0, y),
            Align2::RIGHT_CENTER,
            n.to_string(),
            theme::mono(FONT_SIZE),
            color,
        );
    }
}

/// Signo y texto de la línea a partir de `x`.
fn paint_code(painter: &Painter, x: f32, y: f32, line: &DiffLine, style: &LineStyle) {
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        style.sign,
        theme::mono(FONT_SIZE),
        style.sign_color,
    );
    painter.text(
        pos2(x + SIGN_WIDTH, y),
        Align2::LEFT_CENTER,
        line.text.replace('\t', TAB),
        theme::mono(FONT_SIZE),
        style.text,
    );
}

/// Mensaje centrado para cuando no hay nada que mostrar.
pub fn placeholder(ui: &mut Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).color(theme::colors().text_muted));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: LineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_no: None,
            new_no: None,
            text: text.into(),
        }
    }

    #[test]
    fn pairs_removed_and_added_blocks() {
        use LineKind::*;
        let hunk = Hunk {
            header: "@@".into(),
            lines: vec![
                line(Context, "a"),
                line(Removed, "b"),
                line(Removed, "c"),
                line(Added, "B"),
                line(Context, "d"),
                line(Added, "e"),
                line(Removed, "f"),
            ],
        };
        let rows: Vec<(Option<&str>, Option<&str>)> = split_rows(&hunk)
            .into_iter()
            .map(|(o, n)| (o.map(|l| l.text.as_str()), n.map(|l| l.text.as_str())))
            .collect();
        assert_eq!(
            rows,
            [
                (Some("a"), Some("a")),
                (Some("b"), Some("B")),
                (Some("c"), None),
                (Some("d"), Some("d")),
                (None, Some("e")),
                (Some("f"), None),
            ]
        );
    }
}
