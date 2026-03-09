//! Sandbox and transport fallback helpers shared across Xenobot surfaces.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxTcpProbe {
    pub allowed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxUdsProbe {
    pub supported: bool,
    pub allowed: bool,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxFileGatewayProbe {
    pub dir: String,
    pub writable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRecommendation {
    pub mode: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxDoctorReport {
    pub tcp: SandboxTcpProbe,
    pub uds: SandboxUdsProbe,
    pub file_gateway: SandboxFileGatewayProbe,
    pub recommended: SandboxRecommendation,
}

pub fn shell_quote_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '+'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(unix)]
fn unix_socket_max_path_bytes() -> usize {
    if cfg!(target_os = "macos") { 103 } else { 107 }
}

#[cfg(unix)]
fn unix_socket_path_within_limit(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len() <= unix_socket_max_path_bytes()
}

#[cfg(unix)]
pub fn select_sandbox_safe_unix_socket_path() -> Result<PathBuf> {
    let mut candidate_dirs = Vec::new();
    if let Ok(explicit_dir) = std::env::var("XENOBOT_API_SOCKET_DIR") {
        let trimmed = explicit_dir.trim();
        if !trimmed.is_empty() {
            candidate_dirs.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let trimmed = tmpdir.trim();
        if !trimmed.is_empty() {
            candidate_dirs.push(PathBuf::from(trimmed));
        }
    }
    candidate_dirs.push(PathBuf::from("/tmp"));

    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();

    let mut chosen: Option<PathBuf> = None;
    for dir in candidate_dirs {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        for name in [
            format!("xb-{}-{}.sock", pid, nanos % 100_000),
            format!("xb-{}.sock", pid),
            "xb.sock".to_string(),
        ] {
            let candidate = dir.join(name);
            if unix_socket_path_within_limit(&candidate) {
                chosen = Some(candidate);
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }

    chosen.ok_or_else(|| {
        Error::Validation(format!(
            "cannot build unix socket path within {}-byte limit; set XENOBOT_API_SOCKET_DIR to a short writable directory",
            unix_socket_max_path_bytes()
        ))
    })
}

#[cfg(not(unix))]
pub fn select_sandbox_safe_unix_socket_path() -> Result<PathBuf> {
    Err(Error::Unsupported(
        "unix sockets are not supported on this platform".to_string(),
    ))
}

pub fn select_file_gateway_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Ok(env_dir) = std::env::var("XENOBOT_FILE_API_DIR") {
        let trimmed = env_dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let trimmed = tmpdir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("xenobot-file-api"));
        }
    }
    Ok(PathBuf::from("/tmp").join("xenobot-file-api"))
}

pub fn build_sandbox_start_recommendation(
    tcp_allowed: bool,
    uds_allowed: bool,
    gateway_root: &Path,
) -> SandboxRecommendation {
    let mode = if tcp_allowed {
        "tcp"
    } else if uds_allowed {
        "unix"
    } else {
        "file-gateway"
    };

    let command = match mode {
        "tcp" => "cargo run -p xenobot-cli --features \"api,analysis\" -- api start --host 127.0.0.1 --port 5030 --db-path /tmp/xenobot.db".to_string(),
        "unix" => "cargo run -p xenobot-cli --features \"api,analysis\" -- api start --unix-socket /tmp/xenobot.sock --db-path /tmp/xenobot.db".to_string(),
        _ => format!(
            "cargo run -p xenobot-cli --features \"api,analysis\" -- api start --force-file-gateway --file-gateway-dir {} --db-path /tmp/xenobot.db",
            shell_quote_arg(&gateway_root.to_string_lossy())
        ),
    };

    SandboxRecommendation {
        mode: mode.to_string(),
        command,
    }
}

pub fn diagnose_sandbox(file_gateway_dir: Option<PathBuf>) -> Result<SandboxDoctorReport> {
    let (tcp_allowed, tcp_error) = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => {
            drop(listener);
            (true, None)
        }
        Err(err) => (false, Some(err.to_string())),
    };

    #[cfg(unix)]
    let uds = match select_sandbox_safe_unix_socket_path() {
        Ok(path) => {
            let _ = std::fs::remove_file(&path);
            match std::os::unix::net::UnixListener::bind(&path) {
                Ok(listener) => {
                    drop(listener);
                    let _ = std::fs::remove_file(&path);
                    SandboxUdsProbe {
                        supported: true,
                        allowed: true,
                        path: Some(path.to_string_lossy().to_string()),
                        error: None,
                    }
                }
                Err(err) => SandboxUdsProbe {
                    supported: true,
                    allowed: false,
                    path: Some(path.to_string_lossy().to_string()),
                    error: Some(err.to_string()),
                },
            }
        }
        Err(err) => SandboxUdsProbe {
            supported: true,
            allowed: false,
            path: None,
            error: Some(err.to_string()),
        },
    };

    #[cfg(not(unix))]
    let uds = SandboxUdsProbe {
        supported: false,
        allowed: false,
        path: None,
        error: Some("unix sockets are not supported on this platform".to_string()),
    };

    let gateway_root = select_file_gateway_root(file_gateway_dir)?;
    let (gateway_writable, gateway_error) = match std::fs::create_dir_all(&gateway_root) {
        Ok(_) => {
            let probe = gateway_root.join(format!(
                ".xenobot_probe_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros()
            ));
            match std::fs::write(&probe, b"xenobot-sandbox-probe") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe);
                    (true, None)
                }
                Err(err) => (false, Some(err.to_string())),
            }
        }
        Err(err) => (false, Some(err.to_string())),
    };

    Ok(SandboxDoctorReport {
        tcp: SandboxTcpProbe {
            allowed: tcp_allowed,
            error: tcp_error,
        },
        uds: uds.clone(),
        file_gateway: SandboxFileGatewayProbe {
            dir: gateway_root.to_string_lossy().to_string(),
            writable: gateway_writable,
            error: gateway_error,
        },
        recommended: build_sandbox_start_recommendation(
            tcp_allowed,
            uds.allowed,
            &gateway_root,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendation_prefers_tcp_when_available() {
        let gateway = Path::new("/tmp/xenobot gateway");
        let recommendation = build_sandbox_start_recommendation(true, true, gateway);
        assert_eq!(recommendation.mode, "tcp");
        assert!(recommendation.command.contains("--host 127.0.0.1"));
    }

    #[test]
    fn recommendation_uses_unix_when_tcp_is_blocked() {
        let gateway = Path::new("/tmp/xenobot gateway");
        let recommendation = build_sandbox_start_recommendation(false, true, gateway);
        assert_eq!(recommendation.mode, "unix");
        assert!(recommendation.command.contains("--unix-socket /tmp/xenobot.sock"));
    }

    #[test]
    fn recommendation_quotes_gateway_paths_when_needed() {
        let gateway = Path::new("/tmp/xenobot gateway/with spaces");
        let recommendation = build_sandbox_start_recommendation(false, false, gateway);
        assert_eq!(recommendation.mode, "file-gateway");
        assert!(recommendation.command.contains("--force-file-gateway"));
        assert!(recommendation.command.contains("--file-gateway-dir '/tmp/xenobot gateway/with spaces'"));
    }

    #[test]
    fn recommendation_escapes_single_quotes_in_gateway_paths() {
        let gateway = Path::new("/tmp/xenobot gate'space");
        let recommendation = build_sandbox_start_recommendation(false, false, gateway);
        assert_eq!(recommendation.mode, "file-gateway");
        assert!(recommendation.command.contains("--force-file-gateway"));
        assert!(recommendation.command.contains("--file-gateway-dir '/tmp/xenobot gate'\"'\"'space'"));
    }
}
