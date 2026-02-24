# Xenobot HTTP API (Current Baseline)

This document describes the current HTTP endpoints exposed by `xenobot-api` (Axum).

## Base

- Default local service mode: managed by `xenobot-cli api start`
- Sandbox fallback chain: `TCP -> UDS -> file gateway IPC` (no socket required)

## Migration

- `GET /check-migration`
- `POST /run-migration`

## Import and File Operations

- `GET /select-file`
- `POST /import`
- `POST /import-batch`
- `POST /detect-format`
- `POST /import-with-options`
- `POST /scan-multi-chat-file`

## AI Search

- `POST /ai/search-messages` (keyword search)
- `POST /ai/semantic-search-messages` (query rewrite + chunked embedding + cosine similarity)

### `POST /import-batch` modes

`separate` mode (default):
- Imports files independently
- Supports retry and per-file checkpoints

`merged` mode (`"merge": true`):
- Merges all sources into one session
- Cross-file deduplication
- Checkpoint fast-skip for unchanged files
- Failed parse/no-message files are written back as failed checkpoints
- If all files are unchanged, returns checkpoint-only result and does not create a new session

Example:
```json
{
  "filePaths": ["/path/a.json", "/path/b.json"],
  "merge": true,
  "mergedSessionName": "Merged Authorized Batch"
}
```

## Session Management

- `GET /sessions`
- `GET /sessions/:session_id`
- `DELETE /sessions/:session_id`
- `POST /sessions/:session_id/rename`
- `GET /sessions/:session_id/time-range`

## Incremental Import

- `POST /sessions/:session_id/analyze-incremental-import`
- `POST /sessions/:session_id/incremental-import`

## Member Management

- `GET /sessions/:session_id/members`
- `GET /sessions/:session_id/members-paginated`
- `POST /sessions/:session_id/members/:member_id/aliases`
- `DELETE /sessions/:session_id/members/:member_id`
- `POST /sessions/:session_id/owner`

## Analytics

- `GET /sessions/:session_id/available-years`
- `GET /sessions/:session_id/member-activity`
- `GET /sessions/:session_id/member-name-history/:member_id`
- `GET /sessions/:session_id/hourly-activity`
- `GET /sessions/:session_id/daily-activity`
- `GET /sessions/:session_id/weekday-activity`
- `GET /sessions/:session_id/monthly-activity`
- `GET /sessions/:session_id/yearly-activity`
- `GET /sessions/:session_id/message-length-distribution`
- `GET /sessions/:session_id/message-type-distribution`
- `GET /sessions/:session_id/catchphrase-analysis`
- `GET /sessions/:session_id/mention-analysis`
- `GET /sessions/:session_id/mention-graph`
- `GET /sessions/:session_id/cluster-graph`
- `GET /sessions/:session_id/laugh-analysis`

## SQL and Plugin

- `POST /sessions/:session_id/execute-sql` (read-safe path should remain SELECT-only in callers)
- `GET /sessions/:session_id/schema`
- `POST /sessions/:session_id/plugin-query`
- `POST /plugin-compute`

## Export and Cleanup

- `POST /export-sessions-to-temp-files`
- `POST /cleanup-temp-export-files`

## Event Stream

- `GET /import-progress` (SSE)

## Utility

- `GET /db-directory`
- `GET /supported-formats`

## Compatibility Notes

- Apple Silicon: supported (`aarch64-apple-darwin`)
- GPU path: Metal/MPS modules are available in workspace; endpoint-level GPU usage depends on analysis pipeline integration state
- Legal-safe scope: APIs are designed for authorized exports and local user-owned data
