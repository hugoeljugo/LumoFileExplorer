//! Folder-load benchmarks: Lumo's scanner vs. plain `std::fs::read_dir`.
//!
//! Run with `cargo bench -p lumo-core --bench folder_load`.

// Benchmark setup is not a production path.
#![allow(clippy::unwrap_used)]

use std::fmt::Debug;
use std::fs::{self, File};
use std::hint::black_box;
use std::path::Path;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use lumo_core::{ScanOptions, list_entries, scan};

fn populate(dir: &Path, count: usize) {
    for i in 0..count {
        File::create(dir.join(format!("f{i:06}"))).unwrap();
    }
}

/// Consumes every item, panicking on errors, and returns the count.
fn drain<T, E: Debug>(items: impl Iterator<Item = Result<T, E>>) -> usize {
    items.fold(0, |n, item| {
        black_box(item.unwrap());
        n + 1
    })
}

fn folder_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("folder_load");

    for count in [1_000, 10_000] {
        let dir = tempfile::tempdir().unwrap();
        populate(dir.path(), count);
        let path = dir.path();

        group.bench_with_input(BenchmarkId::new("std_names", count), path, |b, p| {
            b.iter(|| drain(fs::read_dir(p).unwrap()))
        });
        group.bench_with_input(BenchmarkId::new("std_metadata", count), path, |b, p| {
            b.iter(|| drain(fs::read_dir(p).unwrap().map(|e| e.unwrap().metadata())))
        });
        for (name, parallel_metadata) in [("scan_sequential", false), ("scan_parallel", true)] {
            let options = ScanOptions {
                parallel_metadata,
                ..ScanOptions::default()
            };
            group.bench_with_input(BenchmarkId::new(name, count), path, |b, p| {
                b.iter(|| drain(scan(p, &options).unwrap()))
            });
        }
        group.bench_with_input(
            BenchmarkId::new("list_entries_sorted", count),
            path,
            |b, p| b.iter(|| black_box(list_entries(p).unwrap())),
        );
    }

    group.finish();
}

criterion_group!(benches, folder_load);
criterion_main!(benches);
