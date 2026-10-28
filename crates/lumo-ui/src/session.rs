//! Open tabs, saved on every change so they survive a crash or force-close.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub tabs: Vec<PathBuf>,
    pub active: usize,
}

impl Session {
    /// Reads a saved session. A missing or corrupt file yields `None`, never an
    /// error: losing tabs must not stop the app from starting.
    pub fn load(file: &Path) -> Option<Self> {
        let text = fs::read_to_string(file).ok()?;
        let session: Self = serde_json::from_str(&text).ok()?;
        (!session.tabs.is_empty()).then_some(session)
    }

    /// Writes atomically (temp file + rename), so a crash mid-write leaves the
    /// previous session intact instead of a truncated file.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let dir = file.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut tmp, self)?;
        tmp.as_file_mut().sync_all()?;
        tmp.persist(file).map_err(|err| err.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            tabs: vec![PathBuf::from("C:\\a"), PathBuf::from("/b")],
            active: 1,
        }
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested").join("session.json");

        session().save(&file).unwrap();

        assert_eq!(Session::load(&file), Some(session()));
    }

    #[test]
    fn overwrites_previous_session() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.json");
        session().save(&file).unwrap();
        let newer = Session {
            tabs: vec![PathBuf::from("/c")],
            active: 0,
        };

        newer.save(&file).unwrap();

        assert_eq!(Session::load(&file), Some(newer));
    }

    #[test]
    fn missing_corrupt_or_empty_files_load_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.json");
        assert_eq!(Session::load(&file), None);

        fs::write(&file, "{ not json").unwrap();
        assert_eq!(Session::load(&file), None);

        fs::write(&file, r#"{"tabs": [], "active": 0}"#).unwrap();
        assert_eq!(Session::load(&file), None);
    }
}
