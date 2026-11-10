//! Ramas: listar, crear, cambiar, borrar (solo locales) y moverse a un commit.

use super::repo::Repo;
use super::runner::{GitError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// Nombre corto: `main` o `origin/main`.
    pub name: String,
    pub remote: bool,
    /// Es la rama actual (HEAD apunta a ella).
    pub current: bool,
    /// Rama remota a la que sigue, p. ej. `origin/main`.
    pub upstream: Option<String>,
    /// Commit al que apunta.
    pub target: String,
}

impl Branch {
    /// Para una rama remota `origin/feature`, el nombre local `feature`.
    pub fn local_name(&self) -> &str {
        if self.remote {
            self.name.split_once('/').map_or(&self.name, |(_, n)| n)
        } else {
            &self.name
        }
    }
}

/// Ramas locales y remotas, cada grupo ordenado por nombre.
pub fn list_branches(repo: &Repo) -> Result<Vec<Branch>> {
    let out = repo.git(&[
        "for-each-ref",
        "--format=%(refname)%00%(objectname)%00%(HEAD)%00%(upstream:short)",
        "refs/heads",
        "refs/remotes",
    ])?;
    let mut branches = Vec::new();
    for line in out.lines().filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split('\0').collect();
        let [refname, target, head, upstream] = fields[..] else {
            return Err(GitError::Parse(format!(
                "línea de for-each-ref inválida: {line:?}"
            )));
        };
        let (name, remote) = if let Some(n) = refname.strip_prefix("refs/heads/") {
            (n, false)
        } else if let Some(n) = refname.strip_prefix("refs/remotes/") {
            // `origin/HEAD` es un alias, no una rama.
            if n.ends_with("/HEAD") {
                continue;
            }
            (n, true)
        } else {
            continue;
        };
        branches.push(Branch {
            name: name.to_owned(),
            remote,
            current: head == "*",
            upstream: (!upstream.is_empty()).then(|| upstream.to_owned()),
            target: target.to_owned(),
        });
    }
    Ok(branches)
}

/// Comprueba que `name` es un nombre de rama válido para git.
pub fn validate_branch_name(repo: &Repo, name: &str) -> Result<()> {
    match repo.git(&["check-ref-format", "--branch", name]) {
        Ok(_) => Ok(()),
        Err(GitError::Failed { .. }) => Err(GitError::Parse(format!(
            "\"{name}\" no es un nombre de rama válido"
        ))),
        Err(e) => Err(e),
    }
}

/// Crea la rama `name` en `start` (un hash o `HEAD`) y, si `checkout`, cambia a ella.
pub fn create_branch(repo: &Repo, name: &str, start: &str, checkout: bool) -> Result<()> {
    let name = name.trim();
    validate_branch_name(repo, name)?;
    if checkout {
        repo.git(&["switch", "-c", name, start])?;
    } else {
        repo.git(&["branch", "--", name, start])?;
    }
    Ok(())
}

/// Cambia a `branch`. Para una rama remota, usa la local con el mismo nombre
/// si existe, o crea una que la siga.
pub fn switch_branch(repo: &Repo, branch: &Branch, branches: &[Branch]) -> Result<()> {
    if !branch.remote {
        return repo.git(&["switch", &branch.name]).map(drop);
    }
    let local = branch.local_name();
    if branches.iter().any(|b| !b.remote && b.name == local) {
        repo.git(&["switch", local]).map(drop)
    } else {
        repo.git(&["switch", "--track", &branch.name]).map(drop)
    }
}

/// Comprueba que `name` es una rama local (`refs/heads/<name>`).
fn ensure_local_branch(repo: &Repo, name: &str) -> Result<()> {
    let refname = format!("refs/heads/{name}");
    match repo.git(&["show-ref", "--verify", "--quiet", &refname]) {
        Ok(_) => Ok(()),
        Err(GitError::Failed { .. }) => Err(GitError::Refused(format!(
            "\"{name}\" no es una rama local: solo se pueden borrar ramas locales"
        ))),
        Err(e) => Err(e),
    }
}

/// Número de commits de la rama local `name` que no están en ninguna otra
/// rama (local o remota), tag ni en HEAD: los que se perderían al borrarla.
pub fn unmerged_commits(repo: &Repo, name: &str) -> Result<usize> {
    ensure_local_branch(repo, name)?;
    let refname = format!("refs/heads/{name}");
    let exclude = format!("--exclude={name}");
    let out = repo.git(&[
        "rev-list",
        "--count",
        &refname,
        "--not",
        &exclude,
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ])?;
    out.trim()
        .parse()
        .map_err(|_| GitError::Parse(format!("conteo de commits inválido: {out:?}")))
}

/// Borra la rama local `name`. Nunca toca ramas remotas ni el servidor:
/// se niega si `name` no es una rama local o si es la rama actual.
pub fn delete_branch(repo: &Repo, name: &str) -> Result<()> {
    ensure_local_branch(repo, name)?;
    if repo.current_branch()?.as_deref() == Some(name) {
        return Err(GitError::Refused(format!(
            "\"{name}\" es la rama actual: cambia a otra antes de borrarla"
        )));
    }
    // `-D`: el usuario ya confirmó sabiendo si se pierden commits.
    repo.git(&["branch", "-D", "--", name]).map(drop)
}

/// Mueve HEAD al commit `id` sin rama (HEAD separado).
pub fn checkout_commit(repo: &Repo, id: &str) -> Result<()> {
    repo.git(&["switch", "--detach", id]).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    fn names(repo: &Repo) -> Vec<(String, bool, bool)> {
        list_branches(repo)
            .unwrap()
            .into_iter()
            .map(|b| (b.name, b.remote, b.current))
            .collect()
    }

    #[test]
    fn creates_lists_and_switches() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("a.txt", "2\n", "second");
        let repo = Repo::open(t.path()).unwrap();

        // Desde un commit anterior y sin cambiar.
        create_branch(&repo, "old", &first, false).unwrap();
        assert_eq!(
            names(&repo),
            [("main".into(), false, true), ("old".into(), false, false)]
        );
        let old = list_branches(&repo).unwrap().remove(1);
        assert_eq!(old.target, first);

        // Desde HEAD y cambiando.
        create_branch(&repo, "feature/x", "HEAD", true).unwrap();
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("feature/x"));

        let branches = list_branches(&repo).unwrap();
        let main = branches.iter().find(|b| b.name == "main").unwrap();
        switch_branch(&repo, main, &branches).unwrap();
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
    }

    #[test]
    fn checks_out_commit_detaching_head() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("a.txt", "2\n", "second");
        let repo = Repo::open(t.path()).unwrap();

        checkout_commit(&repo, &first).unwrap();
        assert_eq!(repo.current_branch().unwrap(), None);
        assert_eq!(repo.git(&["rev-parse", "HEAD"]).unwrap().trim(), first);
        assert_eq!(
            std::fs::read_to_string(t.path().join("a.txt")).unwrap(),
            "1\n"
        );
    }

    #[test]
    fn checkout_commit_refuses_to_overwrite_local_changes() {
        let t = TestRepo::new();
        let first = t.commit_file("a.txt", "1\n", "first");
        t.commit_file("a.txt", "2\n", "second");
        std::fs::write(t.path().join("a.txt"), "local\n").unwrap();
        let repo = Repo::open(t.path()).unwrap();

        assert!(checkout_commit(&repo, &first).is_err());
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
        assert_eq!(
            std::fs::read_to_string(t.path().join("a.txt")).unwrap(),
            "local\n"
        );
    }

    #[test]
    fn rejects_invalid_names_and_duplicates() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let repo = Repo::open(t.path()).unwrap();
        for bad in ["", "con espacio", "a..b", "-x", "fin/"] {
            assert!(create_branch(&repo, bad, "HEAD", false).is_err(), "{bad:?}");
        }
        assert!(create_branch(&repo, "main", "HEAD", false).is_err());
    }

    #[test]
    fn switches_to_remote_branch_creating_tracking_branch() {
        let origin = TestRepo::new();
        origin.commit_file("a.txt", "1\n", "first");
        origin.git(&["branch", "remote-only"]);

        let t = TestRepo::new();
        let url = origin.path().display().to_string();
        t.git(&["remote", "add", "origin", &url]);
        t.git(&["fetch", "-q", "origin"]);
        t.git(&["switch", "-q", "-c", "main", "--track", "origin/main"]);
        let repo = Repo::open(t.path()).unwrap();

        let branches = list_branches(&repo).unwrap();
        assert!(!branches.iter().any(|b| b.name.ends_with("/HEAD")));
        let main = branches.iter().find(|b| b.name == "main").unwrap();
        assert_eq!(main.upstream.as_deref(), Some("origin/main"));

        let remote = branches
            .iter()
            .find(|b| b.name == "origin/remote-only")
            .unwrap();
        assert_eq!(remote.local_name(), "remote-only");
        switch_branch(&repo, remote, &branches).unwrap();
        assert_eq!(
            repo.current_branch().unwrap().as_deref(),
            Some("remote-only")
        );
        let now = list_branches(&repo).unwrap();
        let local = now.iter().find(|b| b.name == "remote-only").unwrap();
        assert_eq!(local.upstream.as_deref(), Some("origin/remote-only"));
    }

    #[test]
    fn deletes_local_branch_and_counts_unmerged_commits() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.git(&["branch", "merged"]);
        t.git(&["switch", "-q", "-c", "work"]);
        t.commit_file("a.txt", "2\n", "only in work");
        t.git(&["switch", "-q", "main"]);
        let repo = Repo::open(t.path()).unwrap();

        assert_eq!(unmerged_commits(&repo, "merged").unwrap(), 0);
        assert_eq!(unmerged_commits(&repo, "work").unwrap(), 1);

        delete_branch(&repo, "merged").unwrap();
        delete_branch(&repo, "work").unwrap();
        let left: Vec<String> = list_branches(&repo)
            .unwrap()
            .into_iter()
            .map(|b| b.name)
            .collect();
        assert_eq!(left, ["main"]);
    }

    #[test]
    fn refuses_to_delete_current_or_missing_branch() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let repo = Repo::open(t.path()).unwrap();
        assert!(matches!(
            delete_branch(&repo, "main"),
            Err(GitError::Refused(_))
        ));
        assert!(matches!(
            delete_branch(&repo, "nope"),
            Err(GitError::Refused(_))
        ));
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
    }

    #[test]
    fn never_deletes_remote_branches() {
        let origin = TestRepo::new();
        origin.commit_file("a.txt", "1\n", "first");
        origin.git(&["branch", "remote-only"]);

        let t = TestRepo::new();
        let url = origin.path().display().to_string();
        t.git(&["remote", "add", "origin", &url]);
        t.git(&["fetch", "-q", "origin"]);
        t.git(&["switch", "-q", "-c", "main", "--track", "origin/main"]);
        let repo = Repo::open(t.path()).unwrap();

        for name in [
            "origin/remote-only",
            "remotes/origin/remote-only",
            "remote-only",
        ] {
            assert!(
                matches!(delete_branch(&repo, name), Err(GitError::Refused(_))),
                "{name:?}"
            );
            assert!(unmerged_commits(&repo, name).is_err(), "{name:?}");
        }
        // Siguen intactas en local y en el servidor.
        let branches = list_branches(&repo).unwrap();
        assert!(branches.iter().any(|b| b.name == "origin/remote-only"));
        assert!(
            origin
                .git(&["branch", "--list", "remote-only"])
                .contains("remote-only")
        );
    }
}
