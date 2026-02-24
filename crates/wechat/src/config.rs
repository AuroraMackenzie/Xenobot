//! Configuration for WeChat data extraction.

use serde::{Deserialize, Serialize};

/// WeChat-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeChatConfig {
    /// WeChat data directory (default: macOS default location)
    pub data_dir: String,
    /// Working directory for decrypted databases
    pub work_dir: String,
    /// Data encryption key (authorized user-provided)
    pub data_key: Option<String>,
    /// Image encryption key
    pub img_key: Option<String>,
    /// WeChat platform version (2 for macOS, 3 for Windows, etc.)
    pub platform_version: i32,
    /// Enable automatic decryption on file changes
    pub auto_decrypt: bool,
    /// HTTP server address for MCP integration
    pub http_addr: String,
}

impl Default for WeChatConfig {
    fn default() -> Self {
        let default_data_dir = if cfg!(target_os = "macos") {
            format!("{}/Library/Containers/com.tencent.xinWeChat/Data/Library/Application Support/com.tencent.xinWeChat", dirs::home_dir().unwrap().display())
        } else {
            String::new()
        };

        let default_work_dir = dirs::data_dir()
            .map(|p| {
                p.join("xenobot")
                    .join("wechat")
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| "./wechat_data".to_string());

        Self {
            data_dir: default_data_dir,
            work_dir: default_work_dir,
            data_key: None,
            img_key: None,
            platform_version: 2, // macOS
            auto_decrypt: true,
            http_addr: "127.0.0.1:8080".to_string(),
        }
    }
}

/// Trait for configuration providers (compatible with Go interface pattern).
pub trait Config: Send + Sync {
    /// Get data encryption key
    fn get_data_key(&self) -> Option<&str>;
    /// Get data directory
    fn get_data_dir(&self) -> &str;
    /// Get work directory
    fn get_work_dir(&self) -> &str;
    /// Get platform version
    fn get_platform_version(&self) -> i32;
    /// Get HTTP address
    fn get_http_addr(&self) -> &str;
}

impl Config for WeChatConfig {
    fn get_data_key(&self) -> Option<&str> {
        self.data_key.as_deref()
    }

    fn get_data_dir(&self) -> &str {
        &self.data_dir
    }

    fn get_work_dir(&self) -> &str {
        &self.work_dir
    }

    fn get_platform_version(&self) -> i32 {
        self.platform_version
    }

    fn get_http_addr(&self) -> &str {
        &self.http_addr
    }
}
