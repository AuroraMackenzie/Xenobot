//! Windows-specific WeChat detector placeholder.
//!
//! Xenobot currently focuses on macOS arm64 as the primary target.
//! This module keeps the Windows build surface explicit and compilable.

use super::{Account, WeChatDetector};

/// Windows WeChat detector implementation.
pub struct WindowsWeChatDetector;

impl WeChatDetector for WindowsWeChatDetector {
    fn get_running_instances(&self) -> Vec<Account> {
        Vec::new()
    }

    fn extract_keys(&self, _pid: u32) -> Result<(String, String), String> {
        Err("Windows key extraction is not implemented in Xenobot yet".to_string())
    }
}
