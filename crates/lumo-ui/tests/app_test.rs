//! End-to-end UI behavior, driven headlessly through egui_kittest.

mod common;

use std::path::MAIN_SEPARATOR;

use common::{CTRL, active_path, at, harness, selected_name, settle, tree, visible_names};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use lumo_ui::{Action, Config, SortColumn, Start};

#[test]
fn shows_folder_contents_folders_first() {
    let dir = tree(&["beta/", "alpha.txt", "Gamma.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    assert_eq!(visible_names(&h), ["beta", "alpha.txt", "Gamma.txt"]);
    h.get_by_label("alpha.txt");
    h.get_by_label("3 items");
}

#[test]
fn keyboard_opens_folder_and_back_reselects_it() {
    let dir = tree(&["beta/", "alpha.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.key_press(Key::ArrowDown);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("beta"));

    h.key_press(Key::Backspace);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path());
    assert_eq!(selected_name(&h).as_deref(), Some("beta"));

    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("beta"));

    h.key_press_modifiers(Modifiers::ALT, Key::ArrowUp);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path());
}

#[test]
fn double_click_opens_folder() {
    let dir = tree(&["beta/", "alpha.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("beta").click();
    h.step();
    h.get_by_label("beta").click();
    settle(&mut h);

    assert_eq!(active_path(&h), dir.path().join("beta"));
}

#[test]
fn single_click_selects() {
    let dir = tree(&["beta/", "alpha.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("alpha.txt").click();
    h.run();

    assert_eq!(selected_name(&h).as_deref(), Some("alpha.txt"));
    h.get_by_label_contains("1 item selected");
}

#[test]
fn tab_shortcuts() {
    let dir = tree(&["a/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    // New tabs open the home folder, which may take a moment to list.
    h.key_press_modifiers(CTRL, Key::T);
    settle(&mut h);
    assert_eq!((h.state().tabs().len(), h.state().active_index()), (2, 1));

    h.key_press_modifiers(CTRL | Modifiers::SHIFT, Key::Tab);
    settle(&mut h);
    assert_eq!(h.state().active_index(), 0);

    h.key_press_modifiers(CTRL, Key::Tab);
    settle(&mut h);
    assert_eq!(h.state().active_index(), 1);

    h.key_press_modifiers(CTRL, Key::Num1);
    settle(&mut h);
    assert_eq!(h.state().active_index(), 0);

    h.key_press_modifiers(CTRL, Key::W);
    settle(&mut h);
    assert_eq!(h.state().tabs().len(), 1);
    assert_ne!(
        h.state().active_tab().path(),
        dir.path(),
        "closed the first tab"
    );
}

#[test]
fn tabs_keep_their_own_location() {
    let dir = tree(&["a/", "b/"]);
    let mut h = harness(at(&dir.path().join("a")));
    settle(&mut h);
    h.state_mut()
        .queue(Action::OpenInNewTab(dir.path().join("b")));
    settle(&mut h);

    h.key_press_modifiers(CTRL, Key::Tab);
    settle(&mut h);

    assert_eq!(active_path(&h), dir.path().join("a"));
    assert_eq!(h.state().tabs()[1].path(), dir.path().join("b"));
}

#[test]
fn address_bar_navigates_to_typed_path() {
    let dir = tree(&["target/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.key_press_modifiers(CTRL, Key::L);
    h.run();
    assert!(h.state().is_editing_address());
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text(dir.path().join("target").display().to_string()));
    h.key_press(Key::Enter);
    settle(&mut h);

    assert!(!h.state().is_editing_address());
    assert_eq!(active_path(&h), dir.path().join("target"));
}

#[test]
fn address_bar_relative_path_and_escape() {
    let dir = tree(&["sub/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.key_press(Key::F4);
    h.run();
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text("sub".into()));
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().is_editing_address());
    assert_eq!(active_path(&h), dir.path(), "Escape cancels");

    h.key_press(Key::F4);
    h.run();
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text("sub".into()));
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("sub"));
}

#[test]
fn address_bar_autocompletes() {
    let dir = tree(&["Documents/", "Downloads/", "Music/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    let root = format!("{}{MAIN_SEPARATOR}", dir.path().display());

    h.key_press_modifiers(CTRL, Key::L);
    h.run();
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text(format!("{root}doc")));
    h.run();

    // Suggestions popup lists the matching folder.
    h.get_by_label(&format!("{root}Documents"));
    assert!(h.query_by_label(&format!("{root}Music")).is_none());

    // Tab accepts it, Enter goes there.
    h.key_press(Key::Tab);
    h.run();
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("Documents"));
}

#[test]
fn typing_a_missing_path_reports_it() {
    let dir = tree(&[]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.key_press_modifiers(CTRL, Key::L);
    h.run();
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text("does-not-exist".into()));
    h.key_press(Key::Enter);
    h.run();

    assert_eq!(active_path(&h), dir.path());
    assert!(h.state().status().is_some_and(|s| s.contains("Can't find")));
}

#[test]
fn breadcrumb_navigates_to_ancestor() {
    let dir = tree(&["inner/"]);
    let mut h = harness(at(&dir.path().join("inner")));
    settle(&mut h);

    let parent_name = dir
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    h.get_by_label(&parent_name).click();
    settle(&mut h);

    assert_eq!(active_path(&h), dir.path());
}

#[test]
fn clicking_column_headers_sorts() {
    let dir = tree(&["b.txt", "a.txt"]);
    std::fs::write(dir.path().join("b.txt"), "bigger").unwrap();
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("Size").click();
    h.run();
    assert_eq!(h.state().active_tab().sort().column, SortColumn::Size);
    assert_eq!(visible_names(&h), ["a.txt", "b.txt"]);

    h.get_by_label_contains("Size").click();
    h.run();
    assert!(!h.state().active_tab().sort().ascending);
    assert_eq!(visible_names(&h), ["b.txt", "a.txt"]);
}

#[test]
fn hidden_items_toggle() {
    let hidden = if cfg!(windows) {
        "secret.txt"
    } else {
        ".secret"
    };
    let dir = tree(&["shown.txt", hidden]);
    #[cfg(windows)]
    assert!(
        std::process::Command::new("attrib")
            .arg("+h")
            .arg(dir.path().join(hidden))
            .status()
            .unwrap()
            .success()
    );
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    assert_eq!(visible_names(&h), ["shown.txt"]);

    h.key_press_modifiers(CTRL, Key::H);
    h.run();
    assert_eq!(visible_names(&h).len(), 2);

    h.get_by_label("Hidden items").click();
    h.run();
    assert_eq!(visible_names(&h), ["shown.txt"]);
}

#[test]
fn type_ahead_selects_matching_name() {
    let dir = tree(&["apple.txt", "banana.txt", "cherry.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.event(Event::Text("ch".into()));
    h.run();

    assert_eq!(selected_name(&h).as_deref(), Some("cherry.txt"));
}

#[test]
fn session_survives_force_close() {
    let dir = tree(&["a/", "b/", "c/"]);
    let state = tempfile::tempdir().unwrap();
    let session_file = state.path().join("session.json");
    let config = Config {
        start: Start::Restore,
        isolated: true,
        session_file: Some(session_file.clone()),
    };

    let mut first = harness(config.clone());
    first
        .state_mut()
        .queue(Action::Navigate(dir.path().join("a")));
    settle(&mut first);
    first
        .state_mut()
        .queue(Action::OpenInNewTab(dir.path().join("b")));
    settle(&mut first);
    // No shutdown hook runs: dropping the harness is like killing the process.
    drop(first);

    let mut second = harness(config);
    settle(&mut second);
    let paths: Vec<_> = second
        .state()
        .tabs()
        .iter()
        .map(|t| t.path().to_path_buf())
        .collect();
    assert_eq!(paths, [dir.path().join("a"), dir.path().join("b")]);
    assert_eq!(second.state().active_index(), 1);
}

#[test]
fn restore_drops_folders_that_no_longer_exist() {
    let dir = tree(&["keep/", "gone/"]);
    let state = tempfile::tempdir().unwrap();
    let session_file = state.path().join("session.json");
    lumo_ui::Session {
        tabs: vec![dir.path().join("gone"), dir.path().join("keep")],
        active: 1,
    }
    .save(&session_file)
    .unwrap();
    std::fs::remove_dir(dir.path().join("gone")).unwrap();

    let mut h = harness(Config {
        start: Start::Restore,
        session_file: Some(session_file),
        isolated: true,
    });
    settle(&mut h);

    let paths: Vec<_> = h
        .state()
        .tabs()
        .iter()
        .map(|t| t.path().to_path_buf())
        .collect();
    assert_eq!(paths, [dir.path().join("keep")]);
    assert_eq!(h.state().active_index(), 0);
}

#[test]
fn explicit_path_does_not_restore_session() {
    let dir = tree(&["a/", "b/"]);
    let state = tempfile::tempdir().unwrap();
    let session_file = state.path().join("session.json");
    lumo_ui::Session {
        tabs: vec![dir.path().join("a"), dir.path().join("b")],
        active: 0,
    }
    .save(&session_file)
    .unwrap();

    let h = harness(Config {
        start: Start::Path(dir.path().join("b")),
        session_file: Some(session_file),
        isolated: true,
    });

    assert_eq!(h.state().tabs().len(), 1);
    assert_eq!(h.state().active_tab().path(), dir.path().join("b"));
}

#[test]
fn unreadable_folder_shows_error_instead_of_crashing() {
    let dir = tree(&[]);
    let mut h = harness(at(&dir.path().join("missing")));
    settle(&mut h);

    h.get_by_label_contains("path not found");
}

#[test]
fn tabs_keep_their_own_scroll_position() {
    let dir = common::many_files(500);
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    h.key_press(Key::End);
    h.run();
    let scrolled = h.state().tabs()[0].scroll_offset();
    assert!(scrolled > 0.0, "End should scroll the list");

    h.state_mut()
        .queue(Action::OpenInNewTab(dir.path().to_path_buf()));
    settle(&mut h);
    assert_eq!(h.state().tabs()[1].scroll_offset(), 0.0);

    h.key_press_modifiers(CTRL, Key::Num1);
    settle(&mut h);
    assert_eq!(h.state().tabs()[0].scroll_offset(), scrolled);
}

/// Opens the palette and types `query`.
fn palette_query(h: &mut egui_kittest::Harness<'_, lumo_ui::LumoApp>, query: &str) {
    h.key_press_modifiers(CTRL, Key::K);
    h.run();
    assert!(h.state().is_palette_open());
    h.event(Event::Text(query.into()));
    h.run();
}

#[test]
fn palette_runs_commands() {
    let dir = tree(&["a/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    palette_query(&mut h, "new tab");
    h.key_press(Key::Enter);
    settle(&mut h);

    assert!(!h.state().is_palette_open());
    assert_eq!(h.state().tabs().len(), 2);
}

#[test]
fn palette_fuzzy_opens_folders_and_reveals_files() {
    let dir = tree(&["beta-project/", "alpha.txt", "zeta.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    palette_query(&mut h, "zeta");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(selected_name(&h).as_deref(), Some("zeta.txt"));
    assert_eq!(
        active_path(&h),
        dir.path(),
        "files are revealed, not opened"
    );

    palette_query(&mut h, "bp");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("beta-project"));
}

#[test]
fn palette_commands_only_prefix() {
    let dir = tree(&["hidden-notes.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    palette_query(&mut h, ">hidden");
    h.key_press(Key::Enter);
    h.run();

    assert!(h.state().show_hidden(), "ran the command, not the file");
}

#[test]
fn palette_switches_tabs() {
    let dir = tree(&["quokka/", "other/"]);
    let mut h = harness(at(&dir.path().join("quokka")));
    settle(&mut h);
    h.state_mut()
        .queue(Action::OpenInNewTab(dir.path().join("other")));
    settle(&mut h);
    assert_eq!(h.state().active_index(), 1);

    palette_query(&mut h, "quokka");
    h.key_press(Key::Enter);
    settle(&mut h);

    assert_eq!(h.state().active_index(), 0);
    assert_eq!(h.state().tabs().len(), 2, "switched, didn't open a new tab");
}

#[test]
fn palette_escape_and_toggle_close_without_acting() {
    let dir = tree(&["a/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    palette_query(&mut h, "new tab");
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().is_palette_open());

    h.get_by_label("Command palette").click();
    h.run();
    assert!(h.state().is_palette_open());
    h.key_press_modifiers(CTRL, Key::K);
    h.run();
    assert!(!h.state().is_palette_open());
    assert_eq!(h.state().tabs().len(), 1);
}
