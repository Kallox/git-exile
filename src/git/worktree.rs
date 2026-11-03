//! Diffs de los cambios que todavía no están en un commit.

use super::details::truncate;
use super::diff::{DIFF_ARGS, DiffLine, FileDiff, FileStatus, Hunk, LineKind, parse_diff};
use super::repo::Repo;
use super::runner::Result;
use super::status::{ChangeKind, StatusEntry};

/// Archivos nuevos más grandes que esto no se muestran.
const MAX_UNTRACKED_BYTES: u64 = 4 * 1024 * 1024;

/// Diff de un archivo del working tree: `staged` compara índice con HEAD,
/// si no, working tree con índice.
pub fn diff_entry(repo: &Repo, entry: &StatusEntry, staged: bool) -> Result<FileDiff> {
    if entry.kind == ChangeKind::Untracked {
        return untracked_diff(repo, &entry.path);
    }
    let mut args = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    args.extend_from_slice(DIFF_ARGS);
    args.extend(["--", &entry.path]);
    if let Some(orig) = &entry.orig_path {
        args.push(orig);
    }
    let out = repo.git(&args)?;
    let (out, _) = truncate(&out);
    Ok(parse_diff(out)
        .into_iter()
        .next()
        .unwrap_or_else(|| empty_diff(entry)))
}

fn empty_diff(entry: &StatusEntry) -> FileDiff {
    FileDiff {
        old_path: Some(entry.path.clone()),
        new_path: Some(entry.path.clone()),
        status: FileStatus::Modified,
        binary: false,
        hunks: Vec::new(),
    }
}

/// Un archivo sin seguimiento se muestra como si se añadiera entero.
fn untracked_diff(repo: &Repo, path: &str) -> Result<FileDiff> {
    let full = repo.root().join(path);
    let mut diff = FileDiff {
        old_path: None,
        new_path: Some(path.to_owned()),
        status: FileStatus::Added,
        binary: false,
        hunks: Vec::new(),
    };
    if std::fs::metadata(&full)?.len() > MAX_UNTRACKED_BYTES {
        diff.binary = true;
        return Ok(diff);
    }
    let bytes = std::fs::read(&full)?;
    if bytes.contains(&0) {
        diff.binary = true;
        return Ok(diff);
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<DiffLine> = text
        .lines()
        .enumerate()
        .map(|(i, l)| DiffLine {
            kind: LineKind::Added,
            old_no: None,
            new_no: Some(i as u32 + 1),
            text: l.to_owned(),
        })
        .collect();
    if !lines.is_empty() {
        diff.hunks.push(Hunk {
            header: format!("@@ -0,0 +1,{} @@", lines.len()),
            lines,
        });
    }
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::load_status;
    use crate::git::testutil::TestRepo;

    #[test]
    fn diffs_staged_unstaged_and_untracked() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        t.write("a.txt", "2\n");
        t.git(&["add", "a.txt"]);
        t.write("a.txt", "3\n");
        t.write("new.txt", "x\ny\n");
        let repo = Repo::open(t.path()).unwrap();
        let status = load_status(&repo).unwrap();

        let staged = diff_entry(&repo, &status.staged[0], true).unwrap();
        let texts = |d: &FileDiff| -> Vec<String> {
            d.hunks[0]
                .lines
                .iter()
                .map(|l| format!("{:?}:{}", l.kind, l.text))
                .collect()
        };
        assert_eq!(texts(&staged), ["Removed:1", "Added:2"]);

        let unstaged = diff_entry(&repo, &status.unstaged[0], false).unwrap();
        assert_eq!(texts(&unstaged), ["Removed:2", "Added:3"]);

        let untracked = status
            .unstaged
            .iter()
            .find(|e| e.kind == ChangeKind::Untracked)
            .unwrap();
        let new = diff_entry(&repo, untracked, false).unwrap();
        assert_eq!(new.status, FileStatus::Added);
        assert_eq!(new.stats(), (2, 0));
    }

    #[test]
    fn staged_diff_in_empty_repository() {
        let t = TestRepo::new();
        t.write("a.txt", "hola\n");
        t.git(&["add", "a.txt"]);
        let repo = Repo::open(t.path()).unwrap();
        let status = load_status(&repo).unwrap();
        let d = diff_entry(&repo, &status.staged[0], true).unwrap();
        assert_eq!(d.status, FileStatus::Added);
        assert_eq!(d.stats(), (1, 0));
    }
}
