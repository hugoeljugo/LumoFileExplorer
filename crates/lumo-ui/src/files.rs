//! File actions on the active tab: clipboard, Recycle Bin, rename, create,
//! undo and properties, plus the toasts that report them. Anything that can
//! take a while runs as a background [`Job`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use lumo_core::ops::{self, TransferMode};
use lumo_platform::FileClipboard;

use crate::app::LumoApp;
use crate::format;
use crate::jobs::{self, Job, JobOutcome};

/// How long a toast stays up.
const TOAST_SECS: f64 = 6.0;
/// Opening more files than this at once is almost certainly a mistake.
const MAX_OPEN_AT_ONCE: usize = 20;

/// One undoable step. Undo moves things to the Recycle Bin rather than
/// deleting them, so undoing is itself recoverable.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UndoEntry {
    Renamed {
        from: PathBuf,
        to: PathBuf,
    },
    Created {
        path: PathBuf,
    },
    Copied {
        created: Vec<PathBuf>,
    },
    /// `(original, new)` locations.
    Moved {
        pairs: Vec<(PathBuf, PathBuf)>,
    },
    Trashed {
        paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Toast {
    pub message: String,
    /// Shows an Undo button (undoes the latest entry).
    pub undo: bool,
    pub error: bool,
    pub until: f64,
}

/// In-place rename of one row.
#[derive(Debug, Clone)]
pub(crate) struct Renaming {
    pub tab: u64,
    pub path: PathBuf,
    pub text: String,
    /// Select the name without its extension on the first frame, like Explorer.
    pub select_stem: bool,
}

/// Where cut/copied files are kept.
pub(crate) enum Clipboard {
    /// The OS clipboard (interoperates with Explorer).
    System,
    /// Private to this window (tests; nothing touches the user's clipboard).
    Private(Option<FileClipboard>),
}

impl Clipboard {
    fn set(&mut self, paths: &[PathBuf], cut: bool) -> std::io::Result<()> {
        match self {
            Self::System => lumo_platform::set_file_clipboard(paths, cut),
            Self::Private(slot) => {
                *slot = Some(FileClipboard {
                    paths: paths.to_vec(),
                    cut,
                });
                Ok(())
            }
        }
    }

    pub(crate) fn get(&self) -> Option<FileClipboard> {
        match self {
            Self::System => lumo_platform::file_clipboard(),
            Self::Private(slot) => slot.clone(),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::System => lumo_platform::clear_file_clipboard(),
            Self::Private(slot) => *slot = None,
        }
    }
}

/// Where deleted items go.
#[derive(Clone)]
pub(crate) enum TrashBin {
    /// The OS Recycle Bin / trash.
    System,
    /// A private folder (tests); remembers where each item came from.
    Folder {
        dir: PathBuf,
        origins: Arc<Mutex<HashMap<PathBuf, PathBuf>>>,
    },
}

impl TrashBin {
    pub(crate) fn private() -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("lumo-trash-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self::Folder {
            dir,
            origins: Arc::default(),
        })
    }

    /// Returns whether the items can be restored (see `lumo_platform::move_to_trash`).
    pub(crate) fn trash(&self, paths: &[PathBuf]) -> std::io::Result<bool> {
        match self {
            Self::System => lumo_platform::move_to_trash(paths),
            Self::Folder { dir, origins } => {
                let mut origins = origins.lock().unwrap_or_else(PoisonError::into_inner);
                for path in paths {
                    let slot = dir.join(format!("{}-{}", origins.len(), file_name(path)));
                    std::fs::rename(path, &slot)?;
                    origins.insert(path.clone(), slot);
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn restore(&self, paths: &[PathBuf]) -> std::io::Result<()> {
        match self {
            Self::System => lumo_platform::restore_from_trash(paths),
            Self::Folder { origins, .. } => {
                let mut origins = origins.lock().unwrap_or_else(PoisonError::into_inner);
                for path in paths {
                    let slot = origins.remove(path).ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, "not in the trash")
                    })?;
                    std::fs::rename(slot, path)?;
                }
                Ok(())
            }
        }
    }
}

/// Details shown in the Properties panel.
#[derive(Debug, Clone)]
pub(crate) struct Properties {
    pub name: String,
    pub location: String,
    pub kind: String,
    pub size: String,
    pub created: String,
    pub modified: String,
    pub accessed: String,
    pub hidden: bool,
    pub read_only: bool,
}

fn items(n: usize) -> String {
    if n == 1 {
        "1 item".to_owned()
    } else {
        format!("{n} items")
    }
}

fn parent_of(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl LumoApp {
    fn repaint_fn(&self) -> impl Fn() + Send + 'static {
        let ctx = self.ctx.clone();
        move || ctx.request_repaint()
    }

    fn now(&self) -> f64 {
        self.ctx.input(|i| i.time)
    }

    pub(crate) fn toast(&mut self, message: impl Into<String>, undo: bool) {
        self.toast = Some(Toast {
            message: message.into(),
            undo,
            error: false,
            until: self.now() + TOAST_SECS,
        });
        self.ctx.request_repaint_after_secs(TOAST_SECS as f32);
    }

    pub(crate) fn toast_error(&mut self, message: impl Into<String>) {
        self.toast(message, false);
        if let Some(toast) = &mut self.toast {
            toast.error = true;
        }
    }

    pub(crate) fn expire_toast(&mut self) {
        let now = self.now();
        if self.toast.as_ref().is_some_and(|t| now >= t.until) {
            self.toast = None;
        }
    }

    /// Ctrl+C / Ctrl+X.
    pub(crate) fn copy_selection(&mut self, cut: bool) {
        let paths = self.active_tab().selected_paths();
        if paths.is_empty() {
            return;
        }
        match self.clipboard.set(&paths, cut) {
            Ok(()) => {
                self.cut_paths = if cut {
                    paths.iter().cloned().collect()
                } else {
                    Default::default()
                };
                let verb = if cut { "Cut" } else { "Copied" };
                self.toast(format!("{verb} {}", items(paths.len())), false);
            }
            Err(err) => self.toast_error(format!("Couldn't use the clipboard: {err}")),
        }
    }

    /// Ctrl+V into the current folder.
    pub(crate) fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.get() else {
            self.toast("Nothing to paste", false);
            return;
        };
        let dest = self.active_tab().path().to_path_buf();
        let mode = if clipboard.cut {
            TransferMode::Move
        } else {
            TransferMode::Copy
        };
        let mut job = jobs::transfer(clipboard.paths, dest, mode, self.repaint_fn());
        job.from_cut = clipboard.cut;
        self.jobs.push(job);
    }

    /// Delete: to the Recycle Bin, undoable.
    pub(crate) fn trash_selection(&mut self) {
        let paths = self.active_tab().selected_paths();
        if !paths.is_empty() {
            let job = jobs::trash(paths, self.trash_bin.clone(), self.repaint_fn());
            self.jobs.push(job);
        }
    }

    /// Shift+Delete: asks first (the only operation that does).
    pub(crate) fn ask_delete_permanently(&mut self) {
        let paths = self.active_tab().selected_paths();
        if !paths.is_empty() {
            self.confirm_delete = Some(paths);
        }
    }

    pub(crate) fn delete_permanently(&mut self) {
        if let Some(paths) = self.confirm_delete.take() {
            let job = jobs::delete(paths, self.repaint_fn());
            self.jobs.push(job);
        }
    }

    /// F2: rename the focused item in place.
    pub(crate) fn start_rename(&mut self) {
        let tab = self.active_tab();
        let Some(row) = tab.selected_row() else {
            return;
        };
        let path = tab.path().join(&row.entry.name);
        self.renaming = Some(Renaming {
            tab: tab.id(),
            text: row.name.clone(),
            path,
            select_stem: true,
        });
    }

    pub(crate) fn commit_rename(&mut self) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let new_name = renaming.text.trim().to_owned();
        if new_name == file_name(&renaming.path) || new_name.is_empty() {
            return;
        }
        match ops::rename(&renaming.path, &new_name) {
            Ok(to) => {
                self.undo.push(UndoEntry::Renamed {
                    from: renaming.path.clone(),
                    to: to.clone(),
                });
                self.refresh_folders(&[parent_of(&to)], Some(vec![to.clone()]));
                self.toast(format!("Renamed to {new_name}"), true);
            }
            Err(err) => {
                self.toast_error(format!("Couldn't rename: {err}"));
                // Let the user fix the name instead of losing what they typed.
                self.renaming = Some(Renaming {
                    text: new_name,
                    select_stem: false,
                    ..renaming
                });
            }
        }
    }

    /// New folder / new text file, then rename it in place.
    pub(crate) fn create(&mut self, folder: bool) {
        let dir = self.active_tab().path().to_path_buf();
        let created = if folder {
            ops::create_folder(&dir, "New folder")
        } else {
            ops::create_file(&dir, "New text document.txt")
        };
        match created {
            Ok(path) => {
                self.undo.push(UndoEntry::Created { path: path.clone() });
                self.refresh_folders(&[dir], Some(vec![path.clone()]));
                self.rename_when_loaded = Some(path);
            }
            Err(err) => self.toast_error(format!("Couldn't create it: {err}")),
        }
    }

    /// Starts the in-place rename of a just-created item once it is listed.
    pub(crate) fn start_pending_rename(&mut self) {
        let Some(path) = &self.rename_when_loaded else {
            return;
        };
        let tab = self.active_tab();
        if tab.path() != parent_of(path) {
            self.rename_when_loaded = None;
            return;
        }
        let name = file_name(path);
        if tab.selected_row().is_some_and(|row| row.name == name) {
            self.rename_when_loaded = None;
            self.start_rename();
        } else if !tab.is_loading() && tab.selected().is_none() {
            self.rename_when_loaded = None;
        }
    }

    /// Ctrl+Z.
    pub(crate) fn undo_last(&mut self) {
        let Some(entry) = self.undo.pop() else {
            self.toast("Nothing to undo", false);
            return;
        };
        let repaint = self.repaint_fn();
        let job = match entry {
            UndoEntry::Renamed { from, to } => {
                match ops::rename(&to, &file_name(&from)) {
                    Ok(back) => {
                        self.refresh_folders(&[parent_of(&back)], Some(vec![back]));
                        self.toast("Rename undone", false);
                    }
                    Err(err) => self.toast_error(format!("Couldn't undo the rename: {err}")),
                }
                return;
            }
            UndoEntry::Created { path } => jobs::trash(vec![path], self.trash_bin.clone(), repaint),
            UndoEntry::Copied { created } => jobs::trash(created, self.trash_bin.clone(), repaint),
            UndoEntry::Moved { pairs } => {
                let back = pairs
                    .into_iter()
                    .map(|(original, new)| (new, original))
                    .collect();
                jobs::move_back(back, repaint)
            }
            UndoEntry::Trashed { paths } => jobs::restore(paths, self.trash_bin.clone(), repaint),
        };
        let mut job = job;
        job.is_undo = true;
        self.jobs.push(job);
    }

    pub(crate) fn copy_paths(&mut self) {
        let mut paths = self.active_tab().selected_paths();
        if paths.is_empty() {
            paths.push(self.active_tab().path().to_path_buf());
        }
        let text = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        self.ctx.copy_text(text);
        let what = if paths.len() == 1 { "path" } else { "paths" };
        self.toast(format!("Copied {what}"), false);
    }

    pub(crate) fn open_with(&mut self) {
        let tab = self.active_tab();
        let Some(row) = tab.selected_row() else {
            return;
        };
        let path = tab.path().join(&row.entry.name);
        if let Err(err) = lumo_platform::open_with(&path) {
            self.toast_error(format!("Couldn't open \"Open with\": {err}"));
        }
    }

    /// Opens every selected item: folders navigate (one), files launch.
    pub(crate) fn open_selection(&mut self) {
        let rows = self.active_tab().selected_rows();
        match rows.as_slice() {
            [] => {}
            [row] => self.open_row(*row),
            many if many.len() > MAX_OPEN_AT_ONCE => {
                self.toast_error(format!(
                    "Select at most {MAX_OPEN_AT_ONCE} items to open at once"
                ));
            }
            many => {
                let tab = self.active_tab();
                let paths: Vec<PathBuf> = many
                    .iter()
                    .filter_map(|&r| {
                        tab.listing()
                            .map(|l| tab.path().join(&l.rows()[r].entry.name))
                    })
                    .filter(|p| !p.is_dir())
                    .collect();
                for path in paths {
                    if let Err(err) = lumo_platform::open_path(&path) {
                        self.toast_error(format!("Couldn't open {}: {err}", path.display()));
                    }
                }
            }
        }
    }

    pub(crate) fn show_properties(&mut self) {
        let tab = self.active_tab();
        let path = match tab.selected_row() {
            Some(row) => tab.path().join(&row.entry.name),
            None => tab.path().to_path_buf(),
        };
        match properties_of(&path) {
            Ok(properties) => self.properties = Some(properties),
            Err(err) => self.toast_error(format!("Couldn't read properties: {err}")),
        }
    }

    /// Folds in progress and finishes completed jobs.
    pub(crate) fn poll_jobs(&mut self) {
        let mut finished = Vec::new();
        let mut i = 0;
        while i < self.jobs.len() {
            if let Some(result) = self.jobs[i].poll() {
                finished.push((self.jobs.remove(i), result));
            } else {
                i += 1;
            }
        }
        for (job, result) in finished {
            self.finish_job(&job, result);
        }
    }

    pub(crate) fn cancel_jobs(&mut self) {
        for job in &self.jobs {
            job.cancel();
        }
    }

    fn finish_job(&mut self, job: &Job, result: Result<JobOutcome, String>) {
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                self.toast_error(format!("{} failed: {err}", job.label));
                return;
            }
        };
        let undoable = !job.is_undo;
        match outcome {
            JobOutcome::Transferred {
                mode,
                dest_dir,
                report,
            } => {
                let sources: Vec<PathBuf> = report.done.iter().map(|(s, _)| parent_of(s)).collect();
                let created: Vec<PathBuf> = report.done.iter().map(|(_, d)| d.clone()).collect();
                if job.from_cut && !report.cancelled {
                    self.clipboard.clear();
                    self.cut_paths.clear();
                }
                let n = report.done.len();
                if undoable && n > 0 {
                    self.undo.push(match mode {
                        TransferMode::Copy => UndoEntry::Copied {
                            created: created.clone(),
                        },
                        TransferMode::Move => UndoEntry::Moved {
                            pairs: report.done.clone(),
                        },
                    });
                }
                let mut folders = sources;
                folders.push(dest_dir);
                self.refresh_folders(&folders, Some(created));
                let verb = match mode {
                    TransferMode::Copy => "Copied",
                    TransferMode::Move => "Moved",
                };
                if report.cancelled {
                    self.toast(
                        format!("Cancelled — {verb} {} first", items(n)),
                        undoable && n > 0,
                    );
                } else {
                    self.toast(format!("{verb} {}", items(n)), undoable && n > 0);
                }
            }
            JobOutcome::Trashed { paths, recoverable } => {
                let folders: Vec<PathBuf> = paths.iter().map(|p| parent_of(p)).collect();
                self.refresh_folders(&folders, None);
                let n = paths.len();
                if !recoverable {
                    // Never offer an Undo that can't work.
                    self.toast(
                        format!("Deleted {} permanently (no Recycle Bin here)", items(n)),
                        false,
                    );
                } else if undoable {
                    self.undo.push(UndoEntry::Trashed { paths });
                    self.toast(format!("Moved {} to the Recycle Bin", items(n)), true);
                } else {
                    self.toast("Undone", false);
                }
            }
            JobOutcome::Restored { paths } => {
                let folders: Vec<PathBuf> = paths.iter().map(|p| parent_of(p)).collect();
                self.refresh_folders(&folders, Some(paths.clone()));
                self.toast(format!("Restored {}", items(paths.len())), false);
            }
            JobOutcome::Deleted { paths } => {
                let folders: Vec<PathBuf> = paths.iter().map(|p| parent_of(p)).collect();
                self.refresh_folders(&folders, None);
                self.toast(format!("Deleted {} permanently", items(paths.len())), false);
            }
            JobOutcome::MovedBack { pairs } => {
                let mut folders: Vec<PathBuf> = pairs.iter().map(|(c, _)| parent_of(c)).collect();
                let originals: Vec<PathBuf> = pairs.iter().map(|(_, o)| o.clone()).collect();
                folders.extend(originals.iter().map(|o| parent_of(o)));
                self.refresh_folders(&folders, Some(originals));
                self.toast("Move undone", false);
            }
        }
    }

    /// Reloads every tab showing one of `folders`. If `select` is given, the
    /// active tab selects those items when they are in the folder it shows.
    fn refresh_folders(&mut self, folders: &[PathBuf], select: Option<Vec<PathBuf>>) {
        let active = self.active;
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if !folders.iter().any(|f| f == tab.path()) {
                continue;
            }
            let names: Vec<String> = select
                .as_ref()
                .filter(|_| i == active)
                .map(|paths| {
                    paths
                        .iter()
                        .filter(|p| p.parent() == Some(tab.path()))
                        .map(|p| file_name(p))
                        .collect()
                })
                .unwrap_or_default();
            if names.is_empty() {
                tab.refresh();
            } else {
                tab.refresh_selecting(names);
            }
        }
    }
}

fn properties_of(path: &Path) -> std::io::Result<Properties> {
    let meta = std::fs::symlink_metadata(path)?;
    let entry = lumo_core::Entry {
        name: path.file_name().map(Into::into).unwrap_or_default(),
        kind: meta.file_type().into(),
        size: meta.len(),
        modified: meta.modified().ok(),
        hidden: false,
    };
    let size = if meta.is_dir() {
        let count = std::fs::read_dir(path).map(Iterator::count).unwrap_or(0);
        items(count)
    } else {
        format!("{} ({} bytes)", format::size(meta.len()), meta.len())
    };
    #[cfg(windows)]
    let hidden = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x2 != 0
    };
    #[cfg(not(windows))]
    let hidden = file_name(path).starts_with('.');
    Ok(Properties {
        name: {
            let name = file_name(path);
            if name.is_empty() {
                path.display().to_string()
            } else {
                name
            }
        },
        location: path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        kind: format::type_label(&entry),
        size,
        created: format::modified(meta.created().ok()),
        modified: format::modified(meta.modified().ok()),
        accessed: format::modified(meta.accessed().ok()),
        hidden,
        read_only: meta.permissions().readonly(),
    })
}
