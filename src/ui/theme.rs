//! Paleta, fuentes y estilo global (tema oscuro con acento violeta).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle,
};

// Fondos, de más oscuro a más claro.
pub const BG_TABS: Color32 = Color32::from_rgb(0x0A, 0x0B, 0x0F);
pub const BG_CODE: Color32 = Color32::from_rgb(0x0B, 0x0C, 0x10);
pub const BG: Color32 = Color32::from_rgb(0x0E, 0x0F, 0x14);
pub const BG_CARD: Color32 = Color32::from_rgb(0x13, 0x14, 0x1B);
pub const BG_INPUT: Color32 = Color32::from_rgb(0x15, 0x17, 0x20);
pub const BG_BUTTON: Color32 = Color32::from_rgb(0x18, 0x1A, 0x22);
pub const BG_ACTIVE: Color32 = Color32::from_rgb(0x26, 0x29, 0x36);

// Bordes.
pub const BORDER: Color32 = Color32::from_rgb(0x1E, 0x20, 0x29);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(0x1A, 0x1C, 0x24);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x26, 0x29, 0x36);
pub const BORDER_BUTTON: Color32 = Color32::from_rgb(0x2C, 0x2F, 0x3D);
pub const BORDER_BADGE: Color32 = Color32::from_rgb(0x3A, 0x3D, 0x4E);

// Texto.
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xEC);
pub const TEXT_STRONG: Color32 = Color32::WHITE;
pub const TEXT_SOFT: Color32 = Color32::from_rgb(0xC9, 0xCA, 0xD6);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xA9, 0xAA, 0xB8);
pub const TEXT_DATE: Color32 = Color32::from_rgb(0x9A, 0x9B, 0xAA);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8C, 0x8E, 0x9C);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6E, 0x70, 0x80);
/// Texto sobre el color de acento.
pub const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x14, 0x10, 0x1F);

// Acento y colores de estado.
pub const ACCENT: Color32 = Color32::from_rgb(0xA7, 0x8B, 0xFA);
pub const LINK: Color32 = Color32::from_rgb(0xB9, 0xA4, 0xFF);
pub const LINK_HOVER: Color32 = Color32::from_rgb(0xD6, 0xCB, 0xFF);
pub const GREEN: Color32 = Color32::from_rgb(0x6F, 0xD3, 0xA8);
pub const RED: Color32 = Color32::from_rgb(0xF2, 0x8B, 0x8B);
pub const YELLOW: Color32 = Color32::from_rgb(0xF0, 0xC4, 0x5C);
pub const TEAL: Color32 = Color32::from_rgb(0x5C, 0xC8, 0xB8);
pub const ORANGE: Color32 = Color32::from_rgb(0xF2, 0xA2, 0x72);
pub const ORANGE_TEXT: Color32 = Color32::from_rgb(0xF6, 0xBC, 0x97);
pub const BLUE: Color32 = Color32::from_rgb(0x7A, 0xA2, 0xF7);

/// `color` con opacidad `alpha` (0.0 a 1.0).
pub fn alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8)
}

pub fn accent_soft() -> Color32 {
    alpha(ACCENT, 0.12)
}

pub fn accent_line() -> Color32 {
    alpha(ACCENT, 0.35)
}

/// Fondo de la fila seleccionada.
pub fn accent_selected() -> Color32 {
    alpha(ACCENT, 0.14)
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
    ctx.set_visuals(visuals());
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
    let mut v = egui::Visuals::dark();
    let radius = CornerRadius::same(RADIUS);
    v.panel_fill = BG;
    v.window_fill = BG_CARD;
    v.window_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = BG_INPUT;
    v.text_edit_bg_color = Some(BG_INPUT);
    v.faint_bg_color = BG_CARD;
    v.code_bg_color = BG_CODE;
    v.hyperlink_color = LINK;
    v.warn_fg_color = YELLOW;
    v.error_fg_color = RED;
    v.selection.bg_fill = accent_selected();
    v.selection.stroke = Stroke::new(1.0, TEXT_STRONG);
    v.override_text_color = None;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = BG;
    w.noninteractive.weak_bg_fill = BG;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = BG_BUTTON;
    w.inactive.weak_bg_fill = BG_BUTTON;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER_BUTTON);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = BG_ACTIVE;
    w.hovered.weak_bg_fill = BG_ACTIVE;
    w.hovered.bg_stroke = Stroke::new(1.0, BORDER_BADGE);
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = BG_ACTIVE;
    w.active.weak_bg_fill = BG_ACTIVE;
    w.active.bg_stroke = Stroke::new(1.0, accent_line());
    w.active.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = BG_ACTIVE;
    w.open.weak_bg_fill = BG_ACTIVE;
    w.open.bg_stroke = Stroke::new(1.0, accent_line());
    w.open.fg_stroke = Stroke::new(1.0, TEXT);
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
