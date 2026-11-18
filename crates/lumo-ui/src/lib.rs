//! The Lumo window, built on egui.

mod about;
mod address;
mod app;
mod bin;
mod complete;
mod files;
mod format;
mod fuzzy;
mod icons;
mod jobs;
mod listing;
mod nav;
mod palette;
mod session;
mod tab;
mod theme;
mod view;
mod watch;

pub use about::install_crash_handler;
pub use app::{APP_ID, Action, Config, LumoApp, Start};
pub use listing::{Listing, LoadState, Row, SortColumn, SortOrder};
pub use session::Session;
pub use tab::{Move, Tab};

use eframe::egui;

/// Where settings, open tabs and crash reports live. `LUMO_DATA_DIR`
/// relocates it (tests, portable installs).
pub fn data_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LUMO_DATA_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| eframe::storage_dir(APP_ID))
}

/// Opens the main window and blocks until it closes.
pub fn run(start: Start) -> eframe::Result {
    let data_dir = data_dir();
    let config = Config {
        start,
        session_file: data_dir.as_ref().map(|dir| dir.join("session.json")),
        isolated: false,
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
        // On Windows Lumo reads the clipboard itself, so Ctrl+V works when it
        // holds files (egui-winit only reports text pastes). See
        // `LumoApp::raw_input_hook`.
        clipboard_shortcuts: !cfg!(windows),
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(LumoApp::new(cc, config)))),
    )
}
