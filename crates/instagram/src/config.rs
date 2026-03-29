//! Runtime configuration for legal-safe Instagram workflows.

use std::path::{Path, PathBuf};

/// Runtime configuration for Instagram ingestion.
#[derive(Debug, Clone, Default)]
pub struct InstagramConfig {
    authorized_roots: Vec<PathBuf>,
}

impl InstagramConfig {
    /// Create a configuration with explicit authorized roots.
    pub fn with_authorized_roots<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self {
            authorized_roots: paths.into_iter().map(Into::into).collect(),
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
            return path.is_file();
        }

        self.authorized_roots
            .iter()
            .any(|root| path.starts_with(root))
    }
}
