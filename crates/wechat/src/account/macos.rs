//! macOS-specific WeChat detection with legal-safe key policy.

use super::Account;
use super::WeChatDetector;
use plist;
use std::path::Path;
use std::process::Command;
use std::str;
use sysinfo::{Pid, System};

/// macOS WeChat detector implementation.
pub struct MacOSWeChatDetector;

impl WeChatDetector for MacOSWeChatDetector {
    fn get_running_instances(&self) -> Vec<Account> {
        let mut accounts = Vec::new();
        let sys = System::new_all();

        for (pid, process) in sys.processes() {
            let name = process.name();
            if name != "WeChat" && name != "Weixin" {
                continue;
            }

            let exe_path = process
                .exe()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let version_info = get_version_from_plist(&exe_path).unwrap_or_else(|_| VersionInfo {
                major: 4,
                full_version: "4.0.0".to_string(),
            });
            let (data_dir, account_name) = get_data_dir_and_account(*pid, version_info.major)
                .unwrap_or_else(|_| (String::new(), String::new()));

            accounts.push(Account::new(
                (*pid).as_u32(),
                account_name,
                data_dir,
                version_info.major.to_string(),
                version_info.full_version,
                "macOS".to_string(),
            ));
        }

        accounts
    }

    fn extract_keys(&self, _pid: u32) -> Result<(String, String), String> {
        Err(
            "Direct process-memory key extraction is disabled in Xenobot legal-safe mode. Provide keys via authorized input or system secure storage.".to_string(),
        )
    }
}

/// Version information loaded from WeChat app bundle metadata.
struct VersionInfo {
    major: u32,
    full_version: String,
}

/// Read version information from WeChat app `Info.plist`.
fn get_version_from_plist(exe_path: &str) -> Result<VersionInfo, String> {
    let path = Path::new(exe_path);
    let info_plist = path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|parent| parent.join("Info.plist"))
        .ok_or_else(|| "invalid executable path".to_string())?;

    let bytes = std::fs::read(&info_plist).map_err(|e| e.to_string())?;
    let plist_dict: plist::Dictionary = plist::from_bytes(&bytes).map_err(|e| e.to_string())?;

    let full_version = plist_dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .unwrap_or("4.0.0")
        .to_string();
    let major = full_version
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(4);

    Ok(VersionInfo {
        major,
        full_version,
    })
}

/// Resolve WeChat data directory and account identifier via `lsof`.
fn get_data_dir_and_account(pid: Pid, major_version: u32) -> Result<(String, String), String> {
    let output = Command::new("lsof")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-F")
        .arg("n")
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("lsof command failed".to_string());
    }

    let output_str = str::from_utf8(&output.stdout).map_err(|e| e.to_string())?;
    let db_pattern = if major_version == 4 {
        "db_storage/session/session.db"
    } else {
        "Message/msg_0.db"
    };

    for line in output_str.lines() {
        if !line.starts_with('n') {
            continue;
        }
        let file_path = &line[1..];
        if !file_path.contains(db_pattern) {
            continue;
        }

        let path = Path::new(file_path);
        let components: Vec<_> = path.components().collect();
        if components.len() < 4 {
            continue;
        }

        let data_dir = if major_version == 4 {
            path.parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            path.parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };
        let account_name = components[components.len() - if major_version == 4 { 4 } else { 3 }]
            .as_os_str()
            .to_string_lossy()
            .to_string();
        return Ok((data_dir, account_name));
    }

    Err("could not find data directory".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_keys_is_disabled_in_legal_safe_mode() {
        let detector = MacOSWeChatDetector;
        let err = detector
            .extract_keys(12345)
            .expect_err("extract_keys should be disabled");
        assert!(err.to_lowercase().contains("disabled"));
        assert!(err.to_lowercase().contains("legal-safe"));
    }
}
