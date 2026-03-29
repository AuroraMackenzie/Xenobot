//! Machine-readable platform capability matrix for Xenobot.
//!
//! The goal of this module is to keep Xenobot's stated platform coverage aligned
//! with the code that actually exists today. It intentionally separates:
//! - legal-safe ingest/runtime depth per platform, and
//! - downstream analysis features that become available after normalization.

use crate::platform_sources::{legal_safe_runtime_platforms, platform_id};
use crate::types::Platform;
use serde::{Deserialize, Serialize};

/// Practical depth tier for a platform inside Xenobot's current legal-safe scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformDepthTier {
    /// Current deepest reference layer inside Xenobot.
    WechatReference,
    /// Shared legal-safe orchestration layer below the WeChat reference depth.
    LegalSafeOrchestrated,
}

/// Priority wave for parity work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformPriorityWave {
    /// WeChat remains the reference anchor.
    ReferenceAnchor,
    /// First wave of non-WeChat parity targets.
    Wave1,
    /// Second wave of non-WeChat parity targets.
    Wave2,
}

/// Ingest/runtime capabilities for a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformIngestCapabilities {
    /// User-local path discovery is available.
    pub source_discovery: bool,
    /// Authorized export parsing exists.
    pub authorized_export_parsing: bool,
    /// Parsed data can be written into Xenobot's normalized database.
    pub normalized_database_import: bool,
    /// Incremental import is available through the normalized import path.
    pub incremental_import: bool,
    /// Platform media inventory helpers are available.
    pub media_inventory: bool,
    /// Audio payload/file transcoding helpers are available.
    pub audio_transcode: bool,
    /// File-watch preparation exists for authorized directories.
    pub watch_dir_monitor: bool,
    /// Service-level authorized workspace assembly exists.
    pub workspace_orchestration: bool,
    /// Platform-specific runtime detector/orchestrator exists.
    pub native_runtime_detector: bool,
    /// Platform-specific legal-safe decrypt flow exists.
    pub legal_safe_decrypt: bool,
    /// Advanced normalization planners are complete.
    pub advanced_normalization_planners: bool,
}

/// Downstream capabilities available after successful normalized import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDownstreamCapabilities {
    /// Basic activity/statistics endpoints are available.
    pub basic_statistics: bool,
    /// Higher-level analysis endpoints are available.
    pub advanced_analysis: bool,
    /// AI tool routes can work against imported records.
    pub ai_tools: bool,
    /// Semantic search / RAG entrypoints are available.
    pub semantic_search: bool,
    /// SQL Lab can query the imported records.
    pub sql_lab: bool,
    /// MCP tools can query imported records.
    pub mcp_tools: bool,
}

/// A single platform entry in the current capability report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilityEntry {
    /// Stable platform identifier.
    pub platform_id: String,
    /// Human-readable display name.
    pub name: String,
    /// Practical depth tier inside Xenobot's current legal-safe implementation.
    pub tier: PlatformDepthTier,
    /// Priority wave for further parity work.
    pub priority_wave: PlatformPriorityWave,
    /// Whether this platform is the current reference anchor.
    pub wechat_reference_anchor: bool,
    /// Whether this platform has reached the current WeChat reference depth.
    pub at_wechat_depth: bool,
    /// Whether the full planned end state has been reached.
    pub planned_end_state_reached: bool,
    /// Current ingest/runtime coverage.
    pub ingest: PlatformIngestCapabilities,
    /// Current downstream analysis/query coverage after normalized import.
    pub downstream: PlatformDownstreamCapabilities,
    /// Known gaps that still block the full target state.
    pub known_gaps: Vec<String>,
    /// Next focus for this platform.
    pub next_focus: String,
}

/// Scope metadata for the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilityScope {
    /// Whether the matrix only describes Xenobot's legal-safe scope.
    pub legal_safe_only: bool,
    /// Explicitly excluded implementation styles.
    pub excluded_implementation_styles: Vec<String>,
    /// Important interpretation notes for the matrix.
    pub notes: Vec<String>,
}

/// Aggregate summary for the current capability report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilitySummary {
    /// Number of listed platforms.
    pub total_platforms: usize,
    /// Number of platforms currently at WeChat reference depth.
    pub platforms_at_wechat_depth: usize,
    /// Number of platforms still below WeChat reference depth.
    pub platforms_below_wechat_depth: usize,
    /// Number of platforms with a platform-specific runtime detector layer.
    pub platforms_with_runtime_detector: usize,
    /// Number of platforms with a platform-specific legal-safe decrypt path.
    pub platforms_with_legal_safe_decrypt: usize,
    /// Number of platforms with the normalized DB import path.
    pub platforms_with_normalized_import: usize,
    /// Number of platforms with incremental import through the normalized path.
    pub platforms_with_incremental_import: usize,
    /// Number of platforms whose imported data can use the shared downstream stack.
    pub platforms_with_platform_agnostic_analysis: usize,
    /// Whether all platforms have reached the originally requested end state.
    pub all_platforms_at_planned_end_state: bool,
    /// Whether all platforms have reached WeChat reference depth.
    pub all_platforms_at_wechat_depth: bool,
}

/// Complete capability report for the current Xenobot build.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilityReport {
    /// Scope metadata.
    pub scope: PlatformCapabilityScope,
    /// Summary counters.
    pub summary: PlatformCapabilitySummary,
    /// Per-platform capability entries.
    pub platforms: Vec<PlatformCapabilityEntry>,
}

/// Build the current machine-readable capability report.
pub fn platform_capability_report() -> PlatformCapabilityReport {
    let platforms: Vec<_> = legal_safe_runtime_platforms()
        .into_iter()
        .map(build_platform_capability_entry)
        .collect();

    let platforms_at_wechat_depth = platforms
        .iter()
        .filter(|entry| entry.at_wechat_depth)
        .count();
    let platforms_with_runtime_detector = platforms
        .iter()
        .filter(|entry| entry.ingest.native_runtime_detector)
        .count();
    let platforms_with_legal_safe_decrypt = platforms
        .iter()
        .filter(|entry| entry.ingest.legal_safe_decrypt)
        .count();
    let platforms_with_normalized_import = platforms
        .iter()
        .filter(|entry| entry.ingest.normalized_database_import)
        .count();
    let platforms_with_incremental_import = platforms
        .iter()
        .filter(|entry| entry.ingest.incremental_import)
        .count();
    let platforms_with_platform_agnostic_analysis = platforms
        .iter()
        .filter(|entry| {
            entry.downstream.basic_statistics
                && entry.downstream.advanced_analysis
                && entry.downstream.ai_tools
                && entry.downstream.semantic_search
                && entry.downstream.sql_lab
                && entry.downstream.mcp_tools
        })
        .count();

    let total_platforms = platforms.len();

    PlatformCapabilityReport {
        scope: PlatformCapabilityScope {
            legal_safe_only: true,
            excluded_implementation_styles: vec![
                "process-memory scanning".to_string(),
                "key extraction from foreign process memory".to_string(),
                "encryption bypass".to_string(),
                "DLL hook chains".to_string(),
                "reference-project UI or asset reuse".to_string(),
            ],
            notes: vec![
                "Downstream analysis, AI, SQL, and MCP features become available after a platform's data is successfully normalized into Xenobot's database.".to_string(),
                "A platform appearing in the 17-platform matrix does not mean that its real-world native workflow is already at WeChat depth.".to_string(),
                "The normalized import path exists across the 17-platform matrix, but advanced normalization and dedicated incremental planners are still in progress.".to_string(),
            ],
        },
        summary: PlatformCapabilitySummary {
            total_platforms,
            platforms_at_wechat_depth,
            platforms_below_wechat_depth: total_platforms - platforms_at_wechat_depth,
            platforms_with_runtime_detector,
            platforms_with_legal_safe_decrypt,
            platforms_with_normalized_import,
            platforms_with_incremental_import,
            platforms_with_platform_agnostic_analysis,
            all_platforms_at_planned_end_state: platforms
                .iter()
                .all(|entry| entry.planned_end_state_reached),
            all_platforms_at_wechat_depth: platforms.iter().all(|entry| entry.at_wechat_depth),
        },
        platforms,
    }
}

fn build_platform_capability_entry(platform: Platform) -> PlatformCapabilityEntry {
    let platform_id = platform_id(&platform).to_string();
    let name = platform_name(&platform).to_string();
    let priority_wave = priority_wave(&platform);

    match platform {
        Platform::WeChat => PlatformCapabilityEntry {
            platform_id,
            name,
            tier: PlatformDepthTier::WechatReference,
            priority_wave,
            wechat_reference_anchor: true,
            at_wechat_depth: true,
            planned_end_state_reached: false,
            ingest: PlatformIngestCapabilities {
                native_runtime_detector: true,
                legal_safe_decrypt: true,
                ..shared_ingest_capabilities()
            },
            downstream: shared_downstream_capabilities(),
            known_gaps: vec![
                "WeChat is the current reference anchor, but the full planned end state is still open because the shared advanced-normalization and dedicated incremental-planner work is not finished.".to_string(),
                "The current implementation remains intentionally legal-safe and does not include process-memory extraction or encryption bypass.".to_string(),
            ],
            next_focus: "Keep WeChat as the reference anchor while the shared normalization and planner gap is closed for the whole stack.".to_string(),
        },
        _ => PlatformCapabilityEntry {
            platform_id,
            name: name.clone(),
            tier: PlatformDepthTier::LegalSafeOrchestrated,
            priority_wave,
            wechat_reference_anchor: false,
            at_wechat_depth: false,
            planned_end_state_reached: false,
            ingest: shared_ingest_capabilities(),
            downstream: shared_downstream_capabilities(),
            known_gaps: vec![
                format!(
                    "{} still sits below the current WeChat reference depth for platform-specific runtime workflow coverage.",
                    name
                ),
                "The shared normalized import path works, but advanced normalization and dedicated incremental planners are still in progress.".to_string(),
                "Downstream analysis is available after successful normalized import, but that does not yet prove full native export depth for this platform.".to_string(),
            ],
            next_focus: "Raise this platform toward the WeChat reference layer before resuming non-essential frontend work.".to_string(),
        },
    }
}

fn shared_ingest_capabilities() -> PlatformIngestCapabilities {
    PlatformIngestCapabilities {
        source_discovery: true,
        authorized_export_parsing: true,
        normalized_database_import: true,
        incremental_import: true,
        media_inventory: true,
        audio_transcode: true,
        watch_dir_monitor: true,
        workspace_orchestration: true,
        native_runtime_detector: false,
        legal_safe_decrypt: false,
        advanced_normalization_planners: false,
    }
}

fn shared_downstream_capabilities() -> PlatformDownstreamCapabilities {
    PlatformDownstreamCapabilities {
        basic_statistics: true,
        advanced_analysis: true,
        ai_tools: true,
        semantic_search: true,
        sql_lab: true,
        mcp_tools: true,
    }
}

fn priority_wave(platform: &Platform) -> PlatformPriorityWave {
    match platform {
        Platform::WeChat => PlatformPriorityWave::ReferenceAnchor,
        Platform::WhatsApp
        | Platform::Line
        | Platform::Qq
        | Platform::Telegram
        | Platform::Discord
        | Platform::Instagram => PlatformPriorityWave::Wave1,
        Platform::IMessage
        | Platform::Messenger
        | Platform::KakaoTalk
        | Platform::Slack
        | Platform::Teams
        | Platform::Signal
        | Platform::Skype
        | Platform::GoogleChat
        | Platform::Zoom
        | Platform::Viber
        | Platform::Custom(_) => PlatformPriorityWave::Wave2,
    }
}

fn platform_name(platform: &Platform) -> &'static str {
    match platform {
        Platform::WeChat => "WeChat",
        Platform::WhatsApp => "WhatsApp",
        Platform::Line => "LINE",
        Platform::Telegram => "Telegram",
        Platform::Qq => "QQ",
        Platform::Discord => "Discord",
        Platform::Instagram => "Instagram",
        Platform::IMessage => "iMessage",
        Platform::Messenger => "Messenger",
        Platform::KakaoTalk => "KakaoTalk",
        Platform::Slack => "Slack",
        Platform::Teams => "Teams",
        Platform::Signal => "Signal",
        Platform::Skype => "Skype",
        Platform::GoogleChat => "Google Chat",
        Platform::Zoom => "Zoom",
        Platform::Viber => "Viber",
        Platform::Custom(_) => "Custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_lists_all_17_runtime_platforms() {
        let report = platform_capability_report();
        assert_eq!(report.summary.total_platforms, 17);
        assert_eq!(report.platforms.len(), 17);
    }

    #[test]
    fn only_wechat_has_runtime_detector_and_decrypt() {
        let report = platform_capability_report();

        let detector_count = report
            .platforms
            .iter()
            .filter(|entry| entry.ingest.native_runtime_detector)
            .count();
        let decrypt_count = report
            .platforms
            .iter()
            .filter(|entry| entry.ingest.legal_safe_decrypt)
            .count();

        assert_eq!(detector_count, 1);
        assert_eq!(decrypt_count, 1);

        let wechat = report
            .platforms
            .iter()
            .find(|entry| entry.platform_id == "wechat")
            .expect("wechat entry must exist");
        assert!(wechat.at_wechat_depth);
        assert!(wechat.ingest.native_runtime_detector);
        assert!(wechat.ingest.legal_safe_decrypt);
    }

    #[test]
    fn shared_downstream_stack_is_marked_available_for_all_platforms() {
        let report = platform_capability_report();
        assert_eq!(report.summary.platforms_with_platform_agnostic_analysis, 17);
        assert!(report
            .platforms
            .iter()
            .all(|entry| entry.downstream.basic_statistics
                && entry.downstream.advanced_analysis
                && entry.downstream.ai_tools
                && entry.downstream.semantic_search
                && entry.downstream.sql_lab
                && entry.downstream.mcp_tools));
    }
}
