//! Phase 2 gate: 50 tabs stay well under 500MB and closing tabs frees memory.
//!
//! Own test binary: it installs a counting global allocator.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use common::{at, harness, many_files, settle};
use lumo_ui::Action;

struct CountingAlloc;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to the system allocator unchanged; the counters
// are bookkeeping only.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same contract as the caller's.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Relaxed) + layout.size();
            PEAK.fetch_max(now, Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const MB: usize = 1024 * 1024;

#[test]
fn fifty_tabs_stay_bounded_and_closing_frees_memory() {
    let dir = many_files(1_000);
    let mut h = harness(at(dir.path()));
    settle(&mut h);
    let baseline = CURRENT.load(Relaxed);
    PEAK.store(baseline, Relaxed);

    let mut after_close = Vec::new();
    for _cycle in 0..3 {
        for _ in 0..49 {
            h.state_mut()
                .queue(Action::OpenInNewTab(dir.path().to_path_buf()));
            settle(&mut h);
        }
        assert_eq!(h.state().tabs().len(), 50);
        for _ in 0..49 {
            let active = h.state().active_index();
            h.state_mut().queue(Action::CloseTab(active));
            h.step();
        }
        h.run();
        assert_eq!(h.state().tabs().len(), 1);
        after_close.push(CURRENT.load(Relaxed));
    }

    let peak = PEAK.load(Relaxed) - baseline;
    assert!(peak < 500 * MB, "50 tabs peaked at {} MB", peak / MB);
    // Allow for first-cycle warm-up (caches); later cycles must not keep growing.
    let growth = after_close[2].saturating_sub(after_close[1]);
    assert!(
        growth < 2 * MB,
        "memory after closing tabs grew {} KB between cycles: {after_close:?}",
        growth / 1024
    );
}
