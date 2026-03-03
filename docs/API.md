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

### `POST /ai/semantic-search-messages`

Request body (camelCase):

```json
{
  "sessionId": "123",
  "query": "semantic question text",
  "filter": {
    "startTs": 1700000000,
    "endTs": 1701000000
  },
  "senderId": 42,
  "threshold": 0.45,
  "limit": 50,
  "offset": 100
}
```

Response highlights:
- `messages`: ranked rows (each row includes `similarity`)
- `count`: number of rows in current page
- `totalCount`: full matched size before page slicing
- `queryRewritten`: normalized query text after rewrite pass
- `limit`, `offset`, `threshold`, `prefilterCount`

### `POST /import-batch` modes

`separate` mode (default):
- Imports files independently
- Supports retry and per-file checkpoints
- Duplicate file paths in the same request are skipped as `duplicateInputSkipped` (counted in `skippedFiles`)

`merged` mode (`"merge": true`):
- Merges all sources into one session
- Cross-file deduplication
- Duplicate file paths in the same request are skipped as `duplicateInputSkipped` before parse/write
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

Incremental responses include checkpoint diagnostics:
- `sessionId`
- `sourceFingerprint`
- `checkpointMeta` (`fingerprint`, `fileSize`, `modifiedAt`)
- `lastCheckpoint` (present on checkpoint-skip path)

`incremental-import` request supports optimistic consistency guard:
- `expectedFingerprint` (camelCase) or `expected_fingerprint` (snake_case)
- If provided and file fingerprint changed since analyze, API returns:
  - `success: false`
  - `error: "error.source_changed_since_analyze"`
  - `expectedFingerprint`
  - `sourceFingerprint`

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
- `GET /sessions/:session_id/night-owl-analysis`
- `GET /sessions/:session_id/dragon-king-analysis`
- `GET /sessions/:session_id/lurker-analysis`
- `GET /sessions/:session_id/checkin-analysis`
- `GET /sessions/:session_id/repeat-analysis`

### Cluster Graph Notes

- `cluster-graph` now returns per-node `communityId`.
- `stats.algorithm` is currently `weighted_label_propagation` and `stats.iterations` reports convergence rounds.
- Community items include `members`, `internalEdgeWeight`, `externalEdgeWeight`, and `density`.

## SQL and Plugin

- `POST /sessions/:session_id/execute-sql` (read-only SQL guard: SELECT/CTE only, no multi-statement)
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

## Agent

- `GET /agent/tools` (returns 12 legal-safe local query tools)
- `POST /agent/run-stream` (tool-calling stream, up to 5 rounds, SSE chunks)
- `POST /agent/abort/:request_id`

### Agent Run Stream Notes

- Request supports:
  - `requestId`
  - `userMessage`
  - `context.sessionId`
  - optional `forcedTools`, `maxRounds` (capped at `5`)
- Stream chunk `type` values: `think`, `tool_start`, `tool_result`, `content`, `done`, `error`.
- Tool execution is local-readonly on authorized imported data; no process-memory extraction or bypass behavior.

### `GET /agent/tools` contract

- Returns exactly 12 tool definitions.
- Each tool definition includes:
  - `name`
  - `description`
  - `parameters.required` (array)
  - `parameters.optional` (array)
  - `parameters.inferredFromPrompt` (array)
  - `parameters.notes` (string)
- Contract goal: frontend and shim can consume a stable schema without relying on implicit parameter inference.

## MCP Server (HTTP/SSE/WS)

`xenobot-mcp` exposes an MCP-compatible transport service (separate process from `xenobot-api`).

Transport endpoints:
- `GET /health`
- `GET /sse`
- `GET /ws`
- `POST /mcp` (Streamable HTTP JSON-RPC)
- `GET /tools`
- `POST /tools/:tool_name`
- `GET /integrations`
- `GET /integrations/:target`

### MCP Tool List (first batch)

- `current_time`
- `get_current_time` (alias)
- `query_contacts`
- `query_groups`
- `recent_sessions`
- `query_chats` (alias)
- `chat_records`
- `chat_history` (alias)

Legacy-compatible aliases remain available:
- `list_contacts`
- `list_sessions`
- `recent_messages`
- `search_messages`

### `POST /tools/chat_records`

Supports both camelCase and snake_case request keys:

```json
{
  "sessionId": 123,
  "keyword": "release",
  "startTs": 1700000000,
  "endTs": 1701000000,
  "limit": 50,
  "offset": 0
}
```

Response shape:
- success wrapper: `{ "success": true, "tool": "chat_records", "result": { ... } }`
- result fields: `count`, `totalCount`, `hasMore`, `messages`, `sessionId`, `limit`, `offset`

Error semantics:
- tool not found: HTTP `404`, `code = "tool_not_found"`
- tool runtime/argument failure: HTTP `500`, `code = "tool_error"`

### `POST /mcp` (Streamable HTTP JSON-RPC)

Supported methods:
- `initialize`
- `tools/list` (alias: `tool/list`)
- `tools/call` (alias: `tool/call`)

Example call:

```json
{
  "jsonrpc": "2.0",
  "id": "call-1",
  "method": "tools/call",
  "params": {
    "name": "chat_records",
    "arguments": {
      "sessionId": 123,
      "keyword": "release",
      "limit": 20,
      "offset": 0
    }
  }
}
```

Response semantics:
- success: `{"jsonrpc":"2.0","id":"...","result":{"tool":"...","isError":false,"content":[...],"structuredContent":{...}}}`
- JSON-RPC error object (HTTP `200`) for method/param/tool failures:
  - `-32601` `method_not_found`
  - `-32602` `invalid_params`
  - `-32001` `tool_not_found`
  - `-32002` `tool_error`

CLI smoke command:

```bash
cargo run -p xenobot-cli --features "api,analysis" -- api mcp-smoke --url http://127.0.0.1:5030
```

Checks:
- `GET /health`
- `GET /tools`
- `POST /mcp` `initialize`
- `POST /mcp` `tools/list`
- `POST /mcp` `tools/call` (smoke call on `get_current_time`)
- `GET /integrations`
- required first-batch MCP tool aliases and integration preset IDs

CLI preset fetch:

```bash
cargo run -p xenobot-cli --features "api,analysis" -- api mcp-preset --url http://127.0.0.1:5030 --target claude-desktop --format json
```

## Compatibility Notes

- Apple Silicon: supported (`aarch64-apple-darwin`)
- GPU path: Metal/MPS modules are available in workspace; endpoint-level GPU usage depends on analysis pipeline integration state
- Legal-safe scope: APIs are designed for authorized exports and local user-owned data
