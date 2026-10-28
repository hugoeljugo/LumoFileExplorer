//! File and folder abstractions.

mod error;

use std::ffi::OsString;
use std::fs::{self, FileType};
use std::path::Path;

pub use error::FsError;

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

/// A single child of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// File name only (no parent path). Not guaranteed to be valid UTF-8.
    pub name: OsString,
    pub kind: EntryKind,
}

/// Lists the immediate children of `path`: directories first, then by
/// case-insensitive name.
pub fn list_entries(path: &Path) -> Result<Vec<Entry>, FsError> {
    let read_dir = fs::read_dir(path).map_err(|err| classify_read_dir_error(path, err))?;

    let mut entries = read_dir
        .map(|dir_entry| {
            let dir_entry = dir_entry.map_err(|err| FsError::from_io(path, err))?;
            let file_type = dir_entry
                .file_type()
                .map_err(|err| FsError::from_io(&dir_entry.path(), err))?;
            Ok(Entry {
                name: dir_entry.file_name(),
                kind: file_type.into(),
            })
        })
        .collect::<Result<Vec<_>, FsError>>()?;

    entries.sort_by_cached_key(|entry| {
        (
            entry.kind != EntryKind::Directory,
            entry.name.to_string_lossy().to_lowercase(),
        )
    });
    Ok(entries)
}

/// Some platforms report reading a file as a directory with a generic error
/// code, so fall back to checking what the path actually is.
fn classify_read_dir_error(path: &Path, err: std::io::Error) -> FsError {
    match FsError::from_io(path, err) {
        FsError::Io { .. } if path.is_file() => FsError::NotADirectory(path.to_path_buf()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir};

    fn names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| e.name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn empty_directory_has_no_entries() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_entries(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn reports_files_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        create_dir(dir.path().join("sub")).unwrap();

        let entries = list_entries(dir.path()).unwrap();

        assert_eq!(
            entries,
            vec![
                Entry {
                    name: "sub".into(),
                    kind: EntryKind::Directory
                },
                Entry {
                    name: "a.txt".into(),
                    kind: EntryKind::File
                },
            ]
        );
    }

    #[test]
    fn sorts_directories_first_then_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        for file in ["b.txt", "A.txt", "c.txt"] {
            File::create(dir.path().join(file)).unwrap();
        }
        for sub in ["zeta", "Alpha"] {
            create_dir(dir.path().join(sub)).unwrap();
        }

        let entries = list_entries(dir.path()).unwrap();

        assert_eq!(
            names(&entries),
            ["Alpha", "zeta", "A.txt", "b.txt", "c.txt"]
        );
    }

    #[test]
    fn missing_path_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");

        let err = list_entries(&missing).unwrap_err();

        assert!(matches!(err, FsError::NotFound(p) if p == missing));
    }

    #[test]
    fn file_path_is_not_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        File::create(&file).unwrap();

        let err = list_entries(&file).unwrap_err();

        assert!(matches!(err, FsError::NotADirectory(p) if p == file));
    }
}
