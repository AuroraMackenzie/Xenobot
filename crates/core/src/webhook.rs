//! Shared webhook rule matching types and helpers.
//!
//! This module keeps webhook event matching behavior consistent across
//! CLI and API import pipelines.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Persisted webhook rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRule {
    pub id: String,
    pub url: String,
    pub event_type: Option<String>,
    pub sender: Option<String>,
    pub keyword: Option<String>,
    pub created_at: Option<String>,
}

/// Message-created webhook event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookMessageCreatedEvent {
    pub event_type: String,
    pub platform: String,
    pub chat_name: String,
    pub meta_id: i64,
    pub message_id: i64,
    pub sender_id: i64,
    pub sender_name: Option<String>,
    pub ts: i64,
    pub msg_type: i64,
    pub content: Option<String>,
}

/// Aggregated webhook dispatch counters.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDispatchStats {
    pub attempted: usize,
    pub delivered: usize,
    pub failed: usize,
    pub filtered: usize,
}

/// Dead-letter entry for failed webhook deliveries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDeadLetterEntry {
    pub id: String,
    pub webhook_id: String,
    pub webhook_url: String,
    pub event: WebhookMessageCreatedEvent,
    pub attempts: u32,
    pub first_failed_at: i64,
    pub last_failed_at: i64,
    pub last_error: String,
}

/// Current UNIX timestamp in seconds.
pub fn now_unix_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Absolute path of the webhook dead-letter JSONL file.
pub fn webhook_dead_letter_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot")
        .join("webhook_dead_letters.jsonl")
}

/// Build a dead-letter entry from a failed delivery context.
pub fn build_dead_letter_entry(
    rule: &WebhookRule,
    event: &WebhookMessageCreatedEvent,
    attempts: u32,
    last_error: String,
) -> WebhookDeadLetterEntry {
    let now = now_unix_ts();
    WebhookDeadLetterEntry {
        id: format!("dlq_{}_{}_{}", rule.id, event.message_id, now),
        webhook_id: rule.id.clone(),
        webhook_url: rule.url.clone(),
        event: event.clone(),
        attempts,
        first_failed_at: now,
        last_failed_at: now,
        last_error,
    }
}

/// Append one dead-letter entry to persistent JSONL storage.
pub fn append_dead_letter_entry(entry: &WebhookDeadLetterEntry) -> std::io::Result<()> {
    let path = webhook_dead_letter_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::other(format!("serialize dead letter failed: {}", e)))?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Load all dead-letter entries from persistent JSONL storage.
pub fn read_dead_letter_entries() -> std::io::Result<Vec<WebhookDeadLetterEntry>> {
    let path = webhook_dead_letter_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = OpenOptions::new().read(true).open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<WebhookDeadLetterEntry>(trimmed) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Rewrite dead-letter JSONL storage with provided entries.
pub fn overwrite_dead_letter_entries(entries: &[WebhookDeadLetterEntry]) -> std::io::Result<()> {
    let path = webhook_dead_letter_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::other(format!("serialize dead letter failed: {}", e)))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

/// Merge dispatch stats into a target accumulator.
pub fn merge_webhook_dispatch_stats(
    target: &mut WebhookDispatchStats,
    delta: &WebhookDispatchStats,
) {
    target.attempted = target.attempted.saturating_add(delta.attempted);
    target.delivered = target.delivered.saturating_add(delta.delivered);
    target.failed = target.failed.saturating_add(delta.failed);
    target.filtered = target.filtered.saturating_add(delta.filtered);
}

/// Returns true when a rule should receive the message-created event.
pub fn webhook_rule_matches_event(rule: &WebhookRule, event: &WebhookMessageCreatedEvent) -> bool {
    let event_rule = rule
        .event_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase());
    if let Some(v) = event_rule {
        if v != event.event_type.to_ascii_lowercase() {
            return false;
        }
    }

    let sender_rule = rule
        .sender
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase());
    if let Some(v) = sender_rule {
        let sender_name = event
            .sender_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let sender_id = event.sender_id.to_string();
        if v != sender_name && v != sender_id {
            return false;
        }
    }

    let keyword_rule = rule
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase());
    if let Some(v) = keyword_rule {
        let content = event
            .content
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !content.contains(&v) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_rule_match_by_event_sender_and_keyword() {
        let rule = WebhookRule {
            id: "wh_1".to_string(),
            url: "http://127.0.0.1:65535/hook".to_string(),
            event_type: Some("message.created".to_string()),
            sender: Some("alice".to_string()),
            keyword: Some("urgent".to_string()),
            created_at: Some("2026-02-23T00:00:00Z".to_string()),
        };
        let ok = WebhookMessageCreatedEvent {
            event_type: "message.created".to_string(),
            platform: "whatsapp".to_string(),
            chat_name: "Team Chat".to_string(),
            meta_id: 1,
            message_id: 2,
            sender_id: 10,
            sender_name: Some("Alice".to_string()),
            ts: 1_771_800_000,
            msg_type: 0,
            content: Some("urgent check".to_string()),
        };
        let bad = WebhookMessageCreatedEvent {
            content: Some("normal".to_string()),
            ..ok.clone()
        };
        assert!(webhook_rule_matches_event(&rule, &ok));
        assert!(!webhook_rule_matches_event(&rule, &bad));
    }

    #[test]
    fn build_dead_letter_entry_contains_failure_context() {
        let rule = WebhookRule {
            id: "wh_2".to_string(),
            url: "http://127.0.0.1:65535/hook".to_string(),
            event_type: Some("message.created".to_string()),
            sender: None,
            keyword: None,
            created_at: None,
        };
        let event = WebhookMessageCreatedEvent {
            event_type: "message.created".to_string(),
            platform: "discord".to_string(),
            chat_name: "Ops".to_string(),
            meta_id: 9,
            message_id: 42,
            sender_id: 7,
            sender_name: Some("Alice".to_string()),
            ts: 1_771_800_100,
            msg_type: 0,
            content: Some("hello".to_string()),
        };
        let entry = build_dead_letter_entry(&rule, &event, 3, "timeout".to_string());
        assert!(entry.id.starts_with("dlq_wh_2_42_"));
        assert_eq!(entry.webhook_id, "wh_2");
        assert_eq!(entry.webhook_url, "http://127.0.0.1:65535/hook");
        assert_eq!(entry.attempts, 3);
        assert_eq!(entry.last_error, "timeout");
        assert_eq!(entry.event.message_id, 42);
    }

    #[test]
    fn merge_webhook_dispatch_stats_accumulates_counters() {
        let mut total = WebhookDispatchStats {
            attempted: 1,
            delivered: 1,
            failed: 0,
            filtered: 2,
        };
        let delta = WebhookDispatchStats {
            attempted: 3,
            delivered: 1,
            failed: 2,
            filtered: 4,
        };
        merge_webhook_dispatch_stats(&mut total, &delta);
        assert_eq!(total.attempted, 4);
        assert_eq!(total.delivered, 2);
        assert_eq!(total.failed, 2);
        assert_eq!(total.filtered, 6);
    }
}
