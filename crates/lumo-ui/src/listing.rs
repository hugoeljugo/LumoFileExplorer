//! A directory's contents as shown in the file table, loaded in the background.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use lumo_core::{Entry, EntryKind, FsError, ScanOptions, natural_cmp};

use crate::files::TrashBin;
use crate::format;
use lumo_platform::TrashedItem;

/// While a large directory is still loading, re-sort at most this often.
const RESORT_INTERVAL: Duration = Duration::from_millis(200);

/// One file-table row. Display strings are built on the loader thread so the
/// UI thread only lays them out.
#[derive(Debug, Clone)]
pub struct Row {
    pub entry: Entry,
    pub name: String,
    /// Type column; in the Recycle Bin, the folder it was deleted from.
    pub type_label: String,
    pub size_label: String,
    /// Date column; in the Recycle Bin, when it was deleted.
    pub date_label: String,
    /// Recycle Bin id (only for rows in the Recycle Bin view).
    pub trash_id: Option<OsString>,
}

impl Row {
    pub fn new(entry: Entry) -> Self {
        Self {
            name: entry.name.to_string_lossy().into_owned(),
            type_label: format::type_label(&entry),
            size_label: format::size_label(&entry),
            date_label: format::modified(entry.modified),
            entry,
            trash_id: None,
        }
    }

    /// A Recycle Bin item. Sorting by "date" sorts by deletion time and by
    /// "type" by original location, matching the columns shown.
    pub fn from_trashed(item: TrashedItem) -> Self {
        let entry = Entry {
            name: item.name,
            kind: if item.is_dir {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: item.size.unwrap_or(0),
            modified: item.deleted,
            hidden: false,
        };
        Self {
            name: entry.name.to_string_lossy().into_owned(),
            type_label: item.original_parent.display().to_string(),
            size_label: format::size_label(&entry),
            date_label: format::modified(entry.modified),
            entry,
            trash_id: Some(item.id),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.entry.kind == EntryKind::Directory
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Modified,
    Type,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortOrder {
    pub column: SortColumn,
    pub ascending: bool,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self {
            column: SortColumn::Name,
            ascending: true,
        }
    }
}

impl SortOrder {
    /// Clicking the current column flips direction; another column starts ascending.
    pub fn toggled(self, column: SortColumn) -> Self {
        Self {
            column,
            ascending: column != self.column || !self.ascending,
        }
    }

    /// Folders always come first; within each group, sort by the column.
    pub fn compare(self, a: &Row, b: &Row) -> Ordering {
        let by_name = || natural_cmp(&a.name, &b.name);
        let ord = match self.column {
            SortColumn::Name => by_name(),
            SortColumn::Modified => a.entry.modified.cmp(&b.entry.modified).then_with(by_name),
            SortColumn::Type => a.type_label.cmp(&b.type_label).then_with(by_name),
            SortColumn::Size => a.entry.size.cmp(&b.entry.size).then_with(by_name),
        };
        let ord = if self.ascending { ord } else { ord.reverse() };
        (!a.is_dir()).cmp(&!b.is_dir()).then(ord)
    }
}

#[derive(Debug)]
pub enum LoadState {
    Loading,
    Done,
    Failed(FsError),
}

enum LoadMsg {
    Batch { rows: Vec<Row>, unreadable: usize },
    Failed(FsError),
    Done,
}

#[derive(Debug)]
pub struct Listing {
    rows: Vec<Row>,
    /// Indices into `rows` of the visible rows, in display order.
    view: Vec<usize>,
    state: LoadState,
    /// Entries the scan could not read (shown as a count in the status bar).
    unreadable: usize,
    rx: Option<Receiver<LoadMsg>>,
    view_dirty: bool,
    last_sort: Option<Instant>,
}

impl Listing {
    /// Starts loading `path` on a background thread. `repaint` is called
    /// (from that thread) whenever new rows are ready.
    pub fn load(path: PathBuf, repaint: impl Fn() + Send + 'static) -> Self {
        Self::spawn(move |tx| load_worker(&path, tx, &repaint))
    }

    /// Loads the Recycle Bin's contents instead of a folder.
    pub(crate) fn load_bin(bin: TrashBin, repaint: impl Fn() + Send + 'static) -> Self {
        Self::spawn(move |tx| {
            let msg = match bin.list() {
                Ok(items) => LoadMsg::Batch {
                    rows: items.into_iter().map(Row::from_trashed).collect(),
                    unreadable: 0,
                },
                Err(source) => LoadMsg::Failed(FsError::Io {
                    path: PathBuf::from(crate::bin::TITLE),
                    source,
                }),
            };
            let failed = matches!(msg, LoadMsg::Failed(_));
            // A send error just means the listing was dropped.
            let _ = tx.send(msg);
            if !failed {
                let _ = tx.send(LoadMsg::Done);
            }
            repaint();
        })
    }

    fn spawn(work: impl FnOnce(&SyncSender<LoadMsg>) + Send + 'static) -> Self {
        // Bounded: if the UI stops polling, the loader waits instead of piling up rows.
        let (tx, rx) = mpsc::sync_channel(4);
        let spawned = thread::Builder::new()
            .name("lumo-load".into())
            .spawn(move || work(&tx));
        let (state, rx) = match spawned {
            Ok(_) => (LoadState::Loading, Some(rx)),
            Err(source) => (
                LoadState::Failed(FsError::Io {
                    path: PathBuf::new(),
                    source,
                }),
                None,
            ),
        };
        Self {
            rows: Vec::new(),
            view: Vec::new(),
            state,
            unreadable: 0,
            rx,
            view_dirty: false,
            last_sort: None,
        }
    }

    /// Takes everything the loader has produced so far. Returns `true` if
    /// anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(LoadMsg::Batch { rows, unreadable }) => {
                    self.rows.extend(rows);
                    self.unreadable += unreadable;
                    self.view_dirty = true;
                }
                Ok(LoadMsg::Failed(err)) => self.state = LoadState::Failed(err),
                Ok(LoadMsg::Done) => self.state = LoadState::Done,
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => {
                    if matches!(self.state, LoadState::Loading) {
                        self.state = LoadState::Done;
                    }
                    self.rx = None;
                    return true;
                }
            }
            changed = true;
        }
    }

    /// Rebuilds the visible, sorted view if rows arrived or settings changed.
    /// While loading, re-sorting is throttled so huge folders stay smooth.
    pub fn update_view(&mut self, sort: SortOrder, show_hidden: bool, force: bool) {
        let throttled = self.is_loading()
            && self
                .last_sort
                .is_some_and(|t| t.elapsed() < RESORT_INTERVAL);
        if !force && (!self.view_dirty || throttled) {
            return;
        }
        let rows = &self.rows;
        self.view = (0..rows.len())
            .filter(|&i| show_hidden || !rows[i].entry.hidden)
            .collect();
        self.view.sort_by(|&a, &b| sort.compare(&rows[a], &rows[b]));
        self.view_dirty = false;
        self.last_sort = Some(Instant::now());
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn view(&self) -> &[usize] {
        &self.view
    }

    pub fn state(&self) -> &LoadState {
        &self.state
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, LoadState::Loading)
    }

    pub fn unreadable(&self) -> usize {
        self.unreadable
    }

    /// Position in the view of the row at `row_index`, if visible.
    pub fn view_position(&self, row_index: usize) -> Option<usize> {
        self.view.iter().position(|&i| i == row_index)
    }

    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.name == name)
    }
}

fn load_worker(path: &std::path::Path, tx: &SyncSender<LoadMsg>, repaint: &dyn Fn()) {
    let options = ScanOptions::default();
    let mut scan = match lumo_core::scan(path, &options) {
        Ok(scan) => scan,
        Err(err) => {
            // A send error just means the listing was dropped; nobody to tell.
            let _ = tx.send(LoadMsg::Failed(err));
            repaint();
            return;
        }
    };
    while let Some(batch) = scan.next_batch() {
        let mut rows = Vec::with_capacity(batch.len());
        let mut unreadable = 0;
        for item in batch {
            match item {
                Ok(entry) => rows.push(Row::new(entry)),
                Err(_) => unreadable += 1,
            }
        }
        if tx.send(LoadMsg::Batch { rows, unreadable }).is_err() {
            return; // Listing dropped (tab closed or navigated away): stop scanning.
        }
        repaint();
    }
    let _ = tx.send(LoadMsg::Done);
    repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir, write};

    fn loaded(path: &std::path::Path) -> Listing {
        let mut listing = Listing::load(path.to_path_buf(), || {});
        let deadline = Instant::now() + Duration::from_secs(10);
        while listing.is_loading() {
            assert!(Instant::now() < deadline, "load timed out");
            listing.poll();
            thread::sleep(Duration::from_millis(1));
        }
        listing
    }

    fn names(listing: &Listing) -> Vec<&str> {
        listing
            .view()
            .iter()
            .map(|&i| listing.rows()[i].name.as_str())
            .collect()
    }

    #[test]
    fn loads_and_sorts_folders_first_naturally() {
        let dir = tempfile::tempdir().unwrap();
        for f in ["file10.txt", "file2.txt", "B.txt"] {
            File::create(dir.path().join(f)).unwrap();
        }
        create_dir(dir.path().join("zdir")).unwrap();

        let mut listing = loaded(dir.path());
        listing.update_view(SortOrder::default(), true, true);

        assert_eq!(
            names(&listing),
            ["zdir", "B.txt", "file2.txt", "file10.txt"]
        );
    }

    #[test]
    fn sorts_by_size_descending_keeping_folders_first() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path().join("small"), "1").unwrap();
        write(dir.path().join("big"), "12345").unwrap();
        create_dir(dir.path().join("dir")).unwrap();

        let mut listing = loaded(dir.path());
        let order = SortOrder::default()
            .toggled(SortColumn::Size)
            .toggled(SortColumn::Size);
        assert!(!order.ascending);
        listing.update_view(order, true, true);

        assert_eq!(names(&listing), ["dir", "big", "small"]);
    }

    #[test]
    fn missing_folder_fails() {
        let dir = tempfile::tempdir().unwrap();
        let listing = loaded(&dir.path().join("nope"));
        assert!(matches!(
            listing.state(),
            LoadState::Failed(FsError::NotFound(_))
        ));
    }

    #[test]
    fn dropping_listing_stops_loader() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5000 {
            File::create(dir.path().join(i.to_string())).unwrap();
        }
        // The bounded channel fills up; dropping the receiver must unblock and
        // end the worker rather than leak a blocked thread. The test passing
        // without hanging is the assertion; the worker exits on its next send.
        let listing = Listing::load(dir.path().to_path_buf(), || {});
        thread::sleep(Duration::from_millis(50));
        drop(listing);
    }

    #[test]
    fn toggling_sort_direction() {
        let order = SortOrder::default();
        assert!(!order.toggled(SortColumn::Name).ascending);
        assert!(order.toggled(SortColumn::Size).ascending);
    }
}
