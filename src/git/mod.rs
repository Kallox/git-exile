//! Acceso a git ejecutando el binario `git` del sistema.
//!
//! Este módulo no depende de la interfaz gráfica.

mod runner;

pub use runner::{GitError, Result, run, run_with_stdin, version};
