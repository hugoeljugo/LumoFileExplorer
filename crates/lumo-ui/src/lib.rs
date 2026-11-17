//! The Lumo window, built on egui.

mod address;
mod app;
mod complete;
mod format;
mod icons;
mod listing;
mod nav;
mod session;
mod tab;
mod theme;
mod view;

pub use app::{APP_ID, Action, Config, LumoApp, Start};
pub use listing::{Listing, LoadState, Row, SortColumn, SortOrder};
pub use session::Session;
pub use tab::{Move, Tab};

use eframe::egui;

/// Opens the main window and blocks until it closes.
pub fn run(start: Start) -> eframe::Result {
    // LUMO_DATA_DIR relocates all saved state (tests, portable installs).
    let data_dir = std::env::var_os("LUMO_DATA_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| eframe::storage_dir(APP_ID));
    let config = Config {
        start,
        session_file: data_dir.as_ref().map(|dir| dir.join("session.json")),
    };
    let options = eframe::NativeOptions {
        // OpenGL via glow: measured on Windows 11 it opens the window in
        // ~0.5s with ~75-130 MB private memory, vs ~1.1s and ~370 MB for wgpu.
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("Lumo")
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([520.0, 320.0]),
        persistence_path: data_dir,
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(LumoApp::new(cc, config)))),
    )
}
