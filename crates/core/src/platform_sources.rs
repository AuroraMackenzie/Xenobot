//! Legal-safe platform source discovery for supported chat apps.
//!
//! This module does not perform process memory access or encryption bypass.
//! It only discovers user-owned local paths that can be used for authorized
//! export ingestion and monitoring workflows.

use crate::types::Platform;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Source category for a discovered path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Sandbox/container app data directory.
    AppContainer,
    /// User-visible export/download directory.
    ExportDirectory,
    /// User-defined workspace-like location.
    UserWorkspace,
}

/// A single source path candidate for a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCandidate {
    /// Platform enum.
    pub platform: Platform,
    /// Stable platform identifier.
    pub platform_id: String,
    /// Human-readable source label.
    pub label: String,
    /// Source category.
    pub kind: SourceKind,
    /// Candidate path.
    pub path: PathBuf,
    /// Whether path exists.
    pub exists: bool,
    /// Whether path can be read by current user.
    pub readable: bool,
}

/// Runtime platform set covered by Xenobot's legal-safe extraction flow.
pub fn legal_safe_runtime_platforms() -> Vec<Platform> {
    vec![
        Platform::WeChat,
        Platform::WhatsApp,
        Platform::Line,
        Platform::Qq,
        Platform::Discord,
        Platform::Instagram,
        Platform::Telegram,
        Platform::IMessage,
        Platform::Messenger,
        Platform::KakaoTalk,
        Platform::Slack,
        Platform::Teams,
        Platform::Signal,
        Platform::Custom("skype".to_string()),
        Platform::Custom("googlechat".to_string()),
        Platform::Custom("zoom".to_string()),
        Platform::Custom("viber".to_string()),
    ]
}

/// Convert platform enum to a stable platform id.
pub fn platform_id(platform: &Platform) -> &'static str {
    match platform {
        Platform::WeChat => "wechat",
        Platform::WhatsApp => "whatsapp",
        Platform::Line => "line",
        Platform::Telegram => "telegram",
        Platform::Qq => "qq",
        Platform::Discord => "discord",
        Platform::Instagram => "instagram",
        Platform::IMessage => "imessage",
        Platform::Messenger => "messenger",
        Platform::KakaoTalk => "kakaotalk",
        Platform::Slack => "slack",
        Platform::Teams => "teams",
        Platform::Signal => "signal",
        Platform::Custom(name) if name.eq_ignore_ascii_case("skype") => "skype",
        Platform::Custom(name) if name.eq_ignore_ascii_case("googlechat") => "googlechat",
        Platform::Custom(name) if name.eq_ignore_ascii_case("zoom") => "zoom",
        Platform::Custom(name) if name.eq_ignore_ascii_case("viber") => "viber",
        Platform::Custom(_) => "custom",
    }
}

/// Parse a runtime platform id into a known platform enum.
pub fn parse_runtime_platform_id(raw: &str) -> Option<Platform> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "wechat" | "weixin" | "wx" => Some(Platform::WeChat),
        "whatsapp" | "wa" => Some(Platform::WhatsApp),
        "line" => Some(Platform::Line),
        "qq" => Some(Platform::Qq),
        "discord" => Some(Platform::Discord),
        "instagram" | "ig" => Some(Platform::Instagram),
        "telegram" | "tg" => Some(Platform::Telegram),
        "imessage" => Some(Platform::IMessage),
        "messenger" | "facebook" => Some(Platform::Messenger),
        "kakaotalk" | "kakao" => Some(Platform::KakaoTalk),
        "slack" => Some(Platform::Slack),
        "teams" | "msteams" => Some(Platform::Teams),
        "signal" => Some(Platform::Signal),
        "skype" => Some(Platform::Custom("skype".to_string())),
        "googlechat" | "hangouts" => Some(Platform::Custom("googlechat".to_string())),
        "zoom" => Some(Platform::Custom("zoom".to_string())),
        "viber" => Some(Platform::Custom("viber".to_string())),
        _ => Some(Platform::Custom(normalized)),
    }
}

/// Discover path candidates for a single platform.
pub fn discover_sources_for_platform(platform: &Platform) -> Vec<SourceCandidate> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    discover_sources_for_platform_with_home(platform, &home)
}

/// Discover path candidates for all runtime platforms.
pub fn discover_sources_for_all_platforms() -> Vec<SourceCandidate> {
    let mut out = Vec::new();
    for platform in legal_safe_runtime_platforms() {
        out.extend(discover_sources_for_platform(&platform));
    }
    out
}

fn discover_sources_for_platform_with_home(
    platform: &Platform,
    home: &Path,
) -> Vec<SourceCandidate> {
    let mut out = Vec::new();
    for (kind, label, path) in default_source_paths(platform, home) {
        let exists = path.exists();
        let readable = if exists { is_readable(&path) } else { false };
        out.push(SourceCandidate {
            platform: platform.clone(),
            platform_id: platform_id(platform).to_string(),
            label,
            kind,
            path,
            exists,
            readable,
        });
    }
    out
}

fn default_source_paths(platform: &Platform, home: &Path) -> Vec<(SourceKind, String, PathBuf)> {
    let downloads = home.join("Downloads");
    let desktop = home.join("Desktop");
    let documents = home.join("Documents");
    let library = home.join("Library");

    match platform {
        Platform::WeChat => vec![
            (
                SourceKind::AppContainer,
                "macOS WeChat sandbox data".to_string(),
                library
                    .join("Containers")
                    .join("com.tencent.xinWeChat")
                    .join("Data")
                    .join("Library")
                    .join("Application Support")
                    .join("com.tencent.xinWeChat"),
            ),
            (
                SourceKind::ExportDirectory,
                "User export folder".to_string(),
                documents.join("XenobotImports").join("wechat"),
            ),
        ],
        Platform::WhatsApp => vec![
            (
                SourceKind::AppContainer,
                "macOS WhatsApp sandbox data".to_string(),
                library
                    .join("Containers")
                    .join("WhatsApp")
                    .join("Data")
                    .join("Library")
                    .join("Application Support")
                    .join("WhatsApp"),
            ),
            (
                SourceKind::ExportDirectory,
                "WhatsApp chat exports in Downloads".to_string(),
                downloads.join("WhatsApp Chat"),
            ),
        ],
        Platform::Line => vec![
            (
                SourceKind::AppContainer,
                "LINE desktop app data".to_string(),
                library.join("Application Support").join("LINE"),
            ),
            (
                SourceKind::AppContainer,
                "LINE sandbox app data".to_string(),
                library
                    .join("Containers")
                    .join("jp.naver.line.mac")
                    .join("Data")
                    .join("Library")
                    .join("Application Support")
                    .join("LINE"),
            ),
            (
                SourceKind::ExportDirectory,
                "LINE exports in Downloads".to_string(),
                downloads.join("LINE"),
            ),
        ],
        Platform::Qq => vec![
            (
                SourceKind::AppContainer,
                "QQ sandbox app data".to_string(),
                library
                    .join("Containers")
                    .join("com.tencent.qq")
                    .join("Data")
                    .join("Library")
                    .join("Application Support")
                    .join("QQ"),
            ),
            (
                SourceKind::ExportDirectory,
                "QQ exports in Documents".to_string(),
                documents.join("QQExport"),
            ),
        ],
        Platform::Discord => vec![
            (
                SourceKind::AppContainer,
                "Discord desktop app data".to_string(),
                library.join("Application Support").join("discord"),
            ),
            (
                SourceKind::ExportDirectory,
                "Discord exports in Downloads".to_string(),
                downloads.join("discord-export"),
            ),
        ],
        Platform::Instagram => vec![
            (
                SourceKind::ExportDirectory,
                "Instagram account export archive".to_string(),
                downloads.join("instagram-data"),
            ),
            (
                SourceKind::UserWorkspace,
                "User workspace import drop".to_string(),
                desktop.join("xenobot-imports").join("instagram"),
            ),
        ],
        Platform::Telegram => vec![
            (
                SourceKind::AppContainer,
                "Telegram Desktop app data".to_string(),
                library.join("Application Support").join("Telegram Desktop"),
            ),
            (
                SourceKind::ExportDirectory,
                "Telegram exports in Downloads".to_string(),
                downloads.join("Telegram Desktop"),
            ),
        ],
        Platform::IMessage => vec![
            (
                SourceKind::AppContainer,
                "macOS Messages local store".to_string(),
                library.join("Messages"),
            ),
            (
                SourceKind::ExportDirectory,
                "iMessage exports in Documents".to_string(),
                documents.join("iMessageExport"),
            ),
        ],
        Platform::Messenger => vec![
            (
                SourceKind::ExportDirectory,
                "Messenger exports in Downloads".to_string(),
                downloads.join("facebook-messenger-export"),
            ),
            (
                SourceKind::UserWorkspace,
                "User workspace import drop".to_string(),
                desktop.join("xenobot-imports").join("messenger"),
            ),
        ],
        Platform::KakaoTalk => vec![
            (
                SourceKind::AppContainer,
                "KakaoTalk desktop app data".to_string(),
                library.join("Application Support").join("KakaoTalk"),
            ),
            (
                SourceKind::ExportDirectory,
                "KakaoTalk exports in Downloads".to_string(),
                downloads.join("kakaotalk-export"),
            ),
        ],
        Platform::Slack => vec![
            (
                SourceKind::AppContainer,
                "Slack desktop app data".to_string(),
                library.join("Application Support").join("Slack"),
            ),
            (
                SourceKind::ExportDirectory,
                "Slack exports in Downloads".to_string(),
                downloads.join("slack-export"),
            ),
        ],
        Platform::Teams => vec![
            (
                SourceKind::AppContainer,
                "Microsoft Teams app data".to_string(),
                library
                    .join("Application Support")
                    .join("Microsoft")
                    .join("Teams"),
            ),
            (
                SourceKind::ExportDirectory,
                "Teams exports in Downloads".to_string(),
                downloads.join("teams-export"),
            ),
        ],
        Platform::Signal => vec![
            (
                SourceKind::AppContainer,
                "Signal Desktop app data".to_string(),
                library.join("Application Support").join("Signal"),
            ),
            (
                SourceKind::ExportDirectory,
                "Signal exports in Downloads".to_string(),
                downloads.join("signal-export"),
            ),
        ],
        Platform::Custom(name) if name.eq_ignore_ascii_case("skype") => vec![
            (
                SourceKind::AppContainer,
                "Skype local app data".to_string(),
                library.join("Application Support").join("Skype"),
            ),
            (
                SourceKind::ExportDirectory,
                "Skype exports in Downloads".to_string(),
                downloads.join("skype-export"),
            ),
        ],
        Platform::Custom(name) if name.eq_ignore_ascii_case("googlechat") => vec![
            (
                SourceKind::ExportDirectory,
                "Google Chat exports in Downloads".to_string(),
                downloads.join("google-chat-export"),
            ),
            (
                SourceKind::UserWorkspace,
                "User workspace import drop".to_string(),
                desktop.join("xenobot-imports").join("googlechat"),
            ),
        ],
        Platform::Custom(name) if name.eq_ignore_ascii_case("zoom") => vec![
            (
                SourceKind::ExportDirectory,
                "Zoom chat exports in Downloads".to_string(),
                downloads.join("zoom-export"),
            ),
            (
                SourceKind::UserWorkspace,
                "User workspace import drop".to_string(),
                desktop.join("xenobot-imports").join("zoom"),
            ),
        ],
        Platform::Custom(name) if name.eq_ignore_ascii_case("viber") => vec![
            (
                SourceKind::ExportDirectory,
                "Viber exports in Downloads".to_string(),
                downloads.join("viber-export"),
            ),
            (
                SourceKind::UserWorkspace,
                "User workspace import drop".to_string(),
                desktop.join("xenobot-imports").join("viber"),
            ),
        ],
        _ => Vec::new(),
    }
}

fn is_readable(path: &Path) -> bool {
    if path.is_dir() {
        std::fs::read_dir(path).is_ok()
    } else {
        std::fs::File::open(path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_runtime_platform_ids() {
        assert_eq!(parse_runtime_platform_id("wechat"), Some(Platform::WeChat));
        assert_eq!(parse_runtime_platform_id("WA"), Some(Platform::WhatsApp));
        assert_eq!(parse_runtime_platform_id("line"), Some(Platform::Line));
        assert_eq!(parse_runtime_platform_id("QQ"), Some(Platform::Qq));
        assert_eq!(
            parse_runtime_platform_id("discord"),
            Some(Platform::Discord)
        );
        assert_eq!(
            parse_runtime_platform_id("instagram"),
            Some(Platform::Instagram)
        );
        assert_eq!(parse_runtime_platform_id("tg"), Some(Platform::Telegram));
        assert_eq!(
            parse_runtime_platform_id("imessage"),
            Some(Platform::IMessage)
        );
        assert_eq!(
            parse_runtime_platform_id("messenger"),
            Some(Platform::Messenger)
        );
        assert_eq!(
            parse_runtime_platform_id("kakao"),
            Some(Platform::KakaoTalk)
        );
        assert_eq!(parse_runtime_platform_id("slack"), Some(Platform::Slack));
        assert_eq!(parse_runtime_platform_id("teams"), Some(Platform::Teams));
        assert_eq!(
            parse_runtime_platform_id("skype"),
            Some(Platform::Custom("skype".to_string()))
        );
        assert_eq!(
            parse_runtime_platform_id("unknown-platform"),
            Some(Platform::Custom("unknown-platform".to_string()))
        );
    }

    #[test]
    fn wechat_default_paths_are_generated() {
        let home = PathBuf::from("/tmp/xeno-home");
        let items = discover_sources_for_platform_with_home(&Platform::WeChat, &home);
        assert!(items.len() >= 2);
        assert!(items.iter().all(|item| item.platform == Platform::WeChat));
        assert!(items.iter().any(|item| item.platform_id == "wechat"));
    }
}
