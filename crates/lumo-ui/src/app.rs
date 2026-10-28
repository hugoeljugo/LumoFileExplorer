//! Application state and behavior: tabs, actions, shortcuts, session saving.
//! Drawing lives in `view.rs`.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Context, Event, Key, KeyboardShortcut, Modifiers, PointerButton};
use lumo_core::EntryKind;
use lumo_platform::KnownFolder;

use crate::address::AddressBar;
use crate::listing::SortColumn;
use crate::session::Session;
use crate::tab::{Move, Tab};

pub const APP_ID: &str = "Lumo";

/// Recent locations kept for address-bar suggestions.
const MAX_RECENT: usize = 20;
/// Type-ahead resets after this long without typing.
const TYPE_AHEAD_RESET_SECS: f64 = 1.0;

#[derive(Debug, Clone)]
pub enum Start {
    /// Reopen the tabs from the last session (or the home folder).
    Restore,
    /// Open a single tab at this folder; used by `lumo PATH` and shell hooks.
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub start: Start,
    /// Where open tabs are saved; `None` disables saving and restoring.
    pub session_file: Option<PathBuf>,
}

/// Everything the UI can ask for. Views push actions; `apply` performs them
/// after drawing, so state never changes mid-frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Navigate(PathBuf),
    OpenInNewTab(PathBuf),
    /// Text typed in the address bar: a folder, a file, or a relative path.
    OpenTyped(String),
    Back,
    Forward,
    Up,
    Refresh,
    NewTab,
    CloseTab(usize),
    ActivateTab(usize),
    NextTab,
    PrevTab,
    NewWindow,
    Select(Option<usize>),
    /// Open the row at this index (double-click).
    OpenRow(usize),
    OpenSelected,
    MoveSelection(Move),
    Sort(SortColumn),
    ToggleHidden,
    EditAddress,
}

pub struct LumoApp {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) show_hidden: bool,
    pub(crate) address: AddressBar,
    pub(crate) recent: Vec<PathBuf>,
    pub(crate) quick_access: Vec<KnownFolder>,
    pub(crate) drives: Vec<PathBuf>,
    /// Last error worth showing (e.g. a file failed to open); cleared on navigation.
    pub(crate) status: Option<String>,
    pub(crate) actions: Vec<Action>,
    session_file: Option<PathBuf>,
    saved_session: Option<Session>,
    type_ahead: String,
    type_ahead_at: f64,
    title: String,
    /// The (tab id, folder) the shared file table showed last frame.
    pub(crate) table_shown_for: Option<(u64, PathBuf)>,
    ctx: Context,
}

impl LumoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        crate::view::install_fonts(&cc.egui_ctx);
        crate::view::apply_style(&cc.egui_ctx);

        let restored = match (&config.start, &config.session_file) {
            (Start::Restore, Some(file)) => Session::load(file),
            _ => None,
        };
        let (tabs, active) = match (config.start, restored) {
            (Start::Path(path), _) => (vec![Tab::new(path)], 0),
            (Start::Restore, Some(session)) => {
                // Drop tabs whose folders are gone (unplugged drive, deleted dir).
                let active_path = session.tabs.get(session.active).cloned();
                let tabs: Vec<Tab> = session
                    .tabs
                    .into_iter()
                    .filter(|p| p.is_dir())
                    .map(Tab::new)
                    .collect();
                let active = tabs
                    .iter()
                    .position(|t| Some(t.path()) == active_path.as_deref())
                    .unwrap_or(0);
                (tabs, active)
            }
            (Start::Restore, None) => (Vec::new(), 0),
        };
        let tabs = if tabs.is_empty() {
            vec![Tab::new(lumo_platform::home_dir())]
        } else {
            tabs
        };

        Self {
            recent: tabs.iter().map(|t| t.path().to_path_buf()).collect(),
            tabs,
            active,
            show_hidden: false,
            address: AddressBar::default(),
            quick_access: lumo_platform::known_folders(),
            drives: lumo_platform::drives(),
            status: None,
            actions: Vec::new(),
            session_file: config.session_file,
            saved_session: None,
            type_ahead: String::new(),
            type_ahead_at: 0.0,
            title: String::new(),
            table_shown_for: None,
            ctx: cc.egui_ctx.clone(),
        }
    }

    // ---- read-only accessors (used by tests and the view) ----

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn is_editing_address(&self) -> bool {
        self.address.is_editing()
    }

    /// True once the active tab has finished loading.
    pub fn is_idle(&self) -> bool {
        self.active_tab().listing().is_some_and(|l| !l.is_loading())
    }

    /// Queues an action as if the user had triggered it; applied next frame.
    /// For tests and automation (and, later, requests from other processes).
    pub fn queue(&mut self, action: Action) {
        self.actions.push(action);
        self.ctx.request_repaint();
    }

    fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    // ---- per-frame work ----

    /// Starts loading the active tab and folds in rows from every tab's loader.
    pub(crate) fn update_tabs(&mut self) {
        let ctx = self.ctx.clone();
        let show_hidden = self.show_hidden;
        let active = &mut self.tabs[self.active];
        active.ensure_loaded(move || ctx.request_repaint());
        for tab in &mut self.tabs {
            tab.poll();
        }
        self.tabs[self.active].update_view(show_hidden);
    }

    pub(crate) fn handle_input(&mut self, ctx: &Context) {
        let editing = self.address.is_editing();
        let last_tab = self.tabs.len() - 1;
        let active = self.active;
        let mut actions = Vec::new();
        let mut typed = String::new();
        let now = ctx.input(|i| i.time);

        ctx.input_mut(|i| {
            if i.pointer.button_pressed(PointerButton::Extra1) {
                actions.push(Action::Back);
            }
            if i.pointer.button_pressed(PointerButton::Extra2) {
                actions.push(Action::Forward);
            }
            let mut shortcut = |modifiers: Modifiers, key: Key| {
                i.consume_shortcut(&KeyboardShortcut::new(modifiers, key))
            };
            // egui matches extra Shift/Alt loosely, so the more specific
            // combinations must be consumed first.
            if shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab) {
                actions.push(Action::PrevTab);
            }
            if shortcut(Modifiers::CTRL, Key::Tab) {
                actions.push(Action::NextTab);
            }
            if shortcut(Modifiers::CTRL, Key::T) {
                actions.push(Action::NewTab);
            }
            if shortcut(Modifiers::CTRL, Key::W) {
                actions.push(Action::CloseTab(active));
            }
            if shortcut(Modifiers::CTRL, Key::N) {
                actions.push(Action::NewWindow);
            }
            let digits = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
            ];
            for (n, key) in digits.into_iter().enumerate() {
                if shortcut(Modifiers::CTRL, key) {
                    actions.push(Action::ActivateTab(n));
                }
            }
            if shortcut(Modifiers::CTRL, Key::Num9) {
                actions.push(Action::ActivateTab(last_tab));
            }
            if shortcut(Modifiers::ALT, Key::ArrowLeft) {
                actions.push(Action::Back);
            }
            if shortcut(Modifiers::ALT, Key::ArrowRight) {
                actions.push(Action::Forward);
            }
            if shortcut(Modifiers::ALT, Key::ArrowUp) {
                actions.push(Action::Up);
            }
            if shortcut(Modifiers::CTRL, Key::L)
                || shortcut(Modifiers::ALT, Key::D)
                || shortcut(Modifiers::NONE, Key::F4)
            {
                actions.push(Action::EditAddress);
            }
            if shortcut(Modifiers::NONE, Key::F5) {
                actions.push(Action::Refresh);
            }
            if shortcut(Modifiers::CTRL, Key::H) {
                actions.push(Action::ToggleHidden);
            }

            if editing {
                return;
            }
            let moves = [
                (Key::ArrowUp, Move::Up),
                (Key::ArrowDown, Move::Down),
                (Key::PageUp, Move::PageUp),
                (Key::PageDown, Move::PageDown),
                (Key::Home, Move::Home),
                (Key::End, Move::End),
            ];
            for (key, movement) in moves {
                if shortcut(Modifiers::NONE, key) {
                    actions.push(Action::MoveSelection(movement));
                }
            }
            if shortcut(Modifiers::NONE, Key::Enter) {
                actions.push(Action::OpenSelected);
            }
            if shortcut(Modifiers::NONE, Key::Backspace) {
                actions.push(Action::Back);
            }
            if !i.modifiers.ctrl && !i.modifiers.alt {
                for event in &i.events {
                    if let Event::Text(text) = event {
                        typed.push_str(text);
                    }
                }
            }
        });

        if !typed.is_empty() && !editing {
            if now - self.type_ahead_at > TYPE_AHEAD_RESET_SECS {
                self.type_ahead.clear();
            }
            self.type_ahead.push_str(&typed);
            self.type_ahead_at = now;
            let prefix = self.type_ahead.clone();
            self.active_tab_mut().select_prefix(&prefix);
        }
        self.actions.extend(actions);
    }

    /// Applies queued actions. Returns `true` if any were applied.
    pub(crate) fn apply_actions(&mut self) -> bool {
        let actions = std::mem::take(&mut self.actions);
        let any = !actions.is_empty();
        for action in actions {
            self.apply(action);
        }
        any
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Navigate(path) => {
                self.status = None;
                self.active_tab_mut().navigate(path);
                self.remember_location();
            }
            Action::OpenInNewTab(path) => {
                self.tabs.push(Tab::new(path));
                self.active = self.tabs.len() - 1;
                self.remember_location();
            }
            Action::OpenTyped(text) => self.open_typed(&text),
            Action::Back => {
                if self.active_tab_mut().back() {
                    self.status = None;
                    self.remember_location();
                }
            }
            Action::Forward => {
                if self.active_tab_mut().forward() {
                    self.status = None;
                    self.remember_location();
                }
            }
            Action::Up => {
                if self.active_tab_mut().up() {
                    self.status = None;
                    self.remember_location();
                }
            }
            Action::Refresh => self.active_tab_mut().refresh(),
            Action::NewTab => {
                self.tabs.push(Tab::new(lumo_platform::home_dir()));
                self.active = self.tabs.len() - 1;
            }
            Action::CloseTab(index) => self.close_tab(index),
            Action::ActivateTab(index) => {
                if index < self.tabs.len() {
                    self.active = index;
                }
            }
            Action::NextTab => self.active = (self.active + 1) % self.tabs.len(),
            Action::PrevTab => {
                self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
            }
            Action::NewWindow => {
                let path = self.active_tab().path().to_path_buf();
                let spawned = std::env::current_exe()
                    .and_then(|exe| std::process::Command::new(exe).arg(path).spawn());
                if let Err(err) = spawned {
                    self.status = Some(format!("Couldn't open a new window: {err}"));
                }
            }
            Action::Select(row) => self.active_tab_mut().select(row),
            Action::OpenRow(row) => self.open_row(row),
            Action::OpenSelected => {
                if let Some(row) = self.active_tab().selected() {
                    self.open_row(row);
                }
            }
            Action::MoveSelection(movement) => self.active_tab_mut().move_selection(movement),
            Action::Sort(column) => {
                let tab = self.active_tab_mut();
                tab.set_sort(tab.sort().toggled(column));
            }
            Action::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                for tab in &mut self.tabs {
                    tab.invalidate_view();
                }
            }
            Action::EditAddress => {
                let current = self.active_tab().path().to_path_buf();
                self.address.start_editing(&current);
            }
        }
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            // Like a browser: closing the last tab closes the window.
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.tabs.remove(index);
        // Closing the active tab activates its right neighbour (or the new last tab).
        if self.active > index || self.active >= self.tabs.len() {
            self.active -= 1;
        }
    }

    fn open_row(&mut self, row: usize) {
        let tab = self.active_tab();
        let Some(entry) = tab.listing().and_then(|l| l.rows().get(row)) else {
            return;
        };
        let path = tab.path().join(&entry.entry.name);
        let is_folder = match entry.entry.kind {
            EntryKind::Directory => true,
            EntryKind::Symlink => path.is_dir(),
            EntryKind::File | EntryKind::Other => false,
        };
        if is_folder {
            self.apply(Action::Navigate(path));
        } else {
            self.open_file(&path);
        }
    }

    fn open_file(&mut self, path: &Path) {
        if let Err(err) = lumo_platform::open_path(path) {
            self.status = Some(format!("Couldn't open {}: {err}", path.display()));
        }
    }

    fn open_typed(&mut self, text: &str) {
        let Some(path) = resolve_typed(text, self.active_tab().path()) else {
            return;
        };
        if path.is_dir() {
            self.apply(Action::Navigate(path));
        } else if path.is_file() {
            self.open_file(&path);
        } else {
            self.status = Some(format!("Can't find '{}'.", path.display()));
        }
    }

    fn remember_location(&mut self) {
        let path = self.active_tab().path().to_path_buf();
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    pub(crate) fn sync_title(&mut self, ctx: &Context) {
        let title = format!("{} - Lumo", self.active_tab().title());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Saves open tabs whenever they change, so a crash loses nothing.
    pub(crate) fn save_session_if_changed(&mut self) {
        let Some(file) = &self.session_file else {
            return;
        };
        let session = Session {
            tabs: self.tabs.iter().map(|t| t.path().to_path_buf()).collect(),
            active: self.active,
        };
        if self.saved_session.as_ref() == Some(&session) {
            return;
        }
        if let Err(err) = session.save(file) {
            self.status = Some(format!("Couldn't save open tabs: {err}"));
        }
        // Recorded even on failure, so we retry on the next change rather than every frame.
        self.saved_session = Some(session);
    }
}

impl eframe::App for LumoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);
        self.apply_actions();
        self.update_tabs();
        self.draw(ui);
        if self.apply_actions() {
            self.update_tabs();
            ctx.request_repaint();
        }
        self.sync_title(&ctx);
        self.save_session_if_changed();
    }
}

/// Interprets address-bar text: trims quotes, expands `~`, and resolves
/// relative paths against `base`.
fn resolve_typed(text: &str, base: &Path) -> Option<PathBuf> {
    let text = text.trim().trim_matches('"');
    if text.is_empty() {
        return None;
    }
    let path = match text.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            match rest.trim_start_matches(['/', '\\']) {
                "" => lumo_platform::home_dir(),
                rest => lumo_platform::home_dir().join(rest),
            }
        }
        _ => PathBuf::from(text),
    };
    Some(base.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_home_and_quoted_paths() {
        let base = Path::new("base");
        assert_eq!(resolve_typed("  ", base), None);
        assert_eq!(resolve_typed("src", base), Some(base.join("src")));
        assert_eq!(resolve_typed("\"src\"", base), Some(base.join("src")));
        assert_eq!(resolve_typed("~", base), Some(lumo_platform::home_dir()));
        assert_eq!(
            resolve_typed("~/x", base),
            Some(lumo_platform::home_dir().join("x"))
        );
        let abs = std::env::temp_dir();
        assert_eq!(resolve_typed(&abs.display().to_string(), base), Some(abs));
    }
}
