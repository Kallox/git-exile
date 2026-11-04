//! Utilidades para crear repositorios temporales en los tests.

use std::path::Path;

use tempfile::TempDir;

pub struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let t = Self { dir };
        t.git(&["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            t.git(&["config", key, value]);
        }
        t
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn git(&self, args: &[&str]) -> String {
        super::run(self.path(), args).unwrap()
    }

    pub fn write(&self, name: &str, content: &str) {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Escribe un archivo, lo añade y crea un commit. Devuelve el hash.
    pub fn commit_file(&self, name: &str, content: &str, message: &str) -> String {
        self.write(name, content);
        self.git(&["add", "--", name]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }
}
