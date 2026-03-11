//! Configuration for the legal-safe LINE service layer.

use std::path::{Path, PathBuf};

/// Runtime configuration for the LINE service.
#[derive(Debug, Clone, Default)]
pub struct LineConfig {
    authorized_roots: Vec<PathBuf>,
}

impl LineConfig {
    /// Create an empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with an initial authorized root list.
    pub fn with_authorized_roots<I, P>(roots: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self {
            authorized_roots: roots.into_iter().map(Into::into).collect(),
        }
    }

    /// Add an authorized root directory.
    pub fn add_authorized_root<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        self.authorized_roots.push(path.into());
    }

    /// Return the configured authorized roots.
    pub fn authorized_roots(&self) -> &[PathBuf] {
        &self.authorized_roots
    }

    /// Check whether a path is allowed by the current configuration.
    ///
    /// If no authorized root is configured, the service operates in direct-file
    /// mode and accepts any explicitly provided file path.
    pub fn is_authorized_path(&self, path: &Path) -> bool {
        if self.authorized_roots.is_empty() {
            return true;
        }

        self.authorized_roots
            .iter()
            .any(|root| path.starts_with(root))
    }
}
