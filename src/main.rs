mod app;
mod git;
mod ui;
mod worker;

use std::path::PathBuf;

use app::GitExileApp;

fn main() -> eframe::Result<()> {
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);

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
        Box::new(|cc| Ok(Box::new(GitExileApp::new(cc, initial_path)))),
    )
}
