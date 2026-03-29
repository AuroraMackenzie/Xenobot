//! Shared monitor helpers used by platform-specific legal-safe watchers.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Debounce events from a blocking `std::sync::mpsc::Receiver`.
///
/// This helper waits up to the caller-provided timeout for the first event. Once
/// an event arrives, later events within the debounce interval replace the pending
/// event, and the latest event is emitted when either:
/// - no newer event arrives before the debounce interval elapses, or
/// - the total wait since the first event reaches `max_wait`.
#[derive(Debug)]
pub struct DebouncedReceiver<T> {
    inner: Receiver<T>,
    debounce_interval: Duration,
    max_wait: Duration,
    pending_event: Option<T>,
    pending_started_at: Option<Instant>,
    pending_last_update_at: Option<Instant>,
}

impl<T> DebouncedReceiver<T> {
    /// Create a debounced wrapper around an existing blocking receiver.
    pub fn new(inner: Receiver<T>, debounce_interval: Duration, max_wait: Duration) -> Self {
        Self {
            inner,
            debounce_interval,
            max_wait,
            pending_event: None,
            pending_started_at: None,
            pending_last_update_at: None,
        }
    }

    /// Wait up to `initial_timeout` for an event.
    ///
    /// If no event arrives before the initial timeout, `Ok(None)` is returned.
    /// Once a batch has started, this method coalesces subsequent events until the
    /// debounce or max-wait boundary is reached.
    pub fn recv_timeout(
        &mut self,
        initial_timeout: Duration,
    ) -> Result<Option<T>, RecvTimeoutError> {
        if self.pending_event.is_none() {
            match self.inner.recv_timeout(initial_timeout) {
                Ok(event) => {
                    let now = Instant::now();
                    self.pending_event = Some(event);
                    self.pending_started_at = Some(now);
                    self.pending_last_update_at = Some(now);
                }
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => return Err(RecvTimeoutError::Disconnected),
            }
        }

        loop {
            let started_at = self
                .pending_started_at
                .expect("pending batch must have a start timestamp");
            let last_update_at = self
                .pending_last_update_at
                .expect("pending batch must have a last-update timestamp");

            if started_at.elapsed() >= self.max_wait
                || last_update_at.elapsed() >= self.debounce_interval
            {
                return Ok(self.take_event());
            }

            let wait_until_emit = last_update_at + self.debounce_interval;
            let wait_until_max = started_at + self.max_wait;
            let now = Instant::now();
            let sleep_for = wait_until_emit
                .min(wait_until_max)
                .saturating_duration_since(now);

            match self.inner.recv_timeout(sleep_for) {
                Ok(event) => {
                    self.pending_event = Some(event);
                    self.pending_last_update_at = Some(Instant::now());
                }
                Err(RecvTimeoutError::Timeout) => return Ok(self.take_event()),
                Err(RecvTimeoutError::Disconnected) => return Ok(self.take_event()),
            }
        }
    }

    fn take_event(&mut self) -> Option<T> {
        self.pending_started_at = None;
        self.pending_last_update_at = None;
        self.pending_event.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn returns_none_while_idle() {
        let (_tx, rx) = mpsc::channel::<u8>();
        let mut receiver =
            DebouncedReceiver::new(rx, Duration::from_millis(15), Duration::from_millis(50));

        let event = receiver
            .recv_timeout(Duration::from_millis(10))
            .expect("idle timeout should not error");
        assert!(event.is_none());
    }

    #[test]
    fn coalesces_burst_to_latest_event() {
        let (tx, rx) = mpsc::channel();
        let mut receiver =
            DebouncedReceiver::new(rx, Duration::from_millis(20), Duration::from_millis(80));

        tx.send(1).expect("send first");
        thread::sleep(Duration::from_millis(5));
        tx.send(2).expect("send second");

        let event = receiver
            .recv_timeout(Duration::from_millis(10))
            .expect("receive event");
        assert_eq!(event, Some(2));
    }

    #[test]
    fn flushes_pending_event_when_sender_disconnects() {
        let (tx, rx) = mpsc::channel();
        let mut receiver =
            DebouncedReceiver::new(rx, Duration::from_millis(50), Duration::from_millis(120));

        tx.send(7).expect("send event");
        drop(tx);

        let event = receiver
            .recv_timeout(Duration::from_millis(10))
            .expect("disconnect should flush pending event");
        assert_eq!(event, Some(7));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(5)),
            Err(RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn flushes_after_max_wait_under_event_storm() {
        let (tx, rx) = mpsc::channel();
        let mut receiver =
            DebouncedReceiver::new(rx, Duration::from_millis(40), Duration::from_millis(70));

        tx.send(1).expect("send first");
        thread::sleep(Duration::from_millis(25));
        tx.send(2).expect("send second");
        thread::sleep(Duration::from_millis(25));
        tx.send(3).expect("send third");

        let started = Instant::now();
        let event = receiver
            .recv_timeout(Duration::from_millis(10))
            .expect("receive debounced event");
        let elapsed = started.elapsed();

        assert_eq!(event, Some(3));
        assert!(
            elapsed < Duration::from_millis(90),
            "max wait should cap repeated update bursts"
        );
    }
}
