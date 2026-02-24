//! macOS-specific WeChat detection and legal-safe key resolution.

use super::Account;
use super::WeChatDetector;
use crate::error::{WeChatError, WeChatResult};
use hex;
use libc::pid_t;
use mach::kern_return::KERN_SUCCESS;
use mach::traps::{mach_task_self, task_for_pid};
use mach::vm::mach_vm_read_overwrite;
use mach::vm_types::{mach_vm_address_t, mach_vm_size_t};
use plist; // For parsing Info.plist // For hex encoding/decoding

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

            // Get executable path
            let exe_path = process
                .exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Get version from Info.plist
            let version_info = get_version_from_plist(&exe_path).unwrap_or_else(|_| VersionInfo {
                version: 4,
                full_version: "4.0.0".to_string(),
                company_name: String::new(),
            });

            // Get data directory and account name via lsof
            let (data_dir, account_name) = get_data_dir_and_account(*pid, version_info.version)
                .unwrap_or_else(|_| (String::new(), String::new()));

            let account = Account::new(
                (*pid).as_u32(),
                account_name,
                data_dir,
                version_info.version.to_string(),
                version_info.full_version,
                "macOS".to_string(),
            );
            accounts.push(account);
        }

        accounts
    }

    fn extract_keys(&self, _pid: u32) -> Result<(String, String), String> {
        Err(
            "Direct process-memory key extraction is disabled in Xenobot legal-safe mode. Provide keys via authorized input or system secure storage.".to_string(),
        )
    }
}

/// Version information from Info.plist
struct VersionInfo {
    version: u32,
    full_version: String,
    #[allow(dead_code)]
    company_name: String,
}

/// Get version information from WeChat app's Info.plist
fn get_version_from_plist(exe_path: &str) -> Result<VersionInfo, String> {
    let path = Path::new(exe_path);
    // Executable is at Contents/MacOS/WeChat, Info.plist is at ../Info.plist
    let info_plist = path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Info.plist"))
        .ok_or_else(|| "Invalid executable path".to_string())?;

    let bytes = std::fs::read(&info_plist).map_err(|e| e.to_string())?;
    let plist_dict: plist::Dictionary = plist::from_bytes(&bytes).map_err(|e| e.to_string())?;

    let short_version = plist_dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .unwrap_or("4.0.0");
    let copyright = plist_dict
        .get("NSHumanReadableCopyright")
        .and_then(|v| v.as_string())
        .unwrap_or("");

    let version = short_version
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(4);

    Ok(VersionInfo {
        version,
        full_version: short_version.to_string(),
        company_name: copyright.to_string(),
    })
}

/// Get data directory and account name using lsof
fn get_data_dir_and_account(pid: Pid, version: u32) -> Result<(String, String), String> {
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

    // Determine DB file pattern based on version
    let db_pattern = if version == 4 {
        "db_storage/session/session.db"
    } else {
        "Message/msg_0.db"
    };

    for line in output_str.lines() {
        if line.starts_with('n') {
            let file_path = &line[1..];
            if file_path.contains(db_pattern) {
                let path = Path::new(file_path);
                let components: Vec<_> = path.components().collect();
                if components.len() >= 4 {
                    let data_dir = if version == 4 {
                        // /Users/.../xwechat_files/<account_id>/db_storage/session/session.db
                        // data dir is parent of db_storage
                        path.parent()
                            .and_then(|p| p.parent())
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default()
                    } else {
                        // /Users/.../<account_id>/Message/msg_0.db
                        // data dir is parent of Message
                        path.parent()
                            .and_then(|p| p.parent())
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default()
                    };
                    let account_name = components
                        [components.len() - if version == 4 { 4 } else { 3 }]
                    .as_os_str()
                    .to_string_lossy()
                    .to_string();
                    return Ok((data_dir, account_name));
                }
            }
        }
    }

    Err("Could not find data directory".to_string())
}

/// Find database file path for given data directory and version.
fn find_database_file(data_dir: &str, version: u32) -> Option<String> {
    use std::path::Path;
    let path = Path::new(data_dir);

    if version == 4 {
        // macOS V4: db_storage/message/message_0.db
        let db_path = path.join("db_storage/message/message_0.db");
        if db_path.exists() {
            return Some(db_path.to_string_lossy().to_string());
        }
        // Also check session.db
        let session_db = path.join("db_storage/session/session.db");
        if session_db.exists() {
            return Some(session_db.to_string_lossy().to_string());
        }
    } else {
        // macOS V3: Message/msg_0.db
        let db_path = path.join("Message/msg_0.db");
        if db_path.exists() {
            return Some(db_path.to_string_lossy().to_string());
        }
    }
    None
}

/// Check if SIP is disabled
fn is_sip_disabled() -> bool {
    let output = Command::new("csrutil").arg("status").output().ok();

    if let Some(output) = output {
        if let Ok(output_str) = str::from_utf8(&output.stdout) {
            let output_lower = output_str.to_lowercase();
            return output_lower.contains("system integrity protection status: disabled")
                || (output_lower.contains("disabled") && output_lower.contains("debugging"));
        }
    }

    false
}

/// Memory region information
struct MemRegion {
    start: u64,
    end: u64,
    size: u64,
    region_type: String,
    permissions: String,
}

/// Get memory regions using vmmap command
fn get_memory_regions(pid: u32) -> Result<Vec<MemRegion>, String> {
    let output = Command::new("vmmap")
        .arg("-wide")
        .arg(pid.to_string())
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("vmmap command failed".to_string());
    }

    let output_str = str::from_utf8(&output.stdout).map_err(|e| e.to_string())?;
    parse_vmmap_output(output_str)
}

/// Parse vmmap output
fn parse_vmmap_output(output: &str) -> Result<Vec<MemRegion>, String> {
    let mut regions = Vec::new();
    let mut in_writable_section = false;

    // Determine Darwin version for region type filter
    let darwin_version = get_darwin_version();
    let target_region_type = if darwin_version.starts_with("25") {
        "MALLOC_SMALL"
    } else {
        "MALLOC_NANO"
    };

    for line in output.lines() {
        if line.contains("==== Writable regions for") {
            in_writable_section = true;
            continue;
        }
        if !in_writable_section {
            continue;
        }
        if line.trim().is_empty() {
            break;
        }

        // Parse line: REGION TYPE START - END [VSIZE RSDNT DIRTY SWAP] PRT/MAX SHRMOD PURGE REGION DETAIL
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        let region_type = parts[0];
        if region_type != target_region_type {
            continue;
        }

        let address_range = parts[1];
        if !address_range.contains('-') {
            continue;
        }

        let mut split = address_range.split('-');
        let start_str = split.next().unwrap();
        let end_str = split.next().unwrap();

        let start = u64::from_str_radix(start_str, 16).map_err(|e| e.to_string())?;
        let end = u64::from_str_radix(end_str, 16).map_err(|e| e.to_string())?;
        let size = end - start;

        // Check if region is empty
        if line.contains("(empty)") {
            continue;
        }

        let permissions = parts[6].to_string();

        regions.push(MemRegion {
            start,
            end,
            size,
            region_type: region_type.to_string(),
            permissions,
        });
    }

    Ok(regions)
}

/// Get Darwin kernel version
fn get_darwin_version() -> String {
    Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|output| {
            str::from_utf8(&output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default()
}

/// V4 key patterns from chatlog
const V4_KEY_PATTERNS: &[(&[u8], &[i32])] = &[
    (
        &[0x20, 0x66, 0x74, 0x73, 0x35, 0x28, 0x25, 0x00],
        &[16, -80, 64],
    ),
    (
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        &[-32],
    ),
];

/// V4 image key patterns
const V4_IMG_KEY_PATTERNS: &[(&[u8], &[i32])] = &[(
    &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    &[-32],
)];

/// Scan memory for data key (32 bytes) with custom validator
fn scan_for_data_key<F>(memory: &[u8], validator: F) -> Option<String>
where
    F: Fn(&[u8]) -> bool,
{
    for (pattern, offsets) in V4_KEY_PATTERNS {
        // Search for pattern
        let mut index = memory.len();
        let zero_pattern = pattern.iter().all(|&b| b == 0);

        while index > 0 {
            if let Some(pos) = memory[..index]
                .windows(pattern.len())
                .rposition(|window| window == *pattern)
            {
                let mut aligned_pos = pos;

                // Align to 16 bytes for zero pattern
                if zero_pattern {
                    aligned_pos = memory[..pos]
                        .iter()
                        .rposition(|&b| b != 0)
                        .map(|p| p + 1)
                        .unwrap_or(0);
                }

                for &offset in *offsets {
                    let key_offset = aligned_pos as i32 + offset;
                    if key_offset < 0 || key_offset as usize + 32 > memory.len() {
                        continue;
                    }

                    let start = key_offset as usize;
                    let end = start + 32;
                    let key_data = &memory[start..end];

                    // Skip if key contains null bytes (simple heuristic)
                    if key_data.contains(&0x00) && key_data.contains(&0x00) {
                        continue;
                    }

                    // Validate key using provided validator
                    if validator(key_data) {
                        return Some(hex::encode(key_data));
                    }
                }

                index = pos;
            } else {
                break;
            }
        }
    }

    None
}

/// Scan memory for image key (16 bytes) with custom validator
fn scan_for_img_key<F>(memory: &[u8], validator: F) -> Option<String>
where
    F: Fn(&[u8]) -> bool,
{
    for (pattern, offsets) in V4_IMG_KEY_PATTERNS {
        let mut index = memory.len();

        while index > 0 {
            if let Some(pos) = memory[..index]
                .windows(pattern.len())
                .rposition(|window| window == *pattern)
            {
                // Align to 16 bytes for zero pattern
                let aligned_pos = memory[..pos]
                    .iter()
                    .rposition(|&b| b != 0)
                    .map(|p| p + 1)
                    .unwrap_or(0);

                for &offset in *offsets {
                    let key_offset = aligned_pos as i32 + offset;
                    if key_offset < 0 || key_offset as usize + 16 > memory.len() {
                        continue;
                    }

                    let start = key_offset as usize;
                    let end = start + 16;
                    let key_data = &memory[start..end];

                    // Skip if key contains null bytes
                    if key_data.contains(&0x00) && key_data.contains(&0x00) {
                        continue;
                    }

                    // Validate image key using provided validator
                    if validator(key_data) {
                        return Some(hex::encode(key_data));
                    }
                }

                index = pos;
            } else {
                break;
            }
        }
    }

    None
}

/// Validate V4 data key (placeholder - implement using decrypt module)
fn validate_v4_key(key_data: &[u8]) -> bool {
    // TODO: Use actual validation from decrypt module
    // For now, accept any key that looks plausible (not all zeros)
    !key_data.iter().all(|&b| b == 0)
}

/// Validate data key against database file using decrypt module
fn validate_data_key_with_db(key_data: &[u8], db_path: &str) -> bool {
    use crate::decrypt::validate_v4_key as validate;
    // Create dummy image key (16 zeros) because validate_v4_key expects both keys
    let dummy_img_key = vec![0u8; 16];
    validate(std::path::Path::new(db_path), key_data, &dummy_img_key).unwrap_or(false)
}

/// Validate both data and image keys against database file
fn validate_both_keys_with_db(data_key: &[u8], img_key: &[u8], db_path: &str) -> bool {
    use crate::decrypt::validate_v4_key as validate;
    validate(std::path::Path::new(db_path), data_key, img_key).unwrap_or(false)
}

/// Validate image key (placeholder)
fn validate_img_key(key_data: &[u8]) -> bool {
    !key_data.iter().all(|&b| b == 0)
}

/// Helper function to read process memory.
#[allow(unsafe_code)]
unsafe fn read_process_memory(pid: pid_t, address: usize, size: usize) -> WeChatResult<Vec<u8>> {
    let mut task: mach::port::mach_port_name_t = 0;
    let ret = task_for_pid(mach_task_self(), pid, &mut task);
    if ret != KERN_SUCCESS {
        return Err(WeChatError::Platform(format!(
            "task_for_pid failed: {}",
            ret
        )));
    }

    let mut data = vec![0u8; size];
    let mut data_size = size as u64;
    let ret = mach_vm_read_overwrite(
        task,
        address as mach_vm_address_t,
        size as mach_vm_size_t,
        data.as_mut_ptr() as mach_vm_address_t,
        &mut data_size,
    );
    if ret != KERN_SUCCESS {
        return Err(WeChatError::Platform(format!(
            "mach_vm_read_overwrite failed: {}",
            ret
        )));
    }

    Ok(data)
}
