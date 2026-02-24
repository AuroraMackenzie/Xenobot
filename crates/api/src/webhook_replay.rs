//! Background worker that automatically replays webhook dead-letter entries.

use crate::config::{ApiConfig, WebhookReplayConfig};
use futures::stream::{self, StreamExt};
use std::collections::HashSet;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use xenobot_core::webhook::{
    now_unix_ts, overwrite_dead_letter_entries, read_dead_letter_entries, WebhookDeadLetterEntry,
};

/// Spawn a background task that replays webhook dead-letter entries on an interval.
///
/// Returns `None` when replay is disabled by configuration.
pub fn spawn_webhook_dead_letter_replayer(config: &ApiConfig) -> Option<JoinHandle<()>> {
    if !config.webhook_replay.enabled {
        info!("webhook dead-letter replay worker is disabled");
        return None;
    }

    let replay_config = config.webhook_replay.clone();
    Some(tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(
                replay_config.request_timeout_seconds.max(1),
            ))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                warn!(
                    "webhook dead-letter replay worker stopped: failed to build HTTP client: {}",
                    err
                );
                return;
            }
        };

        info!(
            "webhook dead-letter replay worker started (interval={}s, batch={}, concurrency={}, max_attempts={})",
            replay_config.interval_seconds.max(1),
            replay_config.max_entries_per_tick.max(1),
            replay_config.max_concurrency.max(1),
            replay_config.max_attempts
        );

        let mut interval =
            tokio::time::interval(Duration::from_secs(replay_config.interval_seconds.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            if let Err(err) = replay_dead_letters_once(&client, &replay_config).await {
                warn!("webhook dead-letter replay tick failed: {}", err);
            }
        }
    }))
}

async fn replay_dead_letters_once(
    client: &reqwest::Client,
    config: &WebhookReplayConfig,
) -> Result<(), String> {
    let snapshot = load_dead_letter_entries().await?;
    if snapshot.is_empty() {
        return Ok(());
    }

    let limit = config.max_entries_per_tick.max(1);
    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    for entry in snapshot.iter().cloned() {
        if entry.attempts >= config.max_attempts {
            deferred.push(entry);
            continue;
        }
        if selected.len() < limit {
            selected.push(entry);
        } else {
            deferred.push(entry);
        }
    }

    if selected.is_empty() {
        debug!(
            "webhook dead-letter replay skipped: {} entries available, all above max_attempts={}",
            snapshot.len(),
            config.max_attempts
        );
        return Ok(());
    }

    let max_concurrency = config.max_concurrency.max(1);
    let delivery_results = stream::iter(selected.into_iter().map(|entry| async move {
        let result = deliver_dead_letter_entry(client, &entry).await;
        (entry, result)
    }))
    .buffer_unordered(max_concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut replayed_ok = 0usize;
    let mut replay_failed = 0usize;
    let mut updated_failed_entries = Vec::new();
    for (mut entry, result) in delivery_results {
        match result {
            Ok(()) => {
                replayed_ok = replayed_ok.saturating_add(1);
            }
            Err(err) => {
                replay_failed = replay_failed.saturating_add(1);
                entry.attempts = entry.attempts.saturating_add(1);
                entry.last_failed_at = now_unix_ts();
                entry.last_error = err;
                updated_failed_entries.push(entry);
            }
        }
    }

    let snapshot_ids: HashSet<String> = snapshot.iter().map(|v| v.id.clone()).collect();
    let current = load_dead_letter_entries().await?;
    let appended_since_snapshot = current
        .into_iter()
        .filter(|entry| !snapshot_ids.contains(&entry.id))
        .collect::<Vec<_>>();

    let mut next_entries = updated_failed_entries;
    next_entries.extend(deferred);
    next_entries.extend(appended_since_snapshot);
    persist_dead_letter_entries(next_entries).await?;

    info!(
        "webhook dead-letter replay tick completed (snapshot={}, replay_ok={}, replay_failed={}, deferred={})",
        snapshot.len(),
        replayed_ok,
        replay_failed,
        snapshot
            .len()
            .saturating_sub(replayed_ok)
            .saturating_sub(replay_failed)
    );

    Ok(())
}

async fn deliver_dead_letter_entry(
    client: &reqwest::Client,
    entry: &WebhookDeadLetterEntry,
) -> Result<(), String> {
    let response = client
        .post(&entry.webhook_url)
        .header("X-Xenobot-Event", &entry.event.event_type)
        .header("X-Xenobot-Webhook-Id", &entry.webhook_id)
        .json(&entry.event)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        return Ok(());
    }

    Err(format!("http status {}", response.status()))
}

async fn load_dead_letter_entries() -> Result<Vec<WebhookDeadLetterEntry>, String> {
    tokio::task::spawn_blocking(read_dead_letter_entries)
        .await
        .map_err(|e| format!("join dead-letter read task failed: {}", e))?
        .map_err(|e| format!("read dead-letter entries failed: {}", e))
}

async fn persist_dead_letter_entries(entries: Vec<WebhookDeadLetterEntry>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || overwrite_dead_letter_entries(&entries))
        .await
        .map_err(|e| format!("join dead-letter write task failed: {}", e))?
        .map_err(|e| format!("write dead-letter entries failed: {}", e))
}

#[cfg(test)]
mod tests {
    use xenobot_core::webhook::{WebhookDeadLetterEntry, WebhookMessageCreatedEvent};

    fn build_entry(id: &str, attempts: u32) -> WebhookDeadLetterEntry {
        WebhookDeadLetterEntry {
            id: id.to_string(),
            webhook_id: "wh_1".to_string(),
            webhook_url: "http://127.0.0.1:65535/hook".to_string(),
            event: WebhookMessageCreatedEvent {
                event_type: "message.created".to_string(),
                platform: "test".to_string(),
                chat_name: "chat".to_string(),
                meta_id: 1,
                message_id: 1,
                sender_id: 1,
                sender_name: Some("tester".to_string()),
                ts: 0,
                msg_type: 0,
                content: Some("hello".to_string()),
            },
            attempts,
            first_failed_at: 0,
            last_failed_at: 0,
            last_error: "timeout".to_string(),
        }
    }

    #[test]
    fn select_entries_respects_limit_and_attempt_threshold() {
        let snapshot = vec![
            build_entry("a", 0),
            build_entry("b", 1),
            build_entry("c", 20),
            build_entry("d", 0),
        ];

        let mut selected = Vec::new();
        let mut deferred = Vec::new();
        for entry in snapshot {
            if entry.attempts >= 20 {
                deferred.push(entry);
                continue;
            }
            if selected.len() < 2 {
                selected.push(entry);
            } else {
                deferred.push(entry);
            }
        }

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].id, "a");
        assert_eq!(selected[1].id, "b");
        assert_eq!(deferred.len(), 2);
        assert_eq!(deferred[0].id, "c");
        assert_eq!(deferred[1].id, "d");
    }
}
