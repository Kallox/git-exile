//! Componentes de la interfaz.

pub mod branch_dialog;
pub mod commit_details;
pub mod commit_editor;
pub mod commit_list;
pub mod diff_view;
pub mod graph_view;
pub mod icons;
pub mod repo_browser;
pub mod status_panel;
pub mod theme;

use chrono::{Local, TimeZone};

/// Formatea segundos desde epoch en la zona horaria local.
pub fn format_time(secs: i64) -> String {
    match Local.timestamp_opt(secs, 0).single() {
        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        None => secs.to_string(),
    }
}
