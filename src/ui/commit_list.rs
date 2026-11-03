use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Rect, ScrollArea, Sense, Stroke, Ui, pos2,
    vec2,
};

use super::graph_view;
use crate::git::{Commit, Graph, RefKind, RefLabel};

pub const ROW_HEIGHT: f32 = 24.0;
const HASH_WIDTH: f32 = 80.0;
const DATE_WIDTH: f32 = 130.0;
const AUTHOR_WIDTH: f32 = 160.0;
const PADDING: f32 = 6.0;
/// Cuántas filas antes del final se pide la siguiente página.
const PREFETCH_ROWS: usize = 100;

#[derive(Default)]
pub struct CommitList {
    /// Fila que hay que hacer visible en el próximo frame.
    scroll_to: Option<usize>,
    last_offset: f32,
    last_height: f32,
}

#[derive(Default)]
pub struct ListResponse {
    /// Nueva fila seleccionada (con el ratón o el teclado).
    pub selected: Option<usize>,
    /// La vista está cerca del final: conviene cargar más commits.
    pub wants_more: bool,
}

impl CommitList {
    /// Hace visible la fila `row` en el próximo frame.
    pub fn scroll_to(&mut self, row: usize) {
        self.scroll_to = Some(row);
    }

    /// Dibuja la lista. `selected` es la fila seleccionada actualmente.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        commits: &[Commit],
        graph: &Graph,
        selected: Option<usize>,
    ) -> ListResponse {
        let mut response = ListResponse::default();
        if let Some(new) = self.keyboard(ui, commits.len(), selected) {
            response.selected = Some(new);
        }

        let graph_width = graph_view::width(graph.max_width());
        let mut area = ScrollArea::vertical().auto_shrink(false);
        if let Some(row) = self.scroll_to.take() {
            let top = row as f32 * ROW_HEIGHT;
            let bottom = top + ROW_HEIGHT;
            if top < self.last_offset {
                area = area.vertical_scroll_offset(top);
            } else if bottom > self.last_offset + self.last_height {
                area = area.vertical_scroll_offset(bottom - self.last_height);
            }
        }

        let output = area.show_rows(ui, ROW_HEIGHT, commits.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if range.end + PREFETCH_ROWS >= commits.len() {
                response.wants_more = true;
            }
            for i in range {
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
                let is_selected = response.selected.or(selected) == Some(i);
                paint_row(
                    ui,
                    rect,
                    &commits[i],
                    graph.rows.get(i),
                    graph_width,
                    is_selected,
                    resp.hovered(),
                );
                if resp.clicked() {
                    response.selected = Some(i);
                }
            }
        });
        self.last_offset = output.state.offset.y;
        self.last_height = output.inner_rect.height();
        response
    }

    fn keyboard(&mut self, ui: &Ui, len: usize, selected: Option<usize>) -> Option<usize> {
        if len == 0 || ui.ctx().memory(|m| m.focused().is_some()) {
            return None;
        }
        let page = (self.last_height / ROW_HEIGHT).max(1.0) as usize;
        let current = selected.unwrap_or(0);
        let new = ui.input(|i| {
            if i.key_pressed(Key::ArrowDown) {
                Some(if selected.is_none() { 0 } else { current + 1 })
            } else if i.key_pressed(Key::ArrowUp) {
                Some(current.saturating_sub(1))
            } else if i.key_pressed(Key::PageDown) {
                Some(current + page)
            } else if i.key_pressed(Key::PageUp) {
                Some(current.saturating_sub(page))
            } else if i.key_pressed(Key::Home) {
                Some(0)
            } else if i.key_pressed(Key::End) {
                Some(len - 1)
            } else {
                None
            }
        })?;
        let new = new.min(len - 1);
        self.scroll_to = Some(new);
        (Some(new) != selected).then_some(new)
    }
}

fn paint_row(
    ui: &Ui,
    rect: Rect,
    commit: &Commit,
    graph_row: Option<&crate::git::GraphRow>,
    graph_width: f32,
    selected: bool,
    hovered: bool,
) {
    let visuals = ui.visuals();
    let painter = ui.painter_at(rect);
    let bg = if selected {
        visuals.selection.bg_fill
    } else if hovered {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.panel_fill
    };
    if selected || hovered {
        painter.rect_filled(rect, 0.0, bg);
    }

    if let Some(row) = graph_row {
        let is_head = commit
            .refs
            .iter()
            .any(|r| matches!(r.kind, RefKind::Head | RefKind::Branch { current: true }));
        let graph_rect = Rect::from_min_size(rect.min, vec2(graph_width, rect.height()));
        graph_view::paint_row(&painter, graph_rect, row, is_head, bg);
    }

    let text_color = if selected {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    let weak_color = if selected {
        text_color
    } else {
        visuals.weak_text_color()
    };
    let font = FontId::proportional(13.0);
    let mono = FontId::monospace(12.0);
    let y = rect.center().y;

    // Columnas fijas a la derecha.
    let hash_x = rect.right() - HASH_WIDTH;
    let date_x = hash_x - DATE_WIDTH;
    let author_x = date_x - AUTHOR_WIDTH;
    painter.text(
        pos2(hash_x + PADDING, y),
        Align2::LEFT_CENTER,
        &commit.short_id,
        mono,
        weak_color,
    );
    painter.text(
        pos2(date_x + PADDING, y),
        Align2::LEFT_CENTER,
        super::format_time(commit.time),
        font.clone(),
        weak_color,
    );
    painter
        .with_clip_rect(Rect::from_x_y_ranges(
            author_x..=date_x - PADDING,
            rect.y_range(),
        ))
        .text(
            pos2(author_x + PADDING, y),
            Align2::LEFT_CENTER,
            &commit.author_name,
            font.clone(),
            text_color,
        );

    // Etiquetas de ramas/tags y mensaje.
    let summary_left = rect.left() + graph_width;
    let summary_clip = Rect::from_x_y_ranges(summary_left..=author_x - PADDING, rect.y_range());
    let painter = painter.with_clip_rect(summary_clip);
    let mut x = summary_left + PADDING;
    for label in &commit.refs {
        x = paint_ref(&painter, label, x, rect) + 4.0;
    }
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        &commit.summary,
        font,
        text_color,
    );
}

/// Dibuja una etiqueta de referencia y devuelve la x donde termina.
fn paint_ref(painter: &egui::Painter, label: &RefLabel, x: f32, row: Rect) -> f32 {
    let fill = ref_color(label.kind);
    let galley = painter.layout_no_wrap(
        label.name.clone(),
        FontId::proportional(11.5),
        Color32::WHITE,
    );
    let rect = Rect::from_min_size(
        pos2(x, row.center().y - galley.size().y / 2.0 - 2.0),
        galley.size() + vec2(10.0, 4.0),
    );
    painter.rect(
        rect,
        CornerRadius::same(4),
        fill,
        Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
    rect.right()
}

pub fn ref_color(kind: RefKind) -> Color32 {
    match kind {
        RefKind::Head => Color32::from_rgb(190, 60, 60),
        RefKind::Branch { current: true } => Color32::from_rgb(30, 150, 70),
        RefKind::Branch { current: false } => Color32::from_rgb(70, 110, 80),
        RefKind::Remote => Color32::from_rgb(70, 100, 170),
        RefKind::Tag => Color32::from_rgb(170, 130, 40),
    }
}
