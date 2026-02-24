//! WeChat account and instance representation.

use serde::{Deserialize, Serialize};

/// Represents a running WeChat instance or historical account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Process ID (0 for historical accounts)
    pub pid: u32,
    /// Account name (usually WeChat nickname)
    pub name: String,
    /// Data directory path
    pub data_dir: String,
    /// WeChat version string (e.g., "4.0.0.0")
    pub version: String,
    /// Full version string with build number
    pub full_version: String,
    /// Platform (e.g., "macOS", "Windows")
    pub platform: String,
    /// Whether this is the current active account
    pub is_current: bool,
}

impl Account {
    /// Create a new account instance.
    pub fn new(
        pid: u32,
        name: String,
        data_dir: String,
        version: String,
        full_version: String,
        platform: String,
    ) -> Self {
        Self {
            pid,
            name,
            data_dir,
            version,
            full_version,
            platform,
            is_current: false,
        }
    }

    /// Check if this account is running (PID > 0).
    pub fn is_running(&self) -> bool {
        self.pid > 0
    }
}

/// Trait for platform-specific WeChat instance detection.
pub trait WeChatDetector: Send + Sync {
    /// Get all running WeChat instances.
    fn get_running_instances(&self) -> Vec<Account>;

    /// Resolve decryption keys for the selected WeChat context.
    fn extract_keys(&self, pid: u32) -> Result<(String, String), String>; // (data_key, img_key)
}

/// Platform-specific detector implementations.
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
