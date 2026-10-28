//! Phase 1 gate: scan 10K entries in <2s on Windows, <1s on Linux.

mod common;

use std::time::{Duration, Instant};

use lumo_core::{ScanOptions, scan};

const LIMIT: Duration = if cfg!(windows) {
    Duration::from_secs(2)
} else {
    Duration::from_secs(1)
};

#[test]
fn scans_10k_entries_within_phase_1_limit() {
    let dir = tempfile::tempdir().unwrap();
    common::populate(dir.path(), 10_000);

    let started = Instant::now();
    let count = common::count_ok(scan(dir.path(), &ScanOptions::default()).unwrap());
    let elapsed = started.elapsed();

    assert_eq!(count, 10_000);
    assert!(elapsed < LIMIT, "scan took {elapsed:?} (limit {LIMIT:?})");
}
