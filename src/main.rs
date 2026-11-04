// Sin consola de fondo en Windows; en desarrollo se mantiene para ver los mensajes.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod browser;
mod git;
mod repo_tab;
mod ui;
mod worker;

use std::path::PathBuf;

use app::GitExileApp;

fn main() -> eframe::Result<()> {
    // Cada ruta pasada como argumento se abre en una pestaña.
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("git-exile")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "git-exile",
        options,
        Box::new(|cc| Ok(Box::new(GitExileApp::new(cc, paths)))),
    )
}
