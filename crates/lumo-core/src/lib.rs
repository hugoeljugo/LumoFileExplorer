//! Platform-agnostic core of Lumo: filesystem access, scanning and (later) indexing.

pub mod fs;

pub use fs::{Entry, EntryKind, FsError, list_entries};
