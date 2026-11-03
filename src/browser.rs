//! Exploración de carpetas para encontrar repositorios git.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Carpetas que no merece la pena recorrer al buscar repositorios.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "vendor",
    "__pycache__",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_repo: bool,
    pub hidden: bool,
}

/// `true` si `path` es la raíz de un repositorio (o worktree/submódulo).
pub fn is_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Raíz del repositorio que contiene `path`, si lo hay.
pub fn repo_root_of(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| is_repo(p)).map(Path::to_path_buf)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Subcarpetas de `path`, ordenadas por nombre sin distinguir mayúsculas.
pub fn list_dirs(path: &Path) -> io::Result<Vec<DirEntry>> {
    let mut dirs: Vec<DirEntry> = std::fs::read_dir(path)?
        .filter_map(Result::ok)
        // `metadata` sigue enlaces simbólicos: un enlace a carpeta cuenta como carpeta.
        .filter(|e| e.path().metadata().is_ok_and(|m| m.is_dir()))
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            DirEntry {
                hidden: name.starts_with('.'),
                is_repo: is_repo(&path),
                name,
                path,
            }
        })
        .collect();
    dirs.sort_by_key(|d| d.name.to_lowercase());
    Ok(dirs)
}

/// Busca repositorios bajo `root` hasta `max_depth` niveles. No entra en los
/// repositorios encontrados ni en carpetas ocultas. Llama a `found` con cada uno
/// y se detiene si `cancel` se activa o `found` devuelve `false`.
pub fn find_repos(
    root: &Path,
    max_depth: usize,
    cancel: &AtomicBool,
    found: &mut dyn FnMut(PathBuf) -> bool,
) {
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if is_repo(&dir) {
            if !found(dir) {
                return;
            }
            continue;
        }
        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            // Sin seguir enlaces simbólicos, para no entrar en bucles.
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_ref())
            })
            .map(|e| e.path())
            .collect();
        // Orden alfabético en el resultado (la pila invierte el orden).
        children.sort_by_key(|p| std::cmp::Reverse(p.file_name().map(|n| n.to_ascii_lowercase())));
        stack.extend(children.into_iter().map(|p| (p, depth + 1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mkdir(root: &Path, rel: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn lists_directories_and_marks_repos() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkdir(root, "beta/.git");
        mkdir(root, "Alpha");
        mkdir(root, ".hidden");
        std::fs::write(root.join("file.txt"), "x").unwrap();

        let dirs = list_dirs(root).unwrap();
        let names: Vec<_> = dirs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, [".hidden", "Alpha", "beta"]);
        assert!(dirs[2].is_repo && !dirs[1].is_repo);
        assert!(dirs[0].hidden);
    }

    #[test]
    fn finds_root_of_nested_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = mkdir(tmp.path(), "repo");
        mkdir(&repo, ".git");
        let deep = mkdir(&repo, "src/a/b");
        assert_eq!(repo_root_of(&deep), Some(repo));
        assert_eq!(repo_root_of(tmp.path()), None);
    }

    #[test]
    fn searches_repositories_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkdir(root, "work/one/.git");
        mkdir(root, "work/one/nested/.git"); // dentro de otro repo: no se busca
        mkdir(root, "work/deep/a/b/c/.git"); // demasiado profundo
        mkdir(root, "two/.git");
        mkdir(root, ".cache/three/.git"); // oculto
        mkdir(root, "node_modules/four/.git"); // ignorado

        let mut found = Vec::new();
        let cancel = AtomicBool::new(false);
        find_repos(root, 3, &cancel, &mut |p| {
            found.push(p.strip_prefix(root).unwrap().to_path_buf());
            true
        });
        assert_eq!(found, [PathBuf::from("two"), PathBuf::from("work/one")]);
    }

    #[test]
    fn search_stops_when_callback_returns_false() {
        let tmp = tempfile::tempdir().unwrap();
        mkdir(tmp.path(), "a/.git");
        mkdir(tmp.path(), "b/.git");
        let mut count = 0;
        find_repos(tmp.path(), 2, &AtomicBool::new(false), &mut |_| {
            count += 1;
            false
        });
        assert_eq!(count, 1);
    }
}
