//! Application state and behavior: tabs, actions, shortcuts, session saving.
//! Drawing lives in `view.rs`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Context, Event, Key, KeyboardShortcut, Modifiers, PointerButton, Pos2, Rect,
    ThemePreference,
};
use lumo_core::EntryKind;
use lumo_platform::KnownFolder;

use crate::address::AddressBar;
use crate::files::{Clipboard, Properties, Renaming, Toast, TrashBin, UndoEntry};
use crate::icons::{self, FileKind, Icon};
use crate::jobs::Job;
use crate::listing::SortColumn;
use crate::palette::{CommandPalette, ItemKind, PaletteItem};
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
    /// Keep the clipboard and the Recycle Bin private to this window instead
    /// of using the OS ones. For tests: they must never touch the user's
    /// clipboard or fill their Recycle Bin.
    pub isolated: bool,
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
    /// A click on a row, with the modifier keys that were held.
    ClickRow {
        row: usize,
        ctrl: bool,
        shift: bool,
    },
    /// The tick circle on a row: add or remove it from the selection.
    ToggleRow(usize),
    SelectAll,
    ClearSelection,
    /// Open the row at this index (double-click).
    OpenRow(usize),
    OpenSelected,
    OpenSelectedInNewTab,
    /// Arrow keys; `true` extends the selection (Shift).
    MoveSelection(Move, bool),
    Sort(SortColumn),
    ToggleHidden,
    EditAddress,
    /// Select the row at this index and scroll it into view.
    Reveal(usize),
    OpenPalette,
    SetTheme(ThemePreference),
    /// Right-click: the search menu for `row` (or the folder background) at `pos`.
    ContextMenu {
        row: Option<usize>,
        pos: Pos2,
    },
    /// Shift+F10: the search menu for the current selection.
    ContextMenuForSelection,
    Copy,
    Cut,
    Paste,
    /// Move the selection to the Recycle Bin.
    Trash,
    /// Ask before deleting the selection permanently (Shift+Delete).
    DeletePermanently,
    ConfirmDelete,
    CancelDelete,
    Rename,
    CommitRename,
    CancelRename,
    NewFolder,
    NewFile,
    OpenWith,
    CopyPath,
    Properties,
    CloseProperties,
    Undo,
    DismissToast,
    CancelJobs,
}

/// What the command palette's item list was built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaletteKey {
    tab: u64,
    folder: PathBuf,
    rows: usize,
    tabs: usize,
    active: usize,
    show_hidden: bool,
    recent: usize,
    menu: bool,
    selected: usize,
    focus: Option<usize>,
    undo: usize,
}

pub struct LumoApp {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) show_hidden: bool,
    pub(crate) address: AddressBar,
    pub(crate) palette: CommandPalette,
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
    /// Palette items, rebuilt only when what they list changes.
    pub(crate) palette_cache: Option<(PaletteKey, Vec<PaletteItem>)>,
    pub(crate) clipboard: Clipboard,
    pub(crate) trash_bin: TrashBin,
    /// Items cut to the clipboard (drawn dimmed until pasted).
    pub(crate) cut_paths: HashSet<PathBuf>,
    pub(crate) jobs: Vec<Job>,
    pub(crate) undo: Vec<UndoEntry>,
    pub(crate) toast: Option<Toast>,
    pub(crate) renaming: Option<Renaming>,
    /// A just-created item to rename once it shows up in the list.
    pub(crate) rename_when_loaded: Option<PathBuf>,
    /// Items waiting for "Delete permanently?" confirmation.
    pub(crate) confirm_delete: Option<Vec<PathBuf>>,
    pub(crate) properties: Option<Properties>,
    /// Screen rect of the file list (for placing the tray, toasts and menus).
    pub(crate) table_rect: Option<Rect>,
    pub(crate) ctx: Context,
}

impl LumoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        crate::theme::install(&cc.egui_ctx);

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
            palette: CommandPalette::default(),
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
            palette_cache: None,
            clipboard: if config.isolated {
                Clipboard::Private(None)
            } else {
                Clipboard::System
            },
            // If the private bin can't be created, fall back to failing
            // loudly rather than silently using the user's Recycle Bin.
            trash_bin: if config.isolated {
                TrashBin::private().unwrap_or(TrashBin::Folder {
                    dir: PathBuf::from("<unavailable>"),
                    origins: Default::default(),
                })
            } else {
                TrashBin::System
            },
            cut_paths: HashSet::new(),
            jobs: Vec::new(),
            undo: Vec::new(),
            toast: None,
            renaming: None,
            rename_when_loaded: None,
            confirm_delete: None,
            properties: None,
            table_rect: None,
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

    pub fn is_palette_open(&self) -> bool {
        self.palette.is_open()
    }

    pub fn is_renaming(&self) -> bool {
        self.renaming.is_some()
    }

    pub fn is_confirming_delete(&self) -> bool {
        self.confirm_delete.is_some()
    }

    pub fn is_showing_properties(&self) -> bool {
        self.properties.is_some()
    }

    pub fn toast_message(&self) -> Option<&str> {
        self.toast.as_ref().map(|t| t.message.as_str())
    }

    /// True once the active tab has finished loading and no file job runs.
    pub fn is_idle(&self) -> bool {
        self.jobs.is_empty()
            && self.rename_when_loaded.is_none()
            && self.active_tab().listing().is_some_and(|l| !l.is_loading())
    }

    /// A text field has the keyboard (address bar, palette, rename box).
    pub(crate) fn text_input_active(&self) -> bool {
        self.address.is_editing() || self.palette.is_open() || self.renaming.is_some()
    }

    /// Queues an action as if the user had triggered it; applied next frame.
    /// For tests and automation (and, later, requests from other processes).
    pub fn queue(&mut self, action: Action) {
        self.actions.push(action);
        self.ctx.request_repaint();
    }

    pub(crate) fn active_tab_mut(&mut self) -> &mut Tab {
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
        // Typing in a text field: only window-level shortcuts apply.
        let editing = self.address.is_editing() || self.renaming.is_some();
        let has_selection = self.active_tab().selection_count() > 0;
        let last_tab = self.tabs.len() - 1;
        let active = self.active;
        let mut actions = Vec::new();
        let mut typed = String::new();
        let now = ctx.input(|i| i.time);

        let palette_open = self.palette.is_open();
        ctx.input_mut(|i| {
            let palette_key = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::K))
                || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::P));
            if palette_key {
                actions.push(Action::OpenPalette); // toggles
            }
            // While the palette is open it owns the keyboard: it handles
            // Escape, arrows and Enter itself.
            if palette_open {
                return;
            }
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
            // Ctrl+Shift+N must be checked before Ctrl+N (loose matching).
            if shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::N) && !editing {
                actions.push(Action::NewFolder);
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
            // File actions. More specific combinations first (see above).
            if shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::C) {
                actions.push(Action::CopyPath);
            }
            if shortcut(Modifiers::CTRL, Key::C) {
                actions.push(Action::Copy);
            }
            if shortcut(Modifiers::CTRL, Key::X) {
                actions.push(Action::Cut);
            }
            if shortcut(Modifiers::CTRL, Key::V) {
                actions.push(Action::Paste);
            }
            if shortcut(Modifiers::CTRL, Key::A) {
                actions.push(Action::SelectAll);
            }
            if shortcut(Modifiers::CTRL, Key::Z) {
                actions.push(Action::Undo);
            }
            if shortcut(Modifiers::SHIFT, Key::Delete) {
                actions.push(Action::DeletePermanently);
            }
            if shortcut(Modifiers::NONE, Key::Delete) {
                actions.push(Action::Trash);
            }
            if shortcut(Modifiers::NONE, Key::F2) {
                actions.push(Action::Rename);
            }
            if shortcut(Modifiers::SHIFT, Key::F10) {
                actions.push(Action::ContextMenuForSelection);
            }
            if shortcut(Modifiers::ALT, Key::Enter) {
                actions.push(Action::Properties);
            }
            if shortcut(Modifiers::CTRL, Key::Enter) {
                actions.push(Action::OpenSelectedInNewTab);
            }
            if has_selection && shortcut(Modifiers::NONE, Key::Escape) {
                actions.push(Action::ClearSelection);
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
                if shortcut(Modifiers::SHIFT, key) {
                    actions.push(Action::MoveSelection(movement, true));
                }
                if shortcut(Modifiers::NONE, key) {
                    actions.push(Action::MoveSelection(movement, false));
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
            Action::ClickRow { row, ctrl, shift } => {
                let tab = self.active_tab_mut();
                match (ctrl, shift) {
                    (_, true) => tab.extend_to(row),
                    (true, false) => tab.toggle(row),
                    (false, false) => tab.select_only(row),
                }
            }
            Action::ToggleRow(row) => self.active_tab_mut().toggle(row),
            Action::SelectAll => self.active_tab_mut().select_all(),
            Action::ClearSelection => self.active_tab_mut().clear_selection(),
            Action::OpenRow(row) => self.open_row(row),
            Action::OpenSelected => self.open_selection(),
            Action::OpenSelectedInNewTab => {
                let folders: Vec<PathBuf> = self
                    .active_tab()
                    .selected_paths()
                    .into_iter()
                    .filter(|p| p.is_dir())
                    .collect();
                for folder in folders {
                    self.apply(Action::OpenInNewTab(folder));
                }
            }
            Action::MoveSelection(movement, extend) => {
                self.active_tab_mut().move_selection(movement, extend);
            }
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
            Action::Reveal(row) => self.active_tab_mut().reveal(row),
            Action::OpenPalette => {
                if self.palette.is_open() {
                    self.palette.close();
                } else {
                    self.palette.open();
                }
            }
            Action::SetTheme(preference) => self.ctx.set_theme(preference),
            Action::ContextMenu { row, pos } => {
                let tab = self.active_tab_mut();
                match row {
                    // Right-clicking outside the selection acts on that row only.
                    Some(row) if !tab.is_selected(row) => tab.select_only(row),
                    Some(_) => {}
                    None => tab.clear_selection(),
                }
                self.palette.open_menu(pos);
            }
            Action::ContextMenuForSelection => {
                let pos = self.table_rect.map_or(Pos2::new(200.0, 160.0), |r| {
                    r.left_top() + egui::vec2(48.0, 48.0)
                });
                self.palette.open_menu(pos);
            }
            Action::Copy => self.copy_selection(false),
            Action::Cut => self.copy_selection(true),
            Action::Paste => self.paste(),
            Action::Trash => self.trash_selection(),
            Action::DeletePermanently => self.ask_delete_permanently(),
            Action::ConfirmDelete => self.delete_permanently(),
            Action::CancelDelete => self.confirm_delete = None,
            Action::Rename => self.start_rename(),
            Action::CommitRename => self.commit_rename(),
            Action::CancelRename => self.renaming = None,
            Action::NewFolder => self.create(true),
            Action::NewFile => self.create(false),
            Action::OpenWith => self.open_with(),
            Action::CopyPath => self.copy_paths(),
            Action::Properties => self.show_properties(),
            Action::CloseProperties => self.properties = None,
            Action::Undo => self.undo_last(),
            Action::DismissToast => self.toast = None,
            Action::CancelJobs => self.cancel_jobs(),
        }
    }

    /// Identifies what [`Self::palette_items`] depends on.
    pub(crate) fn palette_key(&self) -> PaletteKey {
        let tab = self.active_tab();
        PaletteKey {
            tab: tab.id(),
            folder: tab.path().to_path_buf(),
            rows: tab.listing().map_or(0, |l| l.view().len()),
            tabs: self.tabs.len(),
            active: self.active,
            show_hidden: self.show_hidden,
            recent: self.recent.len(),
            menu: self.palette.is_menu(),
            selected: tab.selection_count(),
            focus: tab.selected(),
            undo: self.undo.len(),
        }
    }

    /// Commands for files: on the selection if there is one, otherwise on
    /// the folder itself (paste, new folder, …).
    fn file_commands(&self) -> Vec<PaletteItem> {
        let command = |title: &str, icon, shortcut, action| PaletteItem {
            title: title.to_owned(),
            detail: String::new(),
            icon,
            shortcut,
            kind: ItemKind::Command,
            action,
        };
        let tab = self.active_tab();
        let count = tab.selection_count();
        let mut items = Vec::new();
        if count > 0 {
            let single = (count == 1).then(|| tab.selected_row()).flatten();
            let single_folder = single.is_some_and(|r| r.is_dir());
            let single_file = single.is_some_and(|r| !r.is_dir());
            items.push(command(
                "Open",
                Icon::ArrowRight,
                Some("Enter"),
                Action::OpenSelected,
            ));
            if single_folder {
                items.push(command(
                    "Open in new tab",
                    Icon::Plus,
                    Some("Ctrl+Enter"),
                    Action::OpenSelectedInNewTab,
                ));
            }
            if single_file {
                items.push(command(
                    "Open with…",
                    Icon::ArrowRight,
                    None,
                    Action::OpenWith,
                ));
            }
            items.push(command("Cut", Icon::Cut, Some("Ctrl+X"), Action::Cut));
            items.push(command("Copy", Icon::Copy, Some("Ctrl+C"), Action::Copy));
            if single.is_some() {
                items.push(command("Rename", Icon::Rename, Some("F2"), Action::Rename));
            }
            items.push(command(
                "Copy path",
                Icon::Copy,
                Some("Ctrl+Shift+C"),
                Action::CopyPath,
            ));
            items.push(command(
                "Move to Recycle Bin",
                Icon::Trash,
                Some("Del"),
                Action::Trash,
            ));
            items.push(command(
                "Delete permanently",
                Icon::Trash,
                Some("Shift+Del"),
                Action::DeletePermanently,
            ));
            if single.is_some() {
                items.push(command(
                    "Properties",
                    Icon::Info,
                    Some("Alt+Enter"),
                    Action::Properties,
                ));
            }
            items.push(command(
                "Select all",
                Icon::Check,
                Some("Ctrl+A"),
                Action::SelectAll,
            ));
        } else {
            if self.clipboard.get().is_some() {
                items.push(command("Paste", Icon::Paste, Some("Ctrl+V"), Action::Paste));
            }
            items.push(command(
                "New folder",
                Icon::NewFolder,
                Some("Ctrl+Shift+N"),
                Action::NewFolder,
            ));
            items.push(command("New text file", Icon::Plus, None, Action::NewFile));
            items.push(command(
                "Select all",
                Icon::Check,
                Some("Ctrl+A"),
                Action::SelectAll,
            ));
            items.push(command(
                "Copy folder path",
                Icon::Copy,
                Some("Ctrl+Shift+C"),
                Action::CopyPath,
            ));
            items.push(command(
                "Folder properties",
                Icon::Info,
                Some("Alt+Enter"),
                Action::Properties,
            ));
        }
        if !self.undo.is_empty() {
            items.push(command("Undo", Icon::Undo, Some("Ctrl+Z"), Action::Undo));
        }
        items
    }

    /// The right-click search menu: file commands plus view options when
    /// clicking the folder background.
    pub(crate) fn menu_items(&self) -> Vec<PaletteItem> {
        let mut items = self.file_commands();
        if self.active_tab().selection_count() == 0 {
            let command = |title: &str, icon, shortcut, action| PaletteItem {
                title: title.to_owned(),
                detail: String::new(),
                icon,
                shortcut,
                kind: ItemKind::Command,
                action,
            };
            let hidden = if self.show_hidden {
                "Hide hidden items"
            } else {
                "Show hidden items"
            };
            items.push(command(
                "Refresh",
                Icon::Refresh,
                Some("F5"),
                Action::Refresh,
            ));
            items.push(command(
                hidden,
                Icon::Eye,
                Some("Ctrl+H"),
                Action::ToggleHidden,
            ));
            for (title, column) in [
                ("Sort by name", SortColumn::Name),
                ("Sort by date modified", SortColumn::Modified),
                ("Sort by type", SortColumn::Type),
                ("Sort by size", SortColumn::Size),
            ] {
                items.push(command(title, Icon::SortDown, None, Action::Sort(column)));
            }
        }
        items
    }

    /// Everything the command palette can offer right now, in a stable order
    /// (commands, other tabs, places, recent locations, current folder).
    pub(crate) fn palette_items(&self) -> Vec<PaletteItem> {
        let command = |title: &str, icon, shortcut, action| PaletteItem {
            title: title.to_owned(),
            detail: String::new(),
            icon,
            shortcut,
            kind: ItemKind::Command,
            action,
        };
        let hidden = if self.show_hidden {
            "Hide hidden items"
        } else {
            "Show hidden items"
        };
        let mut items = self.file_commands();
        items.extend([
            command("New tab", Icon::Plus, Some("Ctrl+T"), Action::NewTab),
            command(
                "Close tab",
                Icon::Close,
                Some("Ctrl+W"),
                Action::CloseTab(self.active),
            ),
            command("New window", Icon::Plus, Some("Ctrl+N"), Action::NewWindow),
            command("Go back", Icon::ArrowLeft, Some("Alt+Left"), Action::Back),
            command(
                "Go forward",
                Icon::ArrowRight,
                Some("Alt+Right"),
                Action::Forward,
            ),
            command("Go up a folder", Icon::ArrowUp, Some("Alt+Up"), Action::Up),
            command("Refresh", Icon::Refresh, Some("F5"), Action::Refresh),
            command(hidden, Icon::Eye, Some("Ctrl+H"), Action::ToggleHidden),
            command(
                "Edit address",
                Icon::ChevronRight,
                Some("Ctrl+L"),
                Action::EditAddress,
            ),
            command(
                "Sort by name",
                Icon::SortDown,
                None,
                Action::Sort(SortColumn::Name),
            ),
            command(
                "Sort by date modified",
                Icon::SortDown,
                None,
                Action::Sort(SortColumn::Modified),
            ),
            command(
                "Sort by type",
                Icon::SortDown,
                None,
                Action::Sort(SortColumn::Type),
            ),
            command(
                "Sort by size",
                Icon::SortDown,
                None,
                Action::Sort(SortColumn::Size),
            ),
            command(
                "Theme: dark",
                Icon::Theme,
                None,
                Action::SetTheme(ThemePreference::Dark),
            ),
            command(
                "Theme: light",
                Icon::Theme,
                None,
                Action::SetTheme(ThemePreference::Light),
            ),
            command(
                "Theme: follow system",
                Icon::Theme,
                None,
                Action::SetTheme(ThemePreference::System),
            ),
        ]);

        let place = |title: String, path: &Path, icon, kind| PaletteItem {
            title,
            detail: path.display().to_string(),
            icon,
            shortcut: None,
            kind,
            action: Action::Navigate(path.to_path_buf()),
        };
        for (i, tab) in self.tabs.iter().enumerate() {
            if i != self.active {
                items.push(PaletteItem {
                    title: tab.title(),
                    detail: format!("Tab · {}", tab.path().display()),
                    icon: Icon::Folder,
                    shortcut: None,
                    kind: ItemKind::Tab,
                    action: Action::ActivateTab(i),
                });
            }
        }
        for folder in &self.quick_access {
            let icon = icons::place_icon(folder.label);
            items.push(place(
                folder.label.to_owned(),
                &folder.path,
                icon,
                ItemKind::Place,
            ));
        }
        for drive in &self.drives {
            items.push(place(
                drive.display().to_string(),
                drive,
                Icon::Drive,
                ItemKind::Place,
            ));
        }
        let current = self.active_tab().path();
        for path in &self.recent {
            let known = self.quick_access.iter().any(|f| f.path == *path);
            if path != current && !known {
                let title = crate::tab::title_for(path);
                items.push(place(title, path, Icon::Folder, ItemKind::Recent));
            }
        }

        if let Some(listing) = self.active_tab().listing() {
            for &row_index in listing.view() {
                let row = &listing.rows()[row_index];
                let (icon, action) = match row.entry.kind {
                    EntryKind::Directory => (
                        Icon::Folder,
                        Action::Navigate(current.join(&row.entry.name)),
                    ),
                    EntryKind::Symlink => (Icon::Link, Action::Reveal(row_index)),
                    EntryKind::File | EntryKind::Other => (
                        Icon::File(FileKind::from_name(&row.name)),
                        Action::Reveal(row_index),
                    ),
                };
                items.push(PaletteItem {
                    title: row.name.clone(),
                    detail: "in this folder".to_owned(),
                    icon,
                    shortcut: None,
                    kind: ItemKind::Entry,
                    action,
                });
            }
        }
        items
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

    pub(crate) fn open_row(&mut self, row: usize) {
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
        self.poll_jobs();
        self.expire_toast();
        self.update_tabs();
        self.start_pending_rename();
        self.draw(ui);
        if self.apply_actions() {
            self.update_tabs();
            ctx.request_repaint();
        }
        self.sync_title(&ctx);
        self.save_session_if_changed();
    }

    /// With eframe's clipboard shortcuts off (see `lib.rs`), Ctrl+C/X/V
    /// reach us as plain keys, so file copy/paste works even when the
    /// clipboard holds files rather than text. When a text field has the
    /// keyboard, turn them back into egui's text clipboard events.
    fn raw_input_hook(&mut self, _ctx: &Context, raw_input: &mut egui::RawInput) {
        if !self.text_input_active() {
            return;
        }
        for event in &mut raw_input.events {
            let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if !modifiers.command || modifiers.shift || modifiers.alt {
                continue;
            }
            match key {
                Key::C => *event = Event::Copy,
                Key::X => *event = Event::Cut,
                Key::V => {
                    if let Some(text) = lumo_platform::clipboard_text() {
                        *event = Event::Paste(text);
                    }
                }
                _ => {}
            }
        }
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
