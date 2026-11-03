//! Operaciones que modifican el índice y crean commits.

use super::repo::Repo;
use super::runner::Result;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;
    use crate::git::{WorkingTreeStatus, load_status};

    fn status(t: &TestRepo) -> WorkingTreeStatus {
        load_status(&Repo::open(t.path()).unwrap()).unwrap()
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
