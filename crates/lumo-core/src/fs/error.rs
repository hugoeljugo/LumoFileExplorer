use std::io;
use std::path::{Path, PathBuf};

/// Errors produced by filesystem operations, always carrying the offending path.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("path not found: {}", .0.display())]
    NotFound(PathBuf),

    #[error("permission denied: {}", .0.display())]
    PermissionDenied(PathBuf),

    #[error("not a directory: {}", .0.display())]
    NotADirectory(PathBuf),

    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl FsError {
    /// Maps a raw I/O error on `path` to the most specific variant.
    pub(crate) fn from_io(path: &Path, source: io::Error) -> Self {
        let path = path.to_path_buf();
        match source.kind() {
            io::ErrorKind::NotFound => Self::NotFound(path),
            io::ErrorKind::PermissionDenied => Self::PermissionDenied(path),
            io::ErrorKind::NotADirectory => Self::NotADirectory(path),
            _ => Self::Io { path, source },
        }
    }
}
