//! Creación de repositorios: inicializar una carpeta o clonar un remoto.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::remote::valid_url;
use super::repo::Repo;
use super::runner::{self, GitError, Result};

/// Repositorio por crear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewRepo {
    Init { path: PathBuf },
    Clone { url: String, path: PathBuf },
}

impl NewRepo {
    /// Carpeta en la que quedará el repositorio.
    pub fn path(&self) -> &Path {
        match self {
            Self::Init { path } | Self::Clone { path, .. } => path,
        }
    }

    pub fn create(&self) -> Result<Repo> {
        let path = self.path();
        // Una ruta relativa dependería de desde dónde se lanzó la aplicación.
        if !path.is_absolute() {
            return Err(GitError::Refused(format!(
                "\"{}\" no es una ruta absoluta",
                path.display()
            )));
        }
        match self {
            Self::Init { .. } => init(path),
            Self::Clone { url, .. } => clone(url, path),
        }
    }
}

/// Crea un repositorio vacío en `path`, creando la carpeta si no existe. Los
/// archivos que ya hubiera se conservan, sin añadir.
fn init(path: &Path) -> Result<Repo> {
    if path.join(".git").exists() {
        return Err(GitError::Refused(format!(
            "{} ya es un repositorio",
            path.display()
        )));
    }
    runner::run_anywhere(&[
        OsStr::new("init"),
        OsStr::new("-q"),
        OsStr::new("--"),
        path.as_os_str(),
    ])?;
    Repo::open(path)
}

fn clone(url: &str, path: &Path) -> Result<Repo> {
    let url = valid_url(url)?;
    if !is_vacant(path) {
        return Err(GitError::Refused(format!(
            "{} ya existe y no está vacía",
            path.display()
        )));
    }
    runner::run_anywhere(&[
        OsStr::new("clone"),
        OsStr::new("--"),
        OsStr::new(url),
        path.as_os_str(),
    ])?;
    Repo::open(path)
}

/// `true` si `path` no existe o es una carpeta vacía.
fn is_vacant(path: &Path) -> bool {
    !path.exists() || std::fs::read_dir(path).is_ok_and(|mut dir| dir.next().is_none())
}

/// Nombre de carpeta que le corresponde a una dirección, como hace `git clone`.
pub fn repo_name_from_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches(['/', '\\']);
    let name = url.rsplit(['/', '\\', ':']).next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::load_status;
    use crate::git::testutil::TestRepo;

    fn init_at(path: &Path) -> Result<Repo> {
        NewRepo::Init {
            path: path.to_path_buf(),
        }
        .create()
    }

    fn clone_to(url: &str, path: &Path) -> Result<Repo> {
        NewRepo::Clone {
            url: url.to_owned(),
            path: path.to_path_buf(),
        }
        .create()
    }

    #[test]
    fn init_creates_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a/b");
        let repo = init_at(&path).unwrap();
        assert!(repo.is_empty().unwrap());
        assert!(path.join(".git").exists());
        assert!(load_status(&repo).unwrap().is_clean());
    }

    #[test]
    fn init_keeps_existing_files_unstaged() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("notas.txt"), "x").unwrap();
        let repo = init_at(tmp.path()).unwrap();
        let status = load_status(&repo).unwrap();
        assert_eq!(status.unstaged.len(), 1);
        assert!(status.staged.is_empty());
    }

    #[test]
    fn init_refuses_an_existing_repository() {
        let t = TestRepo::new();
        assert!(matches!(init_at(t.path()), Err(GitError::Refused(_))));
    }

    #[test]
    fn init_allows_a_folder_inside_a_repository() {
        let t = TestRepo::new();
        let nested = t.path().join("nested");
        let repo = init_at(&nested).unwrap();
        assert!(repo.is_empty().unwrap());
        assert!(nested.join(".git").exists());
    }

    #[test]
    fn clone_brings_the_history() {
        let t = TestRepo::new();
        t.write("a.txt", "a\n");
        t.git(&["add", "."]);
        t.git(&["commit", "-q", "-m", "primero"]);

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("copia");
        let repo = clone_to(&t.path().display().to_string(), &path).unwrap();
        assert!(path.join("a.txt").exists());
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
        assert_eq!(
            repo.git(&["remote"]).unwrap().trim(),
            "origin",
            "debe quedar conectado a su origen"
        );
    }

    #[test]
    fn clone_of_an_empty_repository_works() {
        let t = TestRepo::new();
        let tmp = tempfile::tempdir().unwrap();
        let repo = clone_to(&t.path().display().to_string(), &tmp.path().join("copia")).unwrap();
        assert!(repo.is_empty().unwrap());
    }

    #[test]
    fn clone_into_an_empty_folder_works() {
        let t = TestRepo::new();
        let tmp = tempfile::tempdir().unwrap();
        clone_to(&t.path().display().to_string(), tmp.path()).unwrap();
    }

    #[test]
    fn clone_refuses_a_folder_with_files() {
        let t = TestRepo::new();
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("notas.txt");
        std::fs::write(&file, "x").unwrap();

        let url = t.path().display().to_string();
        assert!(matches!(
            clone_to(&url, tmp.path()),
            Err(GitError::Refused(_))
        ));
        // Tampoco si el destino es un archivo.
        assert!(matches!(clone_to(&url, &file), Err(GitError::Refused(_))));
    }

    #[test]
    fn clone_refuses_what_git_would_read_as_an_option() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("copia");
        let result = clone_to("--upload-pack=touch pwned", &path);
        assert!(matches!(result, Err(GitError::Refused(_))));
    }

    #[test]
    fn failed_clone_leaves_nothing_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("copia");
        let missing = tmp.path().join("no-existe").display().to_string();
        assert!(matches!(
            clone_to(&missing, &path),
            Err(GitError::Failed { .. })
        ));
        assert!(!path.exists());
    }

    #[test]
    fn relative_paths_are_refused() {
        let result = init_at(Path::new("carpeta/relativa"));
        assert!(matches!(result, Err(GitError::Refused(_))));
        assert!(!Path::new("carpeta").exists());
    }

    #[test]
    fn names_folders_after_the_url() {
        for (url, name) in [
            ("git@github.com:usuario/repo.git", Some("repo")),
            ("git@host:repo.git", Some("repo")),
            ("https://github.com/usuario/repo", Some("repo")),
            ("https://github.com/usuario/repo.git/", Some("repo")),
            (" ssh://git@host/grupo/repo.git ", Some("repo")),
            ("/srv/git/repo.git", Some("repo")),
            ("C:\\repos\\repo", Some("repo")),
            ("", None),
            ("https://github.com/usuario/..", None),
            (".git", None),
        ] {
            assert_eq!(repo_name_from_url(url).as_deref(), name, "{url}");
        }
    }
}
