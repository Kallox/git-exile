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

fn icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
        .expect("el icono incluido es un PNG válido")
}

fn main() -> eframe::Result<()> {
    // Cada ruta pasada como argumento se abre en una pestaña.
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("git-exile")
            // En Wayland el icono sale del archivo .desktop con este mismo nombre.
            .with_app_id("git-exile")
            .with_icon(icon())
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([880.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "git-exile",
        options,
        Box::new(|cc| Ok(Box::new(GitExileApp::new(cc, paths)))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_icon_loads_with_transparency() {
        let icon = super::icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        // Esquina transparente y centro opaco (RGBA).
        assert_eq!(icon.rgba[3], 0);
        let center = (128 * 256 + 128) * 4;
        assert_eq!(icon.rgba[center + 3], 255);
    }
}
