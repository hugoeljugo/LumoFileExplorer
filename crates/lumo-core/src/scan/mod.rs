//! Streaming directory traversal.
//!
//! [`scan`] reads a directory in fixed-size batches, so memory stays bounded by
//! the batch size no matter how many entries the directory holds. The optional
//! `tokio` feature adds [`scan_async`], which runs the same scanner on a
//! blocking worker and hands batches over a bounded channel.

#[cfg(feature = "tokio")]
mod stream;

use std::fs::{self, ReadDir};
use std::path::{Path, PathBuf};
use std::{mem, vec};

use rayon::prelude::*;

use crate::fs::{Entry, FsError};

#[cfg(feature = "tokio")]
pub use stream::{ScanStream, scan_async};

/// One scanned entry, or the error that prevented reading it.
pub type ScanItem = Result<Entry, FsError>;

/// Below this many entries, handing a batch to rayon costs more than it saves.
const PARALLEL_THRESHOLD: usize = 64;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Entries read per batch. Bounds the memory a scan holds at once.
    pub batch_size: usize,
    /// Fetch each batch's metadata on the rayon pool. On by default on Unix,
    /// where every entry costs a `stat` call; off on Windows, where directory
    /// enumeration already returns metadata, so there is nothing to parallelize.
    pub parallel_metadata: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            batch_size: 512,
            parallel_metadata: cfg!(unix),
        }
    }
}

/// Opens `path` for a streaming scan. Fails only if the directory itself
/// cannot be opened; per-entry failures are yielded as `Err` items.
pub fn scan(path: &Path, options: &ScanOptions) -> Result<Scan, FsError> {
    let read_dir = fs::read_dir(path).map_err(|err| classify_open_error(path, err))?;
    Ok(Scan {
        dir: path.to_path_buf(),
        read_dir: Some(read_dir),
        pending: Vec::new().into_iter(),
        batch_size: options.batch_size.max(1),
        parallel: options.parallel_metadata,
    })
}

/// An in-progress directory scan, in unsorted on-disk order. Iterate it for
/// single entries or call [`Scan::next_batch`] for whole batches.
#[derive(Debug)]
pub struct Scan {
    dir: PathBuf,
    /// `None` once the directory is exhausted, releasing the OS handle early.
    read_dir: Option<ReadDir>,
    pending: vec::IntoIter<ScanItem>,
    batch_size: usize,
    parallel: bool,
}

impl Scan {
    /// Returns up to `batch_size` items, or `None` once the scan is finished.
    pub fn next_batch(&mut self) -> Option<Vec<ScanItem>> {
        let pending: Vec<_> = mem::take(&mut self.pending).collect();
        let batch = if pending.is_empty() {
            self.read_batch()
        } else {
            pending
        };
        (!batch.is_empty()).then_some(batch)
    }

    /// Reads the next batch from disk; empty only when the directory is exhausted.
    fn read_batch(&mut self) -> Vec<ScanItem> {
        let Some(read_dir) = self.read_dir.as_mut() else {
            return Vec::new();
        };

        let mut batch = Vec::with_capacity(self.batch_size);
        // Raw entries held back for parallel metadata. The sequential path
        // converts each one immediately instead: OS directory entries are
        // large (~600 bytes on Windows), so buffering them costs time and memory.
        let mut deferred = Vec::new();
        let mut exhausted = false;
        while batch.len() + deferred.len() < self.batch_size {
            match read_dir.next() {
                Some(Ok(dir_entry)) if self.parallel => deferred.push(dir_entry),
                Some(Ok(dir_entry)) => batch.push(Entry::from_dir_entry(&dir_entry)),
                // Stop at an enumeration error: some platforms keep repeating it.
                Some(Err(err)) => {
                    batch.push(Err(FsError::from_io(&self.dir, err)));
                    exhausted = true;
                    break;
                }
                None => {
                    exhausted = true;
                    break;
                }
            }
        }
        if exhausted {
            self.read_dir = None;
        }

        if deferred.len() >= PARALLEL_THRESHOLD {
            batch.par_extend(deferred.par_iter().map(Entry::from_dir_entry));
        } else {
            batch.extend(deferred.iter().map(Entry::from_dir_entry));
        }
        batch
    }
}

impl Iterator for Scan {
    type Item = ScanItem;

    fn next(&mut self) -> Option<ScanItem> {
        if let Some(item) = self.pending.next() {
            return Some(item);
        }
        self.pending = self.read_batch().into_iter();
        self.pending.next()
    }
}

/// Some platforms report opening a file as a directory with a generic error
/// code, so fall back to checking what the path actually is.
fn classify_open_error(path: &Path, err: std::io::Error) -> FsError {
    match FsError::from_io(path, err) {
        FsError::Io { .. } if path.is_file() => FsError::NotADirectory(path.to_path_buf()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs::write;

    fn dir_with_files(count: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..count {
            write(dir.path().join(format!("f{i:04}")), i.to_string()).unwrap();
        }
        dir
    }

    fn options(batch_size: usize, parallel_metadata: bool) -> ScanOptions {
        ScanOptions {
            batch_size,
            parallel_metadata,
        }
    }

    #[test]
    fn next_batch_respects_batch_size() {
        let dir = dir_with_files(10);
        let mut scan = scan(dir.path(), &options(3, false)).unwrap();

        let sizes: Vec<_> = std::iter::from_fn(|| scan.next_batch())
            .map(|b| b.len())
            .collect();

        assert_eq!(sizes, [3, 3, 3, 1]);
        assert!(scan.next_batch().is_none());
    }

    #[test]
    fn next_batch_returns_leftovers_after_iterating() {
        let dir = dir_with_files(5);
        let mut scan = scan(dir.path(), &options(4, false)).unwrap();

        assert!(scan.next().is_some());
        assert_eq!(scan.next_batch().map(|b| b.len()), Some(3));
        assert_eq!(scan.next_batch().map(|b| b.len()), Some(1));
        assert!(scan.next_batch().is_none());
    }

    #[test]
    fn zero_batch_size_is_treated_as_one() {
        let dir = dir_with_files(2);
        let mut scan = scan(dir.path(), &options(0, false)).unwrap();

        assert_eq!(scan.next_batch().map(|b| b.len()), Some(1));
    }

    #[test]
    fn parallel_and_sequential_scans_agree() {
        let dir = dir_with_files(PARALLEL_THRESHOLD * 3);
        let collect = |parallel| {
            scan(dir.path(), &options(100, parallel))
                .unwrap()
                .map(|item| {
                    let entry = item.unwrap();
                    (entry.name, entry.size)
                })
                .collect::<BTreeSet<_>>()
        };

        let sequential = collect(false);

        assert_eq!(sequential.len(), PARALLEL_THRESHOLD * 3);
        assert_eq!(sequential, collect(true));
    }
}
