//! Account models for legal-safe Zoom contexts.
//!
//! This module derives account views from user-owned local sources and
//! explicitly authorized export roots. It does not inspect running process
//! memory or bypass any platform protection.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use xenobot_core::platform_sources::{SourceCandidate, SourceKind};

/// Represents a discovered Zoom account context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Process ID placeholder. Legal-safe discovery does not inspect runtime PIDs.
    pub pid: u32,
    /// Human-readable account or source label.
    pub name: String,
    /// Canonical data or export directory for this account context.
    pub data_dir: String,
    /// Runtime version string when known.
    pub version: String,
    /// Full version string when known.
    pub full_version: String,
    /// Platform string such as `macOS`.
    pub platform: String,
    /// Whether this account is considered the primary current context.
    pub is_current: bool,
    /// Source kind backing this account view.
    pub source_kind: SourceKind,
}

impl Account {
    /// Construct an account model from a discovered source candidate.
    pub fn from_source(candidate: &SourceCandidate) -> Self {
        Self {
            pid: 0,
            name: normalize_label(&candidate.label, &candidate.path),
            data_dir: candidate.path.to_string_lossy().to_string(),
            version: "authorized-export".to_string(),
            full_version: "authorized-export".to_string(),
            platform: infer_platform_name(&candidate.path),
            is_current: matches!(candidate.kind, SourceKind::AppContainer),
            source_kind: candidate.kind,
        }
    }

    /// Returns whether this account maps to a running process.
    pub fn is_running(&self) -> bool {
        self.pid > 0
    }

    /// Returns the root path backing this account.
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
                    existing.source_kind = candidate.kind;
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
        .unwrap_or_else(|| "Zoom".to_string())
}

fn infer_platform_name(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.contains("/Library/") {
        "macOS".to_string()
    } else {
        "authorized-export".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xenobot_core::platform_sources::SourceCandidate;
    use xenobot_core::types::Platform;

    fn source(kind: SourceKind, label: &str, path: &str) -> SourceCandidate {
        SourceCandidate {
            platform: Platform::Zoom,
            platform_id: "zoom".to_string(),
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
            "macOS Zoom sandbox data",
            "/Users/test/Library/Containers/Zoom",
        );
        let account = Account::from_source(&candidate);

        assert_eq!(account.name, "macOS Zoom sandbox data");
        assert_eq!(account.platform, "macOS");
        assert!(account.is_current);
        assert!(!account.is_running());
    }

    #[test]
    fn deduplicates_accounts_by_root_path() {
        let candidates = vec![
            source(SourceKind::ExportDirectory, "Downloads export", "/tmp/export"),
            source(SourceKind::AppContainer, "Sandbox", "/tmp/export"),
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

    #[test]
    fn falls_back_to_directory_name_when_label_is_blank() {
        let candidate = source(SourceKind::ExportDirectory, "   ", "/tmp/zoom-export");
        let account = Account::from_source(&candidate);

        assert_eq!(account.name, "zoom-export");
    }

    #[test]
    fn root_path_reflects_data_dir() {
        let candidate = source(SourceKind::ExportDirectory, "Archive", "/tmp/zoom-root");
        let account = Account::from_source(&candidate);

        assert_eq!(account.root_path(), Path::new("/tmp/zoom-root"));
    }

}
