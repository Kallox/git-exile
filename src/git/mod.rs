//! Acceso a git ejecutando el binario `git` del sistema.
//!
//! Este módulo no depende de la interfaz gráfica.

mod branch;
mod commit;
mod details;
mod diff;
mod graph;
mod log;
mod remote;
mod repo;
mod runner;
mod status;
mod sync;
mod worktree;

#[cfg(test)]
pub(crate) mod testutil;

pub use branch::{Branch, checkout_commit, create_branch, list_branches, switch_branch};
pub use commit::{
    author_ident, create_commit, discard, last_commit_message, stage, stage_all, unstage,
    unstage_all,
};
pub use details::{CommitDetails, Person, load_commit};
pub use diff::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};
pub use graph::{Graph, GraphRow, Half};
pub use log::{Commit, RefKind, RefLabel, load_log};
#[cfg(test)]
pub use remote::Hosting;
pub use remote::{Remote, connect_remote, list_remotes};
pub use repo::Repo;
#[cfg(test)]
pub use runner::run;
pub use runner::{Result, version};
pub use status::{ChangeKind, StatusEntry, WorkingTreeStatus, load_status};
pub use sync::SyncOp;
pub use worktree::diff_entry;
