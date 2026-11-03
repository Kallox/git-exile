//! Estado del working tree (`git status --porcelain=v2`).

use super::repo::Repo;
use super::runner::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

impl ChangeKind {
    pub fn letter(self) -> char {
        match self {
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::TypeChanged => 'T',
            Self::Untracked => '?',
            Self::Conflicted => 'U',
        }
    }

    fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            b'A' => Self::Added,
            b'M' => Self::Modified,
            b'D' => Self::Deleted,
            b'R' => Self::Renamed,
            b'C' => Self::Copied,
            b'T' => Self::TypeChanged,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    /// Ruta original en renombrados y copias.
    pub orig_path: Option<String>,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingTreeStatus {
    /// `None` si HEAD está separado.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Cambios en el índice (irán en el próximo commit).
    pub staged: Vec<StatusEntry>,
    /// Cambios en el working tree sin añadir, incluidos los archivos nuevos.
    pub unstaged: Vec<StatusEntry>,
    pub conflicted: Vec<StatusEntry>,
}

impl WorkingTreeStatus {
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty() && self.conflicted.is_empty()
    }

    pub fn change_count(&self) -> usize {
        self.staged.len() + self.unstaged.len() + self.conflicted.len()
    }
}

pub fn load_status(repo: &Repo) -> Result<WorkingTreeStatus> {
    let out = repo.git(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--untracked-files=all",
    ])?;
    parse_status(&out)
}

fn parse_status(out: &str) -> Result<WorkingTreeStatus> {
    let mut status = WorkingTreeStatus::default();
    let mut records = out.split('\0').filter(|r| !r.is_empty());

    while let Some(record) = records.next() {
        let bad = || GitError::Parse(format!("línea de status inválida: {record:?}"));
        match record.as_bytes()[0] {
            b'#' => parse_header(record, &mut status),
            b'1' => {
                let fields: Vec<&str> = record.splitn(9, ' ').collect();
                let (xy, path) = (
                    fields.get(1).ok_or_else(bad)?,
                    fields.get(8).ok_or_else(bad)?,
                );
                push_change(&mut status, xy.as_bytes(), path, None);
            }
            b'2' => {
                let fields: Vec<&str> = record.splitn(10, ' ').collect();
                let (xy, path) = (
                    fields.get(1).ok_or_else(bad)?,
                    fields.get(9).ok_or_else(bad)?,
                );
                // Con -z la ruta original va en el siguiente registro.
                let orig = records.next().map(str::to_owned);
                push_change(&mut status, xy.as_bytes(), path, orig);
            }
            b'u' => {
                let fields: Vec<&str> = record.splitn(11, ' ').collect();
                status.conflicted.push(StatusEntry {
                    path: fields.get(10).ok_or_else(bad)?.to_string(),
                    orig_path: None,
                    kind: ChangeKind::Conflicted,
                });
            }
            b'?' => status.unstaged.push(StatusEntry {
                path: record[2..].to_owned(),
                orig_path: None,
                kind: ChangeKind::Untracked,
            }),
            // Ignorados ('!') u otros registros que no nos interesan.
            _ => {}
        }
    }
    Ok(status)
}

fn parse_header(record: &str, status: &mut WorkingTreeStatus) {
    let mut parts = record.splitn(3, ' ').skip(1);
    let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
        return;
    };
    match key {
        "branch.head" if value != "(detached)" => status.branch = Some(value.to_owned()),
        "branch.upstream" => status.upstream = Some(value.to_owned()),
        "branch.ab" => {
            // `# branch.ab +1 -2`
            let mut nums = value.split_whitespace();
            let mut next = || nums.next().and_then(|n| n[1..].parse().ok()).unwrap_or(0);
            status.ahead = next();
            status.behind = next();
        }
        _ => {}
    }
}

fn push_change(status: &mut WorkingTreeStatus, xy: &[u8], path: &str, orig: Option<String>) {
    let (x, y) = (xy.first().copied(), xy.get(1).copied());
    if let Some(kind) = x.and_then(ChangeKind::from_code) {
        status.staged.push(StatusEntry {
            path: path.to_owned(),
            orig_path: orig.clone(),
            kind,
        });
    }
    if let Some(kind) = y.and_then(ChangeKind::from_code) {
        status.unstaged.push(StatusEntry {
            path: path.to_owned(),
            orig_path: None,
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    fn paths(entries: &[StatusEntry]) -> Vec<(&str, char)> {
        entries
            .iter()
            .map(|e| (e.path.as_str(), e.kind.letter()))
            .collect()
    }

    #[test]
    fn clean_repository() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "a\n", "first");
        let s = load_status(&Repo::open(t.path()).unwrap()).unwrap();
        assert!(s.is_clean());
        assert_eq!(s.branch.as_deref(), Some("main"));
    }

    #[test]
    fn classifies_changes() {
        let t = TestRepo::new();
        t.commit_file("keep.txt", "1\n", "first");
        t.commit_file("gone.txt", "1\n", "second");
        t.commit_file("old name.txt", "same content\n", "third");

        t.write("keep.txt", "2\n"); // modificado sin añadir
        t.write("staged.txt", "new\n");
        t.git(&["add", "staged.txt"]); // nuevo añadido
        t.git(&["rm", "-q", "gone.txt"]); // borrado añadido
        t.git(&["mv", "old name.txt", "new name.txt"]); // renombrado
        t.write("dir/untracked.txt", "?\n");

        let s = load_status(&Repo::open(t.path()).unwrap()).unwrap();
        assert_eq!(
            paths(&s.staged),
            [
                ("gone.txt", 'D'),
                ("new name.txt", 'R'),
                ("staged.txt", 'A')
            ]
        );
        let renamed = &s.staged[1];
        assert_eq!(renamed.orig_path.as_deref(), Some("old name.txt"));
        assert_eq!(
            paths(&s.unstaged),
            [("keep.txt", 'M'), ("dir/untracked.txt", '?')]
        );
        assert_eq!(s.change_count(), 5);
    }

    #[test]
    fn file_staged_and_modified_again_appears_twice() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.write("a.txt", "2\n");
        t.git(&["add", "a.txt"]);
        t.write("a.txt", "3\n");
        let s = load_status(&Repo::open(t.path()).unwrap()).unwrap();
        assert_eq!(paths(&s.staged), [("a.txt", 'M')]);
        assert_eq!(paths(&s.unstaged), [("a.txt", 'M')]);
    }

    #[test]
    fn parses_branch_header() {
        let s = parse_status(
            "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -3\0",
        )
        .unwrap();
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (2, 3));
    }
}
