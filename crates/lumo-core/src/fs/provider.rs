use std::path::Path;

use super::{Entry, FsError, sort_entries};
use crate::scan::{self, ScanItem, ScanOptions};

/// A boxed stream of directory entries, as returned by [`FileSystemProvider::scan`].
pub type EntryIter = Box<dyn Iterator<Item = ScanItem> + Send>;

/// A source of directory listings.
///
/// [`LocalFs`] covers local disks on every platform; remote and archive
/// backends (Phase 11) implement the same trait.
pub trait FileSystemProvider: Send + Sync {
    /// Opens `path` for streaming. Fails only if the directory itself cannot be
    /// opened; problems with individual entries are yielded as `Err` items.
    fn scan(&self, path: &Path, options: &ScanOptions) -> Result<EntryIter, FsError>;

    /// Collects and sorts (see [`sort_entries`]) every child of `path`,
    /// failing on the first entry error.
    fn list_entries(&self, path: &Path) -> Result<Vec<Entry>, FsError> {
        let mut entries = self
            .scan(path, &ScanOptions::default())?
            .collect::<Result<Vec<_>, _>>()?;
        sort_entries(&mut entries);
        Ok(entries)
    }
}

/// The local filesystem, via `std::fs`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalFs;

impl FileSystemProvider for LocalFs {
    fn scan(&self, path: &Path, options: &ScanOptions) -> Result<EntryIter, FsError> {
        Ok(Box::new(scan::scan(path, options)?))
    }
}
