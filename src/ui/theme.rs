//! Temas de color, fuentes y estilo global.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle,
};

/// Colores de un tema. Los fondos van de más oscuro a más claro.
pub struct Palette {
    pub bg_tabs: Color32,
    pub bg_code: Color32,
    pub bg: Color32,
    pub bg_card: Color32,
    pub bg_input: Color32,
    pub bg_button: Color32,
    pub bg_active: Color32,
    pub border: Color32,
    pub border_soft: Color32,
    pub border_strong: Color32,
    pub border_button: Color32,
    pub border_badge: Color32,
    pub text: Color32,
    pub text_strong: Color32,
    pub text_soft: Color32,
    pub text_secondary: Color32,
    pub text_date: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub text_on_accent: Color32,
    pub accent: Color32,
    pub link: Color32,
    pub green: Color32,
    pub red: Color32,
    pub yellow: Color32,
    pub teal: Color32,
    pub orange: Color32,
    pub orange_text: Color32,
    pub blue: Color32,
}

/// Tema por defecto: gris azulado con acento violeta.
const VIOLET: Palette = Palette {
    bg_tabs: Color32::from_rgb(0x0A, 0x0B, 0x0F),
    bg_code: Color32::from_rgb(0x0B, 0x0C, 0x10),
    bg: Color32::from_rgb(0x0E, 0x0F, 0x14),
    bg_card: Color32::from_rgb(0x13, 0x14, 0x1B),
    bg_input: Color32::from_rgb(0x15, 0x17, 0x20),
    bg_button: Color32::from_rgb(0x18, 0x1A, 0x22),
    bg_active: Color32::from_rgb(0x26, 0x29, 0x36),
    border: Color32::from_rgb(0x1E, 0x20, 0x29),
    border_soft: Color32::from_rgb(0x1A, 0x1C, 0x24),
    border_strong: Color32::from_rgb(0x26, 0x29, 0x36),
    border_button: Color32::from_rgb(0x2C, 0x2F, 0x3D),
    border_badge: Color32::from_rgb(0x3A, 0x3D, 0x4E),
    text: Color32::from_rgb(0xE6, 0xE6, 0xEC),
    text_strong: Color32::WHITE,
    text_soft: Color32::from_rgb(0xC9, 0xCA, 0xD6),
    text_secondary: Color32::from_rgb(0xA9, 0xAA, 0xB8),
    text_date: Color32::from_rgb(0x9A, 0x9B, 0xAA),
    text_muted: Color32::from_rgb(0x8C, 0x8E, 0x9C),
    text_faint: Color32::from_rgb(0x6E, 0x70, 0x80),
    text_on_accent: Color32::from_rgb(0x14, 0x10, 0x1F),
    accent: Color32::from_rgb(0xA7, 0x8B, 0xFA),
    link: Color32::from_rgb(0xB9, 0xA4, 0xFF),
    green: Color32::from_rgb(0x6F, 0xD3, 0xA8),
    red: Color32::from_rgb(0xF2, 0x8B, 0x8B),
    yellow: Color32::from_rgb(0xF0, 0xC4, 0x5C),
    teal: Color32::from_rgb(0x5C, 0xC8, 0xB8),
    orange: Color32::from_rgb(0xF2, 0xA2, 0x72),
    orange_text: Color32::from_rgb(0xF6, 0xBC, 0x97),
    blue: Color32::from_rgb(0x7A, 0xA2, 0xF7),
};

/// Gris verdoso con acento verde.
const FOREST: Palette = Palette {
    bg_tabs: Color32::from_rgb(0x0A, 0x0F, 0x0D),
    bg_code: Color32::from_rgb(0x0B, 0x10, 0x0E),
    bg: Color32::from_rgb(0x0E, 0x14, 0x12),
    bg_card: Color32::from_rgb(0x13, 0x1B, 0x18),
    bg_input: Color32::from_rgb(0x15, 0x20, 0x1C),
    bg_button: Color32::from_rgb(0x18, 0x22, 0x1E),
    bg_active: Color32::from_rgb(0x26, 0x36, 0x30),
    border: Color32::from_rgb(0x1E, 0x29, 0x25),
    border_soft: Color32::from_rgb(0x1A, 0x24, 0x20),
    border_strong: Color32::from_rgb(0x26, 0x36, 0x30),
    border_button: Color32::from_rgb(0x2C, 0x3D, 0x36),
    border_badge: Color32::from_rgb(0x3A, 0x4E, 0x46),
    text: Color32::from_rgb(0xE4, 0xEC, 0xE8),
    text_strong: Color32::WHITE,
    text_soft: Color32::from_rgb(0xC8, 0xD6, 0xCF),
    text_secondary: Color32::from_rgb(0xA8, 0xB8, 0xB0),
    text_date: Color32::from_rgb(0x99, 0xAA, 0xA1),
    text_muted: Color32::from_rgb(0x8B, 0x9C, 0x93),
    text_faint: Color32::from_rgb(0x6D, 0x80, 0x76),
    text_on_accent: Color32::from_rgb(0x0E, 0x1F, 0x18),
    accent: Color32::from_rgb(0x5F, 0xD6, 0xA4),
    link: Color32::from_rgb(0x86, 0xE3, 0xBC),
    green: Color32::from_rgb(0xA5, 0xDC, 0x7C),
    red: Color32::from_rgb(0xF2, 0x8B, 0x8B),
    yellow: Color32::from_rgb(0xE8, 0xC6, 0x5C),
    teal: Color32::from_rgb(0x5C, 0xC8, 0xD0),
    orange: Color32::from_rgb(0xF2, 0xA2, 0x72),
    orange_text: Color32::from_rgb(0xF6, 0xBC, 0x97),
    blue: Color32::from_rgb(0x7A, 0xB0, 0xF7),
};

/// Marrón cálido con acento naranja.
const EMBER: Palette = Palette {
    bg_tabs: Color32::from_rgb(0x0F, 0x0B, 0x09),
    bg_code: Color32::from_rgb(0x10, 0x0C, 0x0A),
    bg: Color32::from_rgb(0x14, 0x10, 0x0E),
    bg_card: Color32::from_rgb(0x1B, 0x16, 0x13),
    bg_input: Color32::from_rgb(0x20, 0x1A, 0x16),
    bg_button: Color32::from_rgb(0x22, 0x1C, 0x18),
    bg_active: Color32::from_rgb(0x36, 0x2B, 0x25),
    border: Color32::from_rgb(0x29, 0x22, 0x1E),
    border_soft: Color32::from_rgb(0x24, 0x1E, 0x1A),
    border_strong: Color32::from_rgb(0x36, 0x2B, 0x25),
    border_button: Color32::from_rgb(0x3D, 0x32, 0x2C),
    border_badge: Color32::from_rgb(0x4E, 0x41, 0x3A),
    text: Color32::from_rgb(0xEC, 0xE6, 0xE2),
    text_strong: Color32::WHITE,
    text_soft: Color32::from_rgb(0xD6, 0xCC, 0xC6),
    text_secondary: Color32::from_rgb(0xB8, 0xAC, 0xA5),
    text_date: Color32::from_rgb(0xAA, 0x9D, 0x96),
    text_muted: Color32::from_rgb(0x9C, 0x8F, 0x88),
    text_faint: Color32::from_rgb(0x80, 0x73, 0x6C),
    text_on_accent: Color32::from_rgb(0x1F, 0x14, 0x0E),
    accent: Color32::from_rgb(0xF5, 0xA2, 0x5D),
    link: Color32::from_rgb(0xFB, 0xBC, 0x87),
    green: Color32::from_rgb(0x8F, 0xD6, 0xA0),
    red: Color32::from_rgb(0xF2, 0x84, 0x80),
    yellow: Color32::from_rgb(0xEF, 0xCB, 0x68),
    teal: Color32::from_rgb(0x6C, 0xC9, 0xB4),
    orange: Color32::from_rgb(0xE8, 0x83, 0x6B),
    orange_text: Color32::from_rgb(0xF2, 0xA8, 0x94),
    blue: Color32::from_rgb(0x8A, 0xA8, 0xF0),
};

/// Piedra oscura y oro envejecido, con los colores de rareza de un ARPG.
const EXILE: Palette = Palette {
    bg_tabs: Color32::from_rgb(0x0A, 0x09, 0x08),
    bg_code: Color32::from_rgb(0x0B, 0x0A, 0x08),
    bg: Color32::from_rgb(0x0F, 0x0D, 0x0B),
    bg_card: Color32::from_rgb(0x1A, 0x16, 0x12),
    bg_input: Color32::from_rgb(0x1E, 0x19, 0x14),
    bg_button: Color32::from_rgb(0x21, 0x1B, 0x15),
    bg_active: Color32::from_rgb(0x2E, 0x26, 0x1D),
    border: Color32::from_rgb(0x2A, 0x22, 0x19),
    border_soft: Color32::from_rgb(0x22, 0x1C, 0x15),
    border_strong: Color32::from_rgb(0x3A, 0x2F, 0x22),
    border_button: Color32::from_rgb(0x4A, 0x3B, 0x28),
    border_badge: Color32::from_rgb(0x5A, 0x48, 0x30),
    text: Color32::from_rgb(0xC8, 0xC8, 0xC8),
    text_strong: Color32::from_rgb(0xF0, 0xE6, 0xD2),
    text_soft: Color32::from_rgb(0xBD, 0xB5, 0xA5),
    text_secondary: Color32::from_rgb(0xB2, 0xA9, 0x95),
    text_date: Color32::from_rgb(0xAD, 0xA2, 0x8B),
    text_muted: Color32::from_rgb(0xAA, 0x9E, 0x82),
    text_faint: Color32::from_rgb(0x7D, 0x73, 0x5E),
    text_on_accent: Color32::from_rgb(0x1A, 0x12, 0x09),
    accent: Color32::from_rgb(0xC8, 0xAA, 0x6E),
    link: Color32::from_rgb(0xE2, 0xCB, 0x9A),
    green: Color32::from_rgb(0x5F, 0xD0, 0x68),
    red: Color32::from_rgb(0xE0, 0x50, 0x50),
    yellow: Color32::from_rgb(0xE8, 0xE0, 0x70),
    teal: Color32::from_rgb(0x1B, 0xA2, 0x9B),
    orange: Color32::from_rgb(0xD0, 0x7A, 0x35),
    orange_text: Color32::from_rgb(0xE8, 0xA8, 0x78),
    blue: Color32::from_rgb(0x88, 0x88, 0xFF),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Violet,
    Forest,
    Ember,
    Exile,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Violet, Self::Forest, Self::Ember, Self::Exile];

    /// Nombre para mostrar.
    pub fn name(self) -> &'static str {
        match self {
            Self::Violet => "Violeta",
            Self::Forest => "Bosque",
            Self::Ember => "Brasa",
            Self::Exile => "Exilio",
        }
    }

    /// Identificador con el que se guarda la preferencia.
    pub fn id(self) -> &'static str {
        match self {
            Self::Violet => "violet",
            Self::Forest => "forest",
            Self::Ember => "ember",
            Self::Exile => "exile",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Violet => &VIOLET,
            Self::Forest => &FOREST,
            Self::Ember => &EMBER,
            Self::Exile => &EXILE,
        }
    }
}

/// Posición en `Theme::ALL` del tema activo.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn current() -> Theme {
    Theme::ALL[CURRENT.load(Ordering::Relaxed)]
}

/// Colores del tema activo.
pub fn colors() -> &'static Palette {
    current().palette()
}

/// Cambia el tema activo. Lo que se pinta a mano lo recoge en el próximo frame.
pub fn set(ctx: &egui::Context, theme: Theme) {
    let index = Theme::ALL.iter().position(|t| *t == theme).unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
    let visuals = visuals();
    ctx.all_styles_mut(|style| style.visuals = visuals.clone());
}

/// `color` mezclado con blanco en la proporción `amount` (0.0 a 1.0).
pub fn lighten(color: Color32, amount: f32) -> Color32 {
    color.lerp_to_gamma(Color32::WHITE, amount)
}

/// `color` con opacidad `alpha` (0.0 a 1.0).
pub fn alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8)
}

pub fn accent_soft() -> Color32 {
    alpha(colors().accent, 0.12)
}

pub fn accent_line() -> Color32 {
    alpha(colors().accent, 0.35)
}

/// Fondo de la fila seleccionada.
pub fn accent_selected() -> Color32 {
    alpha(colors().accent, 0.14)
}

/// Familia para títulos y texto seminegrita.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn font_semibold(size: f32) -> FontId {
    FontId::new(size, semibold())
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub const RADIUS: u8 = 8;

/// Aplica fuentes y estilo a todo el contexto.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    set(ctx, current());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, font(11.0)),
            (TextStyle::Body, font(13.0)),
            (TextStyle::Button, font(13.0)),
            (TextStyle::Heading, font_semibold(16.0)),
            (TextStyle::Monospace, mono(12.0)),
        ]
        .into();
        let spacing = &mut style.spacing;
        spacing.item_spacing = egui::vec2(8.0, 6.0);
        spacing.button_padding = egui::vec2(12.0, 5.0);
        spacing.interact_size.y = 26.0;
        spacing.window_margin = Margin::same(16);
        spacing.menu_margin = Margin::same(8);
    });
}

fn visuals() -> egui::Visuals {
    let c = colors();
    let mut v = egui::Visuals::dark();
    let radius = CornerRadius::same(RADIUS);
    v.panel_fill = c.bg;
    v.window_fill = c.bg_card;
    v.window_stroke = Stroke::new(1.0, c.border_strong);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = c.bg_input;
    v.text_edit_bg_color = Some(c.bg_input);
    v.faint_bg_color = c.bg_card;
    v.code_bg_color = c.bg_code;
    v.hyperlink_color = c.link;
    v.warn_fg_color = c.yellow;
    v.error_fg_color = c.red;
    v.selection.bg_fill = accent_selected();
    // Borde de los campos de texto activos.
    v.selection.stroke = Stroke::new(1.0, c.accent);
    v.override_text_color = None;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c.bg;
    w.noninteractive.weak_bg_fill = c.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, c.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = c.bg_button;
    w.inactive.weak_bg_fill = c.bg_button;
    w.inactive.bg_stroke = Stroke::new(1.0, c.border_button);
    w.inactive.fg_stroke = Stroke::new(1.0, c.text);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = c.bg_active;
    w.hovered.weak_bg_fill = c.bg_active;
    w.hovered.bg_stroke = Stroke::new(1.0, c.border_badge);
    w.hovered.fg_stroke = Stroke::new(1.0, c.text_strong);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = c.bg_active;
    w.active.weak_bg_fill = c.bg_active;
    w.active.bg_stroke = Stroke::new(1.0, accent_line());
    w.active.fg_stroke = Stroke::new(1.0, c.text_strong);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = c.bg_active;
    w.open.weak_bg_fill = c.bg_active;
    w.open.bg_stroke = Stroke::new(1.0, accent_line());
    w.open.fg_stroke = Stroke::new(1.0, c.text);
    w.open.corner_radius = radius;
    v
}

/// Usa Inter y JetBrains Mono si están instaladas; si no, las fuentes de egui.
fn fonts() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let files = font_files();
    let find = |names: &[&str]| -> Option<Vec<u8>> {
        names.iter().find_map(|name| {
            files
                .iter()
                .find(|p| p.file_name().is_some_and(|f| f.eq_ignore_ascii_case(name)))
                .and_then(|p| std::fs::read(p).ok())
        })
    };
    let add = |defs: &mut FontDefinitions, key: &str, data: Vec<u8>| {
        defs.font_data
            .insert(key.to_owned(), Arc::new(FontData::from_owned(data)));
    };

    let regular = find(&[
        "Inter-Regular.ttf",
        "Inter-Regular.otf",
        "InterVariable.ttf",
        "Inter.ttc",
    ]);
    let semibold_data = find(&["Inter-SemiBold.ttf", "Inter-SemiBold.otf"]);
    let mono = find(&["JetBrainsMono-Regular.ttf", "JetBrainsMonoNL-Regular.ttf"]);

    if let Some(data) = regular {
        add(&mut defs, "inter", data);
        families(&mut defs, FontFamily::Proportional).insert(0, "inter".into());
    }
    if let Some(data) = semibold_data {
        add(&mut defs, "inter-semibold", data);
        families(&mut defs, semibold()).push("inter-semibold".into());
    }
    // La seminegrita cae en la proporcional si no hay variante específica.
    let proportional = defs.families[&FontFamily::Proportional].clone();
    families(&mut defs, semibold()).extend(proportional);
    if let Some(data) = mono {
        add(&mut defs, "jetbrains-mono", data);
        families(&mut defs, FontFamily::Monospace).insert(0, "jetbrains-mono".into());
    }
    defs
}

fn families(defs: &mut FontDefinitions, family: FontFamily) -> &mut Vec<String> {
    defs.families.entry(family).or_default()
}

/// Archivos de fuentes en las carpetas habituales (Linux).
fn font_files() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
    }
    let mut files = Vec::new();
    for dir in dirs {
        collect(&dir, 0, &mut files);
    }
    files
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 3 {
                collect(&path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|e| {
            ["ttf", "otf", "ttc"]
                .iter()
                .any(|x| e.eq_ignore_ascii_case(x))
        }) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contraste WCAG entre dos colores opacos (de 1 a 21).
    fn contrast(a: Color32, b: Color32) -> f32 {
        let luminance = |c: Color32| {
            let linear = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn theme_ids_round_trip() {
        for theme in Theme::ALL {
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
        }
        assert_eq!(Theme::from_id("otro"), None);
    }

    #[test]
    fn text_is_readable_in_every_theme() {
        for theme in Theme::ALL {
            let c = theme.palette();
            let name = theme.name();
            assert!(contrast(c.text, c.bg) >= 7.0, "{name}: texto");
            assert!(contrast(c.text, c.bg_active) >= 7.0, "{name}: texto activo");
            assert!(contrast(c.text_muted, c.bg) >= 4.5, "{name}: texto apagado");
            assert!(
                contrast(c.text_muted, c.bg_card) >= 4.5,
                "{name}: en tarjeta"
            );
            assert!(
                contrast(c.text_on_accent, c.accent) >= 4.5,
                "{name}: acento"
            );
        }
    }
}
