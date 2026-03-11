//! Configuration for WeChat data extraction.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// Explicitly authorized roots for export ingestion.
    #[serde(default)]
    pub authorized_roots: Vec<PathBuf>,
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
            authorized_roots: vec![],
        }
    }
}

impl WeChatConfig {
    /// Create a configuration with explicit authorized roots.
    pub fn with_authorized_roots<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self {
            authorized_roots: paths.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// Return the configured authorized roots.
    pub fn authorized_roots(&self) -> &[PathBuf] {
        &self.authorized_roots
    }

    /// Add one authorized root at runtime.
    pub fn add_authorized_root<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        self.authorized_roots.push(path.into());
    }

    /// Return whether a candidate file path is inside an authorized root.
    pub fn is_authorized_path(&self, path: &Path) -> bool {
        if self.authorized_roots.is_empty() {
            return true;
        }

        self.authorized_roots.iter().any(|root| path.starts_with(root))
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
