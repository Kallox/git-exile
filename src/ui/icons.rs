//! Iconos vectoriales dibujados con el painter (equivalentes a los SVG del diseño,
//! sobre una cuadrícula de 24x24).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use eframe::egui::{
    self, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    epaint::CubicBezierShape, pos2, vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Folder,
    Close,
    Plus,
    Clock,
    Branch,
    /// Rama simple (dos nodos), para las etiquetas.
    BranchSmall,
    Cloud,
    Search,
    Refresh,
    Copy,
    ChevronDown,
    Merge,
    Home,
    ArrowUp,
}

/// Transforma coordenadas de la cuadrícula 24x24 al rectángulo destino.
struct Grid {
    origin: Pos2,
    scale: f32,
}

impl Grid {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    fn r(&self, r: f32) -> f32 {
        r * self.scale
    }
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32, width: f32) {
    let size = rect.width().min(rect.height());
    let g = Grid {
        origin: rect.center() - vec2(size, size) / 2.0,
        scale: size / 24.0,
    };
    let s = Stroke::new(width, color);
    let line = |pts: &[(f32, f32)]| {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| g.p(x, y)).collect();
        painter.add(Shape::line(pts, s));
    };
    let circle = |x: f32, y: f32, r: f32| {
        painter.circle_stroke(g.p(x, y), g.r(r), s);
    };
    let curve = |a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)| {
        painter.add(CubicBezierShape::from_points_stroke(
            [g.p(a.0, a.1), g.p(b.0, b.1), g.p(c.0, c.1), g.p(d.0, d.1)],
            false,
            Color32::TRANSPARENT,
            s,
        ));
    };
    let arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| {
        let steps = 16;
        let pts: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let a = from + (to - from) * i as f32 / steps as f32;
                g.p(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        painter.add(Shape::line(pts, s));
    };

    match icon {
        Icon::Folder => {
            line(&[
                (3.0, 9.0),
                (3.0, 17.0),
                (4.0, 19.0),
                (20.0, 19.0),
                (21.0, 17.0),
                (21.0, 9.0),
                (20.0, 7.0),
                (11.0, 7.0),
                (9.0, 5.0),
                (5.0, 5.0),
                (3.0, 7.0),
                (3.0, 9.0),
            ]);
        }
        Icon::Close => {
            line(&[(6.0, 6.0), (18.0, 18.0)]);
            line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Plus => {
            line(&[(12.0, 5.0), (12.0, 19.0)]);
            line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Clock => {
            circle(12.0, 12.0, 9.0);
            line(&[(12.0, 7.0), (12.0, 12.0), (15.0, 14.0)]);
        }
        Icon::Branch => {
            circle(6.0, 5.0, 2.0);
            circle(6.0, 19.0, 2.0);
            circle(18.0, 7.0, 2.0);
            line(&[(6.0, 7.0), (6.0, 17.0)]);
            curve((18.0, 9.0), (18.0, 14.0), (12.0, 13.0), (7.0, 17.0));
        }
        Icon::BranchSmall => {
            circle(6.0, 6.0, 2.0);
            circle(6.0, 18.0, 2.0);
            line(&[(6.0, 8.0), (6.0, 16.0)]);
        }
        Icon::Cloud => {
            arc(7.0, 13.0, 5.0, FRAC_PI_2, PI + 0.9);
            arc(12.0, 9.5, 6.0, PI + 0.6, TAU - 0.2);
            arc(18.0, 14.0, 4.0, -FRAC_PI_2 - 0.4, FRAC_PI_2);
            line(&[(7.0, 18.0), (18.0, 18.0)]);
        }
        Icon::Search => {
            circle(11.0, 11.0, 7.0);
            line(&[(20.0, 20.0), (16.5, 16.5)]);
        }
        Icon::Refresh => {
            arc(12.0, 12.0, 8.0, -0.45, TAU - 0.9);
            line(&[(20.0, 4.0), (20.0, 9.5), (14.5, 9.5)]);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(g.p(9.0, 9.0), g.p(20.0, 20.0)),
                g.r(2.0),
                s,
                egui::StrokeKind::Middle,
            );
            line(&[(5.0, 15.0), (5.0, 5.0), (6.0, 3.5), (15.0, 3.5)]);
        }
        Icon::ChevronDown => line(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
        Icon::Merge => {
            circle(6.0, 5.0, 2.0);
            circle(6.0, 19.0, 2.0);
            circle(18.0, 12.0, 2.0);
            line(&[(6.0, 7.0), (6.0, 17.0)]);
            curve((6.0, 7.0), (6.0, 11.0), (10.0, 12.0), (16.0, 12.0));
        }
        Icon::Home => {
            line(&[(3.0, 11.0), (12.0, 3.5), (21.0, 11.0)]);
            line(&[(5.5, 9.5), (5.5, 20.0), (18.5, 20.0), (18.5, 9.5)]);
        }
        Icon::ArrowUp => {
            line(&[(12.0, 19.0), (12.0, 5.0)]);
            line(&[(6.0, 11.0), (12.0, 5.0), (18.0, 11.0)]);
        }
    }
}

/// Reserva un hueco de `size` y dibuja el icono.
pub fn show(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint(ui.painter(), rect, icon, color, 1.8);
    resp
}

/// Botón cuadrado sin fondo con solo un icono.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, icon_size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, super::theme::BG_ACTIVE);
    }
    let color = if resp.hovered() {
        super::theme::TEXT
    } else {
        color
    };
    paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(icon_size, icon_size)),
        icon,
        color,
        2.0,
    );
    resp
}

/// Apariencia de un botón con icono y texto.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Fondo oscuro con borde.
    Secondary,
    /// Relleno con el color de acento.
    Primary,
    /// Sin fondo (barra de pestañas).
    Ghost,
}

/// Botón de 34 px de alto con icono a la izquierda, como los del diseño.
pub fn button(ui: &mut Ui, icon: Icon, text: &str, style: ButtonStyle, enabled: bool) -> Response {
    use super::theme::*;
    let font = if style == ButtonStyle::Primary {
        font_semibold(13.0)
    } else {
        font(13.0)
    };
    let (fg, icon_color) = match style {
        ButtonStyle::Primary => (TEXT_ON_ACCENT, TEXT_ON_ACCENT),
        ButtonStyle::Secondary => (TEXT, TEXT),
        ButtonStyle::Ghost => (TEXT_SECONDARY, TEXT_SECONDARY),
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let height = 34.0;
    let padding = if style == ButtonStyle::Ghost {
        12.0
    } else {
        14.0
    };
    let width = padding * 2.0 + 14.0 + 8.0 + galley.size().x;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), sense);
    let hovered = enabled && resp.hovered();
    let painter = ui.painter();
    let radius = RADIUS as f32;
    match style {
        ButtonStyle::Primary => {
            let fill = if hovered { LINK } else { ACCENT };
            painter.rect_filled(rect, radius, fill);
        }
        ButtonStyle::Secondary => {
            painter.rect(
                rect,
                radius,
                if hovered { BG_ACTIVE } else { BG_BUTTON },
                Stroke::new(1.0, BORDER_BUTTON),
                egui::StrokeKind::Inside,
            );
        }
        ButtonStyle::Ghost => {
            if hovered {
                painter.rect_filled(rect, radius, BG_BUTTON);
            }
        }
    }
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + padding + 7.0, rect.center().y),
        vec2(14.0, 14.0),
    );
    paint(painter, icon_rect, icon, icon_color, 2.2);
    let text_pos = pos2(
        icon_rect.right() + 8.0,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, fg);
    if !enabled {
        painter.rect_filled(rect, radius, alpha(BG, 0.5));
    }
    resp
}
