//! Phase 2.6: file operations, selection, the right-click search menu,
//! undo toasts. Driven end to end through egui_kittest on real files.
//! The harness runs `isolated`, so the user's clipboard and Recycle Bin are
//! never touched.

mod common;

use std::fs;

use common::{CTRL, active_path, at, harness, settle, tree};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use lumo_ui::Action;

fn selected(h: &egui_kittest::Harness<'_, lumo_ui::LumoApp>) -> Vec<String> {
    h.state().active_tab().selected_names()
}

#[test]
fn modifier_clicks_tick_circles_and_the_tray() {
    let dir = tree(&["a.txt", "b.txt", "c.txt", "d.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("a.txt").click();
    h.run();
    h.get_by_label("c.txt").click_modifiers(CTRL);
    h.run();
    assert_eq!(selected(&h), ["a.txt", "c.txt"]);
    h.get_by_label("2 selected");

    h.get_by_label("d.txt").click_modifiers(Modifiers::SHIFT);
    h.run();
    assert_eq!(
        selected(&h),
        ["c.txt", "d.txt"],
        "range from the last clicked item"
    );

    h.get_by_label("Select a.txt").click();
    h.run();
    assert_eq!(selected(&h), ["a.txt", "c.txt", "d.txt"]);

    h.key_press(Key::Escape);
    h.run();
    assert!(selected(&h).is_empty());

    h.key_press_modifiers(CTRL, Key::A);
    h.run();
    assert_eq!(selected(&h).len(), 4);
}

#[test]
fn rename_in_place_and_undo() {
    let dir = tree(&["draft.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("draft.txt").click();
    h.run();
    h.key_press(Key::F2);
    h.run();
    assert!(h.state().is_renaming());
    // Only the stem is selected, so typing keeps the extension.
    h.event(Event::Text("final".into()));
    h.key_press(Key::Enter);
    settle(&mut h);

    assert!(dir.path().join("final.txt").exists());
    assert!(!dir.path().join("draft.txt").exists());
    assert_eq!(selected(&h), ["final.txt"]);

    h.key_press_modifiers(CTRL, Key::Z);
    settle(&mut h);
    assert!(dir.path().join("draft.txt").exists());
}

#[test]
fn rename_escape_cancels_and_bad_names_are_refused() {
    let dir = tree(&["keep.txt", "other.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("keep.txt").click();
    h.run();
    h.key_press(Key::F2);
    h.run();
    h.event(Event::Text("changed".into()));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(!h.state().is_renaming());
    assert!(dir.path().join("keep.txt").exists());

    h.key_press(Key::F2);
    h.run();
    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text("other.txt".into()));
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(dir.path().join("keep.txt").exists(), "never overwrites");
    assert!(
        h.state()
            .toast_message()
            .is_some_and(|m| m.contains("already exists"))
    );
    assert!(
        h.state().is_renaming(),
        "stays open so the name can be fixed"
    );
}

#[test]
fn new_folder_starts_a_rename() {
    let dir = tree(&[]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.key_press_modifiers(CTRL | Modifiers::SHIFT, Key::N);
    settle(&mut h);
    assert!(dir.path().join("New folder").is_dir());
    assert!(h.state().is_renaming());

    h.key_press_modifiers(CTRL, Key::A);
    h.event(Event::Text("Projects".into()));
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(dir.path().join("Projects").is_dir());
}

#[test]
fn copy_paste_between_folders_then_undo() {
    let root = tree(&["src/", "dst/"]);
    fs::write(root.path().join("src").join("a.txt"), "data").unwrap();
    let mut h = harness(at(&root.path().join("src")));
    settle(&mut h);

    h.get_by_label("a.txt").click();
    h.run();
    h.key_press_modifiers(CTRL, Key::C);
    h.run();
    h.state_mut()
        .queue(Action::Navigate(root.path().join("dst")));
    settle(&mut h);
    h.key_press_modifiers(CTRL, Key::V);
    settle(&mut h);

    let pasted = root.path().join("dst").join("a.txt");
    assert_eq!(fs::read_to_string(&pasted).unwrap(), "data");
    assert!(root.path().join("src").join("a.txt").exists());
    assert_eq!(selected(&h), ["a.txt"], "pasted items are selected");
    assert_eq!(h.state().toast_message(), Some("Copied 1 item"));

    h.key_press_modifiers(CTRL, Key::Z);
    settle(&mut h);
    assert!(!pasted.exists());
}

#[test]
fn paste_into_the_same_folder_makes_a_copy() {
    let dir = tree(&["notes.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("notes.txt").click();
    h.run();
    h.key_press_modifiers(CTRL, Key::C);
    h.key_press_modifiers(CTRL, Key::V);
    settle(&mut h);

    assert!(dir.path().join("notes - Copy.txt").exists());
}

#[test]
fn cut_paste_moves_then_undo_moves_back() {
    let root = tree(&["src/", "dst/"]);
    fs::write(root.path().join("src").join("a.txt"), "data").unwrap();
    let mut h = harness(at(&root.path().join("src")));
    settle(&mut h);

    h.get_by_label("a.txt").click();
    h.run();
    h.key_press_modifiers(CTRL, Key::X);
    h.run();
    h.state_mut()
        .queue(Action::Navigate(root.path().join("dst")));
    settle(&mut h);
    h.key_press_modifiers(CTRL, Key::V);
    settle(&mut h);

    assert!(root.path().join("dst").join("a.txt").exists());
    assert!(!root.path().join("src").join("a.txt").exists());

    h.key_press_modifiers(CTRL, Key::Z);
    settle(&mut h);
    assert!(root.path().join("src").join("a.txt").exists());
    assert!(!root.path().join("dst").join("a.txt").exists());
}

#[test]
fn delete_goes_to_the_bin_and_undo_restores() {
    let dir = tree(&["old.txt", "keep.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("old.txt").click();
    h.run();
    h.key_press(Key::Delete);
    settle(&mut h);
    assert!(!dir.path().join("old.txt").exists());
    assert_eq!(
        h.state().toast_message(),
        Some("Moved 1 item to the Recycle Bin")
    );

    // The toast's Undo button.
    h.get_by_label("Undo").click();
    settle(&mut h);
    assert!(dir.path().join("old.txt").exists());
}

#[test]
fn shift_delete_asks_first() {
    let dir = tree(&["gone.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("gone.txt").click();
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::Delete);
    h.run();
    assert!(h.state().is_confirming_delete());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(dir.path().join("gone.txt").exists());

    h.key_press_modifiers(Modifiers::SHIFT, Key::Delete);
    h.run();
    h.get_by_label("Delete").click();
    settle(&mut h);
    assert!(!dir.path().join("gone.txt").exists());
}

#[test]
fn right_click_opens_the_search_menu_for_that_row() {
    let dir = tree(&["a.txt", "b.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("b.txt").click_secondary();
    h.run();
    assert!(h.state().is_palette_open());
    assert_eq!(selected(&h), ["b.txt"]);

    h.event(Event::Text("ren".into()));
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().is_renaming());
}

#[test]
fn right_click_on_empty_space_offers_folder_commands() {
    let dir = tree(&["a.txt"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("File list").click_secondary();
    h.run();
    assert!(h.state().is_palette_open());
    h.get_by_label("New folder");
    assert!(
        h.query_by_label("Rename").is_none(),
        "nothing selected, so no rename"
    );

    h.event(Event::Text("new folder".into()));
    h.run();
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(dir.path().join("New folder").is_dir());
}

#[test]
fn properties_and_copy_path() {
    let dir = tree(&["report.pdf"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("report.pdf").click();
    h.run();
    h.key_press_modifiers(Modifiers::ALT, Key::Enter);
    h.run();
    assert!(h.state().is_showing_properties());
    h.get_by_label("Location");

    h.key_press_modifiers(CTRL | Modifiers::SHIFT, Key::C);
    h.run();
    assert_eq!(h.state().toast_message(), Some("Copied path"));
}

#[test]
fn enter_on_a_multi_selection_of_folders_opens_the_focused_one() {
    let dir = tree(&["one/", "two/"]);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    h.get_by_label("one").click();
    h.run();
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(active_path(&h), dir.path().join("one"));
}

/// Steps the UI (in real time, so the folder watcher can fire) until `done`.
fn wait_until(
    h: &mut egui_kittest::Harness<'_, lumo_ui::LumoApp>,
    what: &str,
    done: impl Fn(&egui_kittest::Harness<'_, lumo_ui::LumoApp>) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(h) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn changes_made_outside_lumo_show_up_and_keep_the_selection() {
    let root = tree(&["here/", "elsewhere/"]);
    let here = root.path().join("here");
    for name in ["cut-me.txt", "keep.txt"] {
        fs::write(here.join(name), "x").unwrap();
    }
    let mut h = harness(at(&here));
    settle(&mut h);

    // Cut in Lumo…
    h.get_by_label("cut-me.txt").click();
    h.run();
    h.key_press_modifiers(CTRL, Key::X);
    h.run();
    h.get_by_label("keep.txt").click();
    h.run();

    // …then another app (Explorer) pastes it elsewhere and adds a file.
    fs::rename(
        here.join("cut-me.txt"),
        root.path().join("elsewhere").join("cut-me.txt"),
    )
    .unwrap();
    fs::write(here.join("downloaded.zip"), "x").unwrap();

    wait_until(&mut h, "the folder to update", |h| {
        common::visible_names(h) == ["downloaded.zip", "keep.txt"]
    });
    assert_eq!(selected(&h), ["keep.txt"], "selection survives the reload");
}
