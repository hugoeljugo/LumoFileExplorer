//! The command palette (Ctrl+K / Ctrl+P): one fuzzy search box over
//! commands, open tabs, places and the current folder's items. Typing `>`
//! first limits it to commands.
//!
//! The same component is Lumo's right-click menu ("search menu"): opened at
//! the cursor with only the commands that fit what was clicked.

use eframe::egui::{
    Align2, Area, Context, CornerRadius, FontSelection, Frame, Id, Key, Margin, Modifiers, Order,
    Pos2, Rect, RichText, Role, ScrollArea, Sense, Stroke, TextEdit, TextFormat, TextStyle, Ui,
    WidgetInfo, WidgetText, pos2, text::LayoutJob, vec2,
};

use crate::app::Action;
use crate::fuzzy::{Match, Query};
use crate::icons::{self, Icon};
use crate::theme::{self, Edge, Palette as Colors};

/// At most this many results are ranked and shown.
const MAX_RESULTS: usize = 60;
const ROW_HEIGHT: f32 = 32.0;
const MAX_WIDTH: f32 = 640.0;
const MENU_WIDTH: f32 = 320.0;
const MAX_MENU_HEIGHT: f32 = 330.0;
const MAX_LIST_HEIGHT: f32 = 380.0;

/// What a palette entry is; also its rank when scores tie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Command,
    Tab,
    Place,
    Recent,
    Entry,
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub title: String,
    /// Secondary text: a path or a short description.
    pub detail: String,
    pub icon: Icon,
    pub shortcut: Option<&'static str>,
    pub kind: ItemKind,
    pub action: Action,
}

#[derive(Debug, Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
    request_focus: bool,
    scroll_to_selected: bool,
    /// Ranked `(item index, match)` for the current query.
    results: Vec<(usize, Match)>,
    /// Item count the results were computed for; a change re-ranks.
    ranked_len: Option<usize>,
    /// Open as the right-click menu at this position instead of the top.
    menu_at: Option<Pos2>,
}

impl CommandPalette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        *self = Self {
            open: true,
            request_focus: true,
            ..Self::default()
        };
    }

    /// Opens with `query` already typed (keys pressed in the same frame as
    /// Ctrl+K would otherwise be lost).
    pub fn open_with_query(&mut self, query: String) {
        self.open();
        self.query = query;
    }

    /// Opens as the right-click search menu at `pos`.
    pub fn open_menu(&mut self, pos: Pos2) {
        self.open();
        self.menu_at = Some(pos);
    }

    pub fn is_menu(&self) -> bool {
        self.open && self.menu_at.is_some()
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Draws the palette over everything; returns the chosen action.
    pub fn show(&mut self, ctx: &Context, items: &[PaletteItem]) -> Option<Action> {
        if !self.open {
            return None;
        }
        if self.ranked_len != Some(items.len()) {
            self.rerank(items);
        }

        // Keys are consumed before the text field sees them.
        let (escape, enter, down, up) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
            )
        });
        if escape {
            self.close();
            return None;
        }
        let count = self.results.len();
        if count > 0 && down {
            self.selected = (self.selected + 1) % count;
            self.scroll_to_selected = true;
        }
        if count > 0 && up {
            self.selected = (self.selected + count - 1) % count;
            self.scroll_to_selected = true;
        }
        if enter {
            return self.choose(items, self.selected);
        }

        let colors = Colors::of(ctx);
        let menu = self.menu_at.is_some();
        let width = if menu {
            MENU_WIDTH
        } else {
            (ctx.content_rect().width() - 80.0).clamp(280.0, MAX_WIDTH)
        };
        let mut clicked = None;
        let area = Area::new(Id::unique("command-palette"))
            .order(Order::Foreground)
            .constrain(true);
        let area = match self.menu_at {
            Some(pos) => area.fixed_pos(pos),
            None => area.anchor(Align2::CENTER_TOP, vec2(0.0, 64.0)),
        };
        let area = area.show(ctx, |ui| {
            Frame::popup(ui.style())
                .fill(colors.elevated)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_width(width);
                    self.search_field(ui, &colors, items);
                    let line_y = ui.cursor().top();
                    ui.painter().add(theme::hairline(
                        &colors,
                        pos2(ui.max_rect().left(), line_y),
                        ui.max_rect().right(),
                    ));
                    clicked = self.result_list(ui, &colors, items);
                    if !menu {
                        self.footer(ui, &colors);
                    }
                });
        });

        // Clicking anywhere outside closes it, like Escape.
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.response.rect.contains(p))
        });
        if let Some(index) = clicked {
            return self.choose(items, index);
        }
        if clicked_outside {
            self.close();
        }
        None
    }

    fn choose(&mut self, items: &[PaletteItem], position: usize) -> Option<Action> {
        let (index, _) = self.results.get(position)?;
        let action = items.get(*index)?.action.clone();
        self.close();
        Some(action)
    }

    fn rerank(&mut self, items: &[PaletteItem]) {
        self.results = rank(&self.query, items, MAX_RESULTS);
        self.ranked_len = Some(items.len());
        self.selected = 0;
        self.scroll_to_selected = true;
    }

    fn search_field(&mut self, ui: &mut Ui, colors: &Colors, items: &[PaletteItem]) {
        Frame::new()
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::inline(ui, Icon::Search, colors.icon);
                    let response = ui.add(
                        TextEdit::singleline(&mut self.query)
                            .id(Id::unique("command-palette-query"))
                            .frame(Frame::NONE)
                            .hint_text(if self.menu_at.is_some() {
                                "Type to filter…"
                            } else {
                                "Search commands, tabs, places and files…"
                            })
                            .desired_width(f32::INFINITY)
                            .lock_focus(true),
                    );
                    if std::mem::take(&mut self.request_focus) {
                        response.request_focus();
                    }
                    if response.changed() {
                        self.rerank(items);
                    }
                });
            });
    }

    /// Returns the position of a clicked result.
    fn result_list(
        &mut self,
        ui: &mut Ui,
        colors: &Colors,
        items: &[PaletteItem],
    ) -> Option<usize> {
        if self.results.is_empty() {
            Frame::new()
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.label(RichText::new("No matches").color(colors.text_secondary));
                });
            return None;
        }
        let mut clicked = None;
        let scroll_to = std::mem::take(&mut self.scroll_to_selected);
        let max_height = if self.menu_at.is_some() {
            MAX_MENU_HEIGHT
        } else {
            MAX_LIST_HEIGHT
        };
        ScrollArea::vertical()
            .max_height(max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (position, (index, matched)) in self.results.iter().enumerate() {
                    let item = &items[*index];
                    let selected = position == self.selected;
                    let (rect, response) = ui.allocate_exact_size(
                        vec2(ui.available_width(), ROW_HEIGHT),
                        Sense::click(),
                    );
                    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &item.title));
                    if selected && scroll_to {
                        ui.scroll_to_rect(rect, None);
                    }
                    if response.hovered() && ui.input(|i| i.pointer.is_moving()) {
                        self.selected = position;
                    }
                    if response.clicked() {
                        clicked = Some(position);
                    }
                    if ui.is_rect_visible(rect) {
                        paint_row(
                            ui,
                            colors,
                            rect,
                            item,
                            matched,
                            selected,
                            response.hovered(),
                        );
                    }
                }
            });
        clicked
    }

    fn footer(&self, ui: &mut Ui, colors: &Colors) {
        Frame::new()
            .inner_margin(Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "↑↓ to move  ·  Enter to run  ·  Esc to close  ·  > for commands only",
                    )
                    .small()
                    .color(colors.text_secondary),
                );
            });
    }
}

fn paint_row(
    ui: &Ui,
    colors: &Colors,
    rect: Rect,
    item: &PaletteItem,
    matched: &Match,
    selected: bool,
    hovered: bool,
) {
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0.0, colors.selected);
        painter.add(theme::lumen(colors, rect, Edge::Left, 120.0));
    } else if hovered {
        painter.rect_filled(rect, 0.0, colors.hover);
    }
    let icon_color = if selected { colors.accent } else { colors.icon };
    let icon_rect = Rect::from_center_size(rect.left_center() + vec2(22.0, 0.0), vec2(16.0, 16.0));
    icons::paint(painter, icon_rect, item.icon, icon_color);

    // Shortcut on the right, then the title and detail in the space left.
    let mut right = rect.right() - 14.0;
    if let Some(shortcut) = item.shortcut {
        let galley = painter.layout_no_wrap(
            shortcut.to_owned(),
            theme::mono(11.5),
            colors.text_secondary,
        );
        right -= galley.size().x;
        painter.galley(
            pos2(right, rect.center().y - galley.size().y / 2.0),
            galley,
            colors.text_secondary,
        );
        right -= 16.0;
    }

    let left = rect.left() + 40.0;
    let title = highlighted(ui, &item.title, &matched.indices, colors);
    let title = WidgetText::from(title).into_galley(
        ui,
        Some(eframe::egui::TextWrapMode::Truncate),
        (right - left).max(0.0),
        FontSelection::Default,
    );
    let title_width = title.size().x;
    painter.galley(
        pos2(left, rect.center().y - title.size().y / 2.0),
        title,
        colors.text,
    );

    let detail_left = left + title_width + 10.0;
    if !item.detail.is_empty() && detail_left < right {
        let detail = WidgetText::from(
            RichText::new(&item.detail)
                .small()
                .color(colors.text_secondary),
        )
        .into_galley(
            ui,
            Some(eframe::egui::TextWrapMode::Truncate),
            right - detail_left,
            FontSelection::Default,
        );
        painter.galley(
            pos2(detail_left, rect.center().y - detail.size().y / 2.0),
            detail,
            colors.text_secondary,
        );
    }
}

/// The title with matched characters drawn in the accent color.
fn highlighted(ui: &Ui, text: &str, indices: &[usize], colors: &Colors) -> LayoutJob {
    let font = TextStyle::Body.resolve(ui.style());
    let normal = TextFormat::simple(font.clone(), colors.text);
    let hit = TextFormat::simple(theme::semibold(font.size), colors.accent);
    let mut job = LayoutJob::default();
    let mut run = String::new();
    let mut run_is_hit = false;
    for (i, c) in text.chars().enumerate() {
        let is_hit = indices.binary_search(&i).is_ok();
        if is_hit != run_is_hit && !run.is_empty() {
            let format = if run_is_hit {
                hit.clone()
            } else {
                normal.clone()
            };
            job.append(&std::mem::take(&mut run), 0.0, format);
        }
        run_is_hit = is_hit;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, if run_is_hit { hit } else { normal });
    }
    job
}

/// Ranks `items` for `query`: best score first, ties broken by kind (commands
/// first) and then original order. A leading `>` keeps only commands.
pub fn rank(query: &str, items: &[PaletteItem], limit: usize) -> Vec<(usize, Match)> {
    let (commands_only, query) = match query.trim_start().strip_prefix('>') {
        Some(rest) => (true, rest),
        None => (false, query),
    };
    let query = Query::new(query);
    let mut ranked: Vec<(usize, Match)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| !commands_only || item.kind == ItemKind::Command)
        .filter_map(|(i, item)| query.matches(&item.title).map(|m| (i, m)))
        .collect();
    ranked.sort_by(|(a, ma), (b, mb)| {
        mb.score
            .cmp(&ma.score)
            .then(items[*a].kind.cmp(&items[*b].kind))
            .then(a.cmp(b))
    });
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, kind: ItemKind) -> PaletteItem {
        PaletteItem {
            title: title.to_owned(),
            detail: String::new(),
            icon: Icon::Plus,
            shortcut: None,
            kind,
            action: Action::Refresh,
        }
    }

    fn titles<'a>(items: &'a [PaletteItem], ranked: &[(usize, Match)]) -> Vec<&'a str> {
        ranked
            .iter()
            .map(|(i, _)| items[*i].title.as_str())
            .collect()
    }

    #[test]
    fn best_match_first() {
        let items = [
            item("Refresh", ItemKind::Command),
            item("report-final.pdf", ItemKind::Entry),
            item("rarefied.txt", ItemKind::Entry),
        ];
        let ranked = rank("rf", &items, 10);
        assert_eq!(titles(&items, &ranked)[0], "report-final.pdf");
    }

    #[test]
    fn empty_query_keeps_kind_order() {
        let items = [
            item("b.txt", ItemKind::Entry),
            item("Downloads", ItemKind::Place),
            item("New tab", ItemKind::Command),
        ];
        let ranked = rank("", &items, 10);
        assert_eq!(titles(&items, &ranked), ["New tab", "Downloads", "b.txt"]);
    }

    #[test]
    fn angle_bracket_limits_to_commands() {
        let items = [
            item("Refresh", ItemKind::Command),
            item("refs", ItemKind::Entry),
        ];
        let ranked = rank("> ref", &items, 10);
        assert_eq!(titles(&items, &ranked), ["Refresh"]);
    }

    #[test]
    fn respects_limit() {
        let items: Vec<_> = (0..100)
            .map(|i| item(&format!("file{i}"), ItemKind::Entry))
            .collect();
        assert_eq!(rank("file", &items, 7).len(), 7);
    }
}
