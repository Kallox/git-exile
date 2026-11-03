//! Calcula cómo dibujar el grafo de commits: en qué columna ("carril")
//! va cada commit y qué líneas lo unen con sus padres.
//!
//! Cada fila se dibuja en dos mitades: de su borde superior al centro (donde
//! está el nodo) y del centro al borde inferior. Los carriles no se compactan,
//! así una rama mantiene su columna mientras está activa.

use super::log::{Commit, RefKind};

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
    /// Rama a la que pertenece el commit: la suya si es la punta de una rama,
    /// o la heredada del carril por el que llega.
    pub branch: Option<String>,
    /// El commit es la punta (el más reciente) de `branch`.
    pub tip: bool,
}

#[derive(Debug, Clone)]
struct Lane {
    /// Commit que se espera encontrar más abajo en este carril.
    expects: String,
    color: usize,
    /// Rama que recorre este carril, si se conoce.
    branch: Option<String>,
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

        // Rama: la propia si el commit es una punta; si no, la del carril.
        let own = tip_branch(commit);
        let inherited = waiting
            .iter()
            .find_map(|&i| self.lanes[i].as_ref().and_then(|l| l.branch.clone()));
        let tip = own.is_some();
        let branch = own.clone().or_else(|| inherited.clone());
        let lane_branch = inherited.or(own);

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
                branch: lane_branch,
            });
            parent_targets.push((lane, color));
        }
        // El segundo padre de un merge suele ser la rama nombrada en el mensaje.
        let merged = merged_branch(&commit.summary);
        for (n, parent) in commit.parents.iter().enumerate().skip(1) {
            let name = if n == 1 { merged.clone() } else { None };
            let existing = self
                .lanes
                .iter()
                .position(|l| l.as_ref().is_some_and(|l| &l.expects == parent));
            let target = match existing {
                Some(i) => {
                    let l = self.lanes[i].as_mut().unwrap();
                    if l.branch.is_none() {
                        l.branch = name;
                    }
                    (i, l.color)
                }
                None => {
                    let i = self.free_slot(lane);
                    let c = self.new_color();
                    self.lanes[i] = Some(Lane {
                        expects: parent.clone(),
                        color: c,
                        branch: name,
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
            branch,
            tip,
        }
    }
}

/// Rama de la que `commit` es la punta: la actual, luego una local y por
/// último una remota.
fn tip_branch(commit: &Commit) -> Option<String> {
    commit
        .refs
        .iter()
        .filter_map(|r| {
            let rank = match r.kind {
                RefKind::Branch { current: true } => 0,
                RefKind::Branch { current: false } => 1,
                RefKind::Remote if !r.name.ends_with("/HEAD") => 2,
                _ => return None,
            };
            Some((rank, &r.name))
        })
        .min_by_key(|&(rank, _)| rank)
        .map(|(_, name)| name.clone())
}

/// Rama fusionada según el mensaje por defecto de `git merge` o de un pull
/// request: `Merge branch 'x'`, `Merge remote-tracking branch 'origin/x'`,
/// `Merge pull request #1 from user/x`.
fn merged_branch(summary: &str) -> Option<String> {
    let quoted = |rest: &str| {
        rest.strip_prefix('\'')?
            .split('\'')
            .next()
            .map(str::to_owned)
    };
    if let Some(rest) = summary.strip_prefix("Merge branch ") {
        quoted(rest)
    } else if let Some(rest) = summary.strip_prefix("Merge remote-tracking branch ") {
        quoted(rest)
    } else if let Some(rest) = summary.strip_prefix("Merge pull request ") {
        let from = rest.split_once(" from ")?.1.split_whitespace().next()?;
        Some(from.split_once('/').map_or(from, |(_, b)| b).to_owned())
    } else {
        None
    }
    .filter(|b| !b.is_empty())
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

    fn with_ref(mut c: Commit, name: &str, kind: RefKind) -> Commit {
        c.refs.push(crate::git::RefLabel {
            name: name.into(),
            kind,
        });
        c
    }

    fn branches(g: &Graph) -> Vec<(Option<&str>, bool)> {
        g.rows
            .iter()
            .map(|r| (r.branch.as_deref(), r.tip))
            .collect()
    }

    #[test]
    fn branch_names_follow_lanes() {
        // x (feature) y m (main) salen de a; main es la actual.
        let g = Graph::new(&[
            with_ref(
                commit("x", &["b"]),
                "feature",
                RefKind::Branch { current: false },
            ),
            commit("b", &["a"]),
            with_ref(
                with_ref(commit("m", &["a"]), "origin/main", RefKind::Remote),
                "main",
                RefKind::Branch { current: true },
            ),
            commit("a", &[]),
        ]);
        assert_eq!(
            branches(&g),
            [
                (Some("feature"), true),
                (Some("feature"), false),
                (Some("main"), true),
                // Donde convergen gana el carril del nodo (el primero).
                (Some("feature"), false),
            ]
        );
    }

    #[test]
    fn merged_branch_is_named_from_the_message() {
        let mut m = commit("m", &["c", "b"]);
        m.summary = "Merge branch 'feature' into main".into();
        let g = Graph::new(&[
            with_ref(m, "main", RefKind::Branch { current: true }),
            commit("c", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        assert_eq!(
            branches(&g),
            [
                (Some("main"), true),
                (Some("main"), false),
                (Some("feature"), false),
                (Some("main"), false),
            ]
        );
    }

    #[test]
    fn merge_messages() {
        assert_eq!(merged_branch("Merge branch 'dev'").as_deref(), Some("dev"));
        assert_eq!(
            merged_branch("Merge remote-tracking branch 'origin/dev'").as_deref(),
            Some("origin/dev")
        );
        assert_eq!(
            merged_branch("Merge pull request #3 from luis/fix-ui").as_deref(),
            Some("fix-ui")
        );
        assert_eq!(merged_branch("feat: merge things"), None);
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
