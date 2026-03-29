# Xenobot HTTP API (Current Baseline)

This document describes the current HTTP endpoints exposed by `xenobot-api` (Axum).

## Base

- Default local service mode: managed by `xenobot-cli api start`
- Sandbox fallback chain: `TCP -> UDS -> file gateway IPC` (no socket required)
- Route mount note:
  - Chat module endpoints in this document are mounted under `/chat`.
  - Example: `GET /sessions` is exposed as `GET /chat/sessions`.

## Service Basics (`xenobot-api`)

- `GET /` service index
- `GET /health` liveness probe (`OK`)
- `GET /status` machine-readable runtime status:
  - `service`, `version`, `status`
  - `bindAddr`, `apiBasePath`, `corsEnabled`
  - `features` matrix
  - `runtime.os`, `runtime.arch`

## Core Operations

- `GET /core/platform-capabilities`

### `GET /core/platform-capabilities`

Returns Xenobot's current machine-readable 17-platform capability matrix.

Use this endpoint when you need to answer questions like:

- how many platforms currently match the WeChat reference depth
- which platforms currently have a platform-specific runtime detector layer
- which platforms currently have a legal-safe decrypt path
- whether downstream analysis availability reflects full native workflow parity or only successful normalized import

Response highlights:

- `scope.legalSafeOnly`
- `scope.excludedImplementationStyles`
- `scope.notes`
- `summary.totalPlatforms`
- `summary.platformsAtWechatDepth`
- `summary.platformsBelowWechatDepth`
- `summary.platformsWithRuntimeDetector`
- `summary.platformsWithLegalSafeDecrypt`
- `summary.allPlatformsAtPlannedEndState`
- `platforms[]`
  - `platformId`
  - `name`
  - `tier`
  - `priorityWave`
  - `atWechatDepth`
  - `plannedEndStateReached`
  - `ingest`
  - `downstream`
  - `knownGaps`
  - `nextFocus`

## Network Operations

- `GET /network/proxy-config`
- `POST /network/proxy-config`
- `POST /network/test-proxy-connection`
- `GET /network/sandbox-doctor`

### `GET /network/sandbox-doctor`

Returns the same capability recommendation model used by the CLI `api sandbox-doctor` flow:

- `tcp.allowed`
- `uds.supported`
- `uds.allowed`
- `fileGateway.dir`
- `fileGateway.writable`
- `recommended.mode`
- `recommended.command`

Optional query params:

- `fileGatewayDir`
- `file_gateway_dir`

Example:

```bash
curl -s "http://127.0.0.1:8080/api/network/sandbox-doctor" | jq
```

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

`POST /import` success payload now includes:
- `sessionId`
- `detectedPlatform`
- `payloadPlatform`
- `sessionName`
- `diagnostics`
- `webhookSummary`

## Media

- `GET /media/resolve` (authorized absolute path validation + metadata)
- `GET /media/file` (authorized file streaming)
- `GET /media/messages/:message_id` (resolve local media path from message content and stream)
- `POST /media/decrypt/dat` (in-memory `.dat` image decrypt; requires `xenobot-api --features wechat`)
- `POST /media/transcode/audio/mp3` (in-memory audio->MP3 transcode; requires `xenobot-api --features wechat` and local `ffmpeg`)

`POST /media/decrypt/dat` request (camelCase):

```json
{
  "path": "/absolute/path/to/image.dat",
  "xorKeyHex": "10",
  "autoDetectXor": true
}
```

or inline payload:

```json
{
  "payloadBase64": "BASE64_BYTES",
  "aesKeyHex": "001122...",
  "aesIvHex": "001122..."
}
```

`POST /media/transcode/audio/mp3` request (camelCase):

```json
{
  "path": "/absolute/path/to/voice.silk",
  "inputFormat": "silk",
  "bitrateKbps": 128,
  "sampleRateHz": 24000,
  "channels": 1,
  "ffmpegPath": "/opt/homebrew/bin/ffmpeg"
}
```

Notes:
- `ffmpegPath` is optional; when omitted, runtime uses `XENOBOT_FFMPEG_PATH` (if set), then falls back to `ffmpeg` from `PATH`.
- `ffmpegPath` may also be provided as `ffmpegBinary` for compatibility.

Both endpoints return in-memory payload bytes as Base64 (`bytes`) and never require a temporary output file path.

## Memory

- `GET /memory/sessions/:session_id/entries`
- `POST /memory/sessions/:session_id/sync-session-summaries`

### `GET /memory/sessions/:session_id/entries`

Returns explicit persisted memory entries for a normalized chat session space.

Query params:

- `kind`
- `limit`
- `offset`

Response highlights:

- `items[]`
  - `id`
  - `metaId`
  - `chatSessionId`
  - `memoryKind`
  - `title`
  - `content`
  - `tags`
  - `sourceLabel`
  - `importance`
  - `createdAt`
  - `updatedAt`
- `count`
- `limit`
- `offset`

Current first-party memory source:

- generated `session_summary` entries are automatically written into `memory_entry`
- existing `chat_session.summary` rows can be backfilled with the sync endpoint

### `POST /memory/sessions/:session_id/sync-session-summaries`

Backfills existing non-empty `chat_session.summary` rows into the explicit memory store.

Response highlights:

- `scanned`
- `upserted`
- `skipped`

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
  - returns `404` when `session_id` does not exist.
- `POST /sessions/:session_id/generate-sql` (AI-assisted safe SQL draft, rule-based local fallback)
  - returns `404` when `session_id` does not exist.
  - generated SQL is session-scoped by default (`msg.meta_id = :session_id`) for message-table intents.
  - request:
    - `prompt` (required)
    - `maxRows` (optional, clamped `1..500`, default `100`)
  - response:
    - `success`
    - `sql`
    - `explanation`
    - `strategy` (`rule_based_safe_sql`)
    - `limit`
    - `warnings` (array)
  - built-in rule intents include:
    - recent/keyword message lookup
    - sender count ranking
    - hourly / weekday / daily / monthly / yearly time buckets
  - generated SQL is re-validated by the same read-only guard used by `execute-sql`.
- `GET /sessions/:session_id/schema`
  - returns `404` when `session_id` does not exist.
  - default response: compatibility array of `{ name, columns }`
  - supports query params:
    - `detailed=true`: returns `{ tables, summary, includesRowCount }`
    - `includeRowCount=true`: valid when `detailed=true`, includes per-table `rowCount`
  - detailed table item fields:
    - `name`
    - `columns`
    - `indexes` (`name`, `unique`, `origin`, `partial`, `columns`)
    - `foreignKeys` (`table`, `from`, `to`, `onUpdate`, `onDelete`, `match`, ...)
    - `rowCount` (`null` when not requested)
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

## LLM Configuration and Chat

- `GET /llm/providers`
- `GET /llm/configs`
- `GET /llm/active-config-id`
- `POST /llm/configs`
- `POST /llm/configs/:id`
- `DELETE /llm/configs/:id`
- `POST /llm/active-config`
- `POST /llm/validate-api-key`
- `GET /llm/has-config`
- `POST /llm/chat`
- `POST /llm/chat-stream`

Validation rules:
- `provider` must exist in the provider catalog (case-insensitive input, normalized to canonical provider id).
- for providers other than `openai-compatible`, `model` (when provided) must be one of that provider's declared models.
- for `openai-compatible`, custom model ids are allowed.
- `baseUrl` (when provided) must be a valid URL.
- `validate-api-key` applies the same provider/model/baseUrl validation before API key checks.
- `POST /llm/chat` and `POST /llm/chat-stream` use provider runtime calls for OpenAI-style providers and Gemini.
- if upstream call fails, runtime returns a local-safe fallback response instead of propagating transport errors.
- optional timeout env: `XENOBOT_LLM_TIMEOUT_MS` (ms, clamped to safe bounds).

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
- `GET /resources`
- `GET /resources/*uri`
- `GET /integrations`
- `GET /integrations/:target`

`GET /tools` response includes:
- `tools` (legacy-compatible list of tool names)
- `toolSpecs` (structured tool metadata with `name`, `description`, `inputSchema`)

### MCP Tool List (first + second batch)

- `current_time`
- `get_current_time` (alias)
- `query_contacts`
- `query_groups`
- `recent_sessions`
- `query_chats` (alias)
- `chat_records`
- `chat_history` (alias)
- `member_stats`
- `get_member_stats` (alias)
- `time_stats`
- `get_time_stats` (alias)
- `session_summary`
- `get_session_summary` (alias)

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
- tool argument failure: HTTP `400`, `code = "invalid_params"`
- tool runtime failure: HTTP `500`, `code = "tool_error"`

### `POST /mcp` (Streamable HTTP JSON-RPC)

Supported methods:
- `initialize`
- `tools/list` (alias: `tool/list`)
- `tools/call` (alias: `tool/call`)
- `resources/list` (alias: `resource/list`)
- `resources/read` (alias: `resource/read`)

`tools/list` response entries include:
- `name`
- `description`
- `inputSchema`

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
  - `-32003` `resource_not_found`

Parameter compatibility notes:
- `tools/call` accepts both flat and nested styles:
  - flat: `params.name` + `params.arguments` (or `params.args`)
  - nested: `params.tool.name` + `params.tool.arguments` (or `params.tool.args` / `params.tool.input`)
- `resources/read` accepts both flat and nested styles:
  - flat: `params.uri` (also `params.resource`, `params.path`, `params.resource_uri`, `params.resourceUri`)
  - nested: `params.resource.uri` (also `params.resource.path`, `params.resource.resource_uri`, `params.resource.resourceUri`)

Built-in resource URIs (current baseline):
- `xenobot://server/info`
- `xenobot://server/capabilities`
- `xenobot://server/integrations`

CLI smoke command:

```bash
cargo run -p xenobot-mcp -- --host 127.0.0.1 --port 8081 --db-path /tmp/xenobot.db
cargo run -p xenobot-cli --features "api,analysis" -- api mcp-smoke --url http://127.0.0.1:8081
```

Checks:
- `GET /health`
- `GET /tools`
- `GET /resources`
- `GET /resources/*uri`
- `POST /mcp` `initialize`
- `POST /mcp` `tools/list`
- `POST /mcp` `resources/list`
- `POST /mcp` `resources/read`
- `POST /mcp` `tool/list` (alias contract)
- `POST /mcp` `resource/list` (alias contract)
- `POST /mcp` `tools/call` (smoke call on `get_current_time`)
- `GET /integrations`
- required first-batch MCP tool aliases and integration preset IDs
  - current built-in targets:
    - `claude-desktop`
    - `chatwise`
    - `opencode`
    - `pencil`

CLI preset fetch:

```bash
cargo run -p xenobot-cli --features "api,analysis" -- api mcp-preset --url http://127.0.0.1:8081 --target claude-desktop --format json
cargo run -p xenobot-cli --features "api,analysis" -- api mcp-preset --url http://127.0.0.1:8081 --target pencil --format json
```

CLI direct tool call (RPC mode):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-call \
  --url http://127.0.0.1:8081 \
  --mode rpc \
  --tool get_current_time \
  --args-json '{}' \
  --format json
```

CLI direct tool call (HTTP mode):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-call \
  --url http://127.0.0.1:8081 \
  --mode http \
  --tool chat_records \
  --args-json '{"session_id":1,"limit":20,"offset":0}' \
  --format text
```

CLI list resources (RPC mode):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-resources \
  --url http://127.0.0.1:8081 \
  --mode rpc \
  --format json
```

CLI read resource (HTTP mode):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-resource \
  --url http://127.0.0.1:8081 \
  --mode http \
  --uri xenobot://server/info \
  --format text
```

CLI sandbox-coexist startup (force file-gateway mode):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api start \
  --force-file-gateway \
  --file-gateway-dir /tmp/xenobot-file-gateway \
  --db-path /tmp/xenobot.db
```

CLI sandbox runtime diagnostic (listener/file-gateway probe + recommended command):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api sandbox-doctor \
  --format json
```

Latest diagnostic evidence in current execution environment (2026-03-05 UTC):
- `tcp.allowed=false` (`Operation not permitted`)
- `uds.allowed=false` (`Operation not permitted`)
- `fileGateway.writable=true`
- recommendation points to `api start --force-file-gateway ...`

CLI file-gateway single call (no socket required):

```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api gateway-call \
  --file-gateway-dir /tmp/xenobot-file-gateway \
  --method GET \
  --path /health \
  --format json
```

One-key wrappers (project root auto-detection):

```bash
scripts/xb api sandbox-doctor
scripts/xb api sandbox-up --db-path /tmp/xenobot.db
scripts/xb api sandbox-health --format json
scripts/xb mcp start --db-path /tmp/xenobot.db
scripts/xb mcp smoke --url http://127.0.0.1:8081
scripts/xb mcp preset --url http://127.0.0.1:8081 --target claude-desktop --format json
scripts/xb mcp preset --url http://127.0.0.1:8081 --target pencil --format json
```

## Compatibility Notes

- Apple Silicon: supported (`aarch64-apple-darwin`)
- GPU path: Metal/MPS modules are available in workspace; endpoint-level GPU usage depends on analysis pipeline integration state
- Legal-safe scope: APIs are designed for authorized exports and local user-owned data
