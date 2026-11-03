use super::repo::Repo;
use super::runner::{GitError, Result};

const RECORD_SEP: char = '\x1e';
const FIELD_SEP: char = '\x1f';
const FORMAT: &str = "--format=%x1e%H%x1f%h%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%D%x1f%s";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// `HEAD` cuando no apunta a ninguna rama.
    Head,
    /// Rama local; `true` si es la rama actual.
    Branch {
        current: bool,
    },
    Remote,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefLabel {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Fecha del autor en segundos desde epoch.
    pub time: i64,
    pub refs: Vec<RefLabel>,
    pub summary: String,
}

/// Carga una página del historial de todas las ramas, tags y HEAD,
/// ordenado de más nuevo a más antiguo.
pub fn load_log(repo: &Repo, skip: usize, limit: usize) -> Result<Vec<Commit>> {
    let empty = repo.is_empty()?;
    if empty && repo.git(&["for-each-ref", "--count=1"])?.trim().is_empty() {
        return Ok(Vec::new());
    }
    let skip = format!("--skip={skip}");
    let limit = format!("--max-count={limit}");
    let mut args = vec![
        "log",
        "--date-order",
        "--decorate=full",
        FORMAT,
        &skip,
        &limit,
        "--branches",
        "--remotes",
        "--tags",
    ];
    if !empty {
        args.push("HEAD");
    }
    args.push("--");
    parse_log(&repo.git(&args)?)
}

fn parse_log(output: &str) -> Result<Vec<Commit>> {
    output
        .split(RECORD_SEP)
        .filter(|r| !r.trim().is_empty())
        .map(parse_record)
        .collect()
}

fn parse_record(record: &str) -> Result<Commit> {
    let record = record.trim_end_matches('\n');
    let fields: Vec<&str> = record.splitn(8, FIELD_SEP).collect();
    let [
        id,
        short_id,
        parents,
        author_name,
        author_email,
        time,
        refs,
        summary,
    ] = fields[..]
    else {
        return Err(GitError::Parse(format!(
            "registro de log incompleto: {record:?}"
        )));
    };
    Ok(Commit {
        id: id.to_owned(),
        short_id: short_id.to_owned(),
        parents: parents.split_whitespace().map(str::to_owned).collect(),
        author_name: author_name.to_owned(),
        author_email: author_email.to_owned(),
        time: time
            .parse()
            .map_err(|_| GitError::Parse(format!("fecha inválida: {time:?}")))?,
        refs: parse_refs(refs),
        summary: summary.to_owned(),
    })
}

/// Interpreta `%D` con `--decorate=full`, p. ej.
/// `HEAD -> refs/heads/main, refs/remotes/origin/main, tag: refs/tags/v1`.
fn parse_refs(decorations: &str) -> Vec<RefLabel> {
    let mut refs = Vec::new();
    for item in decorations.split(", ").filter(|s| !s.is_empty()) {
        if let Some(branch) = item.strip_prefix("HEAD -> ") {
            refs.push(classify(branch, true));
        } else if item == "HEAD" {
            refs.push(RefLabel {
                name: "HEAD".to_owned(),
                kind: RefKind::Head,
            });
        } else if let Some(tag) = item.strip_prefix("tag: ") {
            refs.push(classify(tag, false));
        } else {
            refs.push(classify(item, false));
        }
    }
    refs
}

fn classify(full: &str, current: bool) -> RefLabel {
    let (name, kind) = if let Some(n) = full.strip_prefix("refs/heads/") {
        (n, RefKind::Branch { current })
    } else if let Some(n) = full.strip_prefix("refs/remotes/") {
        (n, RefKind::Remote)
    } else if let Some(n) = full.strip_prefix("refs/tags/") {
        (n, RefKind::Tag)
    } else {
        (full, RefKind::Branch { current })
    };
    RefLabel {
        name: name.to_owned(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    #[test]
    fn empty_repository_has_no_commits() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        assert!(load_log(&repo, 0, 100).unwrap().is_empty());
    }

    #[test]
    fn loads_history_with_merge_and_refs() {
        let t = TestRepo::new();
        let a = t.commit_file("a.txt", "a\n", "first");
        t.git(&["switch", "-q", "-c", "feature"]);
        let b = t.commit_file("b.txt", "b\n", "on feature");
        t.git(&["switch", "-q", "main"]);
        let c = t.commit_file("c.txt", "c\n", "on main");
        t.git(&["merge", "-q", "--no-ff", "-m", "merge feature", "feature"]);
        t.git(&["tag", "v1"]);

        let repo = Repo::open(t.path()).unwrap();
        let log = load_log(&repo, 0, 100).unwrap();
        assert_eq!(log.len(), 4);

        let merge = &log[0];
        assert_eq!(merge.summary, "merge feature");
        assert_eq!(merge.parents, vec![c.clone(), b.clone()]);
        assert_eq!(merge.author_name, "Test");
        assert!(merge.refs.contains(&RefLabel {
            name: "main".into(),
            kind: RefKind::Branch { current: true }
        }));
        assert!(merge.refs.contains(&RefLabel {
            name: "v1".into(),
            kind: RefKind::Tag
        }));

        let feature = log.iter().find(|c| c.id == b).unwrap();
        assert_eq!(
            feature.refs,
            vec![RefLabel {
                name: "feature".into(),
                kind: RefKind::Branch { current: false }
            }]
        );
        assert_eq!(log.last().unwrap().id, a);
        assert!(log.last().unwrap().parents.is_empty());
    }

    #[test]
    fn paginates() {
        let t = TestRepo::new();
        for i in 0..5 {
            t.commit_file("f.txt", &i.to_string(), &format!("c{i}"));
        }
        let repo = Repo::open(t.path()).unwrap();
        let page: Vec<_> = load_log(&repo, 1, 2)
            .unwrap()
            .into_iter()
            .map(|c| c.summary)
            .collect();
        assert_eq!(page, ["c3", "c2"]);
    }

    #[test]
    fn parses_detached_head_and_remotes() {
        let refs = parse_refs("HEAD, refs/remotes/origin/main, tag: refs/tags/v2");
        assert_eq!(
            refs.iter()
                .map(|r| (r.name.as_str(), r.kind))
                .collect::<Vec<_>>(),
            [
                ("HEAD", RefKind::Head),
                ("origin/main", RefKind::Remote),
                ("v2", RefKind::Tag)
            ]
        );
    }
}
