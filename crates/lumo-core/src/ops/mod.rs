//! File operations: create, rename, copy and move, with Explorer-style
//! names for conflicts and progress reporting (and cancelling) for long
//! transfers. Recycle Bin and clipboard live in `lumo-platform`.

mod name;

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub use name::{NameError, copy_name, unique_name, validate_name};

use crate::fs::FsError;

/// Files at least this big are copied in chunks so progress and cancel
/// work mid-file; smaller ones use the OS copy (faster, keeps metadata).
const CHUNKED_COPY_THRESHOLD: u64 = 8 * 1024 * 1024;
const CHUNK: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error(transparent)]
    Name(#[from] NameError),
    #[error("\"{}\" already exists", .0.display())]
    AlreadyExists(PathBuf),
    #[error("can't move or copy a folder into itself: {}", .0.display())]
    IntoItself(PathBuf),
    #[error(transparent)]
    Fs(#[from] FsError),
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> OpError + '_ {
    move |err| OpError::Fs(FsError::from_io(path, err))
}

/// Creates a folder named `name` in `dir`, adding " (2)", " (3)", … if taken.
pub fn create_folder(dir: &Path, name: &str) -> Result<PathBuf, OpError> {
    validate_name(name)?;
    let path = dir.join(unique_name(dir, name));
    fs::create_dir(&path).map_err(io_err(&path))?;
    Ok(path)
}

/// Creates an empty file named `name` in `dir`, adding " (2)", … if taken.
pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, OpError> {
    validate_name(name)?;
    let path = dir.join(unique_name(dir, name));
    File::create_new(&path).map_err(io_err(&path))?;
    Ok(path)
}

/// Renames `path` within its folder. Changing only letter case is allowed.
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, OpError> {
    validate_name(new_name)?;
    let target = path.with_file_name(new_name);
    if target == path {
        return Ok(target);
    }
    let case_only = path
        .file_name()
        .is_some_and(|old| old.eq_ignore_ascii_case(new_name));
    if !case_only && fs::symlink_metadata(&target).is_ok() {
        return Err(OpError::AlreadyExists(target));
    }
    fs::rename(path, &target).map_err(io_err(path))?;
    Ok(target)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    Copy,
    Move,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub items_done: usize,
    pub items_total: usize,
}

/// What a transfer did, for refreshing views and for undo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferReport {
    /// `(source, destination)` for every top-level item that completed.
    pub done: Vec<(PathBuf, PathBuf)>,
    pub cancelled: bool,
}

/// Copies or moves `sources` into `dest_dir`.
///
/// Name conflicts never overwrite: a copy into the same folder becomes
/// "name - Copy", any other conflict becomes "name (2)". Moving an item into
/// the folder it is already in does nothing. `on_progress` is called
/// regularly; returning `false` cancels (a partly copied file is removed).
pub fn transfer(
    sources: &[PathBuf],
    dest_dir: &Path,
    mode: TransferMode,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<TransferReport, OpError> {
    for source in sources {
        if dest_dir.starts_with(source) && source.is_dir() {
            return Err(OpError::IntoItself(source.clone()));
        }
    }

    let mut progress = Progress {
        bytes_total: sources.iter().map(|s| tree_size(s)).sum(),
        items_total: sources.len(),
        ..Progress::default()
    };
    let mut report = TransferReport::default();
    for source in sources {
        if !on_progress(&progress) {
            report.cancelled = true;
            return Ok(report);
        }
        let same_folder = source.parent() == Some(dest_dir);
        if same_folder && mode == TransferMode::Move {
            progress.items_done += 1;
            continue;
        }
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest_name = if same_folder {
            copy_name(dest_dir, &name)
        } else {
            unique_name(dest_dir, &name)
        };
        let dest = dest_dir.join(dest_name);

        let finished = match mode {
            TransferMode::Copy => copy_tree(source, &dest, &mut progress, on_progress)?,
            TransferMode::Move => move_item(source, &dest, &mut progress, on_progress)?,
        };
        if !finished {
            report.cancelled = true;
            return Ok(report);
        }
        progress.items_done += 1;
        report.done.push((source.clone(), dest));
    }
    on_progress(&progress);
    Ok(report)
}

/// Total size in bytes of a file or folder tree (unreadable parts count as 0).
fn tree_size(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::read_dir(path)
            .map(|entries| entries.flatten().map(|e| tree_size(&e.path())).sum())
            .unwrap_or(0),
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

fn move_item(
    source: &Path,
    dest: &Path,
    progress: &mut Progress,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<bool, OpError> {
    match fs::rename(source, dest) {
        Ok(()) => {
            progress.bytes_done += tree_size(dest);
            Ok(true)
        }
        // Different drive: copy, then remove the original.
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            if !copy_tree(source, dest, progress, on_progress)? {
                return Ok(false);
            }
            let removed = if source.is_dir() {
                fs::remove_dir_all(source)
            } else {
                fs::remove_file(source)
            };
            removed.map_err(io_err(source))?;
            Ok(true)
        }
        Err(err) => Err(io_err(source)(err)),
    }
}

/// Copies a file or folder tree. Returns `false` if cancelled.
fn copy_tree(
    source: &Path,
    dest: &Path,
    progress: &mut Progress,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<bool, OpError> {
    let meta = fs::symlink_metadata(source).map_err(io_err(source))?;
    if meta.is_dir() {
        fs::create_dir(dest).map_err(io_err(dest))?;
        for entry in fs::read_dir(source).map_err(io_err(source))? {
            let entry = entry.map_err(io_err(source))?;
            if !copy_tree(
                &entry.path(),
                &dest.join(entry.file_name()),
                progress,
                on_progress,
            )? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if meta.len() < CHUNKED_COPY_THRESHOLD {
        fs::copy(source, dest).map_err(io_err(source))?;
        progress.bytes_done += meta.len();
        return Ok(on_progress(progress));
    }
    let finished = copy_chunked(source, dest, progress, on_progress)?;
    if !finished {
        // Don't leave a half-written file behind.
        let _ = fs::remove_file(dest);
    }
    Ok(finished)
}

fn copy_chunked(
    source: &Path,
    dest: &Path,
    progress: &mut Progress,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<bool, OpError> {
    let mut reader = File::open(source).map_err(io_err(source))?;
    let mut writer = File::create_new(dest).map_err(io_err(dest))?;
    let mut buffer = vec![0; CHUNK];
    loop {
        let read = reader.read(&mut buffer).map_err(io_err(source))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(io_err(dest))?;
        progress.bytes_done += read as u64;
        if !on_progress(progress) {
            return Ok(false);
        }
    }
    // Keep what the OS copy would have kept.
    let meta = reader.metadata().map_err(io_err(source))?;
    if let Ok(modified) = meta.modified() {
        let _ = writer.set_modified(modified);
    }
    let _ = fs::set_permissions(dest, meta.permissions());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir, read_to_string, write};

    fn no_cancel() -> impl FnMut(&Progress) -> bool {
        |_| true
    }

    #[test]
    fn creates_folders_and_files_with_unique_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = create_folder(dir.path(), "New folder").unwrap();
        let b = create_folder(dir.path(), "New folder").unwrap();
        let f = create_file(dir.path(), "notes.txt").unwrap();
        let g = create_file(dir.path(), "notes.txt").unwrap();

        assert_eq!(a.file_name().unwrap(), "New folder");
        assert_eq!(b.file_name().unwrap(), "New folder (2)");
        assert_eq!(f.file_name().unwrap(), "notes.txt");
        assert_eq!(g.file_name().unwrap(), "notes (2).txt");
    }

    #[test]
    fn rename_refuses_to_overwrite_but_allows_case_changes() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path().join("a.txt"), "a").unwrap();
        write(dir.path().join("b.txt"), "b").unwrap();

        let err = rename(&dir.path().join("a.txt"), "b.txt").unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));

        let renamed = rename(&dir.path().join("a.txt"), "A.txt").unwrap();
        assert_eq!(renamed.file_name().unwrap(), "A.txt");
        assert!(matches!(
            rename(&renamed, "bad/name"),
            Err(OpError::Name(_))
        ));
    }

    #[test]
    fn copy_into_same_folder_makes_a_copy_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.pdf");
        write(&file, "data").unwrap();

        let report = transfer(
            std::slice::from_ref(&file),
            dir.path(),
            TransferMode::Copy,
            &mut no_cancel(),
        )
        .unwrap();

        let copy = dir.path().join("report - Copy.pdf");
        assert_eq!(report.done, [(file, copy.clone())]);
        assert_eq!(read_to_string(copy).unwrap(), "data");
    }

    #[test]
    fn copies_folder_trees_and_keeps_both_on_conflict() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let tree = src.path().join("project");
        create_dir(&tree).unwrap();
        create_dir(tree.join("sub")).unwrap();
        write(tree.join("sub").join("main.rs"), "fn main() {}").unwrap();
        create_dir(dst.path().join("project")).unwrap();

        transfer(&[tree], dst.path(), TransferMode::Copy, &mut no_cancel()).unwrap();

        let copied = dst.path().join("project (2)").join("sub").join("main.rs");
        assert_eq!(read_to_string(copied).unwrap(), "fn main() {}");
    }

    #[test]
    fn move_relocates_and_same_folder_move_is_a_no_op() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let file = src.path().join("a.txt");
        write(&file, "a").unwrap();

        let same = transfer(
            std::slice::from_ref(&file),
            src.path(),
            TransferMode::Move,
            &mut no_cancel(),
        )
        .unwrap();
        assert!(same.done.is_empty() && file.exists());

        let moved = transfer(
            std::slice::from_ref(&file),
            dst.path(),
            TransferMode::Move,
            &mut no_cancel(),
        )
        .unwrap();
        assert_eq!(moved.done, [(file.clone(), dst.path().join("a.txt"))]);
        assert!(!file.exists());
    }

    #[test]
    fn refuses_to_copy_a_folder_into_itself() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("f");
        create_dir(&folder).unwrap();
        let err = transfer(
            std::slice::from_ref(&folder),
            &folder,
            TransferMode::Copy,
            &mut no_cancel(),
        );
        assert!(matches!(err, Err(OpError::IntoItself(_))));
    }

    #[test]
    fn reports_progress_and_can_cancel() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let files: Vec<_> = (0..3)
            .map(|i| {
                let f = src.path().join(format!("{i}.bin"));
                write(&f, vec![0u8; 100]).unwrap();
                f
            })
            .collect();

        let mut calls = 0;
        let report = transfer(
            &files,
            dst.path(),
            TransferMode::Copy,
            &mut |p: &Progress| {
                assert_eq!(p.bytes_total, 300);
                calls += 1;
                calls < 3
            },
        )
        .unwrap();

        assert!(report.cancelled);
        assert_eq!(report.done.len(), 1);
    }

    #[test]
    fn large_files_copy_in_chunks_with_contents_intact() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let big = src.path().join("big.bin");
        let data: Vec<u8> = (0..CHUNKED_COPY_THRESHOLD + 12_345)
            .map(|i| (i % 251) as u8)
            .collect();
        write(&big, &data).unwrap();

        let mut updates = 0;
        transfer(&[big], dst.path(), TransferMode::Copy, &mut |_| {
            updates += 1;
            true
        })
        .unwrap();

        assert_eq!(fs::read(dst.path().join("big.bin")).unwrap(), data);
        assert!(updates > 8, "chunked copy should report progress per chunk");
    }
}
