use eframe::egui::{self, Align2, Color32, Key, Rect, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use super::graph_view;
use super::icons::{self, Icon};
use super::theme;
use crate::git::{Commit, Graph, GraphRow, RefKind, RefLabel};

pub const ROW_HEIGHT: f32 = 34.0;
const HEADER_HEIGHT: f32 = 32.0;
const HASH_WIDTH: f32 = 96.0;
const DATE_WIDTH: f32 = 150.0;
const AUTHOR_WIDTH: f32 = 190.0;
const RIGHT_PADDING: f32 = 20.0;
const MIN_GRAPH_WIDTH: f32 = 76.0;
const GAP: f32 = 6.0;
/// Columna con la rama de cada commit, a la izquierda del grafo.
const BRANCH_WIDTH: f32 = 150.0;
/// Opacidad de la rama de un commit que no es la punta (solo al pasar el ratón).
const INHERITED_OPACITY: f32 = 0.4;
/// Opacidad de las filas que no coinciden con la búsqueda.
const DIMMED_OPACITY: f32 = 0.3;
/// Cuántas filas antes del final se pide la siguiente página.
const PREFETCH_ROWS: usize = 100;

/// Fila de la lista: la de cambios sin commitear o un commit (por índice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListRow {
    WorkingTree,
    Commit(usize),
}

#[derive(Default)]
pub struct CommitList {
    /// Fila que hay que hacer visible en el próximo frame.
    scroll_to: Option<ListRow>,
    last_offset: f32,
    last_height: f32,
}

/// Acciones del menú contextual de un commit.
pub enum ContextAction {
    CreateBranch(usize),
    SwitchBranch(String),
    /// Borrar una rama local (nunca remota) por nombre.
    DeleteBranch(String),
    Checkout(usize),
}

#[derive(Default)]
pub struct ListResponse {
    /// Nueva fila seleccionada (con el ratón o el teclado).
    pub selected: Option<ListRow>,
    pub context: Option<ContextAction>,
    /// La vista está cerca del final: conviene cargar más commits.
    pub wants_more: bool,
}

/// Convierte entre filas lógicas y posiciones en pantalla.
#[derive(Clone, Copy)]
struct Layout {
    worktree: bool,
    commits: usize,
}

impl Layout {
    fn len(self) -> usize {
        self.commits + self.worktree as usize
    }

    fn row(self, index: usize) -> ListRow {
        match (self.worktree, index) {
            (true, 0) => ListRow::WorkingTree,
            (true, i) => ListRow::Commit(i - 1),
            (false, i) => ListRow::Commit(i),
        }
    }

    fn index(self, row: ListRow) -> Option<usize> {
        match row {
            ListRow::WorkingTree => self.worktree.then_some(0),
            ListRow::Commit(i) => Some(i + self.worktree as usize),
        }
    }
}

/// Posiciones de las columnas en una fila.
struct Columns {
    branch: f32,
    graph: f32,
    desc: f32,
    author: f32,
    date: f32,
    hash: f32,
}

impl Columns {
    fn new(rect: Rect, graph_width: f32) -> Self {
        let hash = rect.right() - RIGHT_PADDING - HASH_WIDTH;
        let date = hash - DATE_WIDTH;
        let author = date - AUTHOR_WIDTH;
        Self {
            branch: rect.left(),
            graph: rect.left() + BRANCH_WIDTH,
            desc: rect.left() + BRANCH_WIDTH + graph_width,
            author,
            date,
            hash,
        }
    }
}

impl CommitList {
    /// Hace visible la fila `row` en el próximo frame.
    pub fn scroll_to(&mut self, row: ListRow) {
        self.scroll_to = Some(row);
    }

    /// Dibuja la lista. `worktree_changes` es el número de archivos con cambios
    /// sin commitear; si es `Some`, se añade una fila para ellos arriba.
    /// Con `matches` (índices ordenados, de una búsqueda) el resto de filas se atenúa.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        commits: &[Commit],
        graph: &Graph,
        selected: Option<ListRow>,
        worktree_changes: Option<usize>,
        matches: Option<&[usize]>,
    ) -> ListResponse {
        let layout = Layout {
            worktree: worktree_changes.is_some(),
            commits: commits.len(),
        };
        let selected_index = selected.and_then(|r| layout.index(r));
        let mut response = ListResponse::default();
        if let Some(new) = self.keyboard(ui, layout.len(), selected_index) {
            response.selected = Some(layout.row(new));
            self.scroll_to = Some(layout.row(new));
        }

        let graph_width = graph_view::width(graph.max_width()).max(MIN_GRAPH_WIDTH);
        header(ui, graph_width);

        let mut area = ScrollArea::vertical().auto_shrink(false);
        if let Some(row) = self.scroll_to.take().and_then(|r| layout.index(r)) {
            let top = row as f32 * ROW_HEIGHT;
            let bottom = top + ROW_HEIGHT;
            if top < self.last_offset {
                area = area.vertical_scroll_offset(top);
            } else if bottom > self.last_offset + self.last_height {
                area = area.vertical_scroll_offset(bottom - self.last_height);
            }
        }

        let output = area.show_rows(ui, ROW_HEIGHT, layout.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if range.end + PREFETCH_ROWS >= layout.len() {
                response.wants_more = true;
            }
            for index in range {
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
                let row = layout.row(index);
                let is_selected = response.selected.or(selected) == Some(row);
                let matched = match (matches, row) {
                    (None, _) => true,
                    (Some(m), ListRow::Commit(i)) => m.binary_search(&i).is_ok(),
                    (Some(_), ListRow::WorkingTree) => false,
                };
                let opacity = ui.opacity();
                if !matched && !is_selected {
                    ui.multiply_opacity(DIMMED_OPACITY);
                }
                match row {
                    ListRow::WorkingTree => paint_worktree_row(
                        ui,
                        rect,
                        worktree_changes.unwrap_or(0),
                        graph_width,
                        is_selected,
                        resp.hovered(),
                    ),
                    ListRow::Commit(i) => paint_row(
                        ui,
                        rect,
                        &commits[i],
                        graph.rows.get(i),
                        graph_width,
                        is_selected,
                        resp.hovered(),
                    ),
                }
                if let ListRow::Commit(i) = row
                    && let Some(graph_row) = graph.rows.get(i)
                {
                    paint_branch_cell(ui, rect, graph_row, graph_width, resp.hovered());
                }
                ui.set_opacity(opacity);
                if resp.clicked() {
                    response.selected = Some(row);
                }
                if let ListRow::Commit(i) = row {
                    resp.context_menu(|ui| {
                        if ui.button("Crear rama aquí…").clicked() {
                            response.context = Some(ContextAction::CreateBranch(i));
                            ui.close();
                        }
                        for label in &commits[i].refs {
                            if let RefKind::Branch { current: false } = label.kind
                                && ui.button(format!("Cambiar a {}", label.name)).clicked()
                            {
                                response.context =
                                    Some(ContextAction::SwitchBranch(label.name.clone()));
                                ui.close();
                            }
                        }
                        // Con HEAD separado en este commit no hay adónde moverse.
                        let at_head = commits[i].refs.iter().any(|r| r.kind == RefKind::Head);
                        if !at_head
                            && ui
                                .button("Hacer checkout de este commit")
                                .on_hover_text("Mueve HEAD a este commit, sin rama (HEAD separado)")
                                .clicked()
                        {
                            response.context = Some(ContextAction::Checkout(i));
                            ui.close();
                        }
                        // Solo ramas locales que no son la actual; las remotas no se borran.
                        let deletable = commits[i]
                            .refs
                            .iter()
                            .filter(|r| r.kind == RefKind::Branch { current: false });
                        let mut first = true;
                        for label in deletable {
                            if std::mem::take(&mut first) {
                                ui.separator();
                            }
                            if ui.button(format!("Borrar rama {}…", label.name)).clicked() {
                                response.context =
                                    Some(ContextAction::DeleteBranch(label.name.clone()));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Copiar hash").clicked() {
                            ui.ctx().copy_text(commits[i].id.clone());
                            ui.close();
                        }
                    });
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
        (Some(new) != selected).then_some(new)
    }
}

/// Cabecera con los nombres de las columnas.
fn header(ui: &mut Ui, graph_width: f32) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::colors().border_soft),
    );
    let cols = Columns::new(rect, graph_width);
    let y = rect.center().y;
    let font = theme::font_semibold(11.0);
    let text = |x: f32, s: &str| {
        painter.text(
            pos2(x, y),
            Align2::LEFT_CENTER,
            s,
            font.clone(),
            theme::colors().text_muted,
        );
    };
    text(cols.branch + 16.0, "RAMA");
    text(cols.graph + 20.0, "GRAFO");
    text(cols.desc, "DESCRIPCIÓN");
    text(cols.author, "AUTOR");
    text(cols.date, "FECHA");
    text(cols.hash, "COMMIT");
}

fn row_background(ui: &Ui, rect: Rect, selected: bool, hovered: bool) -> Color32 {
    if selected {
        ui.painter()
            .rect_filled(rect, 0.0, theme::accent_selected());
        // Mezcla aproximada del acento sobre el fondo, para rellenar los nodos.
        theme::colors().bg_active
    } else if hovered {
        ui.painter().rect_filled(rect, 0.0, theme::colors().bg_card);
        theme::colors().bg_card
    } else {
        theme::colors().bg
    }
}

fn paint_worktree_row(
    ui: &Ui,
    rect: Rect,
    changes: usize,
    graph_width: f32,
    selected: bool,
    hovered: bool,
) {
    let bg = row_background(ui, rect, selected, hovered);
    let painter = ui.painter_at(rect);
    let cols = Columns::new(rect, graph_width);
    let graph_rect = Rect::from_min_size(
        pos2(cols.graph, rect.top()),
        vec2(graph_width, rect.height()),
    );
    graph_view::paint_worktree_node(&painter, graph_rect, bg);

    let color = if selected {
        theme::colors().text_strong
    } else {
        theme::colors().text_secondary
    };
    let text = match changes {
        1 => "Cambios sin commitear (1 archivo)".to_owned(),
        n => format!("Cambios sin commitear ({n} archivos)"),
    };
    painter.text(
        pos2(cols.desc, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        theme::font(13.0),
        color,
    );
}

fn paint_row(
    ui: &Ui,
    rect: Rect,
    commit: &Commit,
    graph_row: Option<&GraphRow>,
    graph_width: f32,
    selected: bool,
    hovered: bool,
) {
    let bg = row_background(ui, rect, selected, hovered);
    let painter = ui.painter_at(rect);
    let cols = Columns::new(rect, graph_width);
    let y = rect.center().y;

    if let Some(row) = graph_row {
        let is_head = commit
            .refs
            .iter()
            .any(|r| matches!(r.kind, RefKind::Head | RefKind::Branch { current: true }));
        let graph_rect = Rect::from_min_size(
            pos2(cols.graph, rect.top()),
            vec2(graph_width, rect.height()),
        );
        graph_view::paint_row(&painter, graph_rect, row, is_head, selected, bg);
    }

    // Hash.
    painter.text(
        pos2(cols.hash, y),
        Align2::LEFT_CENTER,
        &commit.short_id,
        theme::mono(12.0),
        if selected {
            theme::colors().accent
        } else {
            theme::colors().text_muted
        },
    );

    // Fecha.
    painter
        .with_clip_rect(Rect::from_x_y_ranges(
            cols.date..=cols.hash - GAP,
            rect.y_range(),
        ))
        .text(
            pos2(cols.date, y),
            Align2::LEFT_CENTER,
            super::format_time(commit.time),
            theme::font(13.0),
            theme::colors().text_date,
        );

    // Autor con avatar.
    let author_painter = painter.with_clip_rect(Rect::from_x_y_ranges(
        cols.author..=cols.date - GAP,
        rect.y_range(),
    ));
    let avatar_center = pos2(cols.author + 10.0, y);
    avatar(&author_painter, avatar_center, 10.0, &commit.author_name);
    author_painter.text(
        pos2(cols.author + 28.0, y),
        Align2::LEFT_CENTER,
        &commit.author_name,
        theme::font(13.0),
        theme::colors().text_soft,
    );

    // Etiquetas de ramas/tags y mensaje.
    let desc_clip = Rect::from_x_y_ranges(cols.desc..=cols.author - GAP * 2.0, rect.y_range());
    let painter = painter.with_clip_rect(desc_clip);
    let mut x = cols.desc;
    for label in &commit.refs {
        x = paint_ref(&painter, label, x, y) + GAP;
    }
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        &commit.summary,
        theme::font(13.0),
        if selected {
            theme::colors().text_strong
        } else {
            theme::colors().text
        },
    );
}

/// Columna de rama: la punta de cada rama siempre; el resto solo al pasar el
/// ratón y más transparente.
fn paint_branch_cell(ui: &Ui, rect: Rect, row: &GraphRow, graph_width: f32, hovered: bool) {
    let Some(name) = &row.branch else { return };
    if !(row.tip || hovered) {
        return;
    }
    let cols = Columns::new(rect, graph_width);
    let opacity = if row.tip { 1.0 } else { INHERITED_OPACITY };
    let clip = Rect::from_x_y_ranges(cols.branch..=cols.graph - GAP, rect.y_range());
    paint_branch(
        &ui.painter_at(clip),
        name,
        graph_view::color(row.color),
        cols.branch + 10.0,
        cols.graph - GAP,
        rect.center().y,
        opacity,
    );
}

/// Etiqueta con el nombre de la rama en el color de su carril, recortada con
/// "…" si no cabe entre `x` y `max_x`.
fn paint_branch(
    painter: &egui::Painter,
    name: &str,
    color: Color32,
    x: f32,
    max_x: f32,
    y: f32,
    opacity: f32,
) {
    const PAD: f32 = 7.0;
    const ICON: f32 = 10.0 + 4.0;
    let text_color = color.gamma_multiply(opacity);
    let max_text = (max_x - x - PAD * 2.0 - ICON).max(0.0);
    let mut text = name.to_owned();
    let mut galley = painter.layout_no_wrap(text.clone(), theme::font_semibold(11.0), text_color);
    while galley.size().x > max_text && text.chars().count() > 1 {
        text.pop();
        let shown = format!("{}…", text.trim_end());
        galley = painter.layout_no_wrap(shown, theme::font_semibold(11.0), text_color);
    }
    let rect = Rect::from_min_size(
        pos2(x, y - 10.0),
        vec2(PAD + ICON + galley.size().x + PAD, 20.0),
    );
    painter.rect(
        rect,
        5.0,
        theme::alpha(color, 0.14 * opacity),
        Stroke::new(1.0, theme::alpha(color, 0.4 * opacity)),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        painter,
        Rect::from_center_size(pos2(rect.left() + PAD + 5.0, y), vec2(10.0, 10.0)),
        Icon::BranchSmall,
        text_color,
        2.4,
    );
    painter.galley(
        pos2(rect.left() + PAD + ICON, y - galley.size().y / 2.0),
        galley,
        text_color,
    );
}

/// Iniciales del autor en un círculo de color estable para cada nombre.
pub fn avatar(painter: &egui::Painter, center: egui::Pos2, radius: f32, name: &str) {
    let (bg, fg) = avatar_colors(name);
    painter.circle_filled(center, radius, bg);
    painter.text(
        center,
        Align2::CENTER_CENTER,
        initials(name),
        theme::font_semibold(radius * 0.9),
        fg,
    );
}

fn avatar_colors(name: &str) -> (Color32, Color32) {
    let c = theme::colors();
    let colors = [
        (c.accent, theme::lighten(c.accent, 0.45)),
        (c.teal, theme::lighten(c.teal, 0.45)),
        (c.orange, c.orange_text),
        (c.blue, theme::lighten(c.blue, 0.45)),
        (c.green, theme::lighten(c.green, 0.45)),
        (c.yellow, theme::lighten(c.yellow, 0.45)),
    ];
    let hash = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let (base, fg) = colors[hash as usize % colors.len()];
    (theme::alpha(base, 0.2), fg)
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let first = |w: &str| w.chars().next().map(|c| c.to_uppercase().to_string());
    match words.as_slice() {
        [] => "?".to_owned(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        [a, .., b] => format!(
            "{}{}",
            first(a).unwrap_or_default(),
            first(b).unwrap_or_default()
        ),
    }
}

/// Dibuja una etiqueta de referencia y devuelve la x donde termina.
fn paint_ref(painter: &egui::Painter, label: &RefLabel, x: f32, y: f32) -> f32 {
    let style = ref_style(label.kind);
    let galley = painter.layout_no_wrap(label.name.clone(), theme::font_semibold(11.0), style.text);
    let icon_width = if style.icon.is_some() {
        10.0 + 4.0
    } else {
        0.0
    };
    let rect = Rect::from_min_size(
        pos2(x, y - 10.0),
        vec2(7.0 + icon_width + galley.size().x + 7.0, 20.0),
    );
    painter.rect(
        rect,
        5.0,
        style.fill,
        style.stroke,
        egui::StrokeKind::Inside,
    );
    let mut tx = rect.left() + 7.0;
    if let Some(icon) = style.icon {
        icons::paint(
            painter,
            Rect::from_center_size(pos2(tx + 5.0, y), vec2(10.0, 10.0)),
            icon,
            style.text,
            2.4,
        );
        tx += icon_width;
    }
    painter.galley(pos2(tx, y - galley.size().y / 2.0), galley, style.text);
    rect.right()
}

struct RefStyle {
    fill: Color32,
    stroke: Stroke,
    text: Color32,
    icon: Option<Icon>,
}

fn ref_style(kind: RefKind) -> RefStyle {
    let soft = |c: Color32, text: Color32, icon| RefStyle {
        fill: theme::alpha(c, 0.14),
        stroke: Stroke::new(1.0, theme::alpha(c, 0.4)),
        text,
        icon,
    };
    match kind {
        RefKind::Branch { current: true } => RefStyle {
            fill: theme::colors().accent,
            stroke: Stroke::NONE,
            text: theme::colors().text_on_accent,
            icon: Some(Icon::BranchSmall),
        },
        RefKind::Branch { current: false } => soft(
            theme::colors().orange,
            theme::colors().orange_text,
            Some(Icon::BranchSmall),
        ),
        RefKind::Remote => RefStyle {
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(1.0, theme::colors().border_badge),
            text: theme::colors().text_soft,
            icon: Some(Icon::Cloud),
        },
        RefKind::Tag => soft(theme::colors().yellow, theme::colors().yellow, None),
        RefKind::Head => soft(theme::colors().red, theme::colors().red, None),
    }
}
