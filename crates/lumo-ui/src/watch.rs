//! Watches the folders open in tabs, so changes made outside Lumo (Explorer
//! pasting a cut, a download finishing, a build writing files) show up
//! without pressing F5.
//!
//! Events are only recorded here; the app refreshes a folder once its
//! events have been quiet for a moment, so a burst of changes (copying a
//! thousand files) causes one reload, not a thousand.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// How long a folder must be quiet before it is refreshed.
pub const SETTLE: Duration = Duration::from_millis(150);

type Changes = Arc<Mutex<HashMap<PathBuf, Instant>>>;

pub struct FolderWatcher {
    /// `None` if the OS refused a watcher; Lumo then works without live updates.
    watcher: Option<RecommendedWatcher>,
    watched: HashSet<PathBuf>,
    changes: Changes,
}

impl FolderWatcher {
    pub fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let changes: Changes = Arc::default();
        let sink = Arc::clone(&changes);
        let watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                return;
            };
            // Reading a file is not a change to the listing.
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let now = Instant::now();
            let mut changes = sink.lock().unwrap_or_else(PoisonError::into_inner);
            for path in &event.paths {
                // A child changed (its folder needs a reload), or the
                // watched folder itself was renamed or removed.
                if let Some(parent) = path.parent() {
                    changes.insert(parent.to_path_buf(), now);
                }
                changes.insert(path.clone(), now);
            }
            drop(changes);
            repaint();
        })
        .ok();
        Self {
            watcher,
            watched: HashSet::new(),
            changes,
        }
    }

    /// Watches exactly `folders` (the folders open in tabs).
    pub fn sync(&mut self, folders: HashSet<PathBuf>) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        for gone in self.watched.difference(&folders) {
            let _ = watcher.unwatch(gone);
        }
        for new in folders.difference(&self.watched) {
            // A folder that can't be watched (permissions, some network
            // shares) still works; it just won't update live.
            let _ = watcher.watch(new, RecursiveMode::NonRecursive);
        }
        self.watched = folders;
    }

    /// Watched folders whose changes have settled, and whether others are
    /// still settling (the caller should check again shortly).
    pub fn take_settled(&self) -> (Vec<PathBuf>, bool) {
        let mut changes = self.changes.lock().unwrap_or_else(PoisonError::into_inner);
        // Drop events for folders we don't show (children's own paths etc.).
        changes.retain(|path, _| self.watched.contains(path));
        let mut settled = Vec::new();
        changes.retain(|path, at| {
            let done = at.elapsed() >= SETTLE;
            if done {
                settled.push(path.clone());
            }
            !done
        });
        (settled, !changes.is_empty())
    }

    #[cfg(test)]
    pub fn is_watching(&self, folder: &std::path::Path) -> bool {
        self.watcher.is_some() && self.watched.contains(folder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn wait_for_settled(watcher: &FolderWatcher) -> Vec<PathBuf> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (settled, _) = watcher.take_settled();
            if !settled.is_empty() {
                return settled;
            }
            assert!(Instant::now() < deadline, "no change reported");
            sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn reports_changes_in_watched_folders_once_settled() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().canonicalize().unwrap();
        let mut watcher = FolderWatcher::new(|| {});
        watcher.sync(HashSet::from([folder.clone()]));
        assert!(watcher.is_watching(&folder));

        std::fs::write(folder.join("new.txt"), "x").unwrap();

        assert_eq!(wait_for_settled(&watcher), [folder]);
    }

    #[test]
    fn unwatched_folders_are_ignored() {
        let watched = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let mut watcher = FolderWatcher::new(|| {});
        watcher.sync(HashSet::from([watched.path().canonicalize().unwrap()]));

        std::fs::write(other.path().join("x.txt"), "x").unwrap();
        sleep(SETTLE * 3);

        assert_eq!(watcher.take_settled(), (Vec::new(), false));
    }
}
