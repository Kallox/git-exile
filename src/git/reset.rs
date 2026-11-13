//! Reset de la rama actual a un commit: soft (conserva los cambios) o hard
//! (los descarta).

use super::repo::Repo;
use super::runner::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// `--soft`: la rama se mueve y los cambios de los commits que salen
    /// quedan añadidos (en el índice), junto con los que ya había.
    Soft,
    /// `--hard`: la rama se mueve y el working tree queda igual que el commit.
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Hard => "--hard",
        }
    }
}

/// Lo que pasará con la rama actual si se resetea a un commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetPreview {
    pub branch: String,
    /// Commits que dejan de estar en la rama.
    pub leaving: usize,
    /// De esos, los que no están en ninguna otra rama (local o remota) ni tag.
    pub lost: usize,
}

/// Rama actual; se niega con HEAD separado (no hay rama que mover).
fn current_branch(repo: &Repo) -> Result<String> {
    repo.current_branch()?.ok_or_else(|| {
        GitError::Refused("HEAD está separado: cambia a una rama antes de hacer reset".to_owned())
    })
}

/// Hash completo del commit `target`, comprobando que existe.
fn resolve_commit(repo: &Repo, target: &str) -> Result<String> {
    let spec = format!("{target}^{{commit}}");
    match repo.git(&["rev-parse", "--verify", "-q", &spec]) {
        Ok(out) => Ok(out.trim().to_owned()),
        Err(GitError::Failed { .. }) => Err(GitError::Refused(format!(
            "\"{target}\" no es un commit de este repositorio"
        ))),
        Err(e) => Err(e),
    }
}

fn count(out: &str) -> Result<usize> {
    out.trim()
        .parse()
        .map_err(|_| GitError::Parse(format!("conteo de commits inválido: {out:?}")))
}

/// Calcula qué commits saldrían de la rama actual al resetearla a `target`.
pub fn reset_preview(repo: &Repo, target: &str) -> Result<ResetPreview> {
    let branch = current_branch(repo)?;
    let target = resolve_commit(repo, target)?;
    let not_target = format!("^{target}");
    let leaving = count(&repo.git(&["rev-list", "--count", "HEAD", &not_target])?)?;
    let exclude = format!("--exclude={branch}");
    let lost = count(&repo.git(&[
        "rev-list",
        "--count",
        "HEAD",
        "--not",
        &target,
        &exclude,
        "--branches",
        "--remotes",
        "--tags",
    ])?)?;
    Ok(ResetPreview {
        branch,
        leaving,
        lost,
    })
}

/// Mueve la rama actual a `target` con el modo indicado. `branch` es la rama
/// que el usuario confirmó: si la actual es otra, no se hace nada.
pub fn reset_branch(repo: &Repo, branch: &str, target: &str, mode: ResetMode) -> Result<()> {
    let current = current_branch(repo)?;
    if current != branch {
        return Err(GitError::Refused(format!(
            "La rama actual ahora es \"{current}\", no \"{branch}\": no se ha hecho el reset"
        )));
    }
    let target = resolve_commit(repo, target)?;
    repo.git(&["reset", "-q", mode.flag(), &target, "--"])
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::load_status;
    use crate::git::testutil::TestRepo;

    fn head(t: &TestRepo) -> String {
        t.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    #[test]
    fn previews_leaving_and_lost_commits() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("a.txt", "2\n", "second");
        t.git(&["branch", "keep"]);
        t.commit_file("a.txt", "3\n", "third");
        let repo = Repo::open(t.path()).unwrap();

        let preview = reset_preview(&repo, &first).unwrap();
        assert_eq!(
            preview,
            ResetPreview {
                branch: "main".into(),
                leaving: 2,
                lost: 1,
            }
        );
        let at_head = reset_preview(&repo, "HEAD").unwrap();
        assert_eq!((at_head.leaving, at_head.lost), (0, 0));
    }

    #[test]
    fn soft_reset_keeps_changes_staged() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("b.txt", "b\n", "second");
        t.write("a.txt", "local\n");
        let repo = Repo::open(t.path()).unwrap();

        reset_branch(&repo, "main", &first, ResetMode::Soft).unwrap();
        assert_eq!(head(&t), first);
        assert_eq!(
            std::fs::read_to_string(t.path().join("a.txt")).unwrap(),
            "local\n"
        );
        assert_eq!(
            std::fs::read_to_string(t.path().join("b.txt")).unwrap(),
            "b\n"
        );
        let status = load_status(&repo).unwrap();
        assert!(status.staged.iter().any(|e| e.path == "b.txt"));
        assert!(status.unstaged.iter().any(|e| e.path == "a.txt"));
    }

    #[test]
    fn hard_reset_discards_changes_but_keeps_untracked() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("b.txt", "b\n", "second");
        t.write("a.txt", "local\n");
        t.write("new.txt", "untracked\n");
        let repo = Repo::open(t.path()).unwrap();

        reset_branch(&repo, "main", &first, ResetMode::Hard).unwrap();
        assert_eq!(head(&t), first);
        assert_eq!(
            std::fs::read_to_string(t.path().join("a.txt")).unwrap(),
            "1\n"
        );
        assert!(!t.path().join("b.txt").exists());
        assert!(t.path().join("new.txt").exists());
        let status = load_status(&repo).unwrap();
        assert!(status.staged.is_empty());
        assert!(status.unstaged.iter().all(|e| e.path == "new.txt"));
    }

    #[test]
    fn refuses_detached_head_other_branch_or_bad_target() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        let second = t.commit_file("a.txt", "2\n", "second");
        let repo = Repo::open(t.path()).unwrap();

        assert!(matches!(
            reset_branch(&repo, "other", &first, ResetMode::Hard),
            Err(GitError::Refused(_))
        ));
        assert!(matches!(
            reset_branch(&repo, "main", "nope", ResetMode::Hard),
            Err(GitError::Refused(_))
        ));
        assert_eq!(head(&t), second);

        t.git(&["switch", "-q", "--detach", &first]);
        assert!(matches!(
            reset_preview(&repo, &first),
            Err(GitError::Refused(_))
        ));
        assert!(matches!(
            reset_branch(&repo, "main", &first, ResetMode::Soft),
            Err(GitError::Refused(_))
        ));
    }
}
