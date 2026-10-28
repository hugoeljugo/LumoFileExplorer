use std::io;
use std::path::PathBuf;
use std::vec;

use tokio::sync::{mpsc, oneshot};

use super::{ScanItem, ScanOptions, scan};
use crate::fs::FsError;

/// Batches buffered between the worker and the consumer. Together with the
/// batch size this caps how far the worker can run ahead.
const CHANNEL_DEPTH: usize = 2;

/// Async counterpart of [`Scan`](super::Scan), fed by a blocking worker.
///
/// Dropping the stream stops the worker at its next batch.
#[derive(Debug)]
pub struct ScanStream {
    rx: mpsc::Receiver<Vec<ScanItem>>,
    current: vec::IntoIter<ScanItem>,
}

impl ScanStream {
    /// Returns the next entry, or `None` once the scan is finished.
    pub async fn next(&mut self) -> Option<ScanItem> {
        loop {
            if let Some(item) = self.current.next() {
                return Some(item);
            }
            self.current = self.rx.recv().await?.into_iter();
        }
    }

    /// Returns the next batch of entries, or `None` once the scan is finished.
    pub async fn next_batch(&mut self) -> Option<Vec<ScanItem>> {
        let leftover: Vec<_> = std::mem::take(&mut self.current).collect();
        if leftover.is_empty() {
            self.rx.recv().await
        } else {
            Some(leftover)
        }
    }
}

/// Opens `path` and streams its entries without blocking the async runtime.
///
/// Must be awaited inside a Tokio runtime (it uses `spawn_blocking`).
pub async fn scan_async(
    path: impl Into<PathBuf>,
    options: ScanOptions,
) -> Result<ScanStream, FsError> {
    let path = path.into();
    let (open_tx, open_rx) = oneshot::channel();
    let (tx, rx) = mpsc::channel(CHANNEL_DEPTH);

    let worker_path = path.clone();
    tokio::task::spawn_blocking(move || {
        let mut scan = match scan(&worker_path, &options) {
            Ok(scan) => scan,
            Err(err) => {
                // If the caller is gone there is nobody to report to.
                let _ = open_tx.send(Err(err));
                return;
            }
        };
        if open_tx.send(Ok(())).is_err() {
            return;
        }
        while let Some(batch) = scan.next_batch() {
            // A send error means the ScanStream was dropped: stop scanning.
            if tx.blocking_send(batch).is_err() {
                return;
            }
        }
    });

    match open_rx.await {
        Ok(Ok(())) => Ok(ScanStream {
            rx,
            current: Vec::new().into_iter(),
        }),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(FsError::Io {
            path,
            source: io::Error::other("scan worker stopped unexpectedly"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;

    fn dir_with_files(count: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..count {
            write(dir.path().join(format!("f{i:04}")), b"").unwrap();
        }
        dir
    }

    #[tokio::test]
    async fn streams_every_entry() {
        let dir = dir_with_files(25);
        let options = ScanOptions {
            batch_size: 4,
            ..ScanOptions::default()
        };

        let mut stream = scan_async(dir.path(), options).await.unwrap();
        let mut count = 0;
        while let Some(item) = stream.next().await {
            item.unwrap();
            count += 1;
        }

        assert_eq!(count, 25);
    }

    #[tokio::test]
    async fn next_batch_after_next_returns_leftovers() {
        let dir = dir_with_files(6);
        let options = ScanOptions {
            batch_size: 4,
            ..ScanOptions::default()
        };

        let mut stream = scan_async(dir.path(), options).await.unwrap();
        stream.next().await.unwrap().unwrap();

        assert_eq!(stream.next_batch().await.map(|b| b.len()), Some(3));
        assert_eq!(stream.next_batch().await.map(|b| b.len()), Some(2));
        assert!(stream.next_batch().await.is_none());
    }

    #[tokio::test]
    async fn missing_path_fails_up_front() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");

        let err = scan_async(&missing, ScanOptions::default())
            .await
            .unwrap_err();

        assert!(matches!(err, FsError::NotFound(p) if p == missing));
    }

    // If the worker kept blocking on a full channel after the stream is gone,
    // runtime shutdown at the end of this test would hang forever.
    #[tokio::test]
    async fn dropping_stream_stops_worker() {
        let dir = dir_with_files(200);
        let options = ScanOptions {
            batch_size: 1,
            ..ScanOptions::default()
        };

        let mut stream = scan_async(dir.path(), options).await.unwrap();
        stream.next().await.unwrap().unwrap();
        drop(stream);
    }
}
