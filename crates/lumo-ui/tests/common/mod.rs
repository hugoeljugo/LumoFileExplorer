// Shared test helpers; clippy.toml's test exemption only covers #[test] fns.
#![allow(clippy::unwrap_used, dead_code)]

use std::fs::{File, create_dir};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Modifiers;
use egui_kittest::Harness;
use lumo_ui::{Config, LumoApp, Start};

/// Ctrl as winit reports it on Windows/Linux: both `ctrl` and `command` set.
pub const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};

pub fn harness(config: Config) -> Harness<'static, LumoApp> {
    Harness::builder()
        .with_size([1100.0, 700.0])
        // Real frame pacing, so double-clicks and type-ahead timing behave as in the app.
        .with_step_dt(1.0 / 60.0)
        .build_eframe(move |cc| LumoApp::new(cc, config))
}

/// Opens one tab at `path`, without session saving.
pub fn at(path: &Path) -> Config {
    Config {
        start: Start::Path(path.to_path_buf()),
        session_file: None,
    }
}

/// Steps until the active tab has finished loading, then lets layout settle.
pub fn settle(h: &mut Harness<'_, LumoApp>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.step();
        if h.state().is_idle() {
            break;
        }
        assert!(Instant::now() < deadline, "folder never finished loading");
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
}

/// Names in the active tab, in display order.
pub fn visible_names(h: &Harness<'_, LumoApp>) -> Vec<String> {
    let listing = h.state().active_tab().listing().unwrap();
    listing
        .view()
        .iter()
        .map(|&i| listing.rows()[i].name.clone())
        .collect()
}

pub fn selected_name(h: &Harness<'_, LumoApp>) -> Option<String> {
    h.state()
        .active_tab()
        .selected_row()
        .map(|r| r.name.clone())
}

pub fn active_path(h: &Harness<'_, LumoApp>) -> PathBuf {
    h.state().active_tab().path().to_path_buf()
}

/// A temp folder with the given subfolders (trailing `/`) and files.
pub fn tree(items: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for item in items {
        match item.strip_suffix('/') {
            Some(folder) => create_dir(dir.path().join(folder)).unwrap(),
            None => drop(File::create(dir.path().join(item)).unwrap()),
        }
    }
    dir
}

pub fn many_files(count: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..count {
        File::create(dir.path().join(format!("file{i:05}.txt"))).unwrap();
    }
    dir
}
