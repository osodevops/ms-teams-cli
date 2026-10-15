use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, Notify, OwnedSemaphorePermit, Semaphore};

/// Bytes of output that may wait for stdout before requests get 503.
pub const OUTPUT_BUDGET: usize = 8 * 1024 * 1024;

type Entry = (Vec<String>, OwnedSemaphorePermit);

/// Output waiting for stdout, bounded in bytes, so a slow reader never
/// blocks an HTTP response. A dedicated thread writes entries in order.
///
/// Every line admitted has been acknowledged to the sender, so shutdown
/// closes admission first and then waits for the writer to catch up.
pub struct Queue {
    /// `None` once admission is closed. The lock makes closing atomic with
    /// respect to a push in progress.
    tx: Mutex<Option<mpsc::UnboundedSender<Entry>>>,
    budget: Arc<Semaphore>,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    accepted: AtomicU64,
    written: AtomicU64,
    failed: AtomicBool,
    /// Signalled after each entry and when the writer stops.
    progress: Notify,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Push {
    Queued,
    Full,
    Closed,
}

impl Queue {
    pub fn new(
        capacity: usize,
        mut sink: impl FnMut(&str) -> io::Result<()> + Send + 'static,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Entry>();
        let shared = Arc::new(Shared::default());
        let writer = shared.clone();
        std::thread::Builder::new()
            .name("teams-listen-output".into())
            .spawn(move || {
                // The permit is held until the writes return.
                while let Some((lines, _permit)) = rx.blocking_recv() {
                    for line in &lines {
                        if let Err(e) = sink(line) {
                            eprintln!("Error: Failed to write to stdout: {e}");
                            writer.failed.store(true, Ordering::SeqCst);
                            writer.progress.notify_one();
                            return;
                        }
                        writer.written.fetch_add(1, Ordering::SeqCst);
                    }
                    writer.progress.notify_one();
                }
            })
            .expect("spawn output thread");
        Self {
            tx: Mutex::new(Some(tx)),
            budget: Arc::new(Semaphore::new(capacity)),
            shared,
        }
    }

    /// Enqueue `lines` whole, or refuse them.
    pub fn try_push(&self, lines: Vec<String>) -> Push {
        let tx = self.tx.lock().unwrap();
        let Some(tx) = tx.as_ref().filter(|_| !self.failed()) else {
            return Push::Closed;
        };
        let bytes: usize = lines.iter().map(|l| l.len() + 1).sum();
        let Ok(n) = u32::try_from(bytes) else {
            return Push::Full;
        };
        let Ok(permit) = self.budget.clone().try_acquire_many_owned(n) else {
            return Push::Full;
        };
        // Counted before sending so `written` never runs ahead of it.
        let count = lines.len() as u64;
        self.shared.accepted.fetch_add(count, Ordering::SeqCst);
        if tx.send((lines, permit)).is_err() {
            self.shared.accepted.fetch_sub(count, Ordering::SeqCst);
            return Push::Closed;
        }
        Push::Queued
    }

    /// Refuse further pushes. Lines already queued are still written.
    pub fn close(&self) {
        self.tx.lock().unwrap().take();
    }

    /// Lines acknowledged but not (yet) written.
    pub fn pending(&self) -> u64 {
        let written = self.shared.written.load(Ordering::SeqCst);
        self.shared
            .accepted
            .load(Ordering::SeqCst)
            .saturating_sub(written)
    }

    fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::SeqCst)
    }

    /// Resolves once stdout has failed.
    pub async fn writer_failed(&self) {
        while !self.failed() {
            self.wait_progress().await;
        }
    }

    /// Resolves once everything queued is written or stdout has failed.
    pub async fn drained(&self) {
        while self.pending() > 0 && !self.failed() {
            self.wait_progress().await;
        }
    }

    async fn wait_progress(&self) {
        // The timeout covers a notification consumed by another waiter.
        let _ =
            tokio::time::timeout(Duration::from_millis(100), self.shared.progress.notified()).await;
    }

    /// Close admission and write out the queue, unless `abort` resolves
    /// first. Returns the number of acknowledged lines not written.
    pub async fn shutdown(&self, abort: impl Future<Output = ()>) -> u64 {
        self.close();
        tokio::select! {
            _ = self.drained() => {}
            _ = abort => {}
        }
        self.pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc as std_mpsc;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// A queue whose sink writes one line per message on `release`.
    fn gated(capacity: usize) -> (Queue, std_mpsc::Sender<()>, std_mpsc::Receiver<String>) {
        let (release_tx, release_rx) = std_mpsc::channel::<()>();
        let (out_tx, out_rx) = std_mpsc::channel::<String>();
        let queue = Queue::new(capacity, move |text| {
            release_rx
                .recv()
                .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
            out_tx.send(text.to_string()).unwrap();
            Ok(())
        });
        (queue, release_tx, out_rx)
    }

    #[tokio::test]
    async fn full_queue_refuses_until_the_writer_catches_up() {
        let (queue, release, out) = gated(12);

        // The writer takes the first entry and blocks in the sink holding its
        // bytes; the second fills the budget.
        assert_eq!(queue.try_push(lines(&["aaaaa"])), Push::Queued);
        assert_eq!(queue.try_push(lines(&["bbbbb"])), Push::Queued);
        assert_eq!(queue.try_push(lines(&["c"])), Push::Full);

        release.send(()).unwrap();
        assert_eq!(out.recv().unwrap(), "aaaaa");
        // Space frees once the write returns.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while queue.try_push(lines(&["c"])) != Push::Queued {
            assert!(std::time::Instant::now() < deadline, "space never freed");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        release.send(()).unwrap();
        release.send(()).unwrap();
        assert_eq!(out.recv().unwrap(), "bbbbb");
        assert_eq!(out.recv().unwrap(), "c");
    }

    #[test]
    fn entry_larger_than_the_budget_is_refused() {
        let queue = Queue::new(5, |_| Ok(()));
        assert_eq!(queue.try_push(lines(&["12345"])), Push::Full);
        assert_eq!(queue.try_push(lines(&["1234"])), Push::Queued);
    }

    #[tokio::test]
    async fn closed_queue_refuses_pushes() {
        let (queue, _release, _out) = gated(1024);
        queue.close();
        assert_eq!(queue.try_push(lines(&["a"])), Push::Closed);
        assert_eq!(queue.pending(), 0);
    }

    #[tokio::test]
    async fn shutdown_writes_everything_acknowledged() {
        let (queue, release, out) = gated(1024);
        assert_eq!(queue.try_push(lines(&["a", "b"])), Push::Queued);
        assert_eq!(queue.try_push(lines(&["c"])), Push::Queued);
        for _ in 0..3 {
            release.send(()).unwrap();
        }
        let lost = tokio::time::timeout(
            Duration::from_secs(10),
            queue.shutdown(std::future::pending()),
        )
        .await
        .expect("drain hung");
        assert_eq!(lost, 0);
        let written: Vec<String> = out.try_iter().collect();
        assert_eq!(written, ["a", "b", "c"]);
        assert_eq!(queue.try_push(lines(&["d"])), Push::Closed);
    }

    #[tokio::test]
    async fn aborted_shutdown_reports_lines_not_written() {
        let (queue, release, _out) = gated(1024);
        assert_eq!(queue.try_push(lines(&["a", "b", "c"])), Push::Queued);
        release.send(()).unwrap();
        while queue.pending() > 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // The sink stays blocked on "b", as with a stalled stdout reader.
        assert_eq!(queue.shutdown(std::future::ready(())).await, 2);
        drop(release);
    }

    #[tokio::test]
    async fn failed_stdout_closes_admission_and_counts_the_loss() {
        let (queue, release, _out) = gated(1024);
        assert_eq!(queue.try_push(lines(&["a", "b"])), Push::Queued);
        release.send(()).unwrap();
        drop(release); // the next write fails
        tokio::time::timeout(Duration::from_secs(10), queue.writer_failed())
            .await
            .expect("failure not reported");
        assert_eq!(queue.try_push(lines(&["c"])), Push::Closed);
        assert_eq!(queue.shutdown(std::future::pending()).await, 1);
    }
}
