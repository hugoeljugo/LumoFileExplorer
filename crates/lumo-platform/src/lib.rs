//! OS integration: well-known folders, drives, opening files with their
//! default app, and system UI resources. Windows shell hooks land here in
//! Phase 3.

#[cfg(windows)]
mod windows;

use std::io;
use std::path::{Path, PathBuf};

/// A user folder shown under "Quick access".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFolder {
    pub label: &'static str,
    pub path: PathBuf,
}

/// The user's standard folders that exist on this machine, in display order.
/// Resolved through the OS (e.g. `SHGetKnownFolderPath`), so relocated
/// folders such as OneDrive-backed Documents are found correctly.
pub fn known_folders() -> Vec<KnownFolder> {
    [
        ("Home", dirs::home_dir()),
        ("Desktop", dirs::desktop_dir()),
        ("Documents", dirs::document_dir()),
        ("Downloads", dirs::download_dir()),
        ("Pictures", dirs::picture_dir()),
        ("Music", dirs::audio_dir()),
        ("Videos", dirs::video_dir()),
    ]
    .into_iter()
    .filter_map(|(label, path)| {
        let path = path?;
        path.is_dir().then_some(KnownFolder { label, path })
    })
    .collect()
}

/// The user's home folder, falling back to the filesystem root.
pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR_STR))
}

/// Root folders of all mounted drives (`C:\`, `D:\`, ... on Windows; `/` elsewhere).
/// Does not touch the drives themselves, so disconnected network drives
/// can't stall the caller.
pub fn drives() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        windows::logical_drives()
    }
    #[cfg(not(windows))]
    {
        vec![PathBuf::from("/")]
    }
}

/// Opens `path` with the program the OS associates with it, without waiting.
pub fn open_path(path: &Path) -> io::Result<()> {
    open::that_detached(path)
}

/// The platform's UI font file, if there is a well-known one (Segoe UI on Windows).
pub fn ui_font() -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        let fonts = std::env::var_os("WINDIR").map(|dir| PathBuf::from(dir).join("Fonts"))?;
        std::fs::read(fonts.join("segoeui.ttf")).ok()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Reports a fatal error to the user. GUI builds have no console on Windows,
/// so this shows a message box there; elsewhere it prints to stderr.
pub fn show_fatal_error(title: &str, message: &str) {
    #[cfg(windows)]
    windows::message_box(title, message);
    #[cfg(not(windows))]
    eprintln!("{title}: {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folders_exist_and_start_with_home() {
        let folders = known_folders();
        assert!(folders.iter().all(|f| f.path.is_dir()));
        if let Some(first) = folders.first() {
            assert_eq!(first.label, "Home");
        }
    }

    #[test]
    fn drives_include_the_system_root() {
        let drives = drives();
        assert!(!drives.is_empty());
        let system_root = std::env::temp_dir()
            .ancestors()
            .last()
            .map(Path::to_path_buf)
            .unwrap();
        assert!(
            drives
                .iter()
                .any(|d| d.as_os_str().eq_ignore_ascii_case(system_root.as_os_str())),
            "{drives:?} should contain {system_root:?}"
        );
    }
}
