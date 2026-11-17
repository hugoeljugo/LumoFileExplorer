//! Drawing: tab strip, toolbar, sidebar, file table and status bar.
//! Views never mutate state directly; they push [`Action`]s. Colors and sizes
//! come from [`crate::theme`].

use std::path::Path;

use eframe::egui::{
    self, Align, Align2, Button, CentralPanel, FontSelection, Frame, Label, Layout, Margin, Panel,
    Rect, RichText, Role, ScrollArea, Sense, Shape, TextStyle, TextWrapMode, Ui, WidgetInfo,
    WidgetText, pos2, vec2,
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
                self.palette_cache = Some((key, self.palette_items()));
            }
            if let Some((_, items)) = &self.palette_cache
                && let Some(action) = self.palette.show(ui.ctx(), items)
            {
                self.actions.push(action);
            }
        } else {
            self.palette_cache = None;
        }
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
                (
                    Icon::ArrowUp,
                    "Up",
                    "Alt+Up",
                    tab.path().parent().is_some(),
                    Action::Up,
                ),
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
                let open = self.palette.is_open();
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
                    ui.label(RichText::new("This folder is empty.").color(p.text_secondary));
                }
            });
            return;
        }

        let rows = listing.rows();
        let view = listing.view();
        let selected = tab.selected();
        let sort = tab.sort();
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

        let columns = [
            (SortColumn::Name, "Name"),
            (SortColumn::Modified, "Modified"),
            (SortColumn::Type, "Type"),
            (SortColumn::Size, "Size"),
        ];
        let output = table
            .header(ROW_HEIGHT, |mut header| {
                for (i, (column, label)) in columns.into_iter().enumerate() {
                    header.col(|ui| {
                        if i == 0 {
                            ui.add_space(30.0);
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
                    table_row.col(|ui| {
                        background = Some((ui.painter().clone(), ui.painter().add(Shape::Noop)));
                        ui.add_space(8.0);
                        let icon = match row.entry.kind {
                            EntryKind::Directory => Icon::Folder,
                            EntryKind::Symlink => Icon::Link,
                            EntryKind::File | EntryKind::Other => {
                                Icon::File(FileKind::from_name(&row.name))
                            }
                        };
                        icons::inline(ui, icon, p.icon);
                        ui.add_space(2.0);
                        let color = if row.entry.hidden {
                            p.text_secondary
                        } else {
                            p.text
                        };
                        ui.add(Label::new(RichText::new(&row.name).color(color)).truncate());
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
                    let is_selected = selected == Some(index);
                    if let Some((painter, slot)) = background {
                        let row = Rect::from_x_y_ranges(row_x, response.rect.y_range());
                        painter.set(slot, row_highlight(p, row, is_selected, response.hovered()));
                    }
                    if response.double_clicked() {
                        actions.push(Action::OpenRow(index));
                    } else if response.clicked() {
                        actions.push(Action::Select(Some(index)));
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
                if let Some(row) = tab.selected_row() {
                    let detail = if row.is_dir() {
                        String::new()
                    } else {
                        format!("  {}", row.size_label)
                    };
                    ui.label(small(format!("· 1 item selected{detail}")));
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
