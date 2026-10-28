use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fs::{DirEntry, FileType, Metadata};
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
    /// Hidden by platform convention: the hidden attribute on Windows, a
    /// leading dot elsewhere.
    pub hidden: bool,
}

impl Entry {
    pub(crate) fn from_dir_entry(dir_entry: &DirEntry) -> Result<Self, FsError> {
        let metadata = dir_entry
            .metadata()
            .map_err(|err| FsError::from_io(&dir_entry.path(), err))?;
        let kind = EntryKind::from(metadata.file_type());
        let name = dir_entry.file_name();
        Ok(Self {
            hidden: is_hidden(&name, &metadata),
            name,
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

#[cfg(windows)]
fn is_hidden(_name: &OsStr, metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &OsStr, _metadata: &Metadata) -> bool {
    name.as_encoded_bytes().starts_with(b".")
}

/// Sorts directories first, then by name in [natural order](natural_cmp).
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        (a.kind != EntryKind::Directory)
            .cmp(&(b.kind != EntryKind::Directory))
            .then_with(|| natural_cmp(&a.name.to_string_lossy(), &b.name.to_string_lossy()))
    });
}

/// Compares names the way Explorer does: case-insensitively, with runs of
/// digits compared by numeric value (`file2` < `file10`). Names that differ
/// only in case or leading zeros fall back to plain ordering, so the result is
/// total and deterministic.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural_cmp_loose(a, b).then_with(|| a.cmp(b))
}

fn natural_cmp_loose(mut a: &str, mut b: &str) -> Ordering {
    loop {
        let (Some(ca), Some(cb)) = (a.chars().next(), b.chars().next()) else {
            return a.is_empty().cmp(&b.is_empty()).reverse();
        };
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let (digits_a, rest_a) = split_digits(a);
            let (digits_b, rest_b) = split_digits(b);
            let (num_a, num_b) = (
                digits_a.trim_start_matches('0'),
                digits_b.trim_start_matches('0'),
            );
            // Arbitrarily long digit runs: compare by length, then lexically.
            let ord = num_a.len().cmp(&num_b.len()).then_with(|| num_a.cmp(num_b));
            if ord != Ordering::Equal {
                return ord;
            }
            (a, b) = (rest_a, rest_b);
        } else {
            let ord = ca.to_lowercase().cmp(cb.to_lowercase());
            if ord != Ordering::Equal {
                return ord;
            }
            (a, b) = (&a[ca.len_utf8()..], &b[cb.len_utf8()..]);
        }
    }
}

fn split_digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_cmp_orders_numbers_by_value() {
        let mut names = ["file10", "file2", "File1", "file", "file2a", "a"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["a", "file", "File1", "file2", "file2a", "file10"]);
    }

    #[test]
    fn natural_cmp_is_case_insensitive_with_deterministic_ties() {
        assert_eq!(natural_cmp("abc", "ABD"), Ordering::Less);
        assert_eq!(natural_cmp("x01", "x1"), "x01".cmp("x1"));
        assert_eq!(natural_cmp("Same", "Same"), Ordering::Equal);
    }

    #[test]
    fn natural_cmp_handles_huge_digit_runs() {
        assert_eq!(
            natural_cmp("v99999999999999999999999", "v100000000000000000000000"),
            Ordering::Less
        );
    }
}
