//! Phase 2 timing gates. They measure egui's CPU-side frame (layout +
//! tessellation) through egui_kittest; GPU work for this UI is negligible.
//!
//! Debug builds are far slower than what ships, so these only run in release:
//! `cargo test --release -p lumo-ui --test perf_test`

mod common;

use std::time::{Duration, Instant};

use common::{CTRL, at, harness, many_files, settle};
use eframe::egui::{Key, vec2};
use lumo_ui::Action;

const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

#[test]
#[cfg_attr(debug_assertions, ignore = "timing gate: run with --release")]
fn tab_switch_under_50ms_with_10k_entry_tabs() {
    let dir = many_files(10_000);
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    for _ in 0..9 {
        h.state_mut()
            .queue(Action::OpenInNewTab(dir.path().to_path_buf()));
        settle(&mut h);
    }

    let mut worst = Duration::ZERO;
    for _ in 0..30 {
        let started = Instant::now();
        h.key_press_modifiers(CTRL, Key::Tab);
        h.step();
        worst = worst.max(started.elapsed());
    }

    assert!(
        worst < Duration::from_millis(50),
        "worst tab switch took {worst:?}"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing gate: run with --release")]
fn frames_fit_60fps_while_resizing_a_10k_entry_folder() {
    let dir = many_files(10_000);
    let mut h = harness(at(dir.path()));
    settle(&mut h);

    let mut times: Vec<Duration> = (0..120)
        .map(|i| {
            let i = i as f32;
            h.set_size(vec2(700.0 + (i * 7.0) % 900.0, 450.0 + (i * 5.0) % 500.0));
            let started = Instant::now();
            h.step();
            started.elapsed()
        })
        .collect();
    times.sort();
    let p95 = times[times.len() * 95 / 100];

    assert!(
        p95 < FRAME_BUDGET,
        "p95 frame time {p95:?} exceeds 60fps budget"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing gate: run with --release")]
fn palette_keystrokes_fit_a_frame_in_a_10k_entry_folder() {
    let dir = many_files(10_000);
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    h.key_press_modifiers(CTRL, Key::K);
    h.run();

    let mut worst = Duration::ZERO;
    for text in ["f", "i", "l", "e", "0", "9", "9"] {
        let started = Instant::now();
        h.event(eframe::egui::Event::Text(text.into()));
        h.step();
        worst = worst.max(started.elapsed());
    }

    assert!(
        worst < FRAME_BUDGET,
        "worst palette keystroke frame took {worst:?}"
    );
}
