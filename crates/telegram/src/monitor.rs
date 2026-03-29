//! File monitoring for legal-safe Telegram export directories.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crate::TelegramError;

/// File system event types emitted by the Telegram monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// New file created.
    Created(PathBuf),
    /// Existing file modified.
    Modified(PathBuf),
    /// Existing file removed.
    Deleted(PathBuf),
}

/// File monitoring configuration for Telegram exports.
#[derive(Debug, Clone)]
pub struct FileMonitorConfig {
    /// Directory to watch.
    pub watch_dir: PathBuf,
    /// Regex patterns applied against file paths.
    pub file_patterns: Vec<Regex>,
    /// Whether recursive watching is enabled.
    pub recursive: bool,
}

impl Default for FileMonitorConfig {
    fn default() -> Self {
        Self {
            watch_dir: PathBuf::new(),
            file_patterns: vec![],
            recursive: true,
        }
    }
}

/// File system monitor for Telegram export trees.
pub struct FileMonitor {
    config: FileMonitorConfig,
    watcher: RecommendedWatcher,
    event_rx: Receiver<FileEvent>,
}

impl FileMonitor {
    /// Create a new file monitor.
    pub fn new(config: FileMonitorConfig) -> Result<Self, TelegramError> {
        let (event_tx, event_rx) = mpsc::channel();
        let config_clone = config.clone();

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                Self::handle_event(&event, &config_clone, &event_tx);
            }
        })?;

        Ok(Self {
            config,
            watcher,
            event_rx,
        })
    }

    /// Start monitoring.
    pub fn start(&mut self) -> Result<(), TelegramError> {
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self.watcher.watch(&self.config.watch_dir, mode)?;
        Ok(())
    }

    /// Stop monitoring.
    pub fn stop(&mut self) -> Result<(), TelegramError> {
        self.watcher.unwatch(&self.config.watch_dir)?;
        Ok(())
    }

    /// Get the next event, waiting up to the provided timeout.
    pub fn next_event_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<FileEvent>, std::sync::mpsc::RecvTimeoutError> {
        self.event_rx
            .recv_timeout(timeout)
            .map(Some)
            .or_else(|err| {
                if matches!(err, std::sync::mpsc::RecvTimeoutError::Timeout) {
                    Ok(None)
                } else {
                    Err(err)
                }
            })
    }

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

            let mapped = match event.kind {
                EventKind::Create(_) => FileEvent::Created(path.clone()),
                EventKind::Modify(_) => FileEvent::Modified(path.clone()),
                EventKind::Remove(_) => FileEvent::Deleted(path.clone()),
                _ => continue,
            };

            let _ = event_tx.send(mapped);
        }
    }

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

    /// Default path patterns for Telegram export and media folders.
    pub fn telegram_export_patterns() -> Vec<Regex> {
        vec![
            Regex::new(r"(?i)\.(json|html|txt|zip|csv)$").expect("valid export regex"),
            Regex::new(r"(?i)\.(jpg|jpeg|png|webp|gif|mp4|mov|ogg|opus|pdf|tgs)$")
                .expect("valid media regex"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};

    #[test]
    fn export_patterns_match_common_assets() {
        let patterns = FileMonitor::telegram_export_patterns();
        let export_path = Path::new("/tmp/Export/result.json");
        let image_path = Path::new("/tmp/Media/sticker.tgs");

        assert!(patterns
            .iter()
            .any(|pattern| pattern.is_match(&export_path.to_string_lossy())));
        assert!(patterns
            .iter()
            .any(|pattern| pattern.is_match(&image_path.to_string_lossy())));
    }
    #[test]
    fn export_patterns_do_not_match_unrelated_assets() {
        let patterns = FileMonitor::telegram_export_patterns();
        let unrelated_path = Path::new("/tmp/Media/readme.exe");

        assert!(!patterns
            .iter()
            .any(|pattern| pattern.is_match(&unrelated_path.to_string_lossy())));
    }

    #[test]
    fn empty_pattern_configuration_matches_any_path() {
        let config = FileMonitorConfig::default();
        assert!(FileMonitor::matches_pattern(
            Path::new("/tmp/random.asset"),
            &config
        ));
    }

    #[test]
    fn next_event_timeout_returns_none_when_idle() {
        let monitor = FileMonitor::new(FileMonitorConfig::default()).expect("monitor should build");
        let event = monitor
            .next_event_timeout(Duration::from_millis(5))
            .expect("timeout should not error");
        assert!(event.is_none());
    }

    #[test]
    fn handle_event_maps_supported_event_kinds() {
        let config = FileMonitorConfig::default();
        let (tx, rx) = mpsc::channel();
        let path = PathBuf::from("/tmp/export/chat.json");

        let create = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };
        FileMonitor::handle_event(&create, &config, &tx);
        assert_eq!(
            rx.recv().expect("create event"),
            FileEvent::Created(path.clone())
        );

        let modify = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Any)),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };
        FileMonitor::handle_event(&modify, &config, &tx);
        assert_eq!(
            rx.recv().expect("modify event"),
            FileEvent::Modified(path.clone())
        );

        let remove = Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };
        FileMonitor::handle_event(&remove, &config, &tx);
        assert_eq!(rx.recv().expect("remove event"), FileEvent::Deleted(path));
    }

    #[test]
    fn handle_event_ignores_paths_that_do_not_match_patterns() {
        let config = FileMonitorConfig {
            file_patterns: vec![Regex::new(r"(?i)\.zip$").expect("valid regex")],
            ..FileMonitorConfig::default()
        };
        let (tx, rx) = mpsc::channel();
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/tmp/export/chat.jpg")],
            attrs: Default::default(),
        };

        FileMonitor::handle_event(&event, &config, &tx);
        assert!(rx.try_recv().is_err());
    }
}
