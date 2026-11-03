use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("no se pudo ejecutar git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("`git {args}` terminó con código {code}:\n{stderr}")]
    Failed {
        args: String,
        code: i32,
        stderr: String,
    },
    #[error("salida de git inesperada: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

/// Construye un `git` que se ejecuta en `dir` con una configuración pensada para leer su salida.
fn command<S: AsRef<OsStr>>(dir: Option<&Path>, args: &[S]) -> Command {
    let mut cmd = Command::new("git");
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(["-c", "color.ui=false", "-c", "core.quotepath=false"])
        .args(args)
        // Mensajes estables independientemente del idioma del sistema.
        .env("LC_ALL", "C")
        // Nunca esperar a que el usuario escriba en una terminal que no existe.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Las lecturas (status, log...) no deben bloquear el índice.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn check<S: AsRef<OsStr>>(args: &[S], output: Output) -> Result<String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let args = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        Err(GitError::Failed {
            args,
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// Ejecuta `git <args>` dentro de `dir` y devuelve su salida estándar.
pub fn run<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Result<String> {
    let output = command(Some(dir), args).output()?;
    check(args, output)
}

/// Igual que [`run`] pero enviando `input` por la entrada estándar.
pub fn run_with_stdin<S: AsRef<OsStr>>(dir: &Path, args: &[S], input: &[u8]) -> Result<String> {
    let mut child = command(Some(dir), args).stdin(Stdio::piped()).spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input)?;
    }
    let output = child.wait_with_output()?;
    check(args, output)
}

/// Versión del git instalado, p. ej. `git version 2.55.0`.
pub fn version() -> Result<String> {
    let args = ["--version"];
    let output = command(None, &args).output()?;
    check(&args, output).map(|s| s.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_version() {
        assert!(version().unwrap().starts_with("git version"));
    }

    #[test]
    fn failing_command_returns_stderr() {
        let dir = std::env::temp_dir();
        let err = run(&dir, &["no-such-subcommand"]).unwrap_err();
        match err {
            GitError::Failed { args, stderr, .. } => {
                assert_eq!(args, "no-such-subcommand");
                assert!(stderr.contains("not a git command"));
            }
            other => panic!("error inesperado: {other:?}"),
        }
    }

    #[test]
    fn passes_stdin() {
        let dir = std::env::temp_dir();
        let out = run_with_stdin(&dir, &["hash-object", "--stdin"], b"hola\n").unwrap();
        assert_eq!(out.trim().len(), 40);
    }
}
