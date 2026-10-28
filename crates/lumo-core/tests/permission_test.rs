//! Phase 1 gate: permission-denied paths fail with a typed error, not a panic,
//! and don't stop the parent directory from listing.

// The Denied guard is test-only setup; clippy.toml's exemption only covers #[test] fns.
#![allow(clippy::unwrap_used)]

use std::fs::create_dir;
use std::path::Path;

use lumo_core::{EntryKind, FsError, list_entries};

/// Denies listing `dir` to the current user until dropped.
struct Denied<'a>(&'a Path);

#[cfg(windows)]
impl<'a> Denied<'a> {
    /// Returns `None` if this environment can't revoke access (skip the test).
    fn new(dir: &'a Path) -> Option<Self> {
        // Deny "list folder" to Everyone (S-1-1-0); deny ACEs override allows.
        let status = std::process::Command::new("icacls")
            .arg(dir)
            .args(["/deny", "*S-1-1-0:(RD)"])
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "icacls /deny failed");
        Some(Self(dir))
    }
}

#[cfg(windows)]
impl Drop for Denied<'_> {
    fn drop(&mut self) {
        // Owners keep WRITE_DAC, so this works and lets the temp dir be deleted.
        let _ = std::process::Command::new("icacls")
            .arg(self.0)
            .args(["/remove:d", "*S-1-1-0"])
            .output();
    }
}

#[cfg(unix)]
impl<'a> Denied<'a> {
    fn new(dir: &'a Path) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let guard = Self(dir);
        // root ignores permission bits.
        std::fs::read_dir(dir).is_err().then_some(guard)
    }
}

#[cfg(unix)]
impl Drop for Denied<'_> {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(self.0, std::fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn denied_directory_reports_permission_denied() {
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    create_dir(&locked).unwrap();
    let Some(_guard) = Denied::new(&locked) else {
        eprintln!("skipped: cannot revoke access in this environment");
        return;
    };

    let err = list_entries(&locked).unwrap_err();

    assert!(
        matches!(&err, FsError::PermissionDenied(p) if *p == locked),
        "got {err:?}"
    );
}

#[test]
fn parent_still_lists_denied_child() {
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    create_dir(&locked).unwrap();
    let Some(_guard) = Denied::new(&locked) else {
        eprintln!("skipped: cannot revoke access in this environment");
        return;
    };

    let entries = list_entries(dir.path()).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "locked");
    assert_eq!(entries[0].kind, EntryKind::Directory);
}
