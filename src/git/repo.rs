use std::path::{Path, PathBuf};

use super::runner::{self, GitError, Result};

/// Un repositorio con working tree, identificado por su carpeta raíz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    root: PathBuf,
}

impl Repo {
    /// Abre el repositorio que contiene `path` (puede ser una subcarpeta).
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            return Err(GitError::Parse(format!(
                "{} no es una carpeta",
                path.display()
            )));
        }
        let top = runner::run(path, &["rev-parse", "--show-toplevel"])?;
        let top = top.trim();
        if top.is_empty() {
            return Err(GitError::Parse(format!(
                "{} no tiene working tree",
                path.display()
            )));
        }
        Ok(Self {
            root: PathBuf::from(top),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Nombre corto de la carpeta raíz, para mostrar en la interfaz.
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub fn git<S: AsRef<std::ffi::OsStr>>(&self, args: &[S]) -> Result<String> {
        runner::run(&self.root, args)
    }

    pub fn git_with_stdin<S: AsRef<std::ffi::OsStr>>(
        &self,
        args: &[S],
        input: &[u8],
    ) -> Result<String> {
        runner::run_with_stdin(&self.root, args, input)
    }

    /// Rama actual, o `None` si HEAD apunta directamente a un commit.
    pub fn current_branch(&self) -> Result<Option<String>> {
        match self.git(&["symbolic-ref", "--short", "-q", "HEAD"]) {
            Ok(out) => Ok(Some(out.trim().to_owned())),
            Err(GitError::Failed { code: 1, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Texto que cambia cuando cambia cualquier rama, tag o HEAD.
    /// Sirve para saber si hay que recargar el historial.
    pub fn refs_fingerprint(&self) -> Result<String> {
        let refs = self.git(&["for-each-ref", "--format=%(objectname) %(refname)"])?;
        let head = match self.git(&["rev-parse", "-q", "--verify", "HEAD"]) {
            Ok(h) => h,
            Err(GitError::Failed { code: 1, .. }) => String::new(),
            Err(e) => return Err(e),
        };
        let branch = self.current_branch()?.unwrap_or_default();
        Ok(format!("{head}{branch}\n{refs}"))
    }

    /// `true` si todavía no hay ningún commit.
    pub fn is_empty(&self) -> Result<bool> {
        match self.git(&["rev-parse", "--verify", "-q", "HEAD"]) {
            Ok(_) => Ok(false),
            Err(GitError::Failed { code: 1, .. }) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    #[test]
    fn opens_from_subdirectory() {
        let t = TestRepo::new();
        let sub = t.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        let repo = Repo::open(&sub).unwrap();
        assert_eq!(
            repo.root().canonicalize().unwrap(),
            t.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_plain_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Repo::open(dir.path()).is_err());
    }

    #[test]
    fn branch_and_empty_state() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        assert!(repo.is_empty().unwrap());
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));

        t.commit_file("a.txt", "a\n", "first");
        assert!(!repo.is_empty().unwrap());

        t.git(&["checkout", "-q", "--detach"]);
        assert_eq!(repo.current_branch().unwrap(), None);
    }

    #[test]
    fn fingerprint_changes_with_refs() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        let empty = repo.refs_fingerprint().unwrap();
        t.commit_file("a.txt", "a\n", "first");
        let one = repo.refs_fingerprint().unwrap();
        assert_ne!(empty, one);
        assert_eq!(one, repo.refs_fingerprint().unwrap());
        t.git(&["tag", "v1"]);
        assert_ne!(one, repo.refs_fingerprint().unwrap());
    }
}
