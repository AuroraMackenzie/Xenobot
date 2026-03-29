//! File system monitoring for WeChat data directories.

use crate::error::{WeChatError, WeChatResult};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

/// File system event types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// New file created.
    Created(PathBuf),
    /// File modified.
    Modified(PathBuf),
    /// File deleted.
    Deleted(PathBuf),
}

/// File monitoring configuration.
#[derive(Debug, Clone)]
pub struct FileMonitorConfig {
    /// Directory to watch.
    pub watch_dir: PathBuf,
    /// File patterns to match (regex).
    pub file_patterns: Vec<Regex>,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
    /// Maximum wait time in milliseconds.
    pub max_wait_ms: u64,
    /// Recursive watching.
    pub recursive: bool,
}

impl Default for FileMonitorConfig {
    fn default() -> Self {
        Self {
            watch_dir: PathBuf::new(),
            file_patterns: vec![],
            debounce_ms: 1000,
            max_wait_ms: 10000,
            recursive: true,
        }
    }
}

/// File system monitor for WeChat data files.
pub struct FileMonitor {
    config: FileMonitorConfig,
    watcher: RecommendedWatcher,
    event_rx: DebouncedEventStream,
}

impl FileMonitor {
    /// Create a new file monitor.
    pub fn new(config: FileMonitorConfig) -> WeChatResult<Self> {
        let (event_tx, event_rx) = mpsc::channel(100);

        let event_tx_clone = event_tx.clone();
        let config_clone = config.clone();

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                Self::handle_event(&event, &config_clone, &event_tx_clone);
            }
        })
        .map_err(WeChatError::FileMonitor)?;

        Ok(Self {
            event_rx: DebouncedEventStream::new(event_rx, config.debounce_ms, config.max_wait_ms),
            config,
            watcher,
        })
    }

    /// Start monitoring.
    pub fn start(&mut self) -> WeChatResult<()> {
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        self.watcher
            .watch(&self.config.watch_dir, mode)
            .map_err(WeChatError::FileMonitor)?;

        Ok(())
    }

    /// Stop monitoring.
    pub fn stop(&mut self) -> WeChatResult<()> {
        self.watcher
            .unwatch(&self.config.watch_dir)
            .map_err(WeChatError::FileMonitor)
    }

    /// Get next file event with debouncing.
    pub async fn next_event(&mut self) -> Option<FileEvent> {
        self.event_rx.next().await
    }

    /// Handle raw filesystem event.
    fn handle_event(event: &Event, config: &FileMonitorConfig, event_tx: &mpsc::Sender<FileEvent>) {
        if !matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) {
            return;
        }

        for path in &event.paths {
            if !Self::matches_pattern(path, config) {
                continue;
            }

            let file_event = match event.kind {
                EventKind::Create(_) => FileEvent::Created(path.clone()),
                EventKind::Modify(_) => FileEvent::Modified(path.clone()),
                EventKind::Remove(_) => FileEvent::Deleted(path.clone()),
                _ => continue,
            };

            // Try to send event (non-blocking)
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(file_event).await;
            });
        }
    }

    /// Check if file path matches any pattern.
    fn matches_pattern(path: &Path, config: &FileMonitorConfig) -> bool {
        if config.file_patterns.is_empty() {
            return true;
        }

        let path_str = path.to_string_lossy();
        config
            .file_patterns
            .iter()
            .any(|pattern| pattern.is_match(&path_str))
    }

    /// Get WeChat database file patterns for macOS.
    pub fn wechat_macos_patterns() -> Vec<Regex> {
        vec![
            Regex::new(r"Message/msg_\d+\.db$").unwrap(),
            Regex::new(r"db_storage/session/session\.db$").unwrap(),
            Regex::new(r"db_storage/chat/chat_\d+\.db$").unwrap(),
        ]
    }
}

/// Debounced file event stream.
pub struct DebouncedEventStream {
    inner: mpsc::Receiver<FileEvent>,
    debounce_interval: Duration,
    max_wait: Duration,
    pending_event: Option<FileEvent>,
    pending_started_at: Option<Instant>,
    pending_last_update_at: Option<Instant>,
}

impl DebouncedEventStream {
    /// Create a new debounced stream.
    pub fn new(inner: mpsc::Receiver<FileEvent>, debounce_ms: u64, max_wait_ms: u64) -> Self {
        Self {
            inner,
            debounce_interval: Duration::from_millis(debounce_ms),
            max_wait: Duration::from_millis(max_wait_ms),
            pending_event: None,
            pending_started_at: None,
            pending_last_update_at: None,
        }
    }

    /// Get next debounced event.
    pub async fn next(&mut self) -> Option<FileEvent> {
        loop {
            if self.pending_event.is_none() {
                match self.inner.recv().await {
                    Some(event) => {
                        let now = Instant::now();
                        self.pending_event = Some(event);
                        self.pending_started_at = Some(now);
                        self.pending_last_update_at = Some(now);
                        continue;
                    }
                    None => return None,
                }
            }

            let started_at = self
                .pending_started_at
                .expect("pending event should have a batch start");
            let last_update_at = self
                .pending_last_update_at
                .expect("pending event should have a last-update timestamp");

            if started_at.elapsed() >= self.max_wait
                || last_update_at.elapsed() >= self.debounce_interval
            {
                return self.take_event();
            }

            let wait_until_emit = last_update_at + self.debounce_interval;
            let wait_until_max = started_at + self.max_wait;
            let sleep_until = wait_until_emit.min(wait_until_max);

            tokio::select! {
                event = self.inner.recv() => {
                    match event {
                        Some(event) => {
                            self.pending_event = Some(event);
                            self.pending_last_update_at = Some(Instant::now());
                        }
                        None => return self.take_event(),
                    }
                }
                _ = tokio::time::sleep_until(sleep_until) => {
                    return self.take_event();
                }
            }
        }
    }

    /// Take the pending event.
    fn take_event(&mut self) -> Option<FileEvent> {
        self.pending_started_at = None;
        self.pending_last_update_at = None;
        self.pending_event.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn debounced_stream_blocks_while_idle() {
        let (_tx, rx) = mpsc::channel(8);
        let mut stream = DebouncedEventStream::new(rx, 10, 50);

        let result = timeout(Duration::from_millis(20), stream.next()).await;
        assert!(
            result.is_err(),
            "idle stream should keep waiting for events"
        );
    }

    #[tokio::test]
    async fn debounced_stream_coalesces_burst_to_latest_event() {
        let (tx, rx) = mpsc::channel(8);
        let mut stream = DebouncedEventStream::new(rx, 20, 100);
        let path = PathBuf::from("/tmp/wechat-message.db");

        tx.send(FileEvent::Created(path.clone()))
            .await
            .expect("send create");
        tokio::time::sleep(Duration::from_millis(5)).await;
        tx.send(FileEvent::Modified(path.clone()))
            .await
            .expect("send modify");

        let event = stream.next().await.expect("debounced event");
        assert_eq!(event, FileEvent::Modified(path));
    }

    #[tokio::test]
    async fn debounced_stream_flushes_pending_event_when_sender_closes() {
        let (tx, rx) = mpsc::channel(8);
        let mut stream = DebouncedEventStream::new(rx, 50, 200);
        let path = PathBuf::from("/tmp/wechat-session.db");

        tx.send(FileEvent::Created(path.clone()))
            .await
            .expect("send event");
        drop(tx);

        let event = stream.next().await.expect("flushed event");
        assert_eq!(event, FileEvent::Created(path));
        assert!(
            stream.next().await.is_none(),
            "closed stream should then finish"
        );
    }

    #[tokio::test]
    async fn debounced_stream_flushes_after_max_wait_under_event_storm() {
        let (tx, rx) = mpsc::channel(16);
        let mut stream = DebouncedEventStream::new(rx, 40, 70);
        let path = PathBuf::from("/tmp/wechat-chat.db");

        tx.send(FileEvent::Created(path.clone()))
            .await
            .expect("send initial event");
        tokio::time::sleep(Duration::from_millis(25)).await;
        tx.send(FileEvent::Modified(path.clone()))
            .await
            .expect("send update");
        tokio::time::sleep(Duration::from_millis(25)).await;
        tx.send(FileEvent::Modified(path.clone()))
            .await
            .expect("send second update");

        let started = Instant::now();
        let event = stream.next().await.expect("event after max wait");
        let elapsed = started.elapsed();

        assert_eq!(event, FileEvent::Modified(path));
        assert!(
            elapsed < Duration::from_millis(90),
            "max wait should prevent unbounded burst delays"
        );
    }
}
