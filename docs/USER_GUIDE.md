# Xenobot User Guide (English)

This guide provides a practical, legal-safe workflow for daily Xenobot usage.

For one-command operations and incident flow, use:
- `docs/OPERATIONS_RUNBOOK.md`

## 1) Preconditions

- macOS (Apple Silicon recommended for Metal/MPS acceleration path)
- Rust toolchain installed
- Authorized chat export files available in local user-accessible directories
- No process-memory extraction or decryption bypass is required or supported

## 2) Build and Test

```bash
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
```

## 3) API Runtime Modes

### Standard API mode (TCP)
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api start --db-path /tmp/xenobot.db
```

### Sandbox coexist mode (forced file gateway IPC)
Use this when TCP/UDS listeners are restricted by runtime policy.

```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api start --force-file-gateway --file-gateway-dir /tmp/xenobot-file-gateway --db-path /tmp/xenobot.db
```

### Environment diagnosis
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api sandbox-doctor --format json
```

### Runtime status introspection
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api status --format json
```

### In-process API smoke validation (no listener bind required)
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api smoke --db-path /tmp/xenobot-smoke.db
```

Smoke checks include:
- `GET /health`
- `POST /chat/import` for `wechat`, `qq`, and `discord`
- strict verification of both `detectedPlatform` and `payloadPlatform`
- session generation + member activity
- summary persistence + memory entry creation
- keyword search + semantic search
- `POST /chat/sessions/:session_id/generate-sql` (must include session-scoped `msg.meta_id` filter)
- `POST /chat/sessions/:session_id/execute-sql`

## 4) Import Workflow

### Import data with DB write and incremental consistency
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  import /path/to/authorized-export we-chat --db-path /tmp/xenobot.db --write-db --incremental
```

### Check supported platform coverage
```bash
cd Xenobot
scripts/check_platform_coverage.sh
```

## 5) Query and Analysis

### Query messages
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  query --db-path /tmp/xenobot.db search "keyword" -l 20 -f table
```

### Run analytics
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- analyze --db-path /tmp/xenobot.db stats
```

### LLM chat runtime behavior
- Configure providers with `/llm/configs` (or corresponding frontend settings).
- `provider` / `model` / `baseUrl` are validated before config save and key validation.
- For OpenAI-compatible providers and Gemini, `/llm/chat` and `/llm/chat-stream` try upstream calls first.
- If upstream fails or is unavailable, Xenobot returns a local-safe fallback response instead of failing hard.
- Optional timeout tuning:
```bash
export XENOBOT_LLM_TIMEOUT_MS=15000
```

### Media transcode/decrypt API usage (authorized local files only)
```bash
cd Xenobot
# optional global ffmpeg override
export XENOBOT_FFMPEG_PATH=/opt/homebrew/bin/ffmpeg

curl -s http://127.0.0.1:5030/media/transcode/audio/mp3 \
  -H "content-type: application/json" \
  -d '{
    "path": "/absolute/path/to/voice.silk",
    "inputFormat": "silk",
    "ffmpegPath": "/opt/homebrew/bin/ffmpeg"
  }'
```

Notes:
- `ffmpegPath` request field takes priority when provided.
- If omitted, runtime checks `XENOBOT_FFMPEG_PATH`, then `ffmpeg` from `PATH`.

## 6) Webhook Operations

### Add/list webhook filters
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  webhook add https://example.com/hook --event-type message.created --platform wechat --keyword urgent
cargo run -p xenobot-cli --features "api,analysis" -- webhook list
```

### Dispatch tuning
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch show --format json
cargo run -p xenobot-cli --features "api,analysis" -- \
  webhook dispatch set --batch-size 128 --max-concurrency 16 --flush-interval-ms 100 --retry-attempts 4
```

### Dead-letter retry/clear
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- webhook list-failed
cargo run -p xenobot-cli --features "api,analysis" -- webhook retry-failed --limit 100
cargo run -p xenobot-cli --features "api,analysis" -- webhook clear-failed
```

## 7) MCP Runtime

### Start MCP server
```bash
cd Xenobot
cargo run -p xenobot-mcp -- --db-path /tmp/xenobot.db --host 127.0.0.1 --port 8081
```

### MCP smoke validation
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-smoke --url http://127.0.0.1:8081

# list tool catalog via streamable RPC
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-tools --url http://127.0.0.1:8081 --mode rpc --format json
```

## 8) GPU Baseline (Metal/MPS)

```bash
cd Xenobot
cargo run -p xenobot-gpu --bin xenobot-gpu-bench --offline -- --size 256 --iters 8 --format json
# or wrapper command
scripts/xb gpu bench --size 256 --iters 8 --format json
# contract check for automated CI/local verification
scripts/xb gpu check --size 64 --iters 2
```

The benchmark always reports CPU baseline.  
If Metal/MPS is not available in the current runtime, GPU fields are returned with an error description.

## 9) Performance Baseline Report

```bash
cd Xenobot
scripts/xb perf baseline --messages 20000 --db-path /tmp/xenobot-perf.db
scripts/xb perf check --max-import-ms 90000 --max-merge-import-ms 90000 --max-query-ms 10000
```

This command generates:
- performance report JSON in `reports/perf/`
- per-step logs in `reports/perf/<report-name>_logs/`
- contract check validates report structure + status codes and can enforce latency ceilings.

### Unified quality gate
```bash
cd Xenobot
scripts/xb quality gate --skip-platform --messages 600
```

## 10) Troubleshooting

### `Cargo.toml` not found
Run commands from repo root or use `scripts/xb`.

### SQLx macro error asking for `DATABASE_URL`
```bash
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
```

### Transient Rust metadata cache issue (`.rmeta` missing)
If you see an error similar to:
`extern location for xenobot_api does not exist ... .rmeta`

Run:
```bash
cd Xenobot
cargo clean -p xenobot-cli -p xenobot-api
```

Then rerun your previous `cargo test` / `cargo run` command.

### Frontend install blocked by DNS/network policy
Use:
- `scripts/xb web doctor`
- `scripts/xb web doctor --fix-dns`
- `scripts/xb web doctor --fix-dns-sudo`
- `scripts/xb web bundle verify --input .xenobot/offline/frontend-offline-bundle.tar.gz`
- `scripts/xb web bootstrap --extreme --offline-bundle <bundle>`

Operational notes:
- `--fix-dns` is explicit opt-in and tries DNS update without sudo.
- `--fix-dns-sudo` is explicit opt-in admin mode for environments where system DNS updates need elevated permission.
- Offline bundle restore now validates checksum sidecar (`.sha256`) + metadata sidecar (`.manifest.json`) + internal tar manifest before reuse.

### Local workspace size grows too large
Run audit only first (no deletion):
```bash
cd Xenobot
scripts/xb repo hygiene
```

Selective cleanup (build artifacts only):
```bash
cd Xenobot
scripts/xb repo hygiene --apply --remove-target
```

Broader local cleanup (build + frontend cache + empty dirs):
```bash
cd Xenobot
scripts/xb repo hygiene --apply --remove-target --remove-node-modules --prune-empty-dirs
```

## 11) Legal-safe boundary

- Authorized export + local user-accessible data only
- No offensive capabilities (memory extraction, bypass, injection)
- Local-first processing and least-privilege runtime
