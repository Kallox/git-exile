//! Parser de diffs unificados tal y como los genera `git diff` / `git show`.

/// Argumentos que hacen la salida de un diff predecible
/// sin importar la configuración del usuario.
pub const DIFF_ARGS: &[&str] = &[
    "--no-color",
    "--no-ext-diff",
    "--src-prefix=a/",
    "--dst-prefix=b/",
    "--find-renames",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

impl FileStatus {
    pub fn letter(self) -> char {
        match self {
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Modified => 'M',
            Self::Renamed => 'R',
            Self::Copied => 'C',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
    /// `\ No newline at end of file`
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// Ruta a mostrar: la nueva, o la antigua si el archivo se borró.
    pub fn path(&self) -> &str {
        self.new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .unwrap_or("")
    }

    pub fn stats(&self) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        for line in self.hunks.iter().flat_map(|h| &h.lines) {
            match line.kind {
                LineKind::Added => added += 1,
                LineKind::Removed => removed += 1,
                _ => {}
            }
        }
        (added, removed)
    }
}

pub fn parse_diff(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_no = 0;
    let mut new_no = 0;
    let mut in_hunk = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let (old, new) = split_git_header(rest);
            files.push(FileDiff {
                old_path: Some(old),
                new_path: Some(new),
                status: FileStatus::Modified,
                binary: false,
                hunks: Vec::new(),
            });
            in_hunk = false;
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };

        if line.starts_with("@@") {
            let (o, n) = parse_hunk_header(line);
            old_no = o;
            new_no = n;
            in_hunk = true;
            file.hunks.push(Hunk {
                header: line.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }

        if in_hunk && let Some(hunk) = file.hunks.last_mut() {
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (LineKind::Added, &line[1..]),
                Some(b'-') => (LineKind::Removed, &line[1..]),
                Some(b' ') => (LineKind::Context, &line[1..]),
                Some(b'\\') => (LineKind::NoNewline, line),
                // Línea vacía de contexto (algunos editores recortan el espacio).
                None => (LineKind::Context, ""),
                _ => {
                    in_hunk = false;
                    continue;
                }
            };
            let (o, n) = match kind {
                LineKind::Context => (Some(old_no), Some(new_no)),
                LineKind::Removed => (Some(old_no), None),
                LineKind::Added => (None, Some(new_no)),
                LineKind::NoNewline => (None, None),
            };
            if o.is_some() {
                old_no += 1;
            }
            if n.is_some() {
                new_no += 1;
            }
            hunk.lines.push(DiffLine {
                kind,
                old_no: o,
                new_no: n,
                text: text.to_owned(),
            });
            continue;
        }

        // Cabeceras extendidas.
        if line.starts_with("new file mode") {
            file.status = FileStatus::Added;
            file.old_path = None;
        } else if line.starts_with("deleted file mode") {
            file.status = FileStatus::Deleted;
            file.new_path = None;
        } else if let Some(p) = line.strip_prefix("rename from ") {
            file.status = FileStatus::Renamed;
            file.old_path = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("rename to ") {
            file.new_path = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("copy from ") {
            file.status = FileStatus::Copied;
            file.old_path = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("copy to ") {
            file.new_path = Some(unquote(p));
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            file.binary = true;
        } else if let Some(p) = line.strip_prefix("--- ") {
            if let Some(p) = strip_side(p, "a/") {
                file.old_path = Some(p);
            }
        } else if let Some(p) = line.strip_prefix("+++ ")
            && let Some(p) = strip_side(p, "b/")
        {
            file.new_path = Some(p);
        }
    }
    files
}

/// `a/x b/x` -> (`x`, `x`). Si hay espacios en la ruta la cabecera es ambigua;
/// se asume que ambos lados son iguales (las cabeceras `---`/`+++`
/// o `rename` corrigen después los casos en que no lo son).
fn split_git_header(rest: &str) -> (String, String) {
    if rest.starts_with('"') {
        // Rutas entrecomilladas: "a/x" "b/y"
        if let Some(end) = find_closing_quote(rest) {
            let old = unquote(&rest[..=end]);
            let new = unquote(rest[end + 1..].trim_start());
            return (strip_prefix_owned(old, "a/"), strip_prefix_owned(new, "b/"));
        }
    }
    let half = rest.len() / 2;
    if rest.len() % 2 == 1 && rest.as_bytes().get(half) == Some(&b' ') {
        let (a, b) = (&rest[..half], &rest[half + 1..]);
        if let (Some(a), Some(b)) = (a.strip_prefix("a/"), b.strip_prefix("b/"))
            && a == b
        {
            return (a.to_owned(), b.to_owned());
        }
    }
    match rest.split_once(" b/") {
        Some((a, b)) => (a.strip_prefix("a/").unwrap_or(a).to_owned(), b.to_owned()),
        None => (rest.to_owned(), rest.to_owned()),
    }
}

fn find_closing_quote(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn strip_prefix_owned(s: String, prefix: &str) -> String {
    s.strip_prefix(prefix).map(str::to_owned).unwrap_or(s)
}

fn strip_side(path: &str, prefix: &str) -> Option<String> {
    let path = unquote(path.trim_end_matches('\t'));
    if path == "/dev/null" {
        None
    } else {
        Some(strip_prefix_owned(path, prefix))
    }
}

/// Deshace el entrecomillado estilo C que usa git para rutas raras.
fn unquote(s: &str) -> String {
    let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return s.to_owned();
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.bytes().peekable();
    while let Some(b) = chars.next() {
        if b != b'\\' {
            bytes.push(b);
            continue;
        }
        match chars.next() {
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(b'"') => bytes.push(b'"'),
            Some(b'\\') => bytes.push(b'\\'),
            Some(d @ b'0'..=b'7') => {
                let mut value = (d - b'0') as u32;
                for _ in 0..2 {
                    if let Some(&n @ b'0'..=b'7') = chars.peek() {
                        value = value * 8 + (n - b'0') as u32;
                        chars.next();
                    }
                }
                bytes.push(value as u8);
            }
            Some(other) => bytes.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `@@ -10,7 +12,8 @@ fn x()` -> (10, 12)
fn parse_hunk_header(line: &str) -> (u32, u32) {
    let mut old = 0;
    let mut new = 0;
    for part in line.split_whitespace().skip(1).take(2) {
        let num = |s: &str| {
            s.split(',')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        };
        if let Some(p) = part.strip_prefix('-') {
            old = num(p);
        } else if let Some(p) = part.strip_prefix('+') {
            new = num(p);
        }
    }
    (old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODIFIED: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 83db48f..bf269f4 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@ fn main() {
 one
-two
+dos
+tres
 four

@@ -10,2 +11,2 @@
-x
+y
";

    #[test]
    fn parses_modified_file() {
        let files = parse_diff(MODIFIED);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.status, FileStatus::Modified);
        assert_eq!(f.path(), "src/main.rs");
        assert_eq!(f.hunks.len(), 2);
        assert_eq!(f.stats(), (3, 2));

        let lines = &f.hunks[0].lines;
        assert_eq!(lines.len(), 6);
        assert_eq!(
            lines[1],
            DiffLine {
                kind: LineKind::Removed,
                old_no: Some(2),
                new_no: None,
                text: "two".into()
            }
        );
        assert_eq!((lines[3].new_no, lines[3].text.as_str()), (Some(3), "tres"));
        assert_eq!((lines[4].old_no, lines[4].new_no), (Some(3), Some(4)));
        // Línea vacía de contexto.
        assert_eq!((lines[5].old_no, lines[5].new_no), (Some(4), Some(5)));
        assert_eq!(f.hunks[1].lines[0].old_no, Some(10));
    }

    #[test]
    fn parses_added_deleted_renamed_and_binary() {
        let text = "\
diff --git a/new file.txt b/new file.txt
new file mode 100644
index 0000000..ce01362
--- /dev/null
+++ b/new file.txt
@@ -0,0 +1 @@
+hello
\\ No newline at end of file
diff --git a/old.txt b/old.txt
deleted file mode 100644
index ce01362..0000000
--- a/old.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/a.txt b/b.txt
similarity index 100%
rename from a.txt
rename to b.txt
diff --git a/img.png b/img.png
index 1..2 100644
Binary files a/img.png and b/img.png differ
";
        let files = parse_diff(text);
        assert_eq!(files.len(), 4);

        assert_eq!(files[0].status, FileStatus::Added);
        assert_eq!(files[0].path(), "new file.txt");
        assert_eq!(files[0].old_path, None);
        assert_eq!(files[0].hunks[0].lines[1].kind, LineKind::NoNewline);
        assert_eq!(files[0].stats(), (1, 0));

        assert_eq!(files[1].status, FileStatus::Deleted);
        assert_eq!(files[1].path(), "old.txt");
        assert_eq!(files[1].new_path, None);

        assert_eq!(files[2].status, FileStatus::Renamed);
        assert_eq!(files[2].old_path.as_deref(), Some("a.txt"));
        assert_eq!(files[2].new_path.as_deref(), Some("b.txt"));
        assert!(files[2].hunks.is_empty());

        assert!(files[3].binary);
        assert_eq!(files[3].path(), "img.png");
    }

    #[test]
    fn unquotes_paths() {
        assert_eq!(unquote(r#""tab\there""#), "tab\there");
        assert_eq!(unquote(r#""\303\261""#), "ñ");
        assert_eq!(unquote("plain"), "plain");
        let (a, b) = split_git_header(r#""a/x\ty" "b/x\ty""#);
        assert_eq!((a.as_str(), b.as_str()), ("x\ty", "x\ty"));
    }
}
