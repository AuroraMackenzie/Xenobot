//! File system monitoring for WeChat data directories.

use crate::error::{WeChatError, WeChatResult};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

/// File system event types.
#[derive(Debug, Clone)]
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
    event_tx: mpsc::Sender<FileEvent>,
    event_rx: mpsc::Receiver<FileEvent>,
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
        .map_err(|e| WeChatError::FileMonitor(e.into()))?;

        Ok(Self {
            config,
            watcher,
            event_tx,
            event_rx,
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
            .map_err(|e| WeChatError::FileMonitor(e.into()))?;

        Ok(())
    }

    /// Stop monitoring.
    pub fn stop(&mut self) -> WeChatResult<()> {
        self.watcher
            .unwatch(&self.config.watch_dir)
            .map_err(|e| WeChatError::FileMonitor(e.into()))
    }

    /// Get next file event with debouncing.
    pub async fn next_event(&mut self) -> Option<FileEvent> {
        self.event_rx.recv().await
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
    last_event: Option<(Instant, FileEvent)>,
}

impl DebouncedEventStream {
    /// Create a new debounced stream.
    pub fn new(inner: mpsc::Receiver<FileEvent>, debounce_ms: u64, max_wait_ms: u64) -> Self {
        Self {
            inner,
            debounce_interval: Duration::from_millis(debounce_ms),
            max_wait: Duration::from_millis(max_wait_ms),
            last_event: None,
        }
    }

    /// Get next debounced event.
    pub async fn next(&mut self) -> Option<FileEvent> {
        let start = Instant::now();

        loop {
            let timeout = if let Some((last_time, _)) = &self.last_event {
                let elapsed = last_time.elapsed();
                if elapsed >= self.max_wait {
                    // Max wait reached, emit event
                    return self.take_event();
                }
                self.debounce_interval.saturating_sub(elapsed)
            } else {
                self.debounce_interval
            };

            tokio::select! {
                event = self.inner.recv() => {
                    if let Some(event) = event {
                        self.last_event = Some((Instant::now(), event));
                    } else {
                        return self.take_event();
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    return self.take_event();
                }
            }

            if start.elapsed() >= self.max_wait {
                return self.take_event();
            }
        }
    }

    /// Take the pending event.
    fn take_event(&mut self) -> Option<FileEvent> {
        self.last_event.take().map(|(_, event)| event)
    }
}
