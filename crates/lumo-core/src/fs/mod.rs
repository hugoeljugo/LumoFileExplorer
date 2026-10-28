//! File and folder abstractions.

mod entry;
mod error;
mod provider;

use std::path::Path;

pub use entry::{Entry, EntryKind, natural_cmp, sort_entries};
pub use error::FsError;
pub use provider::{EntryIter, FileSystemProvider, LocalFs};

/// Lists the immediate children of `path` on the local disk: directories
/// first, then by name in natural order.
pub fn list_entries(path: &Path) -> Result<Vec<Entry>, FsError> {
    LocalFs.list_entries(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir, write};

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
    fn reports_kind_size_and_mtime() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path().join("a.txt"), b"hello").unwrap();
        create_dir(dir.path().join("sub")).unwrap();

        let entries = list_entries(dir.path()).unwrap();

        assert_eq!(names(&entries), ["sub", "a.txt"]);
        let (sub, file) = (&entries[0], &entries[1]);
        assert_eq!((sub.kind, sub.size), (EntryKind::Directory, 0));
        assert_eq!((file.kind, file.size), (EntryKind::File, 5));
        assert!(file.modified.is_some());
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

    #[test]
    fn reports_hidden_entries() {
        let dir = tempfile::tempdir().unwrap();
        File::create(dir.path().join("visible.txt")).unwrap();
        let hidden = if cfg!(windows) {
            "secret.txt"
        } else {
            ".secret"
        };
        File::create(dir.path().join(hidden)).unwrap();
        #[cfg(windows)]
        assert!(
            std::process::Command::new("attrib")
                .arg("+h")
                .arg(dir.path().join(hidden))
                .status()
                .unwrap()
                .success()
        );

        let entries = list_entries(dir.path()).unwrap();
        let hidden_names: Vec<_> = entries
            .iter()
            .filter(|e| e.hidden)
            .map(|e| &e.name)
            .collect();

        assert_eq!(hidden_names, [hidden]);
    }
}
