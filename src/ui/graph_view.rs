//! Dibuja una fila del grafo de commits.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, epaint::CubicBezierShape, pos2};

use crate::git::{GraphRow, Half};

pub const LANE_WIDTH: f32 = 14.0;
/// A partir de aquí los carriles se recortan para no comerse la lista.
pub const MAX_VISIBLE_LANES: usize = 16;
const NODE_RADIUS: f32 = 4.0;
const LINE_WIDTH: f32 = 1.6;

const PALETTE: [Color32; 8] = [
    Color32::from_rgb(66, 165, 245),
    Color32::from_rgb(102, 187, 106),
    Color32::from_rgb(239, 83, 80),
    Color32::from_rgb(255, 167, 38),
    Color32::from_rgb(171, 71, 188),
    Color32::from_rgb(38, 198, 218),
    Color32::from_rgb(236, 64, 122),
    Color32::from_rgb(212, 225, 87),
];

pub fn color(index: usize) -> Color32 {
    PALETTE[index % PALETTE.len()]
}

/// Ancho de la columna del grafo para `lanes` carriles.
pub fn width(lanes: usize) -> f32 {
    lanes.clamp(1, MAX_VISIBLE_LANES) as f32 * LANE_WIDTH + LANE_WIDTH / 2.0
}

fn lane_x(rect: Rect, lane: usize) -> f32 {
    rect.left() + LANE_WIDTH / 2.0 + lane as f32 * LANE_WIDTH
}

/// Dibuja la fila `row` en `rect`. `is_head` resalta el commit actual.
pub fn paint_row(painter: &Painter, rect: Rect, row: &GraphRow, is_head: bool, bg: Color32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let cy = rect.center().y;

    for seg in &row.segments {
        let (y0, y1) = match seg.half {
            Half::Top => (rect.top(), cy),
            Half::Bottom => (cy, rect.bottom()),
        };
        let p0 = pos2(lane_x(rect, seg.from), y0);
        let p1 = pos2(lane_x(rect, seg.to), y1);
        line(&painter, p0, p1, color(seg.color));
    }

    let center = pos2(lane_x(rect, row.lane), cy);
    let c = color(row.color);
    if is_head {
        painter.circle(center, NODE_RADIUS + 1.5, bg, Stroke::new(2.5, c));
    } else {
        painter.circle(center, NODE_RADIUS, c, Stroke::new(1.0, bg));
    }
}

fn line(painter: &Painter, p0: Pos2, p1: Pos2, color: Color32) {
    let stroke = Stroke::new(LINE_WIDTH, color);
    if (p0.x - p1.x).abs() < f32::EPSILON {
        painter.line_segment([p0, p1], stroke);
    } else {
        let mid = (p0.y + p1.y) / 2.0;
        painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [p0, pos2(p0.x, mid), pos2(p1.x, mid), p1],
            false,
            Color32::TRANSPARENT,
            stroke,
        )));
    }
}
