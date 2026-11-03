use eframe::egui::{self, Align2, Color32, Key, Rect, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use super::graph_view;
use super::icons::{self, Icon};
use super::theme;
use crate::git::{Commit, Graph, RefKind, RefLabel};

pub const ROW_HEIGHT: f32 = 34.0;
const HEADER_HEIGHT: f32 = 32.0;
const HASH_WIDTH: f32 = 96.0;
const DATE_WIDTH: f32 = 150.0;
const AUTHOR_WIDTH: f32 = 190.0;
const RIGHT_PADDING: f32 = 20.0;
const MIN_GRAPH_WIDTH: f32 = 76.0;
const GAP: f32 = 6.0;
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
struct Layout<'a> {
    worktree: bool,
    commits: usize,
    /// Si hay un filtro, los índices de los commits visibles.
    visible: Option<&'a [usize]>,
}

impl Layout<'_> {
    fn commit_rows(self) -> usize {
        self.visible.map_or(self.commits, <[usize]>::len)
    }

    fn len(self) -> usize {
        self.commit_rows() + self.worktree as usize
    }

    fn row(self, index: usize) -> ListRow {
        let commit = |i: usize| ListRow::Commit(self.visible.map_or(i, |v| v[i]));
        match (self.worktree, index) {
            (true, 0) => ListRow::WorkingTree,
            (true, i) => commit(i - 1),
            (false, i) => commit(i),
        }
    }

    fn index(self, row: ListRow) -> Option<usize> {
        match row {
            ListRow::WorkingTree => self.worktree.then_some(0),
            ListRow::Commit(i) => {
                let pos = match self.visible {
                    Some(v) => v.iter().position(|&x| x == i)?,
                    None => i,
                };
                Some(pos + self.worktree as usize)
            }
        }
    }
}

/// Posiciones de las columnas en una fila.
struct Columns {
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
            graph: rect.left(),
            desc: rect.left() + graph_width,
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
    /// Con `visible` solo se muestran esos commits (búsqueda) y no se dibuja el grafo.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        commits: &[Commit],
        graph: &Graph,
        selected: Option<ListRow>,
        worktree_changes: Option<usize>,
        visible: Option<&[usize]>,
    ) -> ListResponse {
        let layout = Layout {
            worktree: worktree_changes.is_some(),
            commits: commits.len(),
            visible,
        };
        let selected_index = selected.and_then(|r| layout.index(r));
        let mut response = ListResponse::default();
        if let Some(new) = self.keyboard(ui, layout.len(), selected_index) {
            response.selected = Some(layout.row(new));
            self.scroll_to = Some(layout.row(new));
        }

        let show_graph = visible.is_none();
        let graph_width = if show_graph {
            graph_view::width(graph.max_width()).max(MIN_GRAPH_WIDTH)
        } else {
            20.0
        };
        header(ui, graph_width, show_graph);

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
                        graph.rows.get(i).filter(|_| show_graph),
                        graph_width,
                        is_selected,
                        resp.hovered(),
                    ),
                }
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
fn header(ui: &mut Ui, graph_width: f32, show_graph: bool) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER_SOFT),
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
            theme::TEXT_MUTED,
        );
    };
    if show_graph {
        text(cols.graph + 20.0, "GRAFO");
    }
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
        theme::BG_ACTIVE
    } else if hovered {
        ui.painter().rect_filled(rect, 0.0, theme::BG_CARD);
        theme::BG_CARD
    } else {
        theme::BG
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
    let graph_rect = Rect::from_min_size(rect.min, vec2(graph_width, rect.height()));
    graph_view::paint_worktree_node(&painter, graph_rect, bg);

    let color = if selected {
        theme::TEXT_STRONG
    } else {
        theme::TEXT_SECONDARY
    };
    let text = match changes {
        1 => "Cambios sin commitear (1 archivo)".to_owned(),
        n => format!("Cambios sin commitear ({n} archivos)"),
    };
    painter.text(
        pos2(rect.left() + graph_width, rect.center().y),
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
    graph_row: Option<&crate::git::GraphRow>,
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
        let graph_rect = Rect::from_min_size(rect.min, vec2(graph_width, rect.height()));
        graph_view::paint_row(&painter, graph_rect, row, is_head, selected, bg);
    }

    // Hash.
    painter.text(
        pos2(cols.hash, y),
        Align2::LEFT_CENTER,
        &commit.short_id,
        theme::mono(12.0),
        if selected {
            theme::ACCENT
        } else {
            theme::TEXT_MUTED
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
            theme::TEXT_DATE,
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
        theme::TEXT_SOFT,
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
            theme::TEXT_STRONG
        } else {
            theme::TEXT
        },
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
    const COLORS: [(Color32, Color32); 6] = [
        (theme::ACCENT, Color32::from_rgb(0xD6, 0xCB, 0xFF)),
        (theme::TEAL, Color32::from_rgb(0x7F, 0xDC, 0xCD)),
        (theme::ORANGE, theme::ORANGE_TEXT),
        (theme::BLUE, Color32::from_rgb(0xA9, 0xC4, 0xFA)),
        (theme::GREEN, Color32::from_rgb(0xA8, 0xEB, 0xCD)),
        (theme::YELLOW, Color32::from_rgb(0xF6, 0xD9, 0x96)),
    ];
    let hash = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let (base, fg) = COLORS[hash as usize % COLORS.len()];
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
            fill: theme::ACCENT,
            stroke: Stroke::NONE,
            text: theme::TEXT_ON_ACCENT,
            icon: Some(Icon::BranchSmall),
        },
        RefKind::Branch { current: false } => {
            soft(theme::ORANGE, theme::ORANGE_TEXT, Some(Icon::BranchSmall))
        }
        RefKind::Remote => RefStyle {
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(1.0, theme::BORDER_BADGE),
            text: theme::TEXT_SOFT,
            icon: Some(Icon::Cloud),
        },
        RefKind::Tag => soft(theme::YELLOW, theme::YELLOW, None),
        RefKind::Head => soft(theme::RED, theme::RED, None),
    }
}
