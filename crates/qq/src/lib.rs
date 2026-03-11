//! Legal-safe QQ adapter for Xenobot.
//!
//! This crate supports source discovery and authorized export parsing only.
//! It does not implement process-memory key extraction or encryption bypass.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod account;
pub mod audio;
use std::path::{Path, PathBuf};

use thiserror::Error;
use xenobot_analysis::parsers::{ParseError, ParsedChat, ParserRegistry};
use xenobot_core::platform_sources::{discover_sources_for_platform, SourceCandidate};
use xenobot_core::types::Platform;

pub mod config;
pub mod media;
pub mod monitor;
pub mod service;

/// Stable platform identifier.
pub const PLATFORM_ID: &str = "qq";
/// Common result type used by QQ crate operations.
pub type QQResult<T> = Result<T, QQError>;

pub use config::QQConfig;
pub use service::{AuthorizedQQWorkspace, QQService, StagedQQExport};

/// Legal-safe adapter for QQ workflows.
#[derive(Debug, Clone, Copy, Default)]
pub struct QQAdapter;

impl QQAdapter {
    /// Create a new adapter.
    pub fn new() -> Self {
        Self
    }

    /// Return the stable platform identifier.
    pub fn platform_id(&self) -> &'static str {
        PLATFORM_ID
    }

    /// Discover local source candidates for this platform.
    pub fn discover_sources(&self) -> Vec<SourceCandidate> {
        discover_sources_for_platform(&self.platform())
    }

    /// Parse a user-authorized export and ensure platform-level consistency.
    pub fn parse_authorized_export(&self, path: &Path) -> Result<ParsedChat, QQError> {
        let registry = ParserRegistry::new();
        let parsed = registry.detect_and_parse(path).map_err(QQError::Parse)?;

        if parsed.platform.eq_ignore_ascii_case(PLATFORM_ID) {
            Ok(parsed)
        } else {
            Err(QQError::PlatformMismatch {
                expected: PLATFORM_ID.to_string(),
                actual: parsed.platform,
            })
        }
    }

    /// Return the core platform enum used by source discovery.
    pub fn platform(&self) -> Platform {
        Platform::Qq
    }
}

/// Errors returned by the QQ adapter.
#[derive(Debug, Error)]
pub enum QQError {
    /// Parse error returned by analysis parser registry.
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// Path is outside the configured authorized roots.
    #[error("path is outside authorized roots: {path}")]
    UnauthorizedPath {
        /// Rejected source path.
        path: PathBuf,
    },

    /// I/O error while reading export assets.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// File monitoring error.
    #[error("file monitoring error: {0}")]
    FileMonitor(#[from] notify::Error),

    /// Internal orchestration or external tool failure.
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),

    /// Parsed export did not match the expected platform.
    #[error("parsed platform mismatch: expected {expected}, got {actual}")]
    PlatformMismatch {
        /// Expected stable platform identifier.
        expected: String,
        /// Actual parsed platform identifier.
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_expected_platform_id() {
        let adapter = QQAdapter::new();
        assert_eq!(adapter.platform_id(), PLATFORM_ID);
    }

    #[test]
    fn discovers_sources_for_platform() {
        let adapter = QQAdapter::new();
        let sources = adapter.discover_sources();
        assert!(!sources.is_empty());
        assert!(sources
            .iter()
            .all(|candidate| candidate.platform_id == PLATFORM_ID));
    }
}
