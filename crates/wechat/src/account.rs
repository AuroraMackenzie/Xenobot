//! WeChat account and instance representation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use xenobot_core::platform_sources::{SourceCandidate, SourceKind};

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

    /// Construct a legal-safe account view from a discovered source candidate.
    pub fn from_source(candidate: &SourceCandidate) -> Self {
        let mut account = Self::new(
            0,
            normalize_label(&candidate.label, &candidate.path),
            candidate.path.to_string_lossy().to_string(),
            "authorized-export".to_string(),
            "authorized-export".to_string(),
            infer_platform_name(&candidate.path),
        );
        account.is_current = matches!(candidate.kind, SourceKind::AppContainer);
        account
    }

    /// Return the root path backing this account.
    pub fn root_path(&self) -> &Path {
        Path::new(&self.data_dir)
    }
}

/// Build a deduplicated account catalog from source candidates.
pub fn collect_accounts_from_sources(candidates: &[SourceCandidate]) -> Vec<Account> {
    let mut dedup = BTreeMap::<PathBuf, Account>::new();

    for candidate in candidates {
        dedup
            .entry(candidate.path.clone())
            .and_modify(|existing| {
                if matches!(candidate.kind, SourceKind::AppContainer) {
                    existing.is_current = true;
                }
            })
            .or_insert_with(|| Account::from_source(candidate));
    }

    dedup.into_values().collect()
}

/// Return the preferred primary account context when one exists.
pub fn primary_account(candidates: &[SourceCandidate]) -> Option<Account> {
    let mut accounts = collect_accounts_from_sources(candidates);
    accounts.sort_by_key(|account| (!account.is_current, account.name.clone()));
    accounts.into_iter().next()
}

fn normalize_label(label: &str, path: &Path) -> String {
    let trimmed = label.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "WeChat".to_string())
}

fn infer_platform_name(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.contains("/Library/") {
        "macOS".to_string()
    } else {
        "authorized-export".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use xenobot_core::types::Platform;

    fn source(kind: SourceKind, label: &str, path: &str) -> SourceCandidate {
        SourceCandidate {
            platform: Platform::WeChat,
            platform_id: "wechat".to_string(),
            label: label.to_string(),
            kind,
            path: PathBuf::from(path),
            exists: true,
            readable: true,
        }
    }

    #[test]
    fn builds_account_from_source() {
        let candidate = source(
            SourceKind::AppContainer,
            "macOS WeChat sandbox data",
            "/Users/test/Library/Containers/com.tencent.xinWeChat",
        );
        let account = Account::from_source(&candidate);

        assert_eq!(account.name, "macOS WeChat sandbox data");
        assert_eq!(account.platform, "macOS");
        assert!(account.is_current);
        assert!(!account.is_running());
    }

    #[test]
    fn deduplicates_accounts_by_root_path() {
        let candidates = vec![
            source(SourceKind::ExportDirectory, "Downloads export", "/tmp/wechat"),
            source(SourceKind::AppContainer, "Sandbox", "/tmp/wechat"),
        ];

        let accounts = collect_accounts_from_sources(&candidates);
        assert_eq!(accounts.len(), 1);
        assert!(accounts[0].is_current);
    }

    #[test]
    fn picks_primary_account_preferring_current() {
        let candidates = vec![
            source(SourceKind::ExportDirectory, "Downloads export", "/tmp/export"),
            source(SourceKind::AppContainer, "Sandbox", "/tmp/app"),
        ];

        let account = primary_account(&candidates).expect("primary account");
        assert_eq!(account.name, "Sandbox");
        assert!(account.is_current);
    }
}
