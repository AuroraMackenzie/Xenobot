//! Main WeChat service orchestrating data extraction and decryption.

use crate::account::{Account, WeChatDetector};
use crate::config::WeChatConfig;
use crate::decrypt::{decrypt_v4_database, V4DecryptionParams};
use crate::error::{WeChatError, WeChatResult};
use crate::monitor::{FileMonitor, FileMonitorConfig};
use dashmap::DashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{error, info};

/// Main WeChat service.
pub struct WeChatService {
    config: WeChatConfig,
    detector: Arc<dyn WeChatDetector>,
    running: bool,
    tasks: Vec<JoinHandle<()>>,
    event_tx: mpsc::Sender<ServiceEvent>,
    event_rx: mpsc::Receiver<ServiceEvent>,
    accounts: Arc<DashMap<u32, Account>>,
    keys: Arc<DashMap<u32, (Vec<u8>, Vec<u8>)>>, // (data_key, img_key) per PID
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
    /// Create a new WeChat service.
    pub fn new(config: WeChatConfig) -> WeChatResult<Self> {
        let detector = Self::create_detector();

        let (event_tx, event_rx) = mpsc::channel(100);

        Ok(Self {
            config,
            detector: Arc::new(detector),
            running: false,
            tasks: vec![],
            event_tx,
            event_rx,
            accounts: Arc::new(DashMap::new()),
            keys: Arc::new(DashMap::new()),
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

    /// Get all detected accounts.
    pub fn get_accounts(&self) -> Vec<Account> {
        self.accounts
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
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

        let output_path = self.get_output_path(&input_path, pid);

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
        let config = FileMonitorConfig {
            watch_dir: PathBuf::from(&self.config.data_dir),
            file_patterns: FileMonitor::wechat_macos_patterns(),
            debounce_ms: 1000,
            max_wait_ms: 10000,
            recursive: true,
        };

        let mut monitor = FileMonitor::new(config)?;
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
