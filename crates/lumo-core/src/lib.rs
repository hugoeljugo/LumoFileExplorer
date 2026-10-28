//! Platform-agnostic core of Lumo: filesystem access, scanning and (later) indexing.

pub mod fs;
pub mod scan;

pub use fs::{
    Entry, EntryIter, EntryKind, FileSystemProvider, FsError, LocalFs, list_entries, natural_cmp,
    sort_entries,
};
pub use scan::{Scan, ScanItem, ScanOptions, scan};
#[cfg(feature = "tokio")]
pub use scan::{ScanStream, scan_async};
