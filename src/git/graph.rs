//! Calcula cómo dibujar el grafo de commits: en qué columna ("carril")
//! va cada commit y qué líneas lo unen con sus padres.
//!
//! Cada fila se dibuja en dos mitades: de su borde superior al centro (donde
//! está el nodo) y del centro al borde inferior. Los carriles no se compactan,
//! así una rama mantiene su columna mientras está activa.

use super::log::Commit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// Del borde superior de la fila al centro.
    Top,
    /// Del centro al borde inferior.
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub half: Half,
    pub from: usize,
    pub to: usize,
    pub color: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    /// Carril del nodo del commit.
    pub lane: usize,
    pub color: usize,
    pub segments: Vec<Segment>,
    /// Número de carriles que ocupa la fila.
    pub width: usize,
}

#[derive(Debug, Clone)]
struct Lane {
    /// Commit que se espera encontrar más abajo en este carril.
    expects: String,
    color: usize,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub rows: Vec<GraphRow>,
    lanes: Vec<Option<Lane>>,
    next_color: usize,
}

impl Graph {
    #[cfg(test)]
    pub fn new(commits: &[Commit]) -> Self {
        let mut graph = Self::default();
        graph.extend(commits);
        graph
    }

    /// Añade filas para `commits`, que continúan a los ya procesados.
    pub fn extend(&mut self, commits: &[Commit]) {
        for commit in commits {
            let row = self.push(commit);
            self.rows.push(row);
        }
    }

    pub fn max_width(&self) -> usize {
        self.rows.iter().map(|r| r.width).max().unwrap_or(0)
    }

    fn new_color(&mut self) -> usize {
        let c = self.next_color;
        self.next_color += 1;
        c
    }

    fn free_slot(&mut self, exclude: usize) -> usize {
        match (0..self.lanes.len()).find(|&i| i != exclude && self.lanes[i].is_none()) {
            Some(i) => i,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    fn push(&mut self, commit: &Commit) -> GraphRow {
        let mut segments = Vec::new();

        let waiting: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| l.as_ref().is_some_and(|l| l.expects == commit.id))
            .map(|(i, _)| i)
            .collect();

        // Carril y color del commit.
        let (lane, color) = match waiting.first() {
            Some(&i) => (i, self.lanes[i].as_ref().unwrap().color),
            None => {
                let i = self.free_slot(usize::MAX);
                (i, self.new_color())
            }
        };

        // Mitad superior: las ramas que esperaban este commit convergen en él;
        // el resto sigue recto.
        for (i, slot) in self.lanes.iter().enumerate() {
            if let Some(l) = slot {
                let to = if waiting.contains(&i) { lane } else { i };
                segments.push(Segment {
                    half: Half::Top,
                    from: i,
                    to,
                    color: l.color,
                });
            }
        }
        for &i in &waiting {
            self.lanes[i] = None;
        }
        if lane >= self.lanes.len() {
            self.lanes.resize(lane + 1, None);
        }

        // Padres: el primero continúa en el mismo carril; los demás (merges)
        // se unen a un carril que ya los espera o abren uno nuevo.
        let mut parent_targets = Vec::new();
        if let Some(first) = commit.parents.first() {
            self.lanes[lane] = Some(Lane {
                expects: first.clone(),
                color,
            });
            parent_targets.push((lane, color));
        }
        for parent in commit.parents.iter().skip(1) {
            let existing = self
                .lanes
                .iter()
                .position(|l| l.as_ref().is_some_and(|l| &l.expects == parent));
            let target = match existing {
                Some(i) => (i, self.lanes[i].as_ref().unwrap().color),
                None => {
                    let i = self.free_slot(lane);
                    let c = self.new_color();
                    self.lanes[i] = Some(Lane {
                        expects: parent.clone(),
                        color: c,
                    });
                    (i, c)
                }
            };
            parent_targets.push(target);
        }

        // Mitad inferior: las ramas que pasan siguen rectas y el commit
        // se une a cada padre.
        for (i, slot) in self.lanes.iter().enumerate() {
            if let Some(l) = slot
                && !parent_targets.iter().any(|&(t, _)| t == i)
            {
                segments.push(Segment {
                    half: Half::Bottom,
                    from: i,
                    to: i,
                    color: l.color,
                });
            }
        }
        for &(target, c) in &parent_targets {
            segments.push(Segment {
                half: Half::Bottom,
                from: lane,
                to: target,
                // Una línea que sale hacia otro carril toma el color de ese carril.
                color: if target == lane { color } else { c },
            });
        }

        let width = segments
            .iter()
            .map(|s| s.from.max(s.to) + 1)
            .chain([lane + 1])
            .max()
            .unwrap_or(1);

        while matches!(self.lanes.last(), Some(None)) {
            self.lanes.pop();
        }

        GraphRow {
            lane,
            color,
            segments,
            width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            short_id: id.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author_name: String::new(),
            author_email: String::new(),
            time: 0,
            refs: Vec::new(),
            summary: String::new(),
        }
    }

    fn has(row: &GraphRow, half: Half, from: usize, to: usize) -> bool {
        row.segments
            .iter()
            .any(|s| s.half == half && s.from == from && s.to == to)
    }

    #[test]
    fn linear_history_uses_one_lane() {
        let g = Graph::new(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
        assert!(g.rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert!(has(&g.rows[0], Half::Bottom, 0, 0));
        assert!(!has(&g.rows[0], Half::Top, 0, 0));
        assert!(has(&g.rows[1], Half::Top, 0, 0));
        // La raíz no tiene línea hacia abajo.
        assert!(g.rows[2].segments.iter().all(|s| s.half == Half::Top));
        let colors: Vec<_> = g.rows.iter().map(|r| r.color).collect();
        assert_eq!(colors, [0, 0, 0]);
    }

    #[test]
    fn merge_opens_and_closes_a_lane() {
        // m = merge(c, b); c y b salen de a.
        let g = Graph::new(&[
            commit("m", &["c", "b"]),
            commit("c", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        let [m, c, b, a] = &g.rows[..] else { panic!() };
        assert_eq!((m.lane, c.lane, b.lane, a.lane), (0, 0, 1, 0));
        assert!(has(m, Half::Bottom, 0, 0));
        assert!(has(m, Half::Bottom, 0, 1));
        // c continúa en 0 mientras b pasa por 1.
        assert!(has(c, Half::Top, 1, 1) && has(c, Half::Bottom, 1, 1));
        // b converge con a en el carril 0.
        assert!(has(a, Half::Top, 0, 0) && has(a, Half::Top, 1, 0));
        assert_eq!(a.width, 2);
        assert_ne!(b.color, c.color);
        assert_eq!(g.max_width(), 2);
    }

    #[test]
    fn independent_branch_tips_get_new_lanes() {
        // Dos ramas que no se unen todavía: x (feature) y m (main), desde a.
        let g = Graph::new(&[commit("x", &["a"]), commit("m", &["a"]), commit("a", &[])]);
        assert_eq!(g.rows[0].lane, 0);
        assert_eq!(g.rows[1].lane, 1);
        assert!(has(&g.rows[1], Half::Top, 0, 0));
        assert!(has(&g.rows[2], Half::Top, 0, 0) && has(&g.rows[2], Half::Top, 1, 0));
        // Al terminar no queda ningún carril abierto.
        assert!(g.lanes.is_empty());
    }

    #[test]
    fn extend_matches_full_build() {
        let commits = [
            commit("m", &["c", "b"]),
            commit("c", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ];
        let full = Graph::new(&commits);
        let mut paged = Graph::new(&commits[..2]);
        paged.extend(&commits[2..]);
        assert_eq!(full.rows, paged.rows);
    }
}
