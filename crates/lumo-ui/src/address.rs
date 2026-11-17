//! The address bar: clickable breadcrumbs that turn into a text field with
//! autocompletion when clicked (or on Ctrl+L / Alt+D / F4).

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Align2, Area, Button, CornerRadius, Frame, Key, Layout, Modifiers, Order,
    RichText, Role, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, WidgetInfo, vec2,
};

use crate::app::Action;
use crate::complete::Completer;
use crate::icons::{self, Icon};
use crate::theme::{self, CONTROL_RADIUS, Palette, TOOLBAR_CONTROL as HEIGHT};

const MAX_SUGGESTIONS: usize = 8;
/// With more segments than this, the middle ones collapse into "…".
const MAX_CRUMBS: usize = 5;

#[derive(Debug, Default)]
pub struct AddressBar {
    editing: bool,
    text: String,
    suggestions: Vec<String>,
    highlighted: Option<usize>,
    request_focus: bool,
    popup_rect: Option<egui::Rect>,
    completer: Completer,
}

impl AddressBar {
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn start_editing(&mut self, current: &Path) {
        self.editing = true;
        self.text = current.display().to_string();
        self.suggestions.clear();
        self.highlighted = None;
        self.request_focus = true;
    }

    fn stop_editing(&mut self) {
        self.editing = false;
        self.suggestions.clear();
        self.highlighted = None;
        self.popup_rect = None;
    }

    pub fn show(&mut self, ui: &mut Ui, current: &Path, recent: &[PathBuf]) -> Option<Action> {
        if self.editing {
            self.show_editor(ui, current, recent)
        } else {
            self.show_breadcrumbs(ui, current)
        }
    }

    fn show_breadcrumbs(&mut self, ui: &mut Ui, current: &Path) -> Option<Action> {
        let (rect, background) =
            ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::click());
        let background = background.on_hover_text("Click to type a path (Ctrl+L)");
        background.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Edit address"));
        let p = Palette::of(ui.ctx());
        // Zed-style: breadcrumbs sit flat on the toolbar; a faint field
        // appears on hover to show the bar is editable.
        if background.hovered() {
            let radius = CornerRadius::same(CONTROL_RADIUS);
            ui.painter().rect_filled(rect, radius, p.hover);
            ui.painter()
                .rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }

        let mut action = None;
        let crumbs = collapse(breadcrumbs(current));
        let last = crumbs.len().saturating_sub(1);
        // Added after the background, so the crumb buttons win hit-testing.
        let mut crumbs_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect.shrink2(vec2(4.0, 2.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        crumbs_ui.spacing_mut().item_spacing.x = 0.0;
        crumbs_ui.spacing_mut().button_padding = vec2(6.0, 3.0);
        for (i, crumb) in crumbs.into_iter().enumerate() {
            if i > 0 {
                let (chevron, _) = crumbs_ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                let icon_rect = egui::Rect::from_center_size(chevron.center(), vec2(10.0, 10.0));
                icons::paint(crumbs_ui.painter(), icon_rect, Icon::ChevronRight, p.icon);
            }
            match crumb {
                Some((label, path)) => {
                    // The current folder reads strongest; ancestors recede.
                    let color = if i == last { p.text } else { p.text_secondary };
                    let response = crumbs_ui
                        .add(Button::new(RichText::new(label).color(color)))
                        .on_hover_text(path.display().to_string());
                    if response.clicked() {
                        action = Some(Action::Navigate(path));
                    }
                }
                None => {
                    if crumbs_ui
                        .add(Button::new("…"))
                        .on_hover_text("Show full path")
                        .clicked()
                    {
                        self.start_editing(current);
                    }
                }
            }
        }
        if background.clicked() {
            self.start_editing(current);
        }
        action
    }

    fn show_editor(&mut self, ui: &mut Ui, current: &Path, recent: &[PathBuf]) -> Option<Action> {
        let id = ui.make_persistent_id("address-edit");
        if ui.memory(|m| m.has_focus(id)) {
            self.handle_suggestion_keys(ui, current, recent);
        }

        let response = ui.add_sized(
            vec2(ui.available_width(), HEIGHT),
            TextEdit::singleline(&mut self.text)
                .id(id)
                .font(theme::mono(13.0))
                .hint_text("Type a path")
                // Keep Tab for accepting a suggestion instead of moving focus.
                .lock_focus(true)
                .align(Align2::LEFT_CENTER),
        );
        let starting = std::mem::take(&mut self.request_focus);
        if starting {
            response.request_focus();
        }
        if response.changed() {
            self.refresh_suggestions(current, recent);
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let target = self
                .highlighted
                .and_then(|i| self.suggestions.get(i).cloned())
                .unwrap_or_else(|| self.text.clone());
            self.stop_editing();
            return Some(Action::OpenTyped(target));
        }

        if let Some(picked) = self.show_suggestions(ui, response.rect) {
            self.stop_editing();
            return Some(Action::OpenTyped(picked));
        }

        // Escape or clicking elsewhere: leave edit mode, unless the mouse is
        // pressed on the suggestion list (that click lands next frame).
        let pressing_popup = ui.input(|i| {
            i.pointer.primary_down()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| self.popup_rect.is_some_and(|r| r.contains(p)))
        });
        if !starting && !response.has_focus() && !pressing_popup {
            self.stop_editing();
        }
        None
    }

    fn handle_suggestion_keys(&mut self, ui: &Ui, current: &Path, recent: &[PathBuf]) {
        let (down, up, tab) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        let count = self.suggestions.len();
        if count > 0 && down {
            self.highlighted = Some(self.highlighted.map_or(0, |i| (i + 1) % count));
        }
        if count > 0 && up {
            self.highlighted = Some(
                self.highlighted
                    .map_or(count - 1, |i| (i + count - 1) % count),
            );
        }
        if tab && let Some(pick) = self.suggestions.get(self.highlighted.unwrap_or(0)).cloned() {
            self.text = pick;
            self.text.push(std::path::MAIN_SEPARATOR);
            self.refresh_suggestions(current, recent);
            // Moving the cursor to the end needs the text-edit state; egui
            // keeps the cursor index, so place it explicitly.
            let id = ui.make_persistent_id("address-edit");
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
                let end = egui::text::CCursor::new(self.text.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), id);
            }
        }
    }

    fn refresh_suggestions(&mut self, current: &Path, recent: &[PathBuf]) {
        self.suggestions = self
            .completer
            .suggest(&self.text, current, recent, MAX_SUGGESTIONS);
        self.highlighted = None;
    }

    fn show_suggestions(&mut self, ui: &Ui, anchor: egui::Rect) -> Option<String> {
        if self.suggestions.is_empty() {
            self.popup_rect = None;
            return None;
        }
        let mut picked = None;
        let area = Area::new(ui.make_persistent_id("address-suggestions"))
            .order(Order::Foreground)
            .fixed_pos(anchor.left_bottom())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(anchor.width());
                    ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                        for (i, suggestion) in self.suggestions.iter().enumerate() {
                            let selected = self.highlighted == Some(i);
                            if ui
                                .add(Button::selectable(selected, suggestion.as_str()))
                                .clicked()
                            {
                                picked = Some(suggestion.clone());
                            }
                        }
                    });
                });
            });
        self.popup_rect = Some(area.response.rect);
        picked
    }
}

/// `(label, path)` for each ancestor of `path`, root first.
fn breadcrumbs(path: &Path) -> Vec<(String, PathBuf)> {
    let mut crumbs: Vec<_> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| {
            let label = match p.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => p
                    .display()
                    .to_string()
                    .trim_end_matches(['\\', '/'])
                    .to_owned(),
            };
            let label = if label.is_empty() {
                "/".to_owned()
            } else {
                label
            };
            (label, p.to_path_buf())
        })
        .collect();
    crumbs.reverse();
    crumbs
}

/// Keeps the root and the last few segments; `None` marks the collapsed "…".
fn collapse(crumbs: Vec<(String, PathBuf)>) -> Vec<Option<(String, PathBuf)>> {
    if crumbs.len() <= MAX_CRUMBS {
        return crumbs.into_iter().map(Some).collect();
    }
    let tail = crumbs.len() - (MAX_CRUMBS - 2);
    let mut out = Vec::with_capacity(MAX_CRUMBS);
    let mut iter = crumbs.into_iter();
    out.push(iter.next());
    out.push(None);
    out.extend(iter.skip(tail - 1).map(Some));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(crumbs: &[Option<(String, PathBuf)>]) -> Vec<&str> {
        crumbs
            .iter()
            .map(|c| c.as_ref().map_or("…", |(l, _)| l.as_str()))
            .collect()
    }

    #[test]
    #[cfg(windows)]
    fn windows_breadcrumbs() {
        let crumbs = breadcrumbs(Path::new("C:\\Users\\Hugo"));
        let got: Vec<_> = crumbs
            .iter()
            .map(|(l, p)| (l.as_str(), p.clone()))
            .collect();
        assert_eq!(
            got,
            [
                ("C:", PathBuf::from("C:\\")),
                ("Users", PathBuf::from("C:\\Users")),
                ("Hugo", PathBuf::from("C:\\Users\\Hugo")),
            ]
        );
    }

    #[test]
    #[cfg(unix)]
    fn unix_breadcrumbs() {
        let crumbs = breadcrumbs(Path::new("/home/hugo"));
        let got: Vec<_> = crumbs.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(got, ["/", "home", "hugo"]);
    }

    #[test]
    fn long_paths_collapse_the_middle() {
        let path: PathBuf = ["r", "a", "b", "c", "d", "e"].iter().collect();
        let collapsed = collapse(breadcrumbs(&path));
        assert_eq!(labels(&collapsed), ["r", "…", "c", "d", "e"]);
    }

    #[test]
    fn short_paths_are_not_collapsed() {
        let path: PathBuf = ["r", "a", "b"].iter().collect();
        assert_eq!(labels(&collapse(breadcrumbs(&path))), ["r", "a", "b"]);
    }
}
