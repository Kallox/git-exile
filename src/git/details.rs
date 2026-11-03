use super::diff::{DIFF_ARGS, FileDiff, parse_diff};
use super::repo::Repo;
use super::runner::{GitError, Result};

/// Por encima de este tamaño el diff se corta para no bloquear la interfaz.
const MAX_DIFF_BYTES: usize = 8 * 1024 * 1024;
const FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%B%x1e";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub email: String,
    pub time: i64,
}

#[derive(Debug, Clone)]
pub struct CommitDetails {
    pub id: String,
    pub parents: Vec<String>,
    pub author: Person,
    pub committer: Person,
    pub message: String,
    /// Cambios respecto al primer padre (o al árbol vacío si es el commit inicial).
    pub files: Vec<FileDiff>,
    /// `true` si el diff era demasiado grande y se ha cortado.
    pub truncated: bool,
}

pub fn load_commit(repo: &Repo, id: &str) -> Result<CommitDetails> {
    let mut args = vec!["show", FORMAT, "--diff-merges=first-parent"];
    args.extend_from_slice(DIFF_ARGS);
    args.extend(["--end-of-options", id]);
    let output = repo.git(&args)?;

    let (header, diff) = output
        .split_once('\x1e')
        .ok_or_else(|| GitError::Parse("salida de git show sin cabecera".into()))?;
    let fields: Vec<&str> = header.splitn(9, '\x1f').collect();
    let [id, parents, an, ae, at, cn, ce, ct, message] = fields[..] else {
        return Err(GitError::Parse(format!(
            "cabecera de commit inválida: {header:?}"
        )));
    };
    let time = |t: &str| {
        t.parse::<i64>()
            .map_err(|_| GitError::Parse(format!("fecha inválida: {t:?}")))
    };

    let (diff, truncated) = truncate(diff);
    Ok(CommitDetails {
        id: id.to_owned(),
        parents: parents.split_whitespace().map(str::to_owned).collect(),
        author: Person {
            name: an.to_owned(),
            email: ae.to_owned(),
            time: time(at)?,
        },
        committer: Person {
            name: cn.to_owned(),
            email: ce.to_owned(),
            time: time(ct)?,
        },
        message: message.trim_end().to_owned(),
        files: parse_diff(diff),
        truncated,
    })
}

/// Corta el diff en el límite de un archivo si supera [`MAX_DIFF_BYTES`].
pub(crate) fn truncate(diff: &str) -> (&str, bool) {
    if diff.len() <= MAX_DIFF_BYTES {
        return (diff, false);
    }
    let mut end = MAX_DIFF_BYTES;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let cut = diff[..end].rfind("\ndiff --git ").unwrap_or(end);
    (&diff[..cut], true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::FileStatus;
    use crate::git::testutil::TestRepo;

    #[test]
    fn loads_root_and_regular_commits() {
        let t = TestRepo::new();
        let root = t.commit_file("a.txt", "a\n", "first\n\nbody line");
        let second = t.commit_file("a.txt", "a\nb\n", "second");
        let repo = Repo::open(t.path()).unwrap();

        let d = load_commit(&repo, &root).unwrap();
        assert_eq!(d.id, root);
        assert!(d.parents.is_empty());
        assert_eq!(d.message, "first\n\nbody line");
        assert_eq!(d.author.name, "Test");
        assert_eq!(d.files.len(), 1);
        assert_eq!(d.files[0].status, FileStatus::Added);

        let d = load_commit(&repo, &second).unwrap();
        assert_eq!(d.parents, vec![root]);
        assert_eq!(d.files[0].status, FileStatus::Modified);
        assert_eq!(d.files[0].stats(), (1, 0));
        assert!(!d.truncated);
    }

    #[test]
    fn merge_shows_changes_against_first_parent() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "a\n", "first");
        t.git(&["switch", "-q", "-c", "feature"]);
        t.commit_file("b.txt", "b\n", "feature");
        t.git(&["switch", "-q", "main"]);
        t.commit_file("c.txt", "c\n", "main");
        t.git(&["merge", "-q", "--no-ff", "-m", "merge", "feature"]);
        let repo = Repo::open(t.path()).unwrap();

        let d = load_commit(&repo, "HEAD").unwrap();
        assert_eq!(d.parents.len(), 2);
        let paths: Vec<_> = d.files.iter().map(|f| f.path()).collect();
        assert_eq!(paths, ["b.txt"]);
    }

    #[test]
    fn truncates_at_file_boundary() {
        let file = "diff --git a/x b/x\n+line\n";
        let big = file.repeat(MAX_DIFF_BYTES / file.len() + 10);
        let (cut, truncated) = truncate(&big);
        assert!(truncated);
        assert!(cut.len() <= MAX_DIFF_BYTES);
        assert!(cut.ends_with("+line"));
    }
}
