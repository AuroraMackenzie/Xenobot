//! Secure secret storage helpers for API modules.
//!
//! The preferred backend on macOS is Keychain (`security` CLI). On other
//! platforms, a file fallback is used to keep functionality available.

use crate::ApiError;
#[cfg(not(target_os = "macos"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "macos"))]
use std::collections::HashMap;
#[cfg(not(target_os = "macos"))]
use std::fs;
#[cfg(not(target_os = "macos"))]
use std::path::PathBuf;

/// Store a secret value for a scoped key.
pub fn store_secret(scope: &str, key: &str, value: &str) -> Result<(), ApiError> {
    #[cfg(target_os = "macos")]
    {
        return macos_store_secret(scope, key, value);
    }

    #[cfg(not(target_os = "macos"))]
    {
        file_store_secret(scope, key, value)
    }
}

/// Load a secret value for a scoped key.
pub fn load_secret(scope: &str, key: &str) -> Result<Option<String>, ApiError> {
    #[cfg(target_os = "macos")]
    {
        return macos_load_secret(scope, key);
    }

    #[cfg(not(target_os = "macos"))]
    {
        file_load_secret(scope, key)
    }
}

/// Delete a secret value for a scoped key.
pub fn delete_secret(scope: &str, key: &str) -> Result<(), ApiError> {
    #[cfg(target_os = "macos")]
    {
        return macos_delete_secret(scope, key);
    }

    #[cfg(not(target_os = "macos"))]
    {
        file_delete_secret(scope, key)
    }
}

#[cfg(target_os = "macos")]
fn macos_store_secret(scope: &str, key: &str, value: &str) -> Result<(), ApiError> {
    let service = service_name(scope);
    let output = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-a",
            key,
            "-s",
            &service,
            "-w",
            value,
            "-U",
        ])
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    Err(ApiError::Internal(format!(
        "failed to store secret in keychain (scope={}, key={}): {}",
        scope,
        key,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(target_os = "macos")]
fn macos_load_secret(scope: &str, key: &str) -> Result<Option<String>, ApiError> {
    let service = service_name(scope);
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-a", key, "-s", &service, "-w"])
        .output()?;

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            return Ok(None);
        }
        return Ok(Some(value));
    }

    let code = output.status.code().unwrap_or_default();
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if code == 44 || stderr_text.contains("could not be found") {
        return Ok(None);
    }

    Err(ApiError::Internal(format!(
        "failed to load secret from keychain (scope={}, key={}): {}",
        scope,
        key,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(target_os = "macos")]
fn macos_delete_secret(scope: &str, key: &str) -> Result<(), ApiError> {
    let service = service_name(scope);
    let output = std::process::Command::new("security")
        .args(["delete-generic-password", "-a", key, "-s", &service])
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let code = output.status.code().unwrap_or_default();
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if code == 44 || stderr_text.contains("could not be found") {
        return Ok(());
    }

    Err(ApiError::Internal(format!(
        "failed to delete secret from keychain (scope={}, key={}): {}",
        scope,
        key,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(target_os = "macos")]
fn service_name(scope: &str) -> String {
    format!("xenobot.{}", normalize_segment(scope))
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default, Serialize, Deserialize)]
struct FileSecretStore {
    entries: HashMap<String, String>,
}

#[cfg(not(target_os = "macos"))]
fn file_store_secret(scope: &str, key: &str, value: &str) -> Result<(), ApiError> {
    let mut store = read_file_store()?;
    store
        .entries
        .insert(store_key(scope, key), value.to_string());
    write_file_store(&store)
}

#[cfg(not(target_os = "macos"))]
fn file_load_secret(scope: &str, key: &str) -> Result<Option<String>, ApiError> {
    let store = read_file_store()?;
    Ok(store.entries.get(&store_key(scope, key)).cloned())
}

#[cfg(not(target_os = "macos"))]
fn file_delete_secret(scope: &str, key: &str) -> Result<(), ApiError> {
    let mut store = read_file_store()?;
    store.entries.remove(&store_key(scope, key));
    write_file_store(&store)
}

#[cfg(not(target_os = "macos"))]
fn read_file_store() -> Result<FileSecretStore, ApiError> {
    let path = file_store_path()?;
    if !path.exists() {
        return Ok(FileSecretStore::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(FileSecretStore::default());
    }
    serde_json::from_str(&raw).map_err(ApiError::Json)
}

#[cfg(not(target_os = "macos"))]
fn write_file_store(store: &FileSecretStore) -> Result<(), ApiError> {
    let path = file_store_path()?;
    let raw = serde_json::to_string_pretty(store)?;
    fs::write(path, raw)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn file_store_path() -> Result<PathBuf, ApiError> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("secrets.json"))
}

#[cfg(not(target_os = "macos"))]
fn store_key(scope: &str, key: &str) -> String {
    format!("{}:{}", normalize_segment(scope), normalize_segment(key))
}

fn normalize_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
