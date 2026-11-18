//! Drawing: tab strip, toolbar, sidebar, file table and status bar.
//! Views never mutate state directly; they push [`Action`]s. Colors and sizes
//! come from [`crate::theme`].

use std::path::Path;

use eframe::egui::{
    self, Align, Align2, Area, Button, CentralPanel, Color32, Context, CornerRadius, FontSelection,
    Frame, Id, Label, Layout, Margin, Modal, Order, Panel, ProgressBar, Rect, RichText, Role,
    ScrollArea, Sense, Shape, Stroke, TextEdit, TextStyle, TextWrapMode, Ui, WidgetInfo,
    WidgetText, pos2,
    text::{CCursor, CCursorRange},
    vec2,
};
use egui_extras::{Column, TableBuilder};
use lumo_core::EntryKind;

use crate::app::{Action, LumoApp};
use crate::icons::{self, FileKind, Icon};
use crate::listing::{LoadState, SortColumn};
use crate::theme::{self, Edge, NAV_ITEM_HEIGHT, Palette, ROW_HEIGHT, TAB_HEIGHT, TOOLBAR_CONTROL};

const MIN_TAB_WIDTH: f32 = 110.0;
const MAX_TAB_WIDTH: f32 = 220.0;
/// How far the lumen glow reaches into a selected row / sidebar entry.
const ROW_GLOW: f32 = 140.0;
const NAV_GLOW: f32 = 70.0;
const TAB_GLOW: f32 = 12.0;
/// The selection tray appears from this many selected items.
const TRAY_MIN_SELECTION: usize = 2;

impl LumoApp {
    pub(crate) fn draw(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui.ctx());
        Panel::top("lumo-tabs")
            .frame(Frame::new().fill(p.base))
            .show_separator_line(false)
            .show(ui, |ui| self.tab_strip(ui, &p));
        Panel::top("lumo-toolbar")
            .frame(
                Frame::new()
                    .fill(p.surface)
                    .inner_margin(Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| self.toolbar(ui, &p));
        Panel::bottom("lumo-status")
            .frame(
                Frame::new()
                    .fill(p.base)
                    .inner_margin(Margin::symmetric(10, 2)),
            )
            .show(ui, |ui| self.status_bar(ui, &p));
        Panel::left("lumo-sidebar")
            .frame(
                Frame::new()
                    .fill(p.base)
                    .inner_margin(Margin::symmetric(0, 6)),
            )
            .resizable(true)
            .default_size(210.0)
            .size_range(150.0..=420.0)
            .show(ui, |ui| self.sidebar(ui, &p));
        CentralPanel::no_frame()
            .frame(Frame::new().fill(p.surface).inner_margin(Margin {
                left: 0,
                right: 4,
                top: 2,
                bottom: 0,
            }))
            .show(ui, |ui| self.file_table(ui, &p));

        if self.palette.is_open() {
            let key = self.palette_key();
            if self
                .palette_cache
                .as_ref()
                .is_none_or(|(cached, _)| *cached != key)
            {
                let items = if self.palette.is_menu() {
                    self.menu_items()
                } else {
                    self.palette_items()
                };
                self.palette_cache = Some((key, items));
            }
            if let Some((_, items)) = &self.palette_cache
                && let Some(action) = self.palette.show(ui.ctx(), items)
            {
                self.actions.push(action);
            }
        } else {
            self.palette_cache = None;
        }

        let ctx = ui.ctx().clone();
        self.selection_tray(&ctx, &p);
        self.toast_view(&ctx, &p);
        self.delete_confirmation(&ctx, &p);
        self.properties_window(&ctx, &p);
        self.about_window(&ctx, &p);
    }

    fn tab_strip(&mut self, ui: &mut Ui, p: &Palette) {
        let strip = ui.max_rect();
        let plus_width = TOOLBAR_CONTROL + 12.0;
        let count = self.tabs.len() as f32;
        let tab_width = ((strip.width() - plus_width) / count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
        let mut active_span = None;
        ScrollArea::horizontal()
            .id_salt("tab-strip")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let active = i == self.active;
                        let (rect, clicked) =
                            tab_button(ui, p, &tab.title(), tab.path(), tab_width, active);
                        if active {
                            active_span = Some(rect.x_range());
                        }
                        match clicked {
                            Some(TabClick::Activate) => self.actions.push(Action::ActivateTab(i)),
                            Some(TabClick::Close) => self.actions.push(Action::CloseTab(i)),
                            None => {}
                        }
                    }
                    ui.add_space(6.0);
                    let plus =
                        icons::button(ui, p, Icon::Plus, "New tab", TOOLBAR_CONTROL, true, false);
                    if plus.on_hover_text("New tab (Ctrl+T)").clicked() {
                        self.actions.push(Action::NewTab);
                    }
                });
            });
        // Hairline under the tab bar, interrupted by the active tab so it
        // flows into the toolbar below.
        let y = strip.bottom() - 0.5;
        let painter = ui.painter();
        match active_span {
            Some(span) => {
                painter.add(theme::hairline(p, pos2(strip.left(), y), span.min));
                painter.add(theme::hairline(p, pos2(span.max, y), strip.right()));
            }
            None => {
                painter.add(theme::hairline(p, pos2(strip.left(), y), strip.right()));
            }
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, p: &Palette) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let tab = &self.tabs[self.active];
            let history = tab.history();
            let buttons = [
                (
                    Icon::ArrowLeft,
                    "Back",
                    "Alt+Left",
                    history.can_go_back(),
                    Action::Back,
                ),
                (
                    Icon::ArrowRight,
                    "Forward",
                    "Alt+Right",
                    history.can_go_forward(),
                    Action::Forward,
                ),
                (Icon::ArrowUp, "Up", "Alt+Up", tab.can_go_up(), Action::Up),
                (Icon::Refresh, "Refresh", "F5", true, Action::Refresh),
            ];
            for (icon, name, key, enabled, action) in buttons {
                let hint = format!("{name} ({key})");
                if icons::button(ui, p, icon, name, TOOLBAR_CONTROL, enabled, false)
                    .on_hover_text(&hint)
                    .clicked()
                {
                    self.actions.push(action);
                }
            }
            ui.add_space(6.0);
            let current = self.tabs[self.active].path().to_path_buf();
            // Right to left: the palette button takes its space first and the
            // address bar fills whatever is left.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Lit only for the palette itself, not the right-click menu.
                let open = self.palette.is_open() && !self.palette.is_menu();
                if icons::button(
                    ui,
                    p,
                    Icon::Search,
                    "Command palette",
                    TOOLBAR_CONTROL,
                    true,
                    open,
                )
                .on_hover_text("Search commands, tabs, places and files (Ctrl+K)")
                .clicked()
                {
                    self.actions.push(Action::OpenPalette);
                }
                ui.add_space(4.0);
                if let Some(action) = self.address.show(ui, &current, &self.recent) {
                    self.actions.push(action);
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui, p: &Palette) {
        let current = self.tabs[self.active].path().to_path_buf();
        ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            section_header(ui, p, "QUICK ACCESS");
            for folder in &self.quick_access {
                let icon = icons::place_icon(folder.label);
                nav_item(
                    ui,
                    p,
                    icon,
                    folder.label,
                    &folder.path,
                    &current,
                    &mut self.actions,
                );
            }
            let bin_icon = if self.bin_count.is_some_and(|n| n > 0) {
                Icon::TrashFull
            } else {
                Icon::Trash
            };
            nav_item(
                ui,
                p,
                bin_icon,
                crate::bin::TITLE,
                &crate::bin::path(),
                &current,
                &mut self.actions,
            );
            ui.add_space(12.0);
            section_header(ui, p, "DRIVES");
            for drive in &self.drives {
                let label = drive.display().to_string();
                let label = label.trim_end_matches(['\\', '/']);
                let label = if label.is_empty() { "/" } else { label };
                nav_item(
                    ui,
                    p,
                    Icon::Drive,
                    label,
                    drive,
                    &current,
                    &mut self.actions,
                );
            }
        });
    }

    fn file_table(&mut self, ui: &mut Ui, p: &Palette) {
        // The empty area of the list: click clears the selection, right-click
        // opens the folder's menu. Rows are added later, so they sit on top.
        let list_rect = ui.max_rect();
        self.table_rect = Some(list_rect);
        let background = ui.interact(list_rect, Id::unique("lumo-file-list"), Sense::click());
        background.widget_info(|| WidgetInfo::labeled(Role::ListBox, true, "File list"));
        if background.clicked() {
            self.actions.push(Action::ClearSelection);
        }
        if background.secondary_clicked() {
            let pos = background
                .interact_pointer_pos()
                .unwrap_or(list_rect.center());
            self.actions.push(Action::ContextMenu { row: None, pos });
        }

        let scroll_requested = self.tabs[self.active].take_scroll_request();
        let tab = &self.tabs[self.active];
        let Some(listing) = tab.listing() else {
            ui.centered_and_justified(|ui| ui.spinner());
            return;
        };
        if listing.rows().is_empty() {
            ui.centered_and_justified(|ui| match listing.state() {
                LoadState::Failed(err) => {
                    ui.colored_label(ui.visuals().error_fg_color, err.to_string());
                }
                LoadState::Loading => {
                    ui.spinner();
                }
                LoadState::Done => {
                    let empty = if crate::bin::is_bin(tab.path()) {
                        "The Recycle Bin is empty."
                    } else {
                        "This folder is empty."
                    };
                    ui.label(RichText::new(empty).color(p.text_secondary));
                }
            });
            return;
        }

        let rows = listing.rows();
        let view = listing.view();
        let selected = tab.selected();
        let sort = tab.sort();
        let tab_id = tab.id();
        let folder = tab.path().to_path_buf();
        // With several items selected, every row shows its tick circle.
        let multi = tab.selection_count() >= TRAY_MIN_SELECTION;
        let cut_paths = &self.cut_paths;
        let renaming = &mut self.renaming;
        // The table is shared by all tabs, so restore this tab's scroll
        // position whenever it shows a different tab or folder than last frame.
        let shown = (tab.id(), tab.path().to_path_buf());
        let restore_offset =
            (self.table_shown_for.as_ref() != Some(&shown)).then(|| tab.scroll_offset());
        let actions = &mut self.actions;
        // Row highlights span exactly the table's width (cells' rects overshoot it).
        let row_x = ui.max_rect().x_range();
        let table_top = ui.cursor().top();
        let mono = theme::mono(12.5);
        // No full-height column lines (Zed-style); a column's resize line
        // appears only while hovering it.
        ui.visuals_mut().widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::new(1.0, p.border);

        // One table id for every tab: egui keeps per-row state for each table
        // id it has seen, so per-tab ids leaked memory as tabs were closed.
        let mut table = TableBuilder::new(ui)
            .id_salt("file-table")
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            // Not clipped: the row highlight is painted from this cell across
            // the whole row. Long names still truncate (`Label::truncate`).
            .column(Column::initial(380.0).at_least(140.0).clip(false))
            .column(Column::initial(150.0).at_least(80.0).clip(true))
            .column(Column::initial(110.0).at_least(60.0).clip(true))
            .column(Column::remainder().at_least(80.0).clip(true));
        if scroll_requested && let Some(pos) = selected.and_then(|row| listing.view_position(row)) {
            table = table.scroll_to_row(pos, None);
        } else if let Some(offset) = restore_offset {
            table = table.vertical_scroll_offset(offset);
        }

        // In the Recycle Bin the date and type columns hold the deletion
        // time and the original location.
        let in_bin = crate::bin::is_bin(&folder);
        let columns = [
            (SortColumn::Name, "Name"),
            (
                SortColumn::Modified,
                if in_bin { "Deleted" } else { "Modified" },
            ),
            (
                SortColumn::Type,
                if in_bin { "Original location" } else { "Type" },
            ),
            (SortColumn::Size, "Size"),
        ];
        let output = table
            .header(ROW_HEIGHT, |mut header| {
                for (i, (column, label)) in columns.into_iter().enumerate() {
                    header.col(|ui| {
                        if i == 0 {
                            ui.add_space(52.0);
                        }
                        let text = RichText::new(label).small().color(p.text_secondary);
                        if ui
                            .add(Button::new(text).frame(false))
                            .on_hover_text(format!("Sort by {}", label.to_lowercase()))
                            .clicked()
                        {
                            actions.push(Action::Sort(column));
                        }
                        if sort.column == column {
                            let icon = if sort.ascending {
                                Icon::SortUp
                            } else {
                                Icon::SortDown
                            };
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                            icons::paint(ui.painter(), rect, icon, p.accent);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, view.len(), |mut table_row| {
                    let index = view[table_row.index()];
                    let row = &rows[index];
                    // We draw our own highlights instead of egui_extras' default ones.
                    table_row.set_hovered(false);
                    let mut background = None;
                    let is_selected = tab.is_selected(index);
                    let path = folder.join(&row.entry.name);
                    let is_cut = !cut_paths.is_empty() && cut_paths.contains(&path);
                    table_row.col(|ui| {
                        background = Some((ui.painter().clone(), ui.painter().add(Shape::Noop)));
                        ui.add_space(6.0);
                        let row_hovered = ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| {
                            row_x.contains(pos.x) && ui.max_rect().y_range().contains(pos.y)
                        });
                        if tick_circle(ui, p, &row.name, is_selected, row_hovered || multi) {
                            actions.push(Action::ToggleRow(index));
                        }
                        ui.add_space(4.0);
                        let icon = match row.entry.kind {
                            EntryKind::Directory => Icon::Folder,
                            EntryKind::Symlink => Icon::Link,
                            EntryKind::File | EntryKind::Other => {
                                Icon::File(FileKind::from_name(&row.name))
                            }
                        };
                        icons::inline(ui, icon, p.icon);
                        ui.add_space(2.0);
                        match renaming
                            .as_mut()
                            .filter(|r| r.tab == tab_id && r.path == path)
                        {
                            Some(rename) => {
                                if let Some(action) = rename_field(ui, rename, row.is_dir()) {
                                    actions.push(action);
                                }
                            }
                            None => {
                                let color = if row.entry.hidden || is_cut {
                                    p.text_secondary
                                } else {
                                    p.text
                                };
                                ui.add(
                                    Label::new(RichText::new(&row.name).color(color)).truncate(),
                                );
                            }
                        }
                    });
                    table_row.col(|ui| {
                        ui.label(
                            RichText::new(&row.date_label)
                                .font(mono.clone())
                                .color(p.text_secondary),
                        );
                    });
                    table_row.col(|ui| {
                        ui.label(RichText::new(&row.type_label).color(p.text_secondary));
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(&row.size_label)
                                    .font(mono.clone())
                                    .color(p.text_secondary),
                            );
                        });
                    });
                    let response = table_row.response();
                    if let Some((painter, slot)) = background {
                        let row = Rect::from_x_y_ranges(row_x, response.rect.y_range());
                        painter.set(slot, row_highlight(p, row, is_selected, response.hovered()));
                    }
                    if response.double_clicked() {
                        actions.push(Action::OpenRow(index));
                    } else if response.clicked() {
                        let modifiers = response.ctx.input(|i| i.modifiers);
                        actions.push(Action::ClickRow {
                            row: index,
                            ctrl: modifiers.command,
                            shift: modifiers.shift,
                        });
                    }
                    if response.secondary_clicked() {
                        let pos = response
                            .interact_pointer_pos()
                            .unwrap_or(response.rect.center());
                        actions.push(Action::ContextMenu {
                            row: Some(index),
                            pos,
                        });
                    }
                });
            });
        let header_bottom = table_top + ROW_HEIGHT + 1.0;
        ui.painter().add(theme::hairline(
            p,
            pos2(row_x.min, header_bottom),
            row_x.max,
        ));
        self.tabs[self.active].set_scroll_offset(output.state.clamped_offset().y);
        self.table_shown_for = Some(shown);
    }

    fn status_bar(&mut self, ui: &mut Ui, p: &Palette) {
        ui.horizontal(|ui| {
            let small = |text: String| RichText::new(text).small().color(p.text_secondary);
            let tab = &self.tabs[self.active];
            if let Some(listing) = tab.listing() {
                let count = listing.view().len();
                ui.label(small(format!(
                    "{count} item{}",
                    if count == 1 { "" } else { "s" }
                )));
                if listing.is_loading() {
                    ui.spinner();
                }
                if listing.unreadable() > 0 {
                    ui.label(small(format!("· {} unreadable", listing.unreadable())));
                }
                let selected = tab.selection_count();
                if selected == 1
                    && let Some(row) = tab.selected_row()
                {
                    let detail = if row.is_dir() {
                        String::new()
                    } else {
                        format!("  {}", row.size_label)
                    };
                    ui.label(small(format!("· 1 item selected{detail}")));
                } else if selected > 1 {
                    let size = crate::format::size(tab.selected_size());
                    ui.label(small(format!("· {selected} items selected  {size}")));
                }
            }
            if let Some(job) = self.jobs.first() {
                ui.add_space(8.0);
                ui.label(small(format!("{}…", job.label)));
                match job.fraction() {
                    Some(fraction) => {
                        ui.add(
                            ProgressBar::new(fraction)
                                .desired_width(110.0)
                                .desired_height(6.0),
                        );
                    }
                    None => {
                        ui.spinner();
                    }
                }
                if icons::button(ui, p, Icon::Close, "Cancel", 20.0, true, false)
                    .on_hover_text("Cancel")
                    .clicked()
                {
                    self.actions.push(Action::CancelJobs);
                }
            }
            if let Some(message) = &self.status {
                ui.label(
                    RichText::new(message)
                        .small()
                        .color(ui.visuals().error_fg_color),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let icon = if self.show_hidden {
                    Icon::Eye
                } else {
                    Icon::EyeOff
                };
                if icons::button(ui, p, icon, "Hidden items", 22.0, true, self.show_hidden)
                    .on_hover_text("Show hidden items (Ctrl+H)")
                    .clicked()
                {
                    self.actions.push(Action::ToggleHidden);
                }
            });
        });
    }
}

impl LumoApp {
    /// The floating bar shown while several items are selected.
    fn selection_tray(&mut self, ctx: &Context, p: &Palette) {
        let tab = &self.tabs[self.active];
        let count = tab.selection_count();
        let (Some(rect), true) = (self.table_rect, count >= TRAY_MIN_SELECTION) else {
            return;
        };
        let size = crate::format::size(tab.selected_size());
        let in_bin = crate::bin::is_bin(tab.path());
        let mut actions = Vec::new();
        Area::new(Id::unique("lumo-selection-tray"))
            .order(Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(pos2(rect.center().x, rect.bottom() - 16.0))
            .show(ctx, |ui| {
                let frame = overlay_frame(p).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(format!("{count} selected")).color(p.text));
                        ui.label(RichText::new(format!("· {size}")).color(p.text_secondary));
                        ui.add_space(10.0);
                        let more = (
                            Icon::Search,
                            "More actions",
                            "Shift+F10",
                            Action::ContextMenuForSelection,
                        );
                        let buttons = if in_bin {
                            vec![
                                (Icon::Undo, "Restore", "", Action::RestoreSelected),
                                (
                                    Icon::Trash,
                                    "Delete permanently",
                                    "Del",
                                    Action::DeletePermanently,
                                ),
                                more,
                            ]
                        } else {
                            vec![
                                (Icon::Copy, "Copy", "Ctrl+C", Action::Copy),
                                (Icon::Cut, "Cut", "Ctrl+X", Action::Cut),
                                (Icon::Trash, "Move to Recycle Bin", "Del", Action::Trash),
                                more,
                            ]
                        };
                        for (icon, name, key, action) in buttons {
                            let hint = if key.is_empty() {
                                name.to_owned()
                            } else {
                                format!("{name} ({key})")
                            };
                            if icons::button(ui, p, icon, name, 28.0, true, false)
                                .on_hover_text(hint)
                                .clicked()
                            {
                                actions.push(action);
                            }
                        }
                        ui.add_space(4.0);
                        if icons::button(ui, p, Icon::Close, "Clear selection", 28.0, true, false)
                            .on_hover_text("Clear selection (Esc)")
                            .clicked()
                        {
                            actions.push(Action::ClearSelection);
                        }
                    });
                });
                ui.painter()
                    .add(theme::lumen(p, frame.response.rect, Edge::Top, 10.0));
            });
        self.actions.extend(actions);
    }

    /// Short-lived message at the bottom right, with Undo when it applies.
    fn toast_view(&mut self, ctx: &Context, p: &Palette) {
        let (Some(toast), Some(rect)) = (&self.toast, self.table_rect) else {
            return;
        };
        let can_undo = toast.undo && !self.undo.is_empty();
        let color = if toast.error {
            ctx.global_style().visuals.error_fg_color
        } else {
            p.text
        };
        let message = toast.message.clone();
        let mut actions = Vec::new();
        // Above the tray when both are showing.
        let lift = if self.tabs[self.active].selection_count() >= TRAY_MIN_SELECTION {
            64.0
        } else {
            16.0
        };
        Area::new(Id::unique("lumo-toast"))
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(rect.right_bottom() - vec2(16.0, lift))
            .show(ctx, |ui| {
                overlay_frame(p).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(message).color(color));
                        if can_undo {
                            ui.add_space(8.0);
                            if ui
                                .add(Button::new(RichText::new("Undo").color(p.accent)))
                                .on_hover_text("Undo (Ctrl+Z)")
                                .clicked()
                            {
                                actions.push(Action::Undo);
                                actions.push(Action::DismissToast);
                            }
                        }
                        if icons::button(ui, p, Icon::Close, "Dismiss", 22.0, true, false).clicked()
                        {
                            actions.push(Action::DismissToast);
                        }
                    });
                });
            });
        self.actions.extend(actions);
    }

    /// "This can't be undone" — the only operations that ask first.
    fn delete_confirmation(&mut self, ctx: &Context, p: &Palette) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let items = |n: usize| {
            if n == 1 {
                "1 item".to_owned()
            } else {
                format!("{n} items")
            }
        };
        let (title, body, button) = match confirm {
            crate::files::Confirm::DeletePaths(paths) => {
                let what = match paths.as_slice() {
                    [one] => format!(
                        "\"{}\"",
                        one.file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_default()
                    ),
                    many => items(many.len()),
                };
                (
                    "Delete permanently?",
                    format!(
                        "{what} will be deleted without going to the Recycle Bin. This can't be undone."
                    ),
                    "Delete",
                )
            }
            crate::files::Confirm::PurgeItems(ids) => (
                "Delete permanently?",
                format!(
                    "{} will be removed from the Recycle Bin for good. This can't be undone.",
                    items(ids.len())
                ),
                "Delete permanently",
            ),
            crate::files::Confirm::EmptyBin => (
                "Empty the Recycle Bin?",
                match self.bin_count {
                    Some(n) => format!(
                        "{} will be deleted for good. This can't be undone.",
                        items(n)
                    ),
                    None => "Everything in it will be deleted for good. This can't be undone."
                        .to_owned(),
                },
                "Empty Recycle Bin",
            ),
        };
        let mut actions = Vec::new();
        let modal = Modal::new(Id::unique("lumo-confirm-delete"))
            .frame(overlay_frame(p).inner_margin(Margin::same(18)))
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(
                    RichText::new(title)
                        .font(theme::semibold(16.0))
                        .color(p.text),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(body).color(p.text_secondary));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let danger = ctx.global_style().visuals.error_fg_color;
                    if ui
                        .add(Button::new(RichText::new(button).color(Color32::WHITE)).fill(danger))
                        .clicked()
                    {
                        actions.push(Action::ConfirmDelete);
                    }
                    if ui.button("Cancel").clicked() {
                        actions.push(Action::CancelDelete);
                    }
                });
            });
        if modal.should_close() && actions.is_empty() {
            actions.push(Action::CancelDelete);
        }
        self.actions.extend(actions);
    }

    fn properties_window(&mut self, ctx: &Context, p: &Palette) {
        let Some(properties) = &self.properties else {
            return;
        };
        let mut open = true;
        egui::Window::new(RichText::new("Properties").color(p.text))
            .id(Id::unique("lumo-properties"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(
                self.table_rect
                    .map_or(pos2(240.0, 160.0), |r| r.center() - vec2(170.0, 140.0)),
            )
            .frame(overlay_frame(p).inner_margin(Margin::same(14)))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(&properties.name)
                        .font(theme::semibold(16.0))
                        .color(p.text),
                );
                ui.add_space(8.0);
                egui::Grid::new("lumo-properties-grid")
                    .num_columns(2)
                    .spacing(vec2(16.0, 6.0))
                    .show(ui, |ui| {
                        let flags = [
                            properties.hidden.then_some("Hidden"),
                            properties.read_only.then_some("Read-only"),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(", ");
                        for (label, value) in [
                            ("Type", properties.kind.as_str()),
                            ("Location", properties.location.as_str()),
                            ("Size", properties.size.as_str()),
                            ("Created", properties.created.as_str()),
                            ("Modified", properties.modified.as_str()),
                            ("Accessed", properties.accessed.as_str()),
                            ("Attributes", if flags.is_empty() { "—" } else { &flags }),
                        ] {
                            ui.label(RichText::new(label).color(p.text_secondary));
                            ui.label(RichText::new(value).color(p.text));
                            ui.end_row();
                        }
                    });
            });
        if !open {
            self.actions.push(Action::CloseProperties);
        }
    }
}

/// Elevated floating surface: tray, toast, dialogs.
pub(crate) fn overlay_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.elevated)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 6))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(60),
        })
}

/// The round checkbox at the start of a row. Returns `true` when clicked.
fn tick_circle(ui: &mut Ui, p: &Palette, name: &str, selected: bool, visible: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(Role::CheckBox, true, selected, format!("Select {name}"))
    });
    if visible || selected {
        let center = rect.center();
        if selected {
            ui.painter().circle_filled(center, 7.0, p.accent);
            let check = Rect::from_center_size(center, vec2(10.0, 10.0));
            icons::paint(ui.painter(), check, Icon::Check, Color32::WHITE);
        } else {
            let color = if response.hovered() { p.text } else { p.icon };
            ui.painter()
                .circle_stroke(center, 6.5, Stroke::new(1.2, color));
        }
    }
    response.on_hover_text("Select").clicked()
}

/// The in-place rename box. Enter or clicking away commits; Escape cancels.
fn rename_field(ui: &mut Ui, rename: &mut crate::files::Renaming, is_dir: bool) -> Option<Action> {
    let id = Id::unique("lumo-rename-field");
    let response = ui.add(
        TextEdit::singleline(&mut rename.text)
            .id(id)
            .hint_text("New name")
            .desired_width((ui.available_width() - 8.0).max(60.0)),
    );
    if rename.select_stem {
        rename.select_stem = false;
        response.request_focus();
        // Like Explorer: select the name but not the extension.
        let stem = match rename.text.rfind('.') {
            Some(dot) if dot > 0 && !is_dir => rename.text[..dot].chars().count(),
            _ => rename.text.chars().count(),
        };
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(stem))));
            state.store(ui.ctx(), id);
        }
    }
    if response.lost_focus() {
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        return Some(if escaped {
            Action::CancelRename
        } else {
            Action::CommitRename
        });
    }
    None
}

/// Hover/selection background for a file row. Selected rows carry the lumen.
fn row_highlight(p: &Palette, row: Rect, selected: bool, hovered: bool) -> Shape {
    match (selected, hovered) {
        (true, _) => Shape::Vec(vec![
            Shape::rect_filled(row, 0.0, p.selected),
            theme::lumen(p, row, Edge::Left, ROW_GLOW),
        ]),
        (false, true) => Shape::rect_filled(row, 0.0, p.hover),
        (false, false) => Shape::Noop,
    }
}

fn section_header(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(
            RichText::new(text)
                .font(theme::semibold(11.0))
                .color(p.text_secondary),
        );
    });
    ui.add_space(4.0);
}

/// A full-width sidebar entry: icon + label; the current folder carries the lumen.
fn nav_item(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    label: &str,
    path: &Path,
    current: &Path,
    actions: &mut Vec<Action>,
) {
    let selected = current == path;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), NAV_ITEM_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 0.0, p.selected);
            painter.add(theme::lumen(p, rect, Edge::Left, NAV_GLOW));
        } else if response.hovered() {
            painter.rect_filled(rect, 0.0, p.hover);
        }
        let icon_color = if selected { p.accent } else { p.icon };
        let icon_rect =
            Rect::from_center_size(rect.left_center() + vec2(22.0, 0.0), vec2(16.0, 16.0));
        icons::paint(painter, icon_rect, icon, icon_color);
        painter.text(
            rect.left_center() + vec2(38.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            TextStyle::Body.resolve(ui.style()),
            p.text,
        );
    }

    let response =
        response.on_hover_text(format!("{}\nMiddle-click: open in new tab", path.display()));
    if response.clicked() {
        actions.push(Action::Navigate(path.to_path_buf()));
    }
    if response.middle_clicked() {
        actions.push(Action::OpenInNewTab(path.to_path_buf()));
    }
}

enum TabClick {
    Activate,
    Close,
}

/// A flat, Zed-style tab. The active tab shares the toolbar's surface color
/// and carries the lumen along its top edge; the close button shows on the
/// active or hovered tab. Returns the tab's rect and what was clicked.
fn tab_button(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    path: &Path,
    width: f32,
    active: bool,
) -> (Rect, Option<TabClick>) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Tab, true, active, title));
    // Interacted after the tab, so it sits on top for hit-testing.
    let close_rect =
        Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
    let close = ui.interact(close_rect, response.id.with("close"), Sense::click());
    close.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Close tab"));
    let hovered = response.hovered() || close.hovered();

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, 0.0, p.surface);
            painter.add(theme::lumen(p, rect, Edge::Top, TAB_GLOW));
        } else if hovered {
            painter.rect_filled(rect, 0.0, p.hover);
        }
        // Hairline between tabs.
        painter.line_segment(
            [rect.right_top(), rect.right_bottom()],
            egui::Stroke::new(1.0, p.border),
        );

        let icon_rect =
            Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0));
        icons::paint(painter, icon_rect, Icon::Folder, p.icon);

        let text_left = rect.left() + 34.0;
        let text_width = (close_rect.left() - 4.0 - text_left).max(0.0);
        let galley = WidgetText::from(RichText::new(title).text_style(TextStyle::Body))
            .into_galley(
                ui,
                Some(TextWrapMode::Truncate),
                text_width,
                FontSelection::Default,
            );
        let text_pos = pos2(text_left, rect.center().y - galley.size().y / 2.0);
        let color = if active { p.text } else { p.text_secondary };
        painter.galley(text_pos, galley, color);

        if active || hovered {
            if close.hovered() {
                painter.rect_filled(close_rect, 4.0, p.hover);
            }
            let icon_rect = Rect::from_center_size(close_rect.center(), vec2(12.0, 12.0));
            let color = if close.hovered() { p.text } else { p.icon };
            icons::paint(painter, icon_rect, Icon::Close, color);
        }
    }

    let response = response.on_hover_text(path.display().to_string());
    let close = close.on_hover_text("Close tab (Ctrl+W)");
    let clicked = if close.clicked() || response.middle_clicked() {
        Some(TabClick::Close)
    } else if response.clicked() {
        Some(TabClick::Activate)
    } else {
        None
    };
    (rect, clicked)
}
