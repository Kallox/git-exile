//! Dibuja una fila del grafo de commits.

use eframe::egui::{Color32, Painter, Rect, Stroke, pos2};

use super::theme;
use crate::git::{GraphRow, Half};

pub const LANE_WIDTH: f32 = 18.0;
/// Margen izquierdo hasta el centro del primer carril.
const LEFT: f32 = 22.0;
/// A partir de aquí los carriles se recortan para no comerse la lista.
pub const MAX_VISIBLE_LANES: usize = 16;
const NODE_RADIUS: f32 = 4.5;
const LINE_WIDTH: f32 = 2.0;

pub fn color(index: usize) -> Color32 {
    let c = theme::colors();
    let palette = [
        c.accent,
        c.teal,
        c.orange,
        c.blue,
        c.green,
        c.yellow,
        Color32::from_rgb(0xF2, 0x8B, 0xC6),
        c.red,
    ];
    palette[index % palette.len()]
}

/// Ancho de la columna del grafo para `lanes` carriles.
pub fn width(lanes: usize) -> f32 {
    LEFT + (lanes.clamp(1, MAX_VISIBLE_LANES) - 1) as f32 * LANE_WIDTH + LEFT
}

fn lane_x(rect: Rect, lane: usize) -> f32 {
    rect.left() + LEFT + lane as f32 * LANE_WIDTH
}

/// Dibuja la fila `row` en `rect`. `is_head` resalta el commit actual
/// y `selected` el seleccionado.
pub fn paint_row(
    painter: &Painter,
    rect: Rect,
    row: &GraphRow,
    is_head: bool,
    selected: bool,
    bg: Color32,
) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let cy = rect.center().y;

    for seg in &row.segments {
        let (y0, y1) = match seg.half {
            Half::Top => (rect.top(), cy),
            Half::Bottom => (cy, rect.bottom()),
        };
        let p0 = pos2(lane_x(rect, seg.from), y0);
        let p1 = pos2(lane_x(rect, seg.to), y1);
        painter.line_segment([p0, p1], Stroke::new(LINE_WIDTH, color(seg.color)));
    }

    let center = pos2(lane_x(rect, row.lane), cy);
    let c = color(row.color);
    if selected {
        painter.circle(center, 7.0, bg, Stroke::new(2.0, c));
        painter.circle_filled(center, 3.5, c);
    } else if is_head {
        painter.circle(center, 6.0, bg, Stroke::new(2.5, c));
    } else {
        painter.circle_filled(center, NODE_RADIUS, c);
    }
}

/// Nodo hueco para la fila de cambios sin commitear, unido al commit de abajo.
pub fn paint_worktree_node(painter: &Painter, rect: Rect, bg: Color32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let x = lane_x(rect, 0);
    let center = pos2(x, rect.center().y);
    let grey = theme::colors().text_muted;
    let stroke = Stroke::new(LINE_WIDTH, grey);
    // Línea discontinua hacia HEAD.
    let mut y = center.y;
    while y < rect.bottom() {
        painter.line_segment([pos2(x, y), pos2(x, (y + 3.0).min(rect.bottom()))], stroke);
        y += 5.0;
    }
    painter.circle(center, NODE_RADIUS + 0.5, bg, Stroke::new(2.0, grey));
}
