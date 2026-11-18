//! Background file jobs (copy, move, Recycle Bin, delete, restore) so long
//! operations never block the UI. Each job runs on its own thread and
//! reports progress over a channel; dropping or cancelling it stops it at
//! the next file or chunk.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use lumo_core::ops::{self, Progress, TransferMode, TransferReport};

use crate::files::TrashBin;

/// Progress updates are sent at most this often.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum JobOutcome {
    Transferred {
        mode: TransferMode,
        dest_dir: PathBuf,
        report: TransferReport,
    },
    Trashed {
        paths: Vec<PathBuf>,
        /// `false` if the system deleted them permanently instead.
        recoverable: bool,
    },
    Restored {
        paths: Vec<PathBuf>,
    },
    Deleted {
        paths: Vec<PathBuf>,
    },
    /// Items removed from the Recycle Bin for good.
    Purged {
        count: usize,
    },
    /// Undo of a move: each `(current, original)` pair was moved back.
    MovedBack {
        pairs: Vec<(PathBuf, PathBuf)>,
    },
}

enum Msg {
    Progress(Progress),
    Finished(Result<JobOutcome, String>),
}

pub struct Job {
    /// Shown in the status bar, e.g. "Copying 3 items".
    pub label: String,
    /// True when this job undoes an earlier operation (it isn't undoable itself).
    pub is_undo: bool,
    /// Whether a finished cut-paste should clear the clipboard.
    pub from_cut: bool,
    pub progress: Progress,
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
}

type Work = Box<dyn FnOnce(&mut dyn FnMut(&Progress) -> bool) -> Result<JobOutcome, String> + Send>;

impl Job {
    fn spawn(label: String, repaint: impl Fn() + Send + 'static, work: Work) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_tx = tx.clone();
        let spawned = thread::Builder::new()
            .name("lumo-job".into())
            .spawn(move || {
                let mut last_sent = Instant::now() - PROGRESS_INTERVAL;
                let mut report = |progress: &Progress| {
                    if last_sent.elapsed() >= PROGRESS_INTERVAL {
                        last_sent = Instant::now();
                        // A send error only means the UI dropped the job.
                        let _ = worker_tx.send(Msg::Progress(progress.clone()));
                        repaint();
                    }
                    !worker_cancel.load(Ordering::Relaxed)
                };
                let result = work(&mut report);
                let _ = worker_tx.send(Msg::Finished(result));
                repaint();
            });
        if let Err(err) = spawned {
            let _ = tx.send(Msg::Finished(Err(format!("couldn't start: {err}"))));
        }
        Self {
            label,
            is_undo: false,
            from_cut: false,
            progress: Progress::default(),
            rx,
            cancel,
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Folds in progress updates; returns the outcome once the job is done.
    pub fn poll(&mut self) -> Option<Result<JobOutcome, String>> {
        loop {
            match self.rx.try_recv() {
                Ok(Msg::Progress(progress)) => self.progress = progress,
                Ok(Msg::Finished(result)) => return Some(result),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(Err("the job stopped unexpectedly".to_owned()));
                }
            }
        }
    }

    /// 0.0–1.0, or `None` before the total size is known.
    pub fn fraction(&self) -> Option<f32> {
        (self.progress.bytes_total > 0)
            .then(|| self.progress.bytes_done as f32 / self.progress.bytes_total as f32)
    }
}

fn items(n: usize) -> String {
    if n == 1 {
        "1 item".to_owned()
    } else {
        format!("{n} items")
    }
}

pub fn transfer(
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
    mode: TransferMode,
    repaint: impl Fn() + Send + 'static,
) -> Job {
    let verb = match mode {
        TransferMode::Copy => "Copying",
        TransferMode::Move => "Moving",
    };
    let label = format!("{verb} {}", items(sources.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |progress| {
            let report =
                ops::transfer(&sources, &dest_dir, mode, progress).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Transferred {
                mode,
                dest_dir,
                report,
            })
        }),
    )
}

pub fn trash(paths: Vec<PathBuf>, bin: TrashBin, repaint: impl Fn() + Send + 'static) -> Job {
    let label = format!("Moving {} to the Recycle Bin", items(paths.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |_| {
            let recoverable = bin.trash(&paths).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Trashed { paths, recoverable })
        }),
    )
}

pub fn restore(paths: Vec<PathBuf>, bin: TrashBin, repaint: impl Fn() + Send + 'static) -> Job {
    let label = format!("Restoring {}", items(paths.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |_| {
            bin.restore(&paths).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Restored { paths })
        }),
    )
}

/// Restores Recycle Bin items (by id) to where they came from.
pub fn restore_items(
    ids: Vec<std::ffi::OsString>,
    bin: TrashBin,
    repaint: impl Fn() + Send + 'static,
) -> Job {
    let label = format!("Restoring {}", items(ids.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |_| {
            let paths = bin.restore_ids(&ids).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Restored { paths })
        }),
    )
}

pub fn purge(
    ids: Vec<std::ffi::OsString>,
    bin: TrashBin,
    repaint: impl Fn() + Send + 'static,
) -> Job {
    let label = format!("Deleting {} permanently", items(ids.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |_| {
            let count = ids.len();
            bin.purge_ids(&ids).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Purged { count })
        }),
    )
}

pub fn empty_bin(bin: TrashBin, repaint: impl Fn() + Send + 'static) -> Job {
    Job::spawn(
        "Emptying the Recycle Bin".to_owned(),
        repaint,
        Box::new(move |_| {
            let ids: Vec<_> = bin
                .list()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|i| i.id)
                .collect();
            bin.purge_ids(&ids).map_err(|e| e.to_string())?;
            Ok(JobOutcome::Purged { count: ids.len() })
        }),
    )
}

pub fn delete(paths: Vec<PathBuf>, repaint: impl Fn() + Send + 'static) -> Job {
    let label = format!("Deleting {}", items(paths.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |progress| {
            for path in &paths {
                if !progress(&Progress::default()) {
                    break;
                }
                let meta = std::fs::symlink_metadata(path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let removed = if meta.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                removed.map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok(JobOutcome::Deleted { paths })
        }),
    )
}

/// Undo of a move: puts each `current` back at `original`.
pub fn move_back(pairs: Vec<(PathBuf, PathBuf)>, repaint: impl Fn() + Send + 'static) -> Job {
    let label = format!("Moving {} back", items(pairs.len()));
    Job::spawn(
        label,
        repaint,
        Box::new(move |progress| {
            for (current, original) in &pairs {
                if let Err(err) = std::fs::rename(current, original) {
                    if err.kind() != std::io::ErrorKind::CrossesDevices {
                        return Err(format!("{}: {err}", current.display()));
                    }
                    // Different drive: copy back, then fix the name.
                    let parent = original.parent().map(PathBuf::from).unwrap_or_default();
                    let report = ops::transfer(
                        std::slice::from_ref(current),
                        &parent,
                        TransferMode::Move,
                        progress,
                    )
                    .map_err(|e| e.to_string())?;
                    if let Some((_, landed)) = report.done.first()
                        && landed != original
                    {
                        std::fs::rename(landed, original).map_err(|e| e.to_string())?;
                    }
                }
            }
            Ok(JobOutcome::MovedBack { pairs })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish(mut job: Job) -> Result<JobOutcome, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = job.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "job timed out");
            thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn copy_job_reports_what_it_did() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let file = src.path().join("a.txt");
        std::fs::write(&file, "a").unwrap();

        let outcome = finish(transfer(
            vec![file.clone()],
            dst.path().to_path_buf(),
            TransferMode::Copy,
            || {},
        ));

        let Ok(JobOutcome::Transferred { report, .. }) = outcome else {
            panic!("unexpected {outcome:?}");
        };
        assert_eq!(report.done, [(file, dst.path().join("a.txt"))]);
    }

    #[test]
    fn move_back_restores_original_names() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let original = a.path().join("x.txt");
        let moved = b.path().join("x (2).txt");
        std::fs::write(&moved, "x").unwrap();

        finish(move_back(vec![(moved.clone(), original.clone())], || {})).unwrap();

        assert!(original.exists() && !moved.exists());
    }

    #[test]
    fn delete_job_removes_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        let folder = dir.path().join("d");
        std::fs::write(&file, "").unwrap();
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("inner"), "").unwrap();

        finish(delete(vec![file.clone(), folder.clone()], || {})).unwrap();

        assert!(!file.exists() && !folder.exists());
    }

    #[test]
    fn failures_come_back_as_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(finish(delete(vec![missing], || {})).is_err());
    }
}
