//! The Recycle Bin as a location tabs can show.
//!
//! It isn't a folder, so it gets a special path: Windows' own shell id for the
//! Recycle Bin (Explorer's address bar accepts it too). Tabs, history and saved
//! sessions treat it like any other location; listing it goes through
//! `TrashBin` instead of reading a directory.

use std::path::{Path, PathBuf};

/// The Recycle Bin's shell namespace id.
const RECYCLE_BIN: &str = "::{645FF040-5081-101B-9F08-08002B30309D}";

pub const TITLE: &str = "Recycle Bin";

pub fn path() -> PathBuf {
    PathBuf::from(RECYCLE_BIN)
}

pub fn is_bin(path: &Path) -> bool {
    path.as_os_str().eq_ignore_ascii_case(RECYCLE_BIN)
}

/// Address-bar text that means the Recycle Bin: its name, Explorer's
/// `shell:RecycleBinFolder`, or the shell id itself.
pub fn parse_typed(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    [
        "recycle bin",
        "recyclebin",
        "trash",
        "shell:recyclebinfolder",
        RECYCLE_BIN,
    ]
    .iter()
    .any(|alias| text.eq_ignore_ascii_case(alias))
    .then(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_its_own_path_and_typed_aliases() {
        assert!(is_bin(&path()));
        assert!(!is_bin(Path::new("C:\\")));
        assert_eq!(parse_typed(" Recycle Bin "), Some(path()));
        assert_eq!(parse_typed("shell:RecycleBinFolder"), Some(path()));
        assert_eq!(parse_typed("recycle"), None);
    }
}
