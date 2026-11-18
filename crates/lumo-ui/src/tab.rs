//! One browser-style tab: a location, its history, and its loaded listing.

use std::collections::{BTreeSet, HashMap};
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
    /// A reload in progress; the current listing stays on screen until it
    /// finishes, so refreshing never flickers.
    next_listing: Option<Listing>,
    /// A reload was asked for; it starts the next time the tab is shown.
    refresh_requested: bool,
    sort: SortOrder,
    /// Selected rows (indices into the listing's rows, stable across re-sorts).
    selection: BTreeSet<usize>,
    /// The keyboard cursor: the row arrows move from and single-item actions use.
    focus: Option<usize>,
    /// Where Shift+click / Shift+arrow ranges start.
    anchor: Option<usize>,
    /// Names to select once they appear (the folder we came up from, pasted
    /// or renamed items); the first one gets the focus.
    pending_select: Vec<String>,
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
            next_listing: None,
            refresh_requested: false,
            sort: SortOrder::default(),
            selection: BTreeSet::new(),
            focus: None,
            anchor: None,
            pending_select: Vec::new(),
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
        let names = self.selected_names();
        self.refresh_selecting(names);
    }

    /// Reloads the folder and then selects `names` (e.g. just-pasted items).
    /// The current listing stays visible until the new one is ready.
    pub fn refresh_selecting(&mut self, names: Vec<String>) {
        self.pending_select = names;
        if self.listing.is_some() {
            self.refresh_requested = true;
        }
    }

    /// Loading the folder, or reloading it in the background.
    pub fn is_loading(&self) -> bool {
        self.refresh_requested
            || self.next_listing.is_some()
            || self.listing.as_ref().is_none_or(Listing::is_loading)
    }

    fn moved_from(&mut self, from: &Path) {
        self.listing = None;
        self.next_listing = None;
        self.refresh_requested = false;
        self.clear_selection();
        self.scroll_offset = 0.0;
        // Like Explorer: after going up, highlight the folder we came from.
        self.pending_select = (from.parent() == Some(self.path()))
            .then(|| from.file_name())
            .flatten()
            .map(|name| name.to_string_lossy().into_owned())
            .into_iter()
            .collect();
    }

    /// Starts loading if this tab has no listing yet, or a requested reload.
    pub fn ensure_loaded(&mut self, repaint: impl Fn() + Send + Clone + 'static) {
        if self.listing.is_none() {
            self.listing = Some(Listing::load(self.path().to_path_buf(), repaint));
            self.view_stale = true;
        } else if std::mem::take(&mut self.refresh_requested) {
            self.next_listing = Some(Listing::load(self.path().to_path_buf(), repaint));
        }
    }

    /// Takes newly loaded rows, and swaps in a finished reload. Cheap enough
    /// to call for every tab each frame.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.listing.as_mut().is_some_and(Listing::poll);
        if let Some(next) = &mut self.next_listing {
            changed |= next.poll();
            if !next.is_loading() {
                // Row indices refer to the old listing: carry the selection
                // over by name (unless an operation asked for specific names).
                if self.pending_select.is_empty() {
                    self.pending_select = self.selected_names();
                }
                self.clear_selection();
                self.listing = self.next_listing.take();
                self.view_stale = true;
                changed = true;
            }
        }
        changed
    }

    /// Brings the sorted view and pending selection up to date.
    pub fn update_view(&mut self, show_hidden: bool) {
        let Some(listing) = &mut self.listing else {
            return;
        };
        listing.update_view(self.sort, show_hidden, self.view_stale);
        self.view_stale = false;
        // Names to select refer to the reload, not to what is on screen.
        if self.pending_select.is_empty() || self.refresh_requested || self.next_listing.is_some() {
            return;
        }
        let by_name: HashMap<&str, usize> = listing
            .rows()
            .iter()
            .enumerate()
            .map(|(i, row)| (row.name.as_str(), i))
            .collect();
        let found: Vec<usize> = self
            .pending_select
            .iter()
            .filter_map(|name| by_name.get(name.as_str()).copied())
            .collect();
        // Wait for every name unless the folder has finished loading.
        if found.len() == self.pending_select.len() || !listing.is_loading() {
            self.pending_select.clear();
            self.selection = found.iter().copied().collect();
            self.focus = found.first().copied();
            self.anchor = self.focus;
            self.scroll_to_selected = self.focus.is_some();
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

    /// The focused row (the keyboard cursor), if any.
    pub fn selected(&self) -> Option<usize> {
        self.focus
    }

    /// The focused row's data.
    pub fn selected_row(&self) -> Option<&Row> {
        Some(&self.listing.as_ref()?.rows()[self.focus?])
    }

    pub fn is_selected(&self, row: usize) -> bool {
        self.selection.contains(&row)
    }

    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    /// Selected rows in display order.
    pub fn selected_rows(&self) -> Vec<usize> {
        let Some(listing) = &self.listing else {
            return Vec::new();
        };
        listing
            .view()
            .iter()
            .copied()
            .filter(|row| self.selection.contains(row))
            .collect()
    }

    pub fn selected_names(&self) -> Vec<String> {
        let Some(listing) = &self.listing else {
            return Vec::new();
        };
        self.selected_rows()
            .into_iter()
            .map(|row| listing.rows()[row].name.clone())
            .collect()
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        let Some(listing) = &self.listing else {
            return Vec::new();
        };
        self.selected_rows()
            .into_iter()
            .map(|row| self.path().join(&listing.rows()[row].entry.name))
            .collect()
    }

    /// Total size of the selected files (folders count as 0).
    pub fn selected_size(&self) -> u64 {
        let Some(listing) = &self.listing else {
            return 0;
        };
        self.selection
            .iter()
            .map(|&row| listing.rows()[row].entry.size)
            .sum()
    }

    /// Selects just `row`, or clears the selection for `None`.
    pub fn select(&mut self, row: Option<usize>) {
        match row {
            Some(row) => self.select_only(row),
            None => self.clear_selection(),
        }
    }

    pub fn select_only(&mut self, row: usize) {
        self.selection = BTreeSet::from([row]);
        self.focus = Some(row);
        self.anchor = Some(row);
    }

    /// Ctrl+click / tick circle: adds or removes one row.
    pub fn toggle(&mut self, row: usize) {
        if !self.selection.remove(&row) {
            self.selection.insert(row);
        }
        self.focus = Some(row);
        self.anchor = Some(row);
    }

    /// Shift+click: selects the range from the anchor to `row`.
    pub fn extend_to(&mut self, row: usize) {
        let Some(listing) = &self.listing else {
            return;
        };
        let anchor = self.anchor.or(self.focus).unwrap_or(row);
        let (Some(a), Some(b)) = (listing.view_position(anchor), listing.view_position(row)) else {
            self.select_only(row);
            return;
        };
        let (from, to) = (a.min(b), a.max(b));
        self.selection = listing.view()[from..=to].iter().copied().collect();
        self.focus = Some(row);
        self.anchor = Some(anchor);
    }

    pub fn select_all(&mut self) {
        let Some(listing) = &self.listing else {
            return;
        };
        self.selection = listing.view().iter().copied().collect();
        if self.focus.is_none() {
            self.focus = listing.view().first().copied();
            self.anchor = self.focus;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.focus = None;
        self.anchor = None;
    }

    /// Selects `row` and scrolls the table to it.
    pub fn reveal(&mut self, row: usize) {
        self.select_only(row);
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

    /// Arrow-key movement; with `extend` (Shift) the selection grows from the anchor.
    pub fn move_selection(&mut self, movement: Move, extend: bool) {
        let Some(listing) = &self.listing else {
            return;
        };
        let view = listing.view();
        let Some(last) = view.len().checked_sub(1) else {
            return;
        };
        let current = self.focus.and_then(|row| listing.view_position(row));
        let target = match (movement, current) {
            (Move::Home, _) | (Move::Down | Move::PageDown, None) => 0,
            (Move::End, _) | (Move::Up | Move::PageUp, None) => last,
            (Move::Up, Some(pos)) => pos.saturating_sub(1),
            (Move::Down, Some(pos)) => (pos + 1).min(last),
            (Move::PageUp, Some(pos)) => pos.saturating_sub(PAGE),
            (Move::PageDown, Some(pos)) => (pos + PAGE).min(last),
        };
        let target = view[target];
        if extend {
            self.extend_to(target);
        } else {
            self.select_only(target);
        }
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
        if let Some(row) = hit {
            self.select_only(row);
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
        while tab.is_loading() {
            tab.ensure_loaded(|| {});
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

        tab.move_selection(Move::Down, false);
        assert_eq!(selected_name(&tab), Some("a"));
        tab.move_selection(Move::Up, false);
        assert_eq!(selected_name(&tab), Some("a"));
        tab.move_selection(Move::End, false);
        tab.move_selection(Move::Down, false);
        assert_eq!(selected_name(&tab), Some("c"));
        tab.move_selection(Move::PageUp, false);
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
    fn ctrl_shift_and_select_all() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);
        let row = |tab: &Tab, name: &str| tab.listing().unwrap().find_by_name(name).unwrap();
        let (a, b, c) = (row(&tab, "a"), row(&tab, "b"), row(&tab, "c"));

        tab.select_only(a);
        tab.extend_to(c);
        assert_eq!(tab.selected_names(), ["a", "b", "c"]);

        tab.toggle(b);
        assert_eq!(tab.selected_names(), ["a", "c"]);
        assert_eq!(tab.selection_count(), 2);

        tab.clear_selection();
        tab.select_all();
        assert_eq!(tab.selection_count(), 3);
    }

    #[test]
    fn shift_arrows_extend_from_the_anchor() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);

        tab.move_selection(Move::Down, false);
        tab.move_selection(Move::Down, true);
        tab.move_selection(Move::Down, true);
        assert_eq!(tab.selected_names(), ["a", "b", "c"]);
        tab.move_selection(Move::Up, true);
        assert_eq!(tab.selected_names(), ["a", "b"]);
    }

    #[test]
    fn refresh_keeps_a_multi_selection() {
        let dir = fixture();
        let mut tab = Tab::new(dir.path().to_path_buf());
        settle(&mut tab);
        tab.select_all();

        tab.refresh();
        settle(&mut tab);

        assert_eq!(tab.selected_names(), ["a", "b", "c"]);
    }

    #[test]
    fn titles() {
        assert_eq!(title_for(Path::new("/home/user/src")), "src");
        #[cfg(windows)]
        assert_eq!(title_for(Path::new("C:\\")), "C:\\");
    }
}
