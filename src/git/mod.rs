//! Acceso a git ejecutando el binario `git` del sistema.
//!
//! Este módulo no depende de la interfaz gráfica.

mod details;
mod diff;
mod graph;
mod log;
mod repo;
mod runner;
mod status;

#[cfg(test)]
pub(crate) mod testutil;

pub use details::{CommitDetails, Person, load_commit};
pub use diff::{DIFF_ARGS, DiffLine, FileDiff, FileStatus, Hunk, LineKind, parse_diff};
pub use graph::{Graph, GraphRow, Half};
pub use log::{Commit, RefKind, RefLabel, load_log};
pub use repo::Repo;
pub use runner::{GitError, Result, run, run_with_stdin, version};
pub use status::{ChangeKind, StatusEntry, WorkingTreeStatus, load_status};
