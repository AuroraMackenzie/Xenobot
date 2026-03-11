//! Main WeChat service orchestrating data extraction and decryption.

use crate::account::{primary_account as primary_source_account, Account, WeChatDetector};
use crate::audio::{
    has_ffmpeg, transcode_audio_bytes_to_mp3, transcode_audio_to_mp3, AudioTranscodeOptions,
};
use crate::config::WeChatConfig;
use crate::decrypt::{decrypt_v4_database, V4DecryptionParams};
use crate::error::{WeChatError, WeChatResult};
use crate::media::{collect_media_assets, WeChatMediaAsset};
use crate::monitor::{FileMonitor, FileMonitorConfig};
use dashmap::DashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{error, info};
use xenobot_analysis::parsers::{ParsedChat, ParserRegistry};
use xenobot_core::platform_sources::{discover_sources_for_platform, SourceCandidate};
use xenobot_core::types::Platform;

/// Parsed export staged by the WeChat service.
#[derive(Debug, Clone)]
pub struct StagedWeChatExport {
    /// Original source file path.
    pub source_path: PathBuf,
    /// Stable platform identifier.
    pub platform_id: &'static str,
    /// Parsed normalized chat content.
    pub parsed: ParsedChat,
}

/// Aggregated legal-safe workspace assembled from explicit WeChat inputs.
#[derive(Debug, Clone)]
pub struct AuthorizedWeChatWorkspace {
    /// Stable platform identifier.
    pub platform_id: &'static str,
    /// Account views discovered from local sources.
    pub accounts: Vec<Account>,
    /// Preferred primary account view.
    pub primary_account: Option<Account>,
    /// Parsed exports explicitly staged by the user.
    pub staged_exports: Vec<StagedWeChatExport>,
    /// Classified media assets explicitly provided by the user.
    pub media_inventory: Vec<WeChatMediaAsset>,
    /// Optional authorized watch root prepared for incremental monitoring.
    pub watch_dir: Option<PathBuf>,
}

impl AuthorizedWeChatWorkspace {
    /// Return the number of staged exports.
    pub fn export_count(&self) -> usize {
        self.staged_exports.len()
    }

    /// Return the number of indexed media assets.
    pub fn media_count(&self) -> usize {
        self.media_inventory.len()
    }

    /// Return whether the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.staged_exports.is_empty() && self.media_inventory.is_empty()
    }
}

/// Main WeChat service.
type WeChatResolvedKeys = (Vec<u8>, Vec<u8>);

/// Legal-safe WeChat orchestration service combining source discovery, export staging,
/// media inventory assembly, monitor preparation, and authorized audio helpers.
pub struct WeChatService {
    config: WeChatConfig,
    detector: Arc<dyn WeChatDetector>,
    running: bool,
    tasks: Vec<JoinHandle<()>>,
    event_tx: mpsc::Sender<ServiceEvent>,
    event_rx: mpsc::Receiver<ServiceEvent>,
    accounts: Arc<DashMap<u32, Account>>,
    keys: Arc<DashMap<u32, WeChatResolvedKeys>>, // (data_key, img_key) per PID
}

/// Service events.
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    /// New WeChat instance detected.
    InstanceDetected(Account),
    /// WeChat instance terminated.
    InstanceTerminated(u32),
    /// New database file detected.
    DatabaseFile(PathBuf),
    /// Database decryption completed.
    DecryptionComplete {
        /// Path to the encrypted database file.
        input_path: PathBuf,
        /// Path where the decrypted database was saved.
        output_path: PathBuf,
        /// Whether decryption succeeded.
        success: bool,
        /// Error message if decryption failed.
        error: Option<String>,
    },
    /// Key resolution completed.
    KeyExtractionComplete {
        /// Process ID of the WeChat instance.
        pid: u32,
        /// Whether key resolution succeeded.
        success: bool,
        /// Error message if extraction failed.
        error: Option<String>,
    },
    /// Service error.
    Error(String),
}

impl WeChatService {
    /// Stable platform identifier for WeChat workflows.
    pub const PLATFORM_ID: &'static str = "wechat";

    /// Create a new WeChat service.
    pub fn new(config: WeChatConfig) -> WeChatResult<Self> {
        let detector = Arc::new(Self::create_detector());

        Ok(Self::new_with_detector(config, detector))
    }

    fn new_with_detector(config: WeChatConfig, detector: Arc<dyn WeChatDetector>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);

        Self {
            config,
            detector,
            running: false,
            tasks: vec![],
            event_tx,
            event_rx,
            accounts: Arc::new(DashMap::new()),
            keys: Arc::new(DashMap::new()),
        }
    }

    /// Return the stable platform identifier.
    pub fn platform_id(&self) -> &'static str {
        Self::PLATFORM_ID
    }

    /// Return source candidates discovered from the local machine.
    pub fn discover_sources(&self) -> Vec<SourceCandidate> {
        discover_sources_for_platform(&Platform::WeChat)
    }

    /// Discover normalized account views derived from authorized local sources.
    pub fn discover_accounts(&self) -> Vec<Account> {
        let sources = self.discover_sources();
        crate::account::collect_accounts_from_sources(&sources)
    }

    /// Return the preferred primary account view when one is available.
    pub fn primary_account(&self) -> Option<Account> {
        let sources = self.discover_sources();
        primary_source_account(&sources)
    }

    /// Return the currently configured authorized roots.
    pub fn authorized_roots(&self) -> &[PathBuf] {
        self.config.authorized_roots()
    }

    /// Add an authorized root directory at runtime.
    pub fn add_authorized_root<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        self.config.add_authorized_root(path);
    }

    /// Parse one explicitly authorized export file.
    pub fn parse_authorized_export(&self, path: &Path) -> Result<ParsedChat, WeChatError> {
        self.ensure_authorized(path)?;

        let registry = ParserRegistry::new();
        let parsed = registry.detect_and_parse(path)?;

        if parsed.platform.eq_ignore_ascii_case(Self::PLATFORM_ID) {
            Ok(parsed)
        } else {
            Err(WeChatError::PlatformMismatch {
                expected: Self::PLATFORM_ID.to_string(),
                actual: parsed.platform,
            })
        }
    }

    /// Parse and stage multiple explicitly authorized export files.
    pub fn stage_authorized_exports<I, P>(
        &self,
        paths: I,
    ) -> Result<Vec<StagedWeChatExport>, WeChatError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        paths.into_iter()
            .map(|path| {
                let source_path = path.as_ref().to_path_buf();
                let parsed = self.parse_authorized_export(&source_path)?;
                Ok(StagedWeChatExport {
                    source_path,
                    platform_id: self.platform_id(),
                    parsed,
                })
            })
            .collect()
    }

    /// Build a legal-safe media inventory from explicitly authorized asset paths.
    pub fn collect_media_inventory<I, P>(
        &self,
        paths: I,
    ) -> Result<Vec<WeChatMediaAsset>, WeChatError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let authorized_paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .map(|path| {
                self.ensure_authorized(&path)?;
                Ok(path)
            })
            .collect::<Result<_, WeChatError>>()?;

        Ok(collect_media_assets(authorized_paths.iter().map(PathBuf::as_path)))
    }

    /// Build an aggregated legal-safe workspace from explicit exports and assets.
    pub fn build_authorized_workspace<I, P, J, Q>(
        &self,
        export_paths: I,
        media_paths: J,
    ) -> Result<AuthorizedWeChatWorkspace, WeChatError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
        J: IntoIterator<Item = Q>,
        Q: AsRef<Path>,
    {
        let accounts = self.discover_accounts();
        let primary_account = self.primary_account();
        let staged_exports = self.stage_authorized_exports(export_paths)?;
        let media_inventory = self.collect_media_inventory(media_paths)?;

        Ok(AuthorizedWeChatWorkspace {
            platform_id: self.platform_id(),
            accounts,
            primary_account,
            staged_exports,
            media_inventory,
            watch_dir: None,
        })
    }

    /// Prepare a legal-safe workspace and optional export monitor in one step.
    pub fn prepare_authorized_workspace<I, P, J, Q>(
        &self,
        export_paths: I,
        media_paths: J,
        watch_dir: Option<&Path>,
    ) -> Result<(AuthorizedWeChatWorkspace, Option<FileMonitor>), WeChatError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
        J: IntoIterator<Item = Q>,
        Q: AsRef<Path>,
    {
        let mut workspace = self.build_authorized_workspace(export_paths, media_paths)?;
        let monitor = watch_dir
            .map(|path| {
                workspace.watch_dir = Some(path.to_path_buf());
                self.create_export_monitor(path)
            })
            .transpose()?;

        Ok((workspace, monitor))
    }

    /// Return whether `ffmpeg` is available for authorized media transcoding.
    pub fn ffmpeg_available(&self) -> bool {
        has_ffmpeg(None)
    }

    /// Convert one explicitly authorized audio asset into MP3.
    pub fn transcode_audio_asset_to_mp3(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &AudioTranscodeOptions,
    ) -> WeChatResult<()> {
        self.ensure_authorized(input_path)?;
        if let Some(parent) = output_path.parent() {
            self.ensure_authorized(parent)?;
        }
        transcode_audio_to_mp3(input_path, output_path, options)
    }

    /// Convert an in-memory audio payload into MP3 bytes.
    pub fn transcode_audio_payload_to_mp3(
        &self,
        input_bytes: &[u8],
        input_format: &str,
        options: &AudioTranscodeOptions,
    ) -> WeChatResult<Vec<u8>> {
        transcode_audio_bytes_to_mp3(input_bytes, input_format, options)
    }

    /// Create a file monitor rooted in an explicitly authorized export directory.
    pub fn create_export_monitor(
        &self,
        watch_dir: impl AsRef<Path>,
    ) -> Result<FileMonitor, WeChatError> {
        let watch_dir = watch_dir.as_ref();
        self.ensure_authorized(watch_dir)?;

        FileMonitor::new(FileMonitorConfig {
            watch_dir: watch_dir.to_path_buf(),
            file_patterns: FileMonitor::wechat_macos_patterns(),
            debounce_ms: 1000,
            max_wait_ms: 10000,
            recursive: true,
        })
    }

    /// Start the service.
    pub async fn start(&mut self) -> WeChatResult<()> {
        if self.running {
            return Ok(());
        }

        info!("Starting WeChat service");
        self.running = true;

        // Preload authorized static keys if provided by configuration.
        if let Some((data_key, img_key)) = parse_config_key_pair(&self.config)? {
            self.keys.insert(0, (data_key, img_key));
        }

        // Start instance detection
        self.start_instance_detection();

        // Start file monitoring if auto-decrypt enabled
        if self.config.auto_decrypt {
            self.start_file_monitoring().map_err(|e| {
                error!("Failed to start file monitoring: {}", e);
                e
            })?;
        }

        info!("WeChat service started");
        Ok(())
    }

    /// Stop the service.
    pub async fn stop(&mut self) -> WeChatResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping WeChat service");
        self.running = false;

        // Cancel all tasks
        for task in self.tasks.drain(..) {
            task.abort();
        }

        info!("WeChat service stopped");
        Ok(())
    }

    /// Get next service event.
    pub async fn next_event(&mut self) -> Option<ServiceEvent> {
        self.event_rx.recv().await
    }

    /// Get all currently available account views.
    ///
    /// Runtime-detected WeChat instances take precedence. When no live instance cache exists,
    /// fall back to the legal-safe local discovery view so callers still get a stable account
    /// list without having to trigger instance detection first.
    pub fn get_accounts(&self) -> Vec<Account> {
        let runtime_accounts: Vec<Account> = self
            .accounts
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        if runtime_accounts.is_empty() {
            self.discover_accounts()
        } else {
            runtime_accounts
        }
    }

    /// Manually trigger instance detection.
    pub async fn detect_instances(&self) -> WeChatResult<Vec<Account>> {
        let instances = self.detector.get_running_instances();

        for account in &instances {
            if account.is_running() {
                self.accounts.insert(account.pid, account.clone());
                self.apply_fallback_keys_for_pid(account.pid).await?;
            }
        }

        Ok(instances)
    }

    async fn apply_fallback_keys_for_pid(&self, pid: u32) -> WeChatResult<()> {
        if self.keys.contains_key(&pid) {
            return Ok(());
        }

        if let Some((data_key, img_key)) = self.keys.get(&0).map(|entry| entry.clone()) {
            self.keys.insert(pid, (data_key, img_key));
            let _ = self
                .event_tx
                .send(ServiceEvent::KeyExtractionComplete {
                    pid,
                    success: true,
                    error: None,
                })
                .await;
            return Ok(());
        }

        if let Some((data_key, img_key)) = parse_config_key_pair(&self.config)? {
            self.keys.insert(0, (data_key.clone(), img_key.clone()));
            self.keys.insert(pid, (data_key, img_key));
            let _ = self
                .event_tx
                .send(ServiceEvent::KeyExtractionComplete {
                    pid,
                    success: true,
                    error: None,
                })
                .await;
        }

        Ok(())
    }

    /// Manually resolve keys for a WeChat instance.
    pub async fn extract_keys_for_instance(&self, pid: u32) -> WeChatResult<(Vec<u8>, Vec<u8>)> {
        info!("Resolving keys for WeChat instance PID: {}", pid);

        match self.detector.extract_keys(pid) {
            Ok((data_key_hex, img_key_hex)) => {
                // Convert hex strings to bytes
                let data_key = hex::decode(&data_key_hex).map_err(|e| {
                    WeChatError::KeyExtraction(format!("Invalid data key hex: {}", e))
                })?;
                let img_key = hex::decode(&img_key_hex).map_err(|e| {
                    WeChatError::KeyExtraction(format!("Invalid image key hex: {}", e))
                })?;

                self.keys.insert(pid, (data_key.clone(), img_key.clone()));

                let event = ServiceEvent::KeyExtractionComplete {
                    pid,
                    success: true,
                    error: None,
                };

                let _ = self.event_tx.send(event).await;

                Ok((data_key, img_key))
            }
            Err(e) => {
                let event = ServiceEvent::KeyExtractionComplete {
                    pid,
                    success: false,
                    error: Some(e.clone()),
                };

                let _ = self.event_tx.send(event).await;

                Err(WeChatError::KeyExtraction(e))
            }
        }
    }

    /// Manually decrypt a database file.
    pub async fn decrypt_database(&self, input_path: PathBuf, pid: u32) -> WeChatResult<PathBuf> {
        info!("Decrypting database: {:?} for PID: {}", input_path, pid);

        self.ensure_authorized(&input_path)?;
        let output_path = self.get_output_path(&input_path, pid);
        self.ensure_authorized(&output_path)?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(WeChatError::Io)?;
        }

        // Get keys for this PID
        let (data_key, img_key) = self
            .keys
            .get(&pid)
            .map(|entry| (entry.0.clone(), entry.1.clone()))
            .ok_or_else(|| WeChatError::Decryption(format!("No keys found for PID: {}", pid)))?;

        // Extract salt from database
        let salt = crate::decrypt::extract_v4_salt(&input_path)?;

        // Create decryption parameters
        let params = V4DecryptionParams::new(data_key, img_key, salt);

        // Decrypt
        match decrypt_v4_database(&input_path, &output_path, &params) {
            Ok(()) => {
                let event = ServiceEvent::DecryptionComplete {
                    input_path: input_path.clone(),
                    output_path: output_path.clone(),
                    success: true,
                    error: None,
                };

                let _ = self.event_tx.send(event).await;

                Ok(output_path)
            }
            Err(e) => {
                let event = ServiceEvent::DecryptionComplete {
                    input_path: input_path.clone(),
                    output_path: output_path.clone(),
                    success: false,
                    error: Some(e.to_string()),
                };

                let _ = self.event_tx.send(event).await;

                Err(e)
            }
        }
    }

    /// Start instance detection task.
    fn start_instance_detection(&mut self) {
        let detector = self.detector.clone();
        let event_tx = self.event_tx.clone();
        let accounts = self.accounts.clone();
        let keys = self.keys.clone();

        let task = tokio::spawn(async move {
            let mut known_pids: HashSet<u32> = HashSet::new();
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;
                let instances = detector.get_running_instances();
                let mut active_pids: HashSet<u32> = HashSet::new();

                for account in instances {
                    if !account.is_running() {
                        continue;
                    }

                    let pid = account.pid;
                    active_pids.insert(pid);
                    let is_new = !known_pids.contains(&pid);

                    accounts.insert(pid, account.clone());

                    if is_new {
                        known_pids.insert(pid);

                        if let Some(fallback_keys) = keys.get(&0).map(|entry| entry.clone()) {
                            keys.insert(pid, fallback_keys);
                            let _ = event_tx
                                .send(ServiceEvent::KeyExtractionComplete {
                                    pid,
                                    success: true,
                                    error: None,
                                })
                                .await;
                        }

                        let _ = event_tx.send(ServiceEvent::InstanceDetected(account)).await;
                    }
                }

                let terminated: Vec<u32> = known_pids
                    .iter()
                    .copied()
                    .filter(|pid| !active_pids.contains(pid))
                    .collect();
                for pid in terminated {
                    known_pids.remove(&pid);
                    accounts.remove(&pid);
                    keys.remove(&pid);
                    let _ = event_tx.send(ServiceEvent::InstanceTerminated(pid)).await;
                }
            }
        });

        self.tasks.push(task);
    }

    /// Start file monitoring.
    fn start_file_monitoring(&mut self) -> WeChatResult<()> {
        let mut monitor = self.create_export_monitor(Path::new(&self.config.data_dir))?;
        monitor.start()?;
        self.start_monitoring_task(monitor);

        Ok(())
    }

    /// Start file monitoring task.
    fn start_monitoring_task(&mut self, mut monitor: FileMonitor) {
        let event_tx = self.event_tx.clone();
        let keys = self.keys.clone();
        let work_dir = self.config.work_dir.clone();

        let task = tokio::spawn(async move {
            while let Some(file_event) = monitor.next_event().await {
                let db_path = match file_event {
                    crate::monitor::FileEvent::Created(path)
                    | crate::monitor::FileEvent::Modified(path) => path,
                    crate::monitor::FileEvent::Deleted(_) => continue,
                };

                if !is_database_file(&db_path) {
                    continue;
                }

                let _ = event_tx
                    .send(ServiceEvent::DatabaseFile(db_path.clone()))
                    .await;

                let Some((pid, data_key, img_key)) = first_available_key(&keys) else {
                    let _ = event_tx
                        .send(ServiceEvent::Error(
                            "no available keys for auto decrypt".to_string(),
                        ))
                        .await;
                    continue;
                };

                let output_path = build_output_path_from_work_dir(&work_dir, &db_path, pid);
                let input_path = db_path.clone();
                let output_path_for_job = output_path.clone();
                let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
                    let salt =
                        crate::decrypt::extract_v4_salt(&input_path).map_err(|e| e.to_string())?;
                    let params = V4DecryptionParams::new(data_key, img_key, salt);
                    decrypt_v4_database(&input_path, &output_path_for_job, &params)
                        .map_err(|e| e.to_string())
                })
                .await;

                match result {
                    Ok(Ok(())) => {
                        let _ = event_tx
                            .send(ServiceEvent::DecryptionComplete {
                                input_path: db_path,
                                output_path,
                                success: true,
                                error: None,
                            })
                            .await;
                    }
                    Ok(Err(err_msg)) => {
                        let _ = event_tx
                            .send(ServiceEvent::DecryptionComplete {
                                input_path: db_path,
                                output_path,
                                success: false,
                                error: Some(err_msg),
                            })
                            .await;
                    }
                    Err(join_err) => {
                        let _ = event_tx
                            .send(ServiceEvent::DecryptionComplete {
                                input_path: db_path,
                                output_path,
                                success: false,
                                error: Some(format!("decrypt task aborted: {}", join_err)),
                            })
                            .await;
                    }
                }
            }
        });

        self.tasks.push(task);
    }

    /// Get output path for decrypted database.
    fn get_output_path(&self, input_path: &Path, pid: u32) -> PathBuf {
        let file_name = input_path.file_name().unwrap_or_default();
        let output_dir = PathBuf::from(&self.config.work_dir).join(pid.to_string());

        std::fs::create_dir_all(&output_dir).ok();

        output_dir.join(file_name)
    }

    fn ensure_authorized(&self, path: &Path) -> Result<(), WeChatError> {
        if self.config.is_authorized_path(path) {
            Ok(())
        } else {
            Err(WeChatError::UnauthorizedPath {
                path: path.to_path_buf(),
            })
        }
    }

    /// Create platform-specific detector.
    fn create_detector() -> impl WeChatDetector {
        #[cfg(target_os = "macos")]
        {
            crate::account::macos::MacOSWeChatDetector
        }

        #[cfg(target_os = "windows")]
        {
            crate::account::windows::WindowsWeChatDetector
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            struct UnsupportedDetector;
            impl WeChatDetector for UnsupportedDetector {
                fn get_running_instances(&self) -> Vec<Account> {
                    vec![]
                }
                fn extract_keys(&self, _pid: u32) -> Result<(String, String), String> {
                    Err("Platform not supported".to_string())
                }
            }
            UnsupportedDetector
        }
    }
}

impl Clone for WeChatService {
    fn clone(&self) -> Self {
        // Create new service with same config
        // Note: This creates new channels and state
        let (event_tx, event_rx) = mpsc::channel(100);

        Self {
            config: self.config.clone(),
            detector: self.detector.clone(),
            running: false,
            tasks: vec![],
            event_tx,
            event_rx,
            accounts: Arc::new(DashMap::new()),
            keys: Arc::new(DashMap::new()),
        }
    }
}

fn normalize_hex_key(raw: &str, expected_len: usize, label: &str) -> WeChatResult<Vec<u8>> {
    let mut value = raw.trim().to_ascii_lowercase();
    if let Some(rest) = value.strip_prefix("0x") {
        value = rest.to_string();
    }
    if value.len() != expected_len {
        return Err(WeChatError::Config(format!(
            "{} must be {} hex chars, got {}",
            label,
            expected_len,
            value.len()
        )));
    }
    if !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(WeChatError::Config(format!(
            "{} contains non-hex characters",
            label
        )));
    }
    hex::decode(value).map_err(|e| WeChatError::Config(format!("{} decode failed: {}", label, e)))
}

fn parse_config_key_pair(config: &WeChatConfig) -> WeChatResult<Option<(Vec<u8>, Vec<u8>)>> {
    match (&config.data_key, &config.img_key) {
        (Some(data_key), Some(img_key)) => {
            let data = normalize_hex_key(data_key, 64, "data_key")?;
            let image = normalize_hex_key(img_key, 32, "img_key")?;
            Ok(Some((data, image)))
        }
        (None, None) => Ok(None),
        _ => Err(WeChatError::Config(
            "data_key and img_key must be provided together".to_string(),
        )),
    }
}

fn is_database_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("db"))
        .unwrap_or(false)
}

fn first_available_key(keys: &DashMap<u32, (Vec<u8>, Vec<u8>)>) -> Option<(u32, Vec<u8>, Vec<u8>)> {
    keys.iter()
        .find_map(|entry| {
            if *entry.key() == 0 {
                return None;
            }
            Some((
                *entry.key(),
                entry.value().0.clone(),
                entry.value().1.clone(),
            ))
        })
        .or_else(|| {
            keys.get(&0)
                .map(|entry| (0, entry.value().0.clone(), entry.value().1.clone()))
        })
}

fn build_output_path_from_work_dir(work_dir: &str, input_path: &Path, pid: u32) -> PathBuf {
    let file_name = input_path.file_name().unwrap_or_default();
    let output_dir = PathBuf::from(work_dir).join(pid.to_string());
    std::fs::create_dir_all(&output_dir).ok();
    output_dir.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::tempdir;
    use tokio::time::{timeout, Duration as TokioDuration};

    #[derive(Debug)]
    struct MockDetector {
        instances: Vec<Account>,
        key_result: Mutex<Result<(String, String), String>>,
    }

    impl MockDetector {
        fn new(instances: Vec<Account>, key_result: Result<(String, String), String>) -> Self {
            Self {
                instances,
                key_result: Mutex::new(key_result),
            }
        }
    }

    impl WeChatDetector for MockDetector {
        fn get_running_instances(&self) -> Vec<Account> {
            self.instances.clone()
        }

        fn extract_keys(&self, _pid: u32) -> Result<(String, String), String> {
            self.key_result
                .lock()
                .expect("mock detector lock")
                .clone()
        }
    }

    fn write_fixture(path: &Path, content: &str) {
        fs::write(path, content).expect("write fixture");
    }

    fn running_account(pid: u32, data_dir: &Path) -> Account {
        let mut account = Account::new(
            pid,
            format!("WeChat-{pid}"),
            data_dir.to_string_lossy().into_owned(),
            "4.0.0".to_string(),
            "4.0.0-build".to_string(),
            "macOS".to_string(),
        );
        account.is_current = true;
        account
    }

    fn account_signature(account: &Account) -> (u32, String, String, String, String, String, bool) {
        (
            account.pid,
            account.name.clone(),
            account.data_dir.clone(),
            account.version.clone(),
            account.full_version.clone(),
            account.platform.clone(),
            account.is_current,
        )
    }

    fn account_signatures(accounts: &[Account]) -> Vec<(u32, String, String, String, String, String, bool)> {
        accounts.iter().map(account_signature).collect()
    }

    #[test]
    fn returns_platform_id_from_service() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        assert_eq!(service.platform_id(), "wechat");
    }

    #[test]
    fn discovers_sources_for_platform() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let sources = service.discover_sources();
        assert!(!sources.is_empty());
        assert!(sources
            .iter()
            .all(|candidate| candidate.platform_id == "wechat"));
    }

    #[test]
    fn rejects_paths_outside_authorized_roots() {
        let service = WeChatService::new(WeChatConfig::with_authorized_roots([PathBuf::from(
            "/tmp/allowed",
        )]))
        .expect("service should build");

        let err = service
            .parse_authorized_export(Path::new("/tmp/other/export.zip"))
            .expect_err("path outside authorized roots should fail before parsing");

        match err {
            WeChatError::UnauthorizedPath { path } => {
                assert_eq!(path, PathBuf::from("/tmp/other/export.zip"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn discovers_account_views_from_sources() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let accounts = service.discover_accounts();

        assert!(!accounts.is_empty());
        assert!(accounts.iter().all(|account| !account.name.trim().is_empty()));
    }

    #[test]
    fn get_accounts_matches_discovered_accounts() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let discovered = service.discover_accounts();
        let exposed = service.get_accounts();

        assert_eq!(account_signatures(&exposed), account_signatures(&discovered));
    }

    #[test]
    fn builds_media_inventory_for_authorized_assets() {
        let dir = tempdir().expect("tempdir");
        let asset = dir.path().join("image.dat");
        fs::write(&asset, [1_u8, 2, 3]).expect("write media asset");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path()]))
            .expect("service should build");
        let inventory = service
            .collect_media_inventory([asset.as_path()])
            .expect("authorized media inventory");

        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].kind, crate::media::WeChatMediaKind::EncryptedDatImage);
    }

    #[test]
    fn creates_monitor_for_authorized_directory() {
        let dir = tempdir().expect("tempdir");
        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path()]))
            .expect("service should build");

        let monitor = service.create_export_monitor(dir.path());
        assert!(monitor.is_ok());
    }

    #[test]
    fn add_authorized_root_allows_runtime_monitor_creation() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");

        let mut service =
            WeChatService::new(WeChatConfig::with_authorized_roots([other_dir.path()]))
                .expect("service should build");
        assert!(matches!(
            service.create_export_monitor(dir.path()),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        assert!(service.create_export_monitor(dir.path()).is_ok());
    }


    #[test]
    fn build_authorized_workspace_rejects_unauthorized_media_paths() {
        let export_dir = tempdir().expect("tempdir");
        let media_dir = tempdir().expect("tempdir");
        let export = export_dir.path().join("wechat_fixture.json");
        let media = media_dir.path().join("preview.jpg");
        write_fixture(&export, r#"[{"msgSvrId":"1","type":1,"createTime":1735813230,"talker":"alice","content":"hello wechat"}]"#);
        std::fs::write(&media, [1_u8, 2, 3]).expect("media fixture");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([
            export_dir.path().to_path_buf(),
        ]))
        .expect("service");

        match service.build_authorized_workspace([export.as_path()], [media.as_path()]) {
            Err(WeChatError::UnauthorizedPath { path }) => assert_eq!(path, media),
            _ => panic!("expected unauthorized media path"),
        }
    }

    #[test]
    fn build_authorized_workspace_rejects_unauthorized_export_paths() {
        let export_dir = tempdir().expect("tempdir");
        let unauthorized_dir = tempdir().expect("tempdir");
        let export = unauthorized_dir.path().join("wechat_unauthorized_fixture.dat");
        std::fs::write(&export, [1_u8, 2, 3]).expect("export fixture");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([
            export_dir.path().to_path_buf(),
        ]))
        .expect("service");

        match service.build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>()) {
            Err(WeChatError::UnauthorizedPath { path }) => assert_eq!(path, export),
            _ => panic!("expected unauthorized export path"),
        }
    }

    #[test]
    fn builds_authorized_workspace_from_exports_and_media() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");
        let asset = dir.path().join("image.dat");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );
        fs::write(&asset, [1_u8, 2, 3]).expect("write media");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path()]))
            .expect("service should build");
        let workspace = service
            .build_authorized_workspace([export.as_path()], [asset.as_path()])
            .expect("workspace should build");
        let workspace_accounts: Vec<_> = workspace
            .accounts
            .iter()
            .map(|account| {
                (
                    account.pid,
                    account.name.clone(),
                    account.data_dir.clone(),
                    account.version.clone(),
                    account.full_version.clone(),
                    account.platform.clone(),
                    account.is_current,
                )
            })
            .collect();
        let discovered_accounts: Vec<_> = service
            .discover_accounts()
            .iter()
            .map(|account| {
                (
                    account.pid,
                    account.name.clone(),
                    account.data_dir.clone(),
                    account.version.clone(),
                    account.full_version.clone(),
                    account.platform.clone(),
                    account.is_current,
                )
            })
            .collect();
        let workspace_primary = workspace.primary_account.as_ref().map(|account| {
            (
                account.pid,
                account.name.clone(),
                account.data_dir.clone(),
                account.version.clone(),
                account.full_version.clone(),
                account.platform.clone(),
                account.is_current,
            )
        });
        let discovered_primary = service.primary_account().as_ref().map(|account| {
            (
                account.pid,
                account.name.clone(),
                account.data_dir.clone(),
                account.version.clone(),
                account.full_version.clone(),
                account.platform.clone(),
                account.is_current,
            )
        });

        assert_eq!(workspace.platform_id, "wechat");
        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 1);
        assert!(!workspace.accounts.is_empty());
        assert!(workspace.primary_account.is_some());
        assert_eq!(workspace_accounts, discovered_accounts);
        assert_eq!(workspace_primary, discovered_primary);
    }

    #[test]
    fn prepares_authorized_workspace_with_monitor() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path()]))
            .expect("service should build");
        let (workspace, monitor) = service
            .prepare_authorized_workspace([export.as_path()], std::iter::empty::<&Path>(), Some(dir.path()))
            .expect("workspace and monitor should build");
        let workspace_accounts: Vec<_> = workspace
            .accounts
            .iter()
            .map(|account| {
                (
                    account.pid,
                    account.name.clone(),
                    account.data_dir.clone(),
                    account.version.clone(),
                    account.full_version.clone(),
                    account.platform.clone(),
                    account.is_current,
                )
            })
            .collect();
        let discovered_accounts: Vec<_> = service
            .discover_accounts()
            .iter()
            .map(|account| {
                (
                    account.pid,
                    account.name.clone(),
                    account.data_dir.clone(),
                    account.version.clone(),
                    account.full_version.clone(),
                    account.platform.clone(),
                    account.is_current,
                )
            })
            .collect();
        let workspace_primary = workspace.primary_account.as_ref().map(|account| {
            (
                account.pid,
                account.name.clone(),
                account.data_dir.clone(),
                account.version.clone(),
                account.full_version.clone(),
                account.platform.clone(),
                account.is_current,
            )
        });
        let discovered_primary = service.primary_account().as_ref().map(|account| {
            (
                account.pid,
                account.name.clone(),
                account.data_dir.clone(),
                account.version.clone(),
                account.full_version.clone(),
                account.platform.clone(),
                account.is_current,
            )
        });

        assert_eq!(workspace.watch_dir.as_deref(), Some(dir.path()));
        assert!(monitor.is_some());
        assert_eq!(workspace_accounts, discovered_accounts);
        assert_eq!(workspace_primary, discovered_primary);
    }

    #[test]
    fn prepare_authorized_workspace_rejects_unauthorized_watch_directory() {
        let input_dir = tempdir().expect("tempdir");
        let watch_dir = tempdir().expect("tempdir");
        let export = input_dir.path().join("wechat_fixture.json");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([input_dir.path()]))
            .expect("service should build");
        match service.prepare_authorized_workspace(
            [export.as_path()],
            std::iter::empty::<&Path>(),
            Some(watch_dir.path()),
        ) {
            Err(WeChatError::UnauthorizedPath { path }) => {
                assert_eq!(path, watch_dir.path().to_path_buf());
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("unauthorized watch directory should fail"),
        }
    }

    #[test]
    fn rejects_audio_asset_transcoding_when_output_directory_is_not_authorized() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.silk");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, [1_u8, 2, 3]).expect("write input");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([input_dir.path().to_path_buf()])).expect("service should build");
        let error = service
            .transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default())
            .expect_err("unauthorized output directory should fail");

        match error {
            WeChatError::UnauthorizedPath { path } => {
                assert_eq!(path, output_dir.path().to_path_buf());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_audio_asset_transcoding_when_input_path_is_not_authorized() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.silk");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, [1_u8, 2, 3]).expect("write input");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([
            output_dir.path().to_path_buf(),
        ]))
        .expect("service should build");
        let error = service
            .transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default())
            .expect_err("unauthorized input path should fail");

        match error {
            WeChatError::UnauthorizedPath { path } => {
                assert_eq!(path, input);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn add_authorized_root_allows_runtime_workspace_build() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let mut service =
            WeChatService::new(WeChatConfig::with_authorized_roots([other_dir.path()]))
                .expect("service should build");
        assert!(matches!(
            service.build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>()),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let workspace = service
            .build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>())
            .expect("runtime authorization should allow workspace build");
        assert_eq!(workspace.export_count(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_export_parsing() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let mut service =
            WeChatService::new(WeChatConfig::with_authorized_roots([other_dir.path()]))
                .expect("service should build");
        assert!(matches!(
            service.parse_authorized_export(&export),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let parsed = service
            .parse_authorized_export(&export)
            .expect("runtime authorization should allow export parsing");
        assert_eq!(parsed.platform, "wechat");
        assert_eq!(parsed.messages.len(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_export_staging() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");

        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let mut service =
            WeChatService::new(WeChatConfig::with_authorized_roots([other_dir.path()]))
                .expect("service should build");
        assert!(matches!(
            service.stage_authorized_exports([export.as_path()]),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let staged = service
            .stage_authorized_exports([export.as_path()])
            .expect("runtime authorization should allow export staging");
        assert_eq!(staged.len(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_audio_input_validation_to_progress() {
        let input_dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.silk");
        let output = other_dir.path().join("voice.mp3");
        fs::write(&input, []).expect("write empty input");

        let mut service =
            WeChatService::new(WeChatConfig::with_authorized_roots([other_dir.path()]))
                .expect("service should build");
        assert!(matches!(
            service.transcode_audio_asset_to_mp3(
                &input,
                &output,
                &AudioTranscodeOptions::default()
            ),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(input_dir.path().to_path_buf());
        let result =
            service.transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default());
        assert!(
            !matches!(result, Err(WeChatError::UnauthorizedPath { .. })),
            "runtime authorization should move audio validation beyond authorization checks"
        );
    }

    #[test]
    fn add_authorized_root_allows_runtime_media_inventory_collection() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let asset = dir.path().join("image.dat");
        fs::write(&asset, [1_u8, 2, 3]).expect("write media asset");

        let mut service = WeChatService::new(WeChatConfig::with_authorized_roots([
            other_dir.path().to_path_buf(),
        ]))
        .expect("service should build");
        assert!(matches!(
            service.collect_media_inventory([asset.as_path()]),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let inventory = service
            .collect_media_inventory([asset.as_path()])
            .expect("runtime authorization should allow media inventory");
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].path, asset);
    }

    #[test]
    fn add_authorized_root_allows_runtime_audio_output_validation_to_progress() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.silk");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, []).expect("audio input");

        let mut service = WeChatService::new(WeChatConfig::with_authorized_roots([
            input_dir.path().to_path_buf(),
        ]))
        .expect("service should build");
        assert!(matches!(
            service.transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default()),
            Err(WeChatError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(output_dir.path().to_path_buf());
        let result =
            service.transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default());
        assert!(
            !matches!(result, Err(WeChatError::UnauthorizedPath { .. })),
            "runtime authorization should move audio validation beyond output authorization checks"
        );
    }


    #[test]
    fn export_only_workspace_is_not_empty_and_preserves_account_views() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");
        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path().to_path_buf()]))
            .expect("service should initialize");
        let workspace = service
            .build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>())
            .expect("export-only workspace should build");

        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 0);
        assert!(!workspace.is_empty());
        assert_eq!(workspace.accounts.len(), service.discover_accounts().len());
        let expected_primary = service.primary_account().map(|account| account_signature(&account));
        let actual_primary = workspace.primary_account.as_ref().map(account_signature);
        assert_eq!(actual_primary, expected_primary);
    }

    #[test]
    fn media_only_workspace_is_not_empty_and_preserves_account_views() {
        let dir = tempdir().expect("tempdir");
        let asset = dir.path().join("image.dat");
        fs::write(&asset, [1_u8, 2, 3]).expect("write media asset");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path()]))
            .expect("service should build");
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), [asset.as_path()])
            .expect("media-only workspace should build");

        assert_eq!(workspace.platform_id, "wechat");
        assert_eq!(workspace.export_count(), 0);
        assert_eq!(workspace.media_count(), 1);
        assert!(!workspace.is_empty());
        assert_eq!(account_signatures(&workspace.accounts), account_signatures(&service.discover_accounts()));
        assert_eq!(
            workspace.primary_account.as_ref().map(account_signature),
            service.primary_account().as_ref().map(account_signature)
        );
    }

    #[test]
    fn prepare_authorized_workspace_without_watch_dir_leaves_monitor_absent() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let (workspace, monitor) = service
            .prepare_authorized_workspace(
                std::iter::empty::<&Path>(),
                std::iter::empty::<&Path>(),
                None,
            )
            .expect("workspace should build without watch directory");

        assert!(monitor.is_none());
        assert!(workspace.watch_dir.is_none());
        assert_eq!(account_signatures(&workspace.accounts), account_signatures(&service.discover_accounts()));
        assert_eq!(
            workspace.primary_account.as_ref().map(account_signature),
            service.primary_account().as_ref().map(account_signature)
        );
    }

    #[test]
    fn rejects_empty_audio_payload_transcoding() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let error = service
            .transcode_audio_payload_to_mp3(&[], "silk", &AudioTranscodeOptions::default())
            .expect_err("empty payload should fail");

        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn authorized_workspace_reports_empty_when_no_exports_or_media_are_staged() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("empty workspace should still build");

        assert_eq!(workspace.export_count(), 0);
        assert_eq!(workspace.media_count(), 0);
        assert!(workspace.is_empty());
    }

    #[test]
    fn workspace_account_views_match_service_discovery() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should build");
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("workspace should build");
        assert_eq!(account_signatures(&workspace.accounts), account_signatures(&service.discover_accounts()));
        assert_eq!(
            workspace.primary_account.as_ref().map(account_signature),
            service.primary_account().as_ref().map(account_signature)
        );
    }

    #[test]
    fn parse_config_key_pair_accepts_prefixed_hex_values() {
        let config = WeChatConfig {
            data_key: Some(format!("0x{}", "11".repeat(32))),
            img_key: Some(format!("0x{}", "22".repeat(16))),
            ..WeChatConfig::default()
        };

        let (data_key, image_key) = parse_config_key_pair(&config)
            .expect("key parsing should succeed")
            .expect("key pair should exist");

        assert_eq!(data_key.len(), 32);
        assert_eq!(image_key.len(), 16);
        assert!(data_key.iter().all(|byte| *byte == 0x11));
        assert!(image_key.iter().all(|byte| *byte == 0x22));
    }

    #[test]
    fn parse_config_key_pair_rejects_partial_key_configuration() {
        let config = WeChatConfig {
            data_key: Some("11".repeat(32)),
            img_key: None,
            ..WeChatConfig::default()
        };

        let error = parse_config_key_pair(&config).expect_err("partial key config must fail");
        assert!(error
            .to_string()
            .contains("data_key and img_key must be provided together"));
    }

    #[test]
    fn first_available_key_prefers_explicit_runtime_pid_before_fallback() {
        let keys = DashMap::new();
        keys.insert(0, (vec![0xAA; 32], vec![0xBB; 16]));
        keys.insert(42, (vec![0x11; 32], vec![0x22; 16]));

        let (pid, data_key, image_key) =
            first_available_key(&keys).expect("one runtime key should be available");

        assert_eq!(pid, 42);
        assert_eq!(data_key, vec![0x11; 32]);
        assert_eq!(image_key, vec![0x22; 16]);
    }

    #[test]
    fn build_output_path_from_work_dir_uses_pid_scoped_directory() {
        let dir = tempdir().expect("tempdir");
        let input = dir.path().join("message.db");

        let output = build_output_path_from_work_dir(
            dir.path().to_string_lossy().as_ref(),
            &input,
            7788,
        );

        assert_eq!(output.file_name().and_then(|name| name.to_str()), Some("message.db"));
        assert_eq!(
            output.parent().and_then(|path| path.file_name()).and_then(|name| name.to_str()),
            Some("7788")
        );
    }

    #[tokio::test]
    async fn detect_instances_updates_runtime_account_cache() {
        let dir = tempdir().expect("tempdir");
        let account = running_account(42, dir.path());
        let detector = Arc::new(MockDetector::new(vec![account.clone()], Err("unused".into())));
        let service = WeChatService::new_with_detector(WeChatConfig::default(), detector);

        let instances = service.detect_instances().await.expect("detect instances");
        let cached_accounts = service.get_accounts();

        assert_eq!(instances.len(), 1);
        assert_eq!(cached_accounts.len(), 1);
        assert_eq!(instances[0].pid, 42);
        assert_eq!(cached_accounts[0].pid, 42);
        assert!(cached_accounts[0].is_current);
    }

    #[tokio::test]
    async fn detect_instances_applies_configured_fallback_keys() {
        let dir = tempdir().expect("tempdir");
        let account = running_account(77, dir.path());
        let detector = Arc::new(MockDetector::new(vec![account], Err("unused".into())));
        let config = WeChatConfig {
            data_key: Some("ab".repeat(32)),
            img_key: Some("cd".repeat(16)),
            ..WeChatConfig::default()
        };
        let service = WeChatService::new_with_detector(config, detector);

        service.detect_instances().await.expect("detect instances");

        let cached_keys = service.keys.get(&77).expect("fallback keys for pid");
        assert_eq!(cached_keys.0, vec![0xab; 32]);
        assert_eq!(cached_keys.1, vec![0xcd; 16]);
        assert!(service.keys.contains_key(&0));
    }

    #[tokio::test]
    async fn extract_keys_for_instance_caches_success_and_emits_event() {
        let detector = Arc::new(MockDetector::new(
            vec![],
            Ok(("ab".repeat(32), "cd".repeat(16))),
        ));
        let mut service = WeChatService::new_with_detector(WeChatConfig::default(), detector);

        let (data_key, img_key) = service
            .extract_keys_for_instance(88)
            .await
            .expect("key extraction should succeed");

        assert_eq!(data_key, vec![0xab; 32]);
        assert_eq!(img_key, vec![0xcd; 16]);

        let event = timeout(TokioDuration::from_millis(50), service.next_event())
            .await
            .expect("event should arrive")
            .expect("event should exist");
        match event {
            ServiceEvent::KeyExtractionComplete {
                pid,
                success,
                error,
            } => {
                assert_eq!(pid, 88);
                assert!(success);
                assert!(error.is_none());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn extract_keys_for_instance_emits_failure_event() {
        let detector = Arc::new(MockDetector::new(vec![], Err("mock failure".to_string())));
        let mut service = WeChatService::new_with_detector(WeChatConfig::default(), detector);

        let error = service
            .extract_keys_for_instance(99)
            .await
            .expect_err("key extraction should fail");

        match error {
            WeChatError::KeyExtraction(message) => assert_eq!(message, "mock failure"),
            other => panic!("unexpected error: {other:?}"),
        }

        let event = timeout(TokioDuration::from_millis(50), service.next_event())
            .await
            .expect("event should arrive")
            .expect("event should exist");
        match event {
            ServiceEvent::KeyExtractionComplete {
                pid,
                success,
                error,
            } => {
                assert_eq!(pid, 99);
                assert!(!success);
                assert_eq!(error.as_deref(), Some("mock failure"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_and_stop_toggle_running_state() {
        let detector = Arc::new(MockDetector::new(vec![], Err("unused".into())));
        let config = WeChatConfig {
            auto_decrypt: false,
            ..WeChatConfig::default()
        };
        let mut service = WeChatService::new_with_detector(config, detector);

        assert!(!service.running);
        service.start().await.expect("service should start");
        assert!(service.running);
        service.stop().await.expect("service should stop");
        assert!(!service.running);
    }

    #[tokio::test]
    async fn decrypt_database_rejects_unauthorized_input_before_key_lookup() {
        let allowed_dir = tempdir().expect("tempdir");
        let blocked_dir = tempdir().expect("tempdir");
        let input = blocked_dir.path().join("message.db");
        fs::write(&input, [0u8; 16]).expect("write input");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([allowed_dir.path()]))
            .expect("service should build");

        let error = service
            .decrypt_database(input.clone(), 55)
            .await
            .expect_err("unauthorized input should fail before key lookup");

        match error {
            WeChatError::UnauthorizedPath { path } => assert_eq!(path, input),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn decrypt_database_rejects_unauthorized_output_path() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("message.db");
        fs::write(&input, [0u8; 16]).expect("write input");

        let mut config = WeChatConfig::with_authorized_roots([input_dir.path()]);
        config.work_dir = output_dir.path().join("wechat-out").to_string_lossy().into_owned();
        let service = WeChatService::new(config).expect("service should build");
        service.keys.insert(88, (vec![0x11; 32], vec![0x22; 16]));

        let error = service
            .decrypt_database(input, 88)
            .await
            .expect_err("unauthorized output path should fail");

        match error {
            WeChatError::UnauthorizedPath { path } => {
                assert!(path.starts_with(output_dir.path()));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn decrypt_database_creates_output_directory_and_emits_failure_event() {
        let input_dir = tempdir().expect("tempdir");
        let work_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("message.db");
        fs::write(&input, [0u8; 16]).expect("write input");

        let work_root = work_dir.path().join("wechat-work");
        let mut config = WeChatConfig::with_authorized_roots([input_dir.path(), work_root.as_path()]);
        config.work_dir = work_root.to_string_lossy().into_owned();

        let mut service = WeChatService::new(config).expect("service should build");
        service.keys.insert(101, (vec![0x11; 32], vec![0x22; 16]));

        let error = service
            .decrypt_database(input.clone(), 101)
            .await
            .expect_err("invalid database should fail decryption");
        assert!(
            matches!(error, WeChatError::Io(_) | WeChatError::Decryption(_)),
            "unexpected error: {error:?}"
        );

        let output_path = service.get_output_path(&input, 101);
        let parent = output_path.parent().expect("pid-scoped parent");
        assert!(parent.exists(), "output parent should be created eagerly");

        let event = timeout(TokioDuration::from_millis(50), service.next_event())
            .await
            .expect("event should arrive")
            .expect("event should exist");
        match event {
            ServiceEvent::DecryptionComplete {
                input_path,
                output_path: emitted_output,
                success,
                error,
            } => {
                assert_eq!(input_path, input);
                assert_eq!(emitted_output, output_path);
                assert!(!success);
                assert!(error.is_some());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn primary_account_belongs_to_discovered_accounts_when_present() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should initialize");
        let accounts = service.discover_accounts();
        let account_signatures = account_signatures(&accounts);

        if let Some(primary) = service.primary_account() {
            assert!(account_signatures.contains(&account_signature(&primary)));
        }
    }

    #[test]
    fn workspace_primary_account_belongs_to_workspace_accounts_when_present() {
        let service = WeChatService::new(WeChatConfig::default()).expect("service should initialize");
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("workspace should build");
        let account_signatures = account_signatures(&workspace.accounts);

        if let Some(primary) = workspace.primary_account.clone() {
            assert!(account_signatures.contains(&account_signature(&primary)));
        }
    }



    #[test]
    fn prepared_workspace_with_monitor_preserves_export_and_media_counts() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("wechat_fixture.json");
        let asset = dir.path().join("voice.opus");
        write_fixture(
            &export,
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
        );
        fs::write(&asset, [1_u8, 2, 3]).expect("write media");

        let service = WeChatService::new(WeChatConfig::with_authorized_roots([dir.path().to_path_buf()]))
            .expect("service should initialize");
        let (workspace, monitor) = service
            .prepare_authorized_workspace([export.as_path()], [asset.as_path()], Some(dir.path()))
            .expect("workspace and monitor should build");

        assert!(monitor.is_some());
        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 1);
        assert_eq!(workspace.watch_dir.as_deref(), Some(dir.path()));
        assert_eq!(workspace.accounts.len(), service.discover_accounts().len());
        let expected_primary = service.primary_account().map(|account| account_signature(&account));
        let actual_primary = workspace
            .primary_account
            .as_ref()
            .map(account_signature);
        assert_eq!(actual_primary, expected_primary);
    }

}
