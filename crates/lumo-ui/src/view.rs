//! Drawing: tab strip, toolbar, sidebar, file table and status bar.
//! Views never mutate state directly; they push [`Action`]s.

use std::path::Path;

use eframe::egui::{
    Align, Align2, Button, CentralPanel, Context, FontData, FontFamily, Label, Layout, Panel, Rect,
    RichText, Role, ScrollArea, Sense, TextStyle, Ui, WidgetInfo,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    vec2,
};
use egui_extras::{Column, TableBuilder};
use lumo_core::EntryKind;

use crate::app::{Action, LumoApp};
use crate::icons::{self, Icon};
use crate::listing::{LoadState, SortColumn};

const ROW_HEIGHT: f32 = 24.0;
const TOOL_BUTTON: f32 = 28.0;
const MAX_TAB_TITLE: usize = 28;

/// Uses the platform UI font (Segoe UI on Windows) ahead of egui's defaults,
/// which stay as fallback for missing glyphs.
pub fn install_fonts(ctx: &Context) {
    if let Some(bytes) = lumo_platform::ui_font() {
        ctx.add_font(FontInsert {
            name: "system-ui".to_owned(),
            data: FontData::from_owned(bytes),
            families: vec![InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            }],
        });
    }
}

pub fn apply_style(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        // Clicks on file names should select rows, not start text selection.
        style.interaction.selectable_labels = false;
        style.spacing.button_padding = vec2(6.0, 3.0);
        for font in style.text_styles.values_mut() {
            font.size += 1.0;
        }
    });
}

impl LumoApp {
    pub(crate) fn draw(&mut self, ui: &mut Ui) {
        Panel::top("lumo-tabs").show(ui, |ui| self.tab_strip(ui));
        Panel::top("lumo-toolbar").show(ui, |ui| self.toolbar(ui));
        Panel::bottom("lumo-status").show(ui, |ui| self.status_bar(ui));
        Panel::left("lumo-sidebar")
            .resizable(true)
            .default_size(200.0)
            .size_range(140.0..=420.0)
            .show(ui, |ui| self.sidebar(ui));
        CentralPanel::default_margins().show(ui, |ui| self.file_table(ui));
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        ScrollArea::horizontal()
            .id_salt("tab-strip")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let title = truncate(&tab.title(), MAX_TAB_TITLE);
                        let response = ui
                            .add(Button::selectable(i == self.active, title))
                            .on_hover_text(tab.path().display().to_string());
                        if response.clicked() {
                            self.actions.push(Action::ActivateTab(i));
                        }
                        if response.middle_clicked() {
                            self.actions.push(Action::CloseTab(i));
                        }
                        if icons::button(ui, Icon::Close, "Close tab", 18.0, true)
                            .on_hover_text("Close tab (Ctrl+W)")
                            .clicked()
                        {
                            self.actions.push(Action::CloseTab(i));
                        }
                        ui.add_space(4.0);
                    }
                    if ui.button("+").on_hover_text("New tab (Ctrl+T)").clicked() {
                        self.actions.push(Action::NewTab);
                    }
                });
            });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let tab = &self.tabs[self.active];
            let history = tab.history();
            let buttons = [
                (
                    Icon::Back,
                    "Back",
                    "Alt+Left",
                    history.can_go_back(),
                    Action::Back,
                ),
                (
                    Icon::Forward,
                    "Forward",
                    "Alt+Right",
                    history.can_go_forward(),
                    Action::Forward,
                ),
                (
                    Icon::Up,
                    "Up",
                    "Alt+Up",
                    tab.path().parent().is_some(),
                    Action::Up,
                ),
                (Icon::Refresh, "Refresh", "F5", true, Action::Refresh),
            ];
            for (icon, name, key, enabled, action) in buttons {
                let hint = format!("{name} ({key})");
                if icons::button(ui, icon, name, TOOL_BUTTON, enabled)
                    .on_hover_text(&hint)
                    .clicked()
                {
                    self.actions.push(action);
                }
            }
            let current = self.tabs[self.active].path().to_path_buf();
            if let Some(action) = self.address.show(ui, &current, &self.recent) {
                self.actions.push(action);
            }
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let current = self.tabs[self.active].path().to_path_buf();
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Quick access").strong());
            for folder in &self.quick_access {
                nav_item(
                    ui,
                    Icon::Folder,
                    folder.label,
                    &folder.path,
                    &current,
                    &mut self.actions,
                );
            }
            ui.separator();
            ui.label(RichText::new("This PC").strong());
            for drive in &self.drives {
                let label = drive.display().to_string();
                nav_item(ui, Icon::Drive, &label, drive, &current, &mut self.actions);
            }
        });
    }

    fn file_table(&mut self, ui: &mut Ui) {
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
                    ui.weak("This folder is empty.");
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

        // One table id for every tab: egui keeps per-row state for each table
        // id it has seen, so per-tab ids leaked memory as tabs were closed.
        let mut table = TableBuilder::new(ui)
            .id_salt("file-table")
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::initial(340.0).at_least(120.0).clip(true))
            .column(Column::initial(150.0).at_least(80.0).clip(true))
            .column(Column::initial(120.0).at_least(60.0).clip(true))
            .column(Column::remainder().at_least(80.0).clip(true));
        if scroll_requested && let Some(pos) = selected.and_then(|row| listing.view_position(row)) {
            table = table.scroll_to_row(pos, None);
        } else if let Some(offset) = restore_offset {
            table = table.vertical_scroll_offset(offset);
        }

        let columns = [
            (SortColumn::Name, "Name"),
            (SortColumn::Modified, "Date modified"),
            (SortColumn::Type, "Type"),
            (SortColumn::Size, "Size"),
        ];
        let output = table
            .header(ROW_HEIGHT, |mut header| {
                for (column, label) in columns {
                    header.col(|ui| {
                        let text = RichText::new(label).strong();
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
                            let color = ui.visuals().weak_text_color();
                            icons::paint(ui.painter(), rect, icon, color, ui.visuals());
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, view.len(), |mut table_row| {
                    let index = view[table_row.index()];
                    let row = &rows[index];
                    table_row.set_selected(selected == Some(index));
                    table_row.col(|ui| {
                        icons::inline(
                            ui,
                            match row.entry.kind {
                                EntryKind::Directory => Icon::Folder,
                                EntryKind::Symlink => Icon::Link,
                                EntryKind::File | EntryKind::Other => Icon::File,
                            },
                        );
                        let mut name = RichText::new(&row.name);
                        if row.entry.hidden {
                            name = name.weak();
                        }
                        ui.add(Label::new(name).truncate());
                    });
                    table_row.col(|ui| {
                        ui.label(&row.date_label);
                    });
                    table_row.col(|ui| {
                        ui.label(&row.type_label);
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(8.0);
                            ui.label(&row.size_label);
                        });
                    });
                    let response = table_row.response();
                    if response.double_clicked() {
                        actions.push(Action::OpenRow(index));
                    } else if response.clicked() {
                        actions.push(Action::Select(Some(index)));
                    }
                });
            });
        self.tabs[self.active].set_scroll_offset(output.state.clamped_offset().y);
        self.table_shown_for = Some(shown);
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let tab = &self.tabs[self.active];
            if let Some(listing) = tab.listing() {
                let count = listing.view().len();
                ui.label(format!("{count} item{}", if count == 1 { "" } else { "s" }));
                if listing.is_loading() {
                    ui.spinner();
                }
                if listing.unreadable() > 0 {
                    ui.weak(format!("· {} unreadable", listing.unreadable()));
                }
                if let Some(row) = tab.selected_row() {
                    let detail = if row.is_dir() {
                        String::new()
                    } else {
                        format!("  {}", row.size_label)
                    };
                    ui.label(format!("· 1 item selected{detail}"));
                }
            }
            if let Some(message) = &self.status {
                ui.colored_label(ui.visuals().error_fg_color, message);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut show_hidden = self.show_hidden;
                if ui
                    .checkbox(&mut show_hidden, "Hidden items")
                    .on_hover_text("Show hidden files (Ctrl+H)")
                    .changed()
                {
                    self.actions.push(Action::ToggleHidden);
                }
            });
        });
    }
}

/// A full-width sidebar entry: icon + label, highlighted when it is the
/// current folder.
fn nav_item(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    path: &Path,
    current: &Path,
    actions: &mut Vec<Action>,
) {
    let selected = current == path;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT + 2.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }
    let icon_rect = Rect::from_center_size(rect.left_center() + vec2(16.0, 0.0), vec2(16.0, 16.0));
    icons::paint(
        ui.painter(),
        icon_rect,
        icon,
        visuals.text_color(),
        ui.visuals(),
    );
    ui.painter().text(
        rect.left_center() + vec2(32.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        TextStyle::Button.resolve(ui.style()),
        visuals.text_color(),
    );

    let response =
        response.on_hover_text(format!("{}\nMiddle-click: open in new tab", path.display()));
    if response.clicked() {
        actions.push(Action::Navigate(path.to_path_buf()));
    }
    if response.middle_clicked() {
        actions.push(Action::OpenInNewTab(path.to_path_buf()));
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max_chars - 1).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncates_long_titles_with_ellipsis() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }
}
