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

/// Shows the OS "Open with…" chooser for `path` (Windows only for now).
pub fn open_with(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("rundll32.exe")
            .arg("shell32.dll,OpenAs_RunDLL")
            .arg(path)
            .spawn()
            .map(drop)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "\"Open with\" isn't available on this platform yet",
        ))
    }
}

/// Moves `paths` to the Recycle Bin (the trash on Linux).
///
/// Returns whether every item can be restored. Windows deletes permanently
/// when a drive or account has no Recycle Bin (network shares, some USB
/// drives, the bin turned off, service accounts), so callers must not offer
/// "Undo" when this is `false`.
pub fn move_to_trash(paths: &[PathBuf]) -> io::Result<bool> {
    trash::delete_all(paths).map_err(io::Error::other)?;
    #[cfg(any(
        windows,
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    {
        let trashed = trash::os_limited::list().map_err(io::Error::other)?;
        Ok(paths.iter().all(|path| {
            trashed
                .iter()
                .any(|item| same_path(&item.original_path(), path))
        }))
    }
    #[cfg(not(any(
        windows,
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    {
        Ok(true)
    }
}

/// Path equality as the file system sees it: case-insensitive on Windows.
fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    } else {
        a == b
    }
}

/// Puts back items that were moved to the Recycle Bin from `paths` (the
/// most recent deletion of each). Fails if one of them can't be found.
pub fn restore_from_trash(paths: &[PathBuf]) -> io::Result<()> {
    #[cfg(any(
        windows,
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    {
        let trashed = trash::os_limited::list().map_err(io::Error::other)?;
        let mut items = Vec::with_capacity(paths.len());
        for path in paths {
            let item = trashed
                .iter()
                .filter(|item| same_path(&item.original_path(), path))
                .max_by_key(|item| item.time_deleted)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("{} is no longer in the Recycle Bin", path.display()),
                    )
                })?;
            items.push(item.clone());
        }
        trash::os_limited::restore_all(items).map_err(io::Error::other)
    }
    #[cfg(not(any(
        windows,
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    {
        let _ = paths;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "restoring from the trash isn't supported on this platform",
        ))
    }
}

/// An item sitting in the Recycle Bin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedItem {
    /// Opaque, system-specific id used to restore or purge this item.
    pub id: std::ffi::OsString,
    pub name: std::ffi::OsString,
    /// The folder it was deleted from.
    pub original_parent: PathBuf,
    pub deleted: Option<std::time::SystemTime>,
    /// Size in bytes (files only).
    pub size: Option<u64>,
    pub is_dir: bool,
}

#[cfg(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
mod bin {
    use super::*;
    use std::ffi::OsString;
    use std::time::{Duration, UNIX_EPOCH};

    /// Everything in the Recycle Bin (all drives, merged like Explorer shows it).
    pub fn list_trash() -> io::Result<Vec<TrashedItem>> {
        let items = trash::os_limited::list().map_err(io::Error::other)?;
        Ok(items
            .into_iter()
            .map(|item| {
                let size = trash::os_limited::metadata(&item).ok().map(|m| m.size);
                TrashedItem {
                    deleted: u64::try_from(item.time_deleted)
                        .ok()
                        .map(|secs| UNIX_EPOCH + Duration::from_secs(secs)),
                    size: size.and_then(|s| s.size()),
                    is_dir: size.is_some_and(|s| s.entries().is_some()),
                    id: item.id,
                    name: item.name,
                    original_parent: item.original_parent,
                }
            })
            .collect())
    }

    fn select(ids: &[OsString]) -> io::Result<Vec<trash::TrashItem>> {
        let items: Vec<_> = trash::os_limited::list()
            .map_err(io::Error::other)?
            .into_iter()
            .filter(|item| ids.contains(&item.id))
            .collect();
        if items.len() < ids.len() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "some items are no longer in the Recycle Bin",
            ));
        }
        Ok(items)
    }

    /// Puts the given items back where they were deleted from. Fails (and
    /// restores nothing) if something already exists at a destination.
    pub fn restore_trashed(ids: &[OsString]) -> io::Result<()> {
        trash::os_limited::restore_all(select(ids)?).map_err(|err| match err {
            trash::Error::RestoreCollision { path, .. } => io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists", path.display()),
            ),
            other => io::Error::other(other),
        })
    }

    /// Deletes the given items from the Recycle Bin for good.
    pub fn purge_trashed(ids: &[OsString]) -> io::Result<()> {
        trash::os_limited::purge_all(select(ids)?).map_err(io::Error::other)
    }
}

#[cfg(not(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
mod bin {
    use super::*;

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the Recycle Bin isn't supported on this platform yet",
        )
    }

    pub fn list_trash() -> io::Result<Vec<TrashedItem>> {
        Err(unsupported())
    }

    pub fn restore_trashed(_: &[std::ffi::OsString]) -> io::Result<()> {
        Err(unsupported())
    }

    pub fn purge_trashed(_: &[std::ffi::OsString]) -> io::Result<()> {
        Err(unsupported())
    }
}

pub use bin::{list_trash, purge_trashed, restore_trashed};

/// Files on the clipboard, and whether they were cut (move) or copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileClipboard {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

/// Puts files on the clipboard. On Windows this is the system clipboard in
/// Explorer's format, so pasting works in both directions.
pub fn set_file_clipboard(paths: &[PathBuf], cut: bool) -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::set_file_clipboard(paths, cut)
    }
    #[cfg(not(windows))]
    {
        *fallback_clipboard() = Some(FileClipboard {
            paths: paths.to_vec(),
            cut,
        });
        Ok(())
    }
}

/// The files currently on the clipboard, if any.
pub fn file_clipboard() -> Option<FileClipboard> {
    #[cfg(windows)]
    {
        windows::file_clipboard()
    }
    #[cfg(not(windows))]
    {
        fallback_clipboard().clone()
    }
}

/// Text on the system clipboard (for text fields; see `lumo-ui`'s input hook).
pub fn clipboard_text() -> Option<String> {
    #[cfg(windows)]
    {
        clipboard_win::get_clipboard_string()
            .ok()
            .map(|text| text.replace("\r\n", "\n"))
            .filter(|text| !text.is_empty())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Empties the file clipboard (after a cut has been pasted, like Explorer).
pub fn clear_file_clipboard() {
    #[cfg(windows)]
    windows::clear_clipboard();
    #[cfg(not(windows))]
    {
        *fallback_clipboard() = None;
    }
}

/// In-process clipboard until Linux desktop clipboard support lands (Phase 4).
#[cfg(not(windows))]
fn fallback_clipboard() -> std::sync::MutexGuard<'static, Option<FileClipboard>> {
    static CLIPBOARD: std::sync::Mutex<Option<FileClipboard>> = std::sync::Mutex::new(None);
    CLIPBOARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The user's system accent color as RGB, where the OS has one (Windows).
/// Not used yet: Lumo defaults to its own orange; a "follow system accent"
/// preference will use this.
pub fn accent_color() -> Option<[u8; 3]> {
    #[cfg(windows)]
    {
        windows::accent_color()
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
    fn trash_and_restore_round_trip() {
        // A folder under the user's home: temp dirs on another volume may
        // have no trash on Linux.
        let dir = tempfile::tempdir_in(home_dir()).unwrap();
        let file = dir
            .path()
            .join(format!("lumo-trash-test-{}.txt", std::process::id()));
        std::fs::write(&file, "keep me").unwrap();

        let recoverable = move_to_trash(std::slice::from_ref(&file)).unwrap();
        assert!(!file.exists());
        if !recoverable {
            // CI runners run as service accounts without a Recycle Bin:
            // Windows deletes permanently and we must report that, which
            // is what `recoverable == false` checks.
            eprintln!("no Recycle Bin for this account; restore not exercised");
            return;
        }

        let listed = list_trash()
            .unwrap()
            .into_iter()
            .find(|item| same_path(&item.original_parent.join(&item.name), &file))
            .expect("trashed file is listed");
        assert_eq!(listed.size, Some(7));
        assert!(!listed.is_dir);
        assert!(listed.deleted.is_some());

        restore_from_trash(std::slice::from_ref(&file)).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep me");
    }

    /// Touches the real system clipboard, so it doesn't run by default:
    /// `cargo test -p lumo-platform -- --ignored clipboard`
    #[test]
    #[ignore = "overwrites the system clipboard"]
    fn clipboard_round_trip_with_cut_flag() {
        let paths = vec![std::env::temp_dir()];
        set_file_clipboard(&paths, true).unwrap();
        assert_eq!(
            file_clipboard(),
            Some(FileClipboard {
                paths: paths.clone(),
                cut: true
            })
        );
        set_file_clipboard(&paths, false).unwrap();
        assert!(!file_clipboard().unwrap().cut);
        clear_file_clipboard();
        assert_eq!(file_clipboard(), None);
    }

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
