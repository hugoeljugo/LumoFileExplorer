use std::ffi::OsString;
use std::fs::{DirEntry, FileType};
use std::time::SystemTime;

use super::FsError;

/// What kind of filesystem object an [`Entry`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<FileType> for EntryKind {
    fn from(file_type: FileType) -> Self {
        if file_type.is_symlink() {
            Self::Symlink
        } else if file_type.is_dir() {
            Self::Directory
        } else if file_type.is_file() {
            Self::File
        } else {
            Self::Other
        }
    }
}

/// A single child of a directory. Symlinks describe the link itself, not its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// File name only (no parent path). Not guaranteed to be valid UTF-8.
    pub name: OsString,
    pub kind: EntryKind,
    /// Size in bytes; always 0 for directories so platforms agree.
    pub size: u64,
    /// Last modification time, if the platform reports one.
    pub modified: Option<SystemTime>,
}

impl Entry {
    pub(crate) fn from_dir_entry(dir_entry: &DirEntry) -> Result<Self, FsError> {
        let metadata = dir_entry
            .metadata()
            .map_err(|err| FsError::from_io(&dir_entry.path(), err))?;
        let kind = EntryKind::from(metadata.file_type());
        Ok(Self {
            name: dir_entry.file_name(),
            kind,
            size: if kind == EntryKind::Directory {
                0
            } else {
                metadata.len()
            },
            modified: metadata.modified().ok(),
        })
    }
}

/// Sorts directories first, then by case-insensitive name.
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|entry| {
        (
            entry.kind != EntryKind::Directory,
            entry.name.to_string_lossy().to_lowercase(),
        )
    });
}
