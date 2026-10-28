//! Display strings for the file table.

use std::path::Path;
use std::time::SystemTime;

use lumo_core::{Entry, EntryKind};

/// Human-readable size with binary units: `512 B`, `1.5 KB`, `12 KB`, `3.4 GB`.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// Local date and time, e.g. `2026-10-28 18:41`; empty when unknown.
pub fn modified(time: Option<SystemTime>) -> String {
    time.map(|t| {
        chrono::DateTime::<chrono::Local>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
    .unwrap_or_default()
}

/// Explorer-style type column: `Folder`, `TOML File`, `File`.
pub fn type_label(entry: &Entry) -> String {
    match entry.kind {
        EntryKind::Directory => "Folder".to_owned(),
        EntryKind::Symlink => "Link".to_owned(),
        EntryKind::Other => "System file".to_owned(),
        EntryKind::File => match Path::new(&entry.name).extension() {
            Some(ext) => format!("{} File", ext.to_string_lossy().to_uppercase()),
            None => "File".to_owned(),
        },
    }
}

/// Size column text: empty for folders, like Explorer.
pub fn size_label(entry: &Entry) -> String {
    if entry.kind == EntryKind::Directory {
        String::new()
    } else {
        size(entry.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry {
            name: name.into(),
            kind,
            size: 0,
            modified: None,
            hidden: false,
        }
    }

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(12 * 1024), "12 KB");
        assert_eq!(size(3 * 1024 * 1024 * 1024 + 400 * 1024 * 1024), "3.4 GB");
    }

    #[test]
    fn type_labels() {
        assert_eq!(type_label(&entry("src", EntryKind::Directory)), "Folder");
        assert_eq!(
            type_label(&entry("Cargo.toml", EntryKind::File)),
            "TOML File"
        );
        assert_eq!(type_label(&entry("LICENSE", EntryKind::File)), "File");
        assert_eq!(type_label(&entry(".gitignore", EntryKind::File)), "File");
    }

    #[test]
    fn folders_have_no_size_label() {
        assert_eq!(size_label(&entry("src", EntryKind::Directory)), "");
        assert_eq!(size_label(&entry("a", EntryKind::File)), "0 B");
    }

    #[test]
    fn unknown_mtime_is_blank() {
        assert_eq!(modified(None), "");
        assert_eq!(
            modified(Some(SystemTime::now())).len(),
            "2026-10-28 18:41".len()
        );
    }
}
