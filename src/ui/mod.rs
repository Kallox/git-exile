//! Componentes de la interfaz.

pub mod branch_dialog;
pub mod commit_details;
pub mod commit_editor;
pub mod commit_list;
pub mod connect_remote_dialog;
pub mod diff_view;
pub mod discard_dialog;
pub mod graph_view;
pub mod icons;
pub mod new_repo_dialog;
pub mod repo_browser;
pub mod status_panel;
pub mod theme;
pub mod update_bar;

use chrono::{Local, TimeZone};

/// Formatea segundos desde epoch en la zona horaria local.
pub fn format_time(secs: i64) -> String {
    match Local.timestamp_opt(secs, 0).single() {
        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        None => secs.to_string(),
    }
}

/// Fecha larga en español, p. ej. `3 nov 2026, 14:49`.
pub fn format_date_long(secs: i64) -> String {
    use chrono::{Datelike, Timelike};
    const MONTHS: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ];
    match Local.timestamp_opt(secs, 0).single() {
        Some(t) => format!(
            "{} {} {}, {:02}:{:02}",
            t.day(),
            MONTHS[t.month0() as usize],
            t.year(),
            t.hour(),
            t.minute()
        ),
        None => secs.to_string(),
    }
}
