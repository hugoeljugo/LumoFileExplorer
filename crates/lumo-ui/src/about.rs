//! Version info, the About window, "Report a problem" and crash reports.
//!
//! Nothing here sends data anywhere: crash reports are written to a local
//! file and "Report a problem" only opens a pre-filled GitHub page in the
//! browser, which the user can edit or abandon.

use std::path::{Path, PathBuf};

use eframe::egui::{self, CollapsingHeader, Context, Id, Margin, RichText, ScrollArea, vec2};

use crate::app::{Action, LumoApp};
use crate::theme::{self, Edge, Palette};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git commit (with `-dirty` for uncommitted changes), from `build.rs`.
pub const COMMIT: &str = env!("LUMO_COMMIT");
pub const REPO: &str = "https://github.com/hugoeljugo/LumoFileExplorer";

const LUMO_LICENSE: &str = include_str!("../../../LICENSE");
const PLEX_LICENSE: &str = include_str!("../assets/fonts/IBMPlexSans-OFL.txt");
const LILEX_LICENSE: &str = include_str!("../assets/fonts/Lilex-OFL.txt");

/// A new-issue page on GitHub, pre-filled with the version and system.
pub fn report_url(os: &str) -> String {
    let body = format!(
        "**Lumo:** {VERSION} ({COMMIT})\n**System:** {os}\n\n\
         **What happened?**\n\n\n\
         **What did you expect instead?**\n\n\n\
         **Steps to reproduce**\n1. \n\n\
         <!-- If Lumo crashed, attach the crash report file it showed you. -->\n"
    );
    format!("{REPO}/issues/new?body={}", percent_encode(&body))
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The text of a crash report.
pub fn crash_report(message: &str, location: &str, thread: &str, os: &str) -> String {
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z");
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "Lumo crash report\n\
         =================\n\
         Version: {VERSION} ({COMMIT})\n\
         System:  {os}\n\
         Time:    {time}\n\
         Thread:  {thread}\n\n\
         {message}\n\
         at {location}\n\n\
         Backtrace:\n{backtrace}\n"
    )
}

/// Writes a report file for every panic. A crash on the main thread also
/// shows a dialog saying where the report is, since release builds have no
/// console and would otherwise just vanish. Panics on worker threads don't
/// close Lumo (the job reports an error), so they are only logged.
pub fn install_crash_handler(dir: Option<PathBuf>) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_owned());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("unnamed");
        let report = crash_report(
            &message,
            &location,
            thread,
            &lumo_platform::os_description(),
        );
        let saved = dir.as_deref().and_then(|dir| save_report(dir, &report));
        if thread == "main" {
            let where_ = saved.map_or_else(
                || "(the report couldn't be saved)".to_owned(),
                |path| format!("A crash report was saved to:\n{}", path.display()),
            );
            lumo_platform::show_fatal_error(
                "Lumo crashed",
                &format!(
                    "Sorry — Lumo ran into a problem and has to close.\n\n{where_}\n\n\
                     Please send that file to whoever gave you Lumo, or attach it to an \
                     issue at {REPO}/issues"
                ),
            );
        }
        default_hook(info);
    }));
}

fn save_report(dir: &Path, report: &str) -> Option<PathBuf> {
    let name = chrono::Local::now()
        .format("crash-%Y%m%d-%H%M%S.txt")
        .to_string();
    let path = dir.join(name);
    std::fs::create_dir_all(dir).ok()?;
    std::fs::write(&path, report).ok()?;
    Some(path)
}

impl LumoApp {
    pub(crate) fn about_window(&mut self, ctx: &Context, p: &Palette) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        let mut actions = Vec::new();
        let data_dir = self.data_dir();
        egui::Window::new(RichText::new("About").color(p.text))
            .id(Id::unique("lumo-about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .default_pos(self.table_rect.map_or(egui::pos2(240.0, 120.0), |r| {
                r.center() - vec2(210.0, 200.0)
            }))
            .frame(crate::view::overlay_frame(p).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                let title = ui.label(
                    RichText::new("Lumo")
                        .font(theme::semibold(26.0))
                        .color(p.text),
                );
                let underline = egui::Rect::from_min_size(
                    title.rect.left_bottom() + vec2(0.0, 2.0),
                    vec2(title.rect.width(), 10.0),
                );
                ui.painter().add(theme::lumen(p, underline, Edge::Top, 8.0));
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("Version {VERSION} (build {COMMIT})")).color(p.text),
                );
                ui.label(RichText::new(lumo_platform::os_description()).color(p.text_secondary));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Early alpha — thank you for testing! Your feedback shapes Lumo.",
                    )
                    .color(p.text_secondary),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Report a problem").clicked() {
                        actions.push(Action::ReportProblem);
                    }
                    if ui.button("GitHub").on_hover_text(REPO).clicked() {
                        actions.push(Action::OpenUrl(REPO.to_owned()));
                    }
                    if let Some(dir) = &data_dir
                        && ui
                            .button("Open data folder")
                            .on_hover_text(format!(
                                "Settings, open tabs and crash reports\n{}",
                                dir.display()
                            ))
                            .clicked()
                    {
                        actions.push(Action::Navigate(dir.clone()));
                    }
                });
                ui.add_space(10.0);
                CollapsingHeader::new(RichText::new("Licenses").color(p.text_secondary))
                    .id_salt("lumo-about-licenses")
                    .show(ui, |ui| {
                        ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                            for (title, text) in [
                                ("Lumo — MIT License", LUMO_LICENSE),
                                (
                                    "IBM Plex Sans font — SIL Open Font License 1.1",
                                    PLEX_LICENSE,
                                ),
                                ("Lilex font — SIL Open Font License 1.1", LILEX_LICENSE),
                            ] {
                                ui.label(
                                    RichText::new(title)
                                        .font(theme::semibold(13.0))
                                        .color(p.text),
                                );
                                ui.label(RichText::new(text).small().color(p.text_secondary));
                                ui.add_space(8.0);
                            }
                        });
                    });
            });
        if !open {
            actions.push(Action::CloseAbout);
        }
        self.actions.extend(actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_only_unreserved_characters() {
        assert_eq!(percent_encode("a-Z_0.~"), "a-Z_0.~");
        assert_eq!(percent_encode("a b\n*"), "a%20b%0A%2A");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn report_url_carries_version_and_system() {
        let url = report_url("Windows 11 (build 26200)");
        assert!(url.starts_with(&format!("{REPO}/issues/new?body=")));
        assert!(url.contains(&percent_encode(VERSION)));
        assert!(url.contains("Windows%2011"));
    }

    #[test]
    fn crash_report_includes_what_a_developer_needs() {
        let report = crash_report(
            "index out of bounds",
            "src/tab.rs:10:5",
            "main",
            "Windows 11",
        );
        for needle in [
            VERSION,
            COMMIT,
            "Windows 11",
            "main",
            "index out of bounds",
            "src/tab.rs:10:5",
            "Backtrace",
        ] {
            assert!(report.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn reports_are_saved_with_a_timestamped_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_report(dir.path(), "report").unwrap();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("crash-")
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "report");
    }
}
