//! Phase 1 gate: streaming memory stays bounded regardless of directory size.
//!
//! Lives in its own test binary because it installs a counting global
//! allocator; tests in the same binary run in parallel, so it has one test
//! that runs by default and one large variant that must be run alone.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use lumo_core::{ScanOptions, scan};

struct PeakAlloc;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to the system allocator unchanged; the counters
// are bookkeeping only.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for PeakAlloc {
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
static ALLOCATOR: PeakAlloc = PeakAlloc;

/// Scans `dir` and returns (entry count, peak heap growth in bytes during the scan).
#[allow(clippy::unwrap_used)] // test helper, not a #[test] fn
fn scan_peak(dir: &Path) -> (usize, usize) {
    let baseline = CURRENT.load(Relaxed);
    PEAK.store(baseline, Relaxed);

    let count = common::count_ok(scan(dir, &ScanOptions::default()).unwrap());

    (count, PEAK.load(Relaxed) - baseline)
}

#[test]
fn streaming_10k_entries_stays_under_256_kib() {
    let dir = tempfile::tempdir().unwrap();
    common::populate(dir.path(), 10_000);

    let (count, peak) = scan_peak(dir.path());

    // ~60 KiB measured on Windows with the default 512-entry batch.
    assert_eq!(count, 10_000);
    assert!(peak < 256 * 1024, "peak heap growth {peak} bytes");
}

/// Roadmap gate: 1M entries under 500MB. Creating the files takes minutes,
/// so run on demand: `cargo test -p lumo-core --test scan_memory_test -- --ignored`
#[test]
#[ignore = "creates 1M files; run explicitly"]
fn streaming_1m_entries_stays_under_500_mb() {
    let dir = tempfile::tempdir().unwrap();
    common::populate(dir.path(), 1_000_000);

    let (count, peak) = scan_peak(dir.path());

    assert_eq!(count, 1_000_000);
    assert!(peak < 500 * 1024 * 1024, "peak heap growth {peak} bytes");
}
