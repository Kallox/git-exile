//! Sincronización con los remotos: fetch, pull y push.

use super::remote::list_remotes;
use super::repo::Repo;
use super::runner::{GitError, Result};
use super::status::load_status;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOp {
    Fetch,
    Pull,
    Push,
}

impl SyncOp {
    pub fn run(self, repo: &Repo) -> Result<()> {
        match self {
            Self::Fetch => fetch(repo),
            Self::Pull => pull(repo),
            Self::Push => push(repo),
        }
    }
}

/// Baja las novedades de todos los remotos sin tocar tus ramas ni tus archivos.
fn fetch(repo: &Repo) -> Result<()> {
    repo.git(&["fetch", "--all"]).map(drop)
}

/// Avanza la rama actual hasta su remota. Solo si no hay que combinar nada: la
/// aplicación no sabría terminar un merge o un rebase con conflictos a medias.
fn pull(repo: &Repo) -> Result<()> {
    explain(
        repo.git(&["pull", "--ff-only"]),
        "Not possible to fast-forward",
        "La rama local y la remota han divergido: pull solo avanza la rama si no hay que \
         combinar nada. Haz el merge o el rebase desde la terminal.",
    )
}

/// Sube la rama actual. La primera vez la crea en el remoto y la deja seguida.
fn push(repo: &Repo) -> Result<()> {
    let status = load_status(repo)?;
    if status.branch.is_none() {
        return Err(GitError::Refused(
            "Con HEAD separado no hay ninguna rama que subir.".to_owned(),
        ));
    }
    let result = if status.upstream.is_some() {
        repo.git(&["push"])
    } else {
        let remotes = list_remotes(repo)?;
        let remote = remotes.first().ok_or_else(|| {
            GitError::Refused("El repositorio no tiene ningún remoto.".to_owned())
        })?;
        repo.git(&["push", "--set-upstream", &remote.name, "HEAD"])
    };
    explain(
        result,
        "[rejected]",
        "El remoto tiene commits que no están aquí. Haz pull antes de volver a subir.",
    )
}

/// Sustituye un error de git que contiene `marker` por una explicación.
fn explain(result: Result<String>, marker: &str, message: &str) -> Result<()> {
    match result {
        Ok(_) => Ok(()),
        Err(GitError::Failed { stderr, .. }) if stderr.contains(marker) => {
            Err(GitError::Refused(message.to_owned()))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::run;
    use crate::git::testutil::TestRepo;
    use tempfile::TempDir;

    /// Un remoto vacío y dos clones suyos, con un commit inicial ya subido.
    struct Remote {
        _bare: TempDir,
        mine: TestRepo,
        other: TestRepo,
    }

    fn clone_into(url: &str) -> TestRepo {
        let t = TestRepo::new();
        t.git(&["remote", "add", "origin", url]);
        t.git(&["fetch", "-q", "origin"]);
        t
    }

    fn remote() -> Remote {
        let bare = tempfile::tempdir().unwrap();
        run(bare.path(), &["init", "-q", "--bare", "-b", "main"]).unwrap();
        let url = bare.path().display().to_string();

        let mine = clone_into(&url);
        mine.commit_file("a.txt", "1\n", "first");
        mine.git(&["push", "-q", "--set-upstream", "origin", "main"]);

        let other = clone_into(&url);
        other.git(&["reset", "-q", "--hard", "origin/main"]);
        other.git(&["branch", "-q", "--set-upstream-to=origin/main"]);
        Remote {
            _bare: bare,
            mine,
            other,
        }
    }

    fn repo(t: &TestRepo) -> Repo {
        Repo::open(t.path()).unwrap()
    }

    fn head(t: &TestRepo, rev: &str) -> String {
        t.git(&["rev-parse", rev]).trim().to_owned()
    }

    #[test]
    fn fetch_updates_remote_refs_but_not_the_branch() {
        let r = remote();
        let before = head(&r.mine, "HEAD");
        let new = r.other.commit_file("b.txt", "1\n", "second");
        r.other.git(&["push", "-q"]);

        SyncOp::Fetch.run(&repo(&r.mine)).unwrap();
        assert_eq!(head(&r.mine, "origin/main"), new);
        assert_eq!(head(&r.mine, "HEAD"), before);
        assert_eq!(load_status(&repo(&r.mine)).unwrap().behind, 1);
    }

    #[test]
    fn pull_fast_forwards_the_branch() {
        let r = remote();
        let new = r.other.commit_file("b.txt", "1\n", "second");
        r.other.git(&["push", "-q"]);

        SyncOp::Pull.run(&repo(&r.mine)).unwrap();
        assert_eq!(head(&r.mine, "HEAD"), new);
        assert!(r.mine.path().join("b.txt").exists());
    }

    #[test]
    fn pull_refuses_diverged_branches_and_changes_nothing() {
        let r = remote();
        r.other.commit_file("b.txt", "1\n", "theirs");
        r.other.git(&["push", "-q"]);
        let local = r.mine.commit_file("c.txt", "1\n", "mine");

        let err = SyncOp::Pull.run(&repo(&r.mine)).unwrap_err();
        assert!(err.to_string().contains("divergido"), "{err}");
        assert_eq!(head(&r.mine, "HEAD"), local);
    }

    #[test]
    fn pull_ignores_a_configured_rebase() {
        let r = remote();
        r.mine.git(&["config", "pull.rebase", "true"]);
        r.other.commit_file("b.txt", "1\n", "theirs");
        r.other.git(&["push", "-q"]);
        let local = r.mine.commit_file("c.txt", "1\n", "mine");

        assert!(SyncOp::Pull.run(&repo(&r.mine)).is_err());
        assert_eq!(head(&r.mine, "HEAD"), local);
    }

    #[test]
    fn push_uploads_commits() {
        let r = remote();
        let new = r.mine.commit_file("b.txt", "1\n", "second");
        SyncOp::Push.run(&repo(&r.mine)).unwrap();
        r.other.git(&["fetch", "-q", "origin"]);
        assert_eq!(head(&r.other, "origin/main"), new);
    }

    #[test]
    fn push_publishes_a_new_branch_and_tracks_it() {
        let r = remote();
        r.mine.git(&["switch", "-q", "-c", "feature/x"]);
        let new = r.mine.commit_file("b.txt", "1\n", "second");

        SyncOp::Push.run(&repo(&r.mine)).unwrap();
        assert_eq!(head(&r.mine, "origin/feature/x"), new);
        let status = load_status(&repo(&r.mine)).unwrap();
        assert_eq!(status.upstream.as_deref(), Some("origin/feature/x"));
    }

    #[test]
    fn rejected_push_is_explained_and_never_forced() {
        let r = remote();
        let theirs = r.other.commit_file("b.txt", "1\n", "theirs");
        r.other.git(&["push", "-q"]);
        r.mine.commit_file("c.txt", "1\n", "mine");

        let err = SyncOp::Push.run(&repo(&r.mine)).unwrap_err();
        assert!(err.to_string().contains("Haz pull"), "{err}");
        r.other.git(&["fetch", "-q", "origin"]);
        assert_eq!(head(&r.other, "origin/main"), theirs);
    }

    #[test]
    fn push_needs_a_branch_and_a_remote() {
        let r = remote();
        r.mine.git(&["switch", "-q", "--detach"]);
        assert!(SyncOp::Push.run(&repo(&r.mine)).is_err());

        let local = TestRepo::new();
        local.commit_file("a.txt", "1\n", "first");
        let err = SyncOp::Push.run(&repo(&local)).unwrap_err();
        assert!(err.to_string().contains("ningún remoto"), "{err}");
    }
}
