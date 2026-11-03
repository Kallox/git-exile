//! Acceso a git ejecutando el binario `git` del sistema.
//!
//! Este módulo no depende de la interfaz gráfica.

mod repo;
mod runner;

#[cfg(test)]
pub(crate) mod testutil;

pub use repo::Repo;
pub use runner::{GitError, Result, run, run_with_stdin, version};
