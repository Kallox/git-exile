//! Operaciones que modifican el índice y crean commits.

use std::path::Path;

use super::repo::Repo;
use super::runner::{GitError, Result};
use super::status::{ChangeKind, StatusEntry};

/// Añade los archivos al índice (incluidos borrados y archivos nuevos).
pub fn stage(repo: &Repo, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    repo.git(&args).map(drop)
}

/// Saca los archivos del índice sin tocar el working tree.
pub fn unstage(repo: &Repo, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = if repo.is_empty()? {
        // Sin commits no hay HEAD con el que restaurar.
        vec!["rm", "--cached", "-r", "-q", "--ignore-unmatch", "--"]
    } else {
        vec!["restore", "--staged", "--"]
    };
    args.extend(paths.iter().map(String::as_str));
    repo.git(&args).map(drop)
}

pub fn stage_all(repo: &Repo) -> Result<()> {
    repo.git(&["add", "-A"]).map(drop)
}

pub fn unstage_all(repo: &Repo) -> Result<()> {
    if repo.is_empty()? {
        repo.git(&["rm", "--cached", "-r", "-q", "--ignore-unmatch", "."])
            .map(drop)
    } else {
        repo.git(&["reset", "-q"]).map(drop)
    }
}

/// Descarta los cambios de un archivo. Con `staged` (lista de preparados) lo deja
/// como en HEAD, incluidos sus cambios sin añadir; sin él solo deshace lo que no
/// está en el índice. Los archivos que HEAD no conoce se borran del disco.
pub fn discard(repo: &Repo, entry: &StatusEntry, staged: bool) -> Result<()> {
    // Las rutas son literales: un archivo llamado `[a].txt` no debe afectar a `a.txt`.
    let git = |args: &[&str]| {
        let mut all = vec!["--literal-pathspecs"];
        all.extend(args);
        repo.git(&all).map(drop)
    };
    let path = entry.path.as_str();
    let restore_from_head = ["restore", "--source=HEAD", "--staged", "--worktree", "--"];
    match (entry.kind, staged) {
        (ChangeKind::Untracked, _) if path.ends_with('/') => Err(GitError::Refused(format!(
            "{path} es otro repositorio git: no se borra desde aquí"
        ))),
        (ChangeKind::Untracked, _) => {
            git(&["clean", "-f", "-q", "--", path])?;
            remove_empty_parents(repo.root(), path);
            Ok(())
        }
        // Sin `staged`, `Added` es un `git add -N`: restaurarlo vaciaría el archivo.
        (ChangeKind::Added, _) | (ChangeKind::Copied, true) => git(&["rm", "-f", "-q", "--", path]),
        (ChangeKind::Renamed, true) => {
            if let Some(orig) = &entry.orig_path {
                git(&[&restore_from_head[..], &[orig.as_str()]].concat())?;
            }
            git(&["rm", "-f", "-q", "--", path])
        }
        (_, true) => git(&[&restore_from_head[..], &[path]].concat()),
        (_, false) => git(&["restore", "--worktree", "--", path]),
    }
}

/// Borra las carpetas que `git clean` deja vacías, como hace `git rm`.
fn remove_empty_parents(root: &Path, path: &str) {
    let parents = Path::new(path).ancestors().skip(1);
    for dir in parents.take_while(|d| !d.as_os_str().is_empty()) {
        if std::fs::remove_dir(root.join(dir)).is_err() {
            break;
        }
    }
}

/// Crea un commit con lo que hay en el índice. Git aplica la configuración
/// del usuario: identidad, hooks, firma, plantillas de limpieza, etc.
/// Con `reset_author`, el amend pone como autor la identidad actual en lugar de
/// conservar la original. Devuelve el hash del nuevo commit.
pub fn create_commit(
    repo: &Repo,
    message: &str,
    amend: bool,
    reset_author: bool,
) -> Result<String> {
    let mut args = vec!["commit", "-F", "-"];
    if amend {
        args.push("--amend");
        if reset_author {
            args.push("--reset-author");
        }
    }
    repo.git_with_stdin(&args, message.as_bytes())?;
    Ok(repo.git(&["rev-parse", "HEAD"])?.trim().to_owned())
}

/// Mensaje completo del último commit (para hacer amend).
pub fn last_commit_message(repo: &Repo) -> Result<String> {
    Ok(repo
        .git(&["log", "-1", "--format=%B"])?
        .trim_end()
        .to_owned())
}

/// Identidad con la que git firmará los commits, p. ej. `Nombre <correo>`.
pub fn author_ident(repo: &Repo) -> Result<String> {
    let ident = repo.git(&["var", "GIT_AUTHOR_IDENT"])?;
    // Formato: `Nombre <correo> 1700000000 +0100`
    let ident = ident.trim();
    Ok(match ident.rfind('>') {
        Some(end) => ident[..=end].to_owned(),
        None => ident.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;
    use crate::git::{WorkingTreeStatus, load_status};

    fn status(t: &TestRepo) -> WorkingTreeStatus {
        load_status(&Repo::open(t.path()).unwrap()).unwrap()
    }

    fn read(t: &TestRepo, name: &str) -> Option<String> {
        std::fs::read_to_string(t.path().join(name)).ok()
    }

    /// Descarta la única entrada de la lista indicada.
    fn discard_only(t: &TestRepo, staged: bool) {
        let s = status(t);
        let list = if staged { &s.staged } else { &s.unstaged };
        assert_eq!(list.len(), 1, "{list:?}");
        discard(&Repo::open(t.path()).unwrap(), &list[0], staged).unwrap();
    }

    #[test]
    fn discards_unstaged_changes() {
        let t = TestRepo::new();
        t.commit_file("mod.txt", "1\n", "first");
        t.commit_file("del.txt", "1\n", "second");

        t.write("mod.txt", "2\n");
        discard_only(&t, false);
        assert_eq!(read(&t, "mod.txt").as_deref(), Some("1\n"));

        std::fs::remove_file(t.path().join("del.txt")).unwrap();
        discard_only(&t, false);
        assert_eq!(read(&t, "del.txt").as_deref(), Some("1\n"));

        t.write("dir/sub/new.txt", "x\n");
        discard_only(&t, false);
        assert!(!t.path().join("dir").exists());
        assert!(status(&t).is_clean());
    }

    #[test]
    fn discarding_unstaged_keeps_the_staged_version() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.write("a.txt", "2\n");
        t.git(&["add", "a.txt"]);
        t.write("a.txt", "3\n");

        discard_only(&t, false);
        assert_eq!(read(&t, "a.txt").as_deref(), Some("2\n"));
        assert_eq!(status(&t).staged.len(), 1);
    }

    #[test]
    fn discards_staged_changes_back_to_head() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.commit_file("b.txt", "1\n", "second");

        // Modificado, añadido y vuelto a modificar.
        t.write("a.txt", "2\n");
        t.git(&["add", "a.txt"]);
        t.write("a.txt", "3\n");
        discard_only(&t, true);
        assert_eq!(read(&t, "a.txt").as_deref(), Some("1\n"));
        assert!(status(&t).is_clean());

        // Borrado.
        t.git(&["rm", "-q", "a.txt"]);
        discard_only(&t, true);
        assert_eq!(read(&t, "a.txt").as_deref(), Some("1\n"));

        // Nuevo, añadido y borrado del disco después.
        t.write("new.txt", "x\n");
        t.git(&["add", "new.txt"]);
        std::fs::remove_file(t.path().join("new.txt")).unwrap();
        discard_only(&t, true);
        assert!(status(&t).is_clean());

        // Renombrado.
        t.git(&["mv", "b.txt", "c.txt"]);
        discard_only(&t, true);
        assert_eq!(read(&t, "b.txt").as_deref(), Some("1\n"));
        assert_eq!(read(&t, "c.txt"), None);
        assert!(status(&t).is_clean());
    }

    #[test]
    fn discards_new_file_in_repo_without_commits() {
        let t = TestRepo::new();
        t.write("a.txt", "1\n");
        t.git(&["add", "a.txt"]);
        discard_only(&t, true);
        assert_eq!(read(&t, "a.txt"), None);
        assert!(status(&t).is_clean());
    }

    #[test]
    fn discarding_intent_to_add_removes_the_file() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.write("new.txt", "x\n");
        t.git(&["add", "-N", "new.txt"]);
        discard_only(&t, false);
        assert_eq!(read(&t, "new.txt"), None);
        assert!(status(&t).is_clean());
    }

    #[test]
    fn discard_treats_paths_literally() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        // Como patrón, `[a].txt` coincidiría con `a.txt`.
        t.commit_file("[a].txt", "1\n", "second");
        t.write("a.txt", "2\n");
        t.write("[a].txt", "2\n");
        t.write("[n]ew.bin", "x\n");
        t.write("new.bin", "x\n");
        let repo = Repo::open(t.path()).unwrap();
        let entry = |path: &str, kind| StatusEntry {
            path: path.into(),
            orig_path: None,
            kind,
        };

        discard(&repo, &entry("[a].txt", ChangeKind::Modified), false).unwrap();
        assert_eq!(read(&t, "[a].txt").as_deref(), Some("1\n"));
        assert_eq!(read(&t, "a.txt").as_deref(), Some("2\n"));

        discard(&repo, &entry("[n]ew.bin", ChangeKind::Untracked), false).unwrap();
        assert_eq!(read(&t, "[n]ew.bin"), None);
        assert_eq!(read(&t, "new.bin").as_deref(), Some("x\n"));
    }

    #[test]
    fn refuses_to_discard_nested_repository() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let nested = TestRepo::new();
        nested.commit_file("f.txt", "1\n", "first");
        let dest = t.path().join("sub");
        crate::git::run(
            t.path(),
            &["clone", "-q", &nested.path().display().to_string(), "sub"],
        )
        .unwrap();

        let s = status(&t);
        assert_eq!(s.unstaged[0].path, "sub/");
        let repo = Repo::open(t.path()).unwrap();
        assert!(discard(&repo, &s.unstaged[0], false).is_err());
        assert!(dest.join("f.txt").exists());
    }

    #[test]
    fn stage_and_unstage_files() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.commit_file("b.txt", "1\n", "second");
        t.write("a.txt", "2\n");
        std::fs::remove_file(t.path().join("b.txt")).unwrap();
        t.write("c.txt", "new\n");
        let repo = Repo::open(t.path()).unwrap();

        stage(&repo, &["a.txt".into(), "b.txt".into(), "c.txt".into()]).unwrap();
        let s = status(&t);
        assert_eq!(s.staged.len(), 3);
        assert!(s.unstaged.is_empty());

        unstage(&repo, &["c.txt".into()]).unwrap();
        let s = status(&t);
        assert_eq!(s.staged.len(), 2);
        assert_eq!(s.unstaged[0].path, "c.txt");

        unstage_all(&repo).unwrap();
        assert!(status(&t).staged.is_empty());
        stage_all(&repo).unwrap();
        assert_eq!(status(&t).staged.len(), 3);
    }

    #[test]
    fn creates_and_amends_commits() {
        let t = TestRepo::new();
        t.write("a.txt", "1\n");
        let repo = Repo::open(t.path()).unwrap();
        stage_all(&repo).unwrap();

        let id = create_commit(&repo, "Primer commit\n\nCon cuerpo", false, false).unwrap();
        assert_eq!(id, t.git(&["rev-parse", "HEAD"]).trim());
        assert_eq!(
            last_commit_message(&repo).unwrap(),
            "Primer commit\n\nCon cuerpo"
        );
        assert!(status(&t).is_clean());

        let amended = create_commit(&repo, "Corregido", true, false).unwrap();
        assert_ne!(amended, id);
        assert_eq!(last_commit_message(&repo).unwrap(), "Corregido");
        assert_eq!(t.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
    }

    #[test]
    fn commit_errors_are_reported() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        // Nada en el índice.
        let err = create_commit(&repo, "vacío", false, false).unwrap_err();
        assert!(err.to_string().contains("nothing"), "{err}");
    }

    #[test]
    fn amend_can_reset_the_author() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.git(&["config", "user.name", "Otro"]);
        t.git(&["config", "user.email", "otro@example.com"]);
        let repo = Repo::open(t.path()).unwrap();
        let author = || {
            t.git(&["log", "-1", "--format=%an <%ae>"])
                .trim()
                .to_owned()
        };

        create_commit(&repo, "first", true, false).unwrap();
        assert_eq!(author(), "Test <test@example.com>");
        create_commit(&repo, "first", true, true).unwrap();
        assert_eq!(author(), "Otro <otro@example.com>");
    }

    #[test]
    fn reads_author_identity() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        assert_eq!(author_ident(&repo).unwrap(), "Test <test@example.com>");
    }

    #[test]
    fn unstage_in_empty_repository() {
        let t = TestRepo::new();
        t.write("a.txt", "1\n");
        t.write("b.txt", "1\n");
        let repo = Repo::open(t.path()).unwrap();
        stage_all(&repo).unwrap();
        unstage(&repo, &["a.txt".into()]).unwrap();
        let s = status(&t);
        assert_eq!(s.staged.len(), 1);
        unstage_all(&repo).unwrap();
        assert!(status(&t).staged.is_empty());
    }
}
