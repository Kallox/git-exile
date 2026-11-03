//! Ramas: listar, crear y cambiar.

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
}
