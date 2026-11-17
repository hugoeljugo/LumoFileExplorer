//! One browser-style tab: a location, its history, and its loaded listing.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::listing::{Listing, Row, SortOrder};
use crate::nav::History;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A keyboard selection movement in the file table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Rows a PageUp/PageDown jumps.
const PAGE: usize = 20;

#[derive(Debug)]
pub struct Tab {
    id: u64,
    history: History,
    /// `None` until the tab is first shown, so restoring many tabs is cheap.
    listing: Option<Listing>,
    sort: SortOrder,
    /// Selected row (index into the listing's rows, stable across re-sorts).
    selected: Option<usize>,
    /// Name to select once it appears, e.g. the folder we just came up from.
    pending_select: Option<String>,
    scroll_to_selected: bool,
    /// Where the file table was scrolled the last time this tab was shown.
    scroll_offset: f32,
    view_stale: bool,
}

impl Tab {
    pub fn new(path: PathBuf) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            history: History::new(path),
            listing: None,
            sort: SortOrder::default(),
            selected: None,
            pending_select: None,
            scroll_to_selected: false,
            scroll_offset: 0.0,
            view_stale: true,
        }
    }

    /// Unique per tab for the lifetime of the process.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn path(&self) -> &Path {
        self.history.current()
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn listing(&self) -> Option<&Listing> {
        self.listing.as_ref()
    }

    pub fn sort(&self) -> SortOrder {
        self.sort
    }

    /// Folder name, or the drive/root itself (`C:\`, `/`).
    pub fn title(&self) -> String {
        title_for(self.path())
    }

    pub fn navigate(&mut self, path: PathBuf) {
        if path == self.path() {
            return;
        }
        let from = self.path().to_path_buf();
        self.history.visit(path);
        self.moved_from(&from);
    }

    pub fn back(&mut self) -> bool {
        let from = self.path().to_path_buf();
        let moved = self.history.back();
        if moved {
            self.moved_from(&from);
        }
        moved
    }

    pub fn forward(&mut self) -> bool {
        let from = self.path().to_path_buf();
        let moved = self.history.forward();
        if moved {
            self.moved_from(&from);
        }
        moved
    }

    pub fn up(&mut self) -> bool {
        match self.path().parent() {
            Some(parent) => {
                self.navigate(parent.to_path_buf());
                true
            }
            None => false,
        }
    }

    /// Reloads the folder, keeping the current selection if it still exists.
    pub fn refresh(&mut self) {
        self.pending_select = self.selected_row().map(|row| row.name.clone());
        self.listing = None;
        self.selected = None;
    }

    fn moved_from(&mut self, from: &Path) {
        self.listing = None;
        self.selected = None;
        self.scroll_offset = 0.0;
        // Like Explorer: after going up, highlight the folder we came from.
        self.pending_select = (from.parent() == Some(self.path()))
            .then(|| from.file_name())
            .flatten()
            .map(|name| name.to_string_lossy().into_owned());
    }

    /// Starts loading if this tab has no listing yet.
    pub fn ensure_loaded(&mut self, repaint: impl Fn() + Send + 'static) {
        if self.listing.is_none() {
            self.listing = Some(Listing::load(self.path().to_path_buf(), repaint));
            self.view_stale = true;
        }
    }

    /// Takes newly loaded rows. Cheap enough to call for every tab each frame.
    pub fn poll(&mut self) -> bool {
        self.listing.as_mut().is_some_and(Listing::poll)
    }

    /// Brings the sorted view and pending selection up to date.
    pub fn update_view(&mut self, show_hidden: bool) {
        let Some(listing) = &mut self.listing else {
            return;
        };
        listing.update_view(self.sort, show_hidden, self.view_stale);
        self.view_stale = false;
        if let Some(name) = &self.pending_select {
            if let Some(row) = listing.find_by_name(name) {
                self.selected = Some(row);
                self.scroll_to_selected = true;
                self.pending_select = None;
            } else if !listing.is_loading() {
                self.pending_select = None;
            }
        }
    }

    /// Forces the next [`Tab::update_view`] to rebuild (e.g. hidden-files toggle).
    pub fn invalidate_view(&mut self) {
        self.view_stale = true;
    }

    pub fn set_sort(&mut self, sort: SortOrder) {
        self.sort = sort;
        self.view_stale = true;
        self.scroll_to_selected = true;
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_row(&self) -> Option<&Row> {
        Some(&self.listing.as_ref()?.rows()[self.selected?])
    }

    pub fn select(&mut self, row: Option<usize>) {
        self.selected = row;
    }

    /// Selects `row` and scrolls the table to it.
    pub fn reveal(&mut self, row: usize) {
        self.selected = Some(row);
        self.scroll_to_selected = true;
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = offset;
    }

    /// Returns (and clears) whether the table should scroll to the selection.
    pub fn take_scroll_request(&mut self) -> bool {
        std::mem::take(&mut self.scroll_to_selected)
    }

    pub fn move_selection(&mut self, movement: Move) {
        let Some(listing) = &self.listing else {
            return;
        };
        let view = listing.view();
        let Some(last) = view.len().checked_sub(1) else {
            return;
        };
        let current = self.selected.and_then(|row| listing.view_position(row));
        let target = match (movement, current) {
            (Move::Home, _) | (Move::Down | Move::PageDown, None) => 0,
            (Move::End, _) | (Move::Up | Move::PageUp, None) => last,
            (Move::Up, Some(pos)) => pos.saturating_sub(1),
            (Move::Down, Some(pos)) => (pos + 1).min(last),
            (Move::PageUp, Some(pos)) => pos.saturating_sub(PAGE),
            (Move::PageDown, Some(pos)) => (pos + PAGE).min(last),
        };
        self.selected = Some(view[target]);
        self.scroll_to_selected = true;
    }

    /// Type-ahead: selects the first visible row whose name starts with
    /// `prefix` (case-insensitive).
    pub fn select_prefix(&mut self, prefix: &str) {
        let Some(listing) = &self.listing else {
            return;
        };
        let prefix = prefix.to_lowercase();
        let hit = listing
            .view()
            .iter()
            .copied()
            .find(|&i| listing.rows()[i].name.to_lowercase().starts_with(&prefix));
        if hit.is_some() {
            self.selected = hit;
            self.scroll_to_selected = true;
        }
    }
}

pub fn title_for(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir;
    use std::time::{Duration, Instant};

    fn settle(tab: &mut Tab) {
        tab.ensure_loaded(|| {});
        let deadline = Instant::now() + Duration::from_secs(10);
        while tab.listing().is_some_and(Listing::is_loading) {
            assert!(Instant::now() < deadline, "load timed out");
            tab.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        tab.poll();
        tab.update_view(true);
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for d in ["a", "b", "c"] {
            create_dir(dir.path().join(d)).unwrap();
        }
        dir
    }

    fn selected_name(tab: &Tab) -> Option<&str> {
        tab.selected_row().map(|r| r.name.as_str())
    }

    #[test]
    fn going_up_selects_the_folder_we_left() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().join("b"));
        assert!(tab.up());
        settle(&mut tab);

        assert_eq!(tab.path(), dir.path());
        assert_eq!(selected_name(&tab), Some("b"));
    }

    #[test]
    fn keyboard_movement_clamps_at_the_ends() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);

        tab.move_selection(Move::Down);
        assert_eq!(selected_name(&tab), Some("a"));
        tab.move_selection(Move::Up);
        assert_eq!(selected_name(&tab), Some("a"));
        tab.move_selection(Move::End);
        tab.move_selection(Move::Down);
        assert_eq!(selected_name(&tab), Some("c"));
        tab.move_selection(Move::PageUp);
        assert_eq!(selected_name(&tab), Some("a"));
    }

    #[test]
    fn type_ahead_selects_by_prefix() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);

        tab.select_prefix("C");
        assert_eq!(selected_name(&tab), Some("c"));
        tab.select_prefix("zzz");
        assert_eq!(selected_name(&tab), Some("c"));
    }

    #[test]
    fn refresh_keeps_the_selection() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);
        tab.select_prefix("b");

        tab.refresh();
        settle(&mut tab);

        assert_eq!(selected_name(&tab), Some("b"));
    }

    #[test]
    fn titles() {
        assert_eq!(title_for(Path::new("/home/user/src")), "src");
        #[cfg(windows)]
        assert_eq!(title_for(Path::new("C:\\")), "C:\\");
    }
}
