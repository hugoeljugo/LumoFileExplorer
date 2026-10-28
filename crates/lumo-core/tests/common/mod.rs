// Shared test helpers; clippy.toml's test exemption only covers #[test] fns.
#![allow(clippy::unwrap_used, dead_code)]

use std::fmt::Debug;
use std::fs::File;
use std::path::Path;

/// Creates `count` empty files named `f000000`, `f000001`, ... in `dir`.
pub fn populate(dir: &Path, count: usize) {
    for i in 0..count {
        File::create(dir.join(format!("f{i:06}"))).unwrap();
    }
}

/// Consumes a stream without collecting it, panicking on any error item.
pub fn count_ok<T, E: Debug>(items: impl Iterator<Item = Result<T, E>>) -> usize {
    items.fold(0, |n, item| {
        item.unwrap();
        n + 1
    })
}
