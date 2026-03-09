# Xenobot

### Do Not Lose Memories, Xenobot Keeps Them
<br>

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

### Documentation
- API reference: `docs/API.md`
- User guide: `docs/USER_GUIDE.md`
- Quality gate: `docs/QUALITY_GATE.md`
- Operations runbook: `docs/OPERATIONS_RUNBOOK.md`

### Reference Boundaries
- ChatLab, chatlog, and CipherTalk are treated as reference projects only.
- Xenobot does not directly reuse their licensed UI assets, icons, page structure, or source code.
- Windows-only DLL, memory-scan, and process-hook workflows from reference projects are intentionally excluded from Xenobot.

### Legal and Safe Defaults
- Authorized export files and user-accessible local directories only.
- No process-memory key extraction, decryption bypass, or offensive capability.
- Principle of least privilege and local-first processing.

### Current Capabilities
- Multi-platform parser registry and format sniffing.
- 17 legal-safe platform adapter crates with automated coverage verification.
- Incremental checkpoint semantics with failure writeback.
- Batch import in `separate` and `merged` modes.
- Axum HTTP API, clap CLI, and ratatui TUI foundations.
- MCP server runtime with HTTP/SSE/WS + JSON-RPC transports (`xenobot-mcp`).
- LLM runtime path with provider/model/baseUrl validation and automatic local-safe fallback on upstream failure.
- In-memory media pipeline endpoints for authorized processing:
  - `POST /media/decrypt/dat`
  - `POST /media/transcode/audio/mp3`
  - audio transcode supports `ffmpegPath` request override (or `XENOBOT_FFMPEG_PATH` env fallback).
- Apple Silicon compatible path with Metal/MPS integration scaffolding.

### Runtime Resilience
If one startup path is blocked by the environment, Xenobot automatically switches to another safe local path and keeps running.

The frontend settings surface now exposes:
- local runtime probes (`/api/health`, `/api/status`, `/api/`)
- sandbox transport recommendation (`TCP` / `UDS` / `file-gateway`)
- MCP integration presets, including `pencil`

### Legal-Safe Boundary
- No process-memory key extraction or decryption bypass logic.
- Authorized export and local user-accessible data workflows only.
- Platform adapters keep legal-safe behavior explicit in runtime responses.

### Sandbox Coexist Mode (No TCP/UDS Requirement)
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api start --force-file-gateway --file-gateway-dir /tmp/xenobot-file-gateway --db-path /tmp/xenobot.db
```

This mode directly uses local file IPC and is suitable for restricted/sandboxed runtime environments.

Runtime diagnostic (auto-recommends startup mode):
```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api sandbox-doctor --format json
```

Latest execution evidence (2026-03-05 UTC in current environment):
- `tcp.allowed=false` with `Operation not permitted (os error 1)`
- `uds.allowed=false` with `Operation not permitted (os error 1)`
- `fileGateway.writable=true`
- recommended mode: `file-gateway`

### Quick Start
```bash
git clone https://github.com/AuroraMackenzie/Xenobot.git
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```
`api smoke` now validates three contracts in-process: `GET /health`, `POST /chat/sessions/:id/generate-sql` (session-scoped SQL), and `POST /chat/sessions/:id/execute-sql`.

### Wrapper Commands (Recommended)
```bash
cd Xenobot
scripts/xb api status
scripts/xb api status --format json
scripts/xb api start --db-path /tmp/xenobot.db
scripts/xb api sandbox-doctor
scripts/xb api sandbox-up --db-path /tmp/xenobot.db
scripts/xb api sandbox-health
scripts/xb repo hygiene
scripts/xb repo hygiene --strict-source
scripts/xb repo hygiene --apply --remove-target
scripts/xb mcp start --db-path /tmp/xenobot.db
scripts/xb mcp smoke --url http://127.0.0.1:8081
scripts/xb mcp tools --url http://127.0.0.1:8081 --mode rpc --format json
scripts/xb gpu bench --size 256 --iters 8 --format json
scripts/xb gpu check --size 64 --iters 2
scripts/xb perf baseline --messages 20000 --db-path /tmp/xenobot-perf.db
scripts/xb perf check --max-import-ms 90000 --max-merge-import-ms 90000 --max-query-ms 10000
scripts/xb quality gate --skip-platform --messages 600
scripts/xb web bootstrap --with-typecheck
```

### Frontend Bootstrap (Official npm Registry Only)
```bash
cd Xenobot
scripts/xb web doctor
scripts/xb web doctor --fix-dns
scripts/xb web doctor --fix-dns-sudo
scripts/frontend_bootstrap.sh
scripts/frontend_bootstrap.sh --with-typecheck
```

If DNS/network is unavailable, the script exits safely with a clear message and does not block Rust backend development.
`--fix-dns` is explicit opt-in and only updates system DNS when you request it.
`--fix-dns-sudo` is explicit opt-in admin mode when system DNS changes require elevated permission.

### LLM Runtime Notes
- `POST /llm/chat` and `POST /llm/chat-stream` attempt provider runtime calls for OpenAI-compatible providers and Gemini.
- If upstream is unavailable or invalid, runtime falls back to a deterministic local-safe response.
- request timeout can be tuned with `XENOBOT_LLM_TIMEOUT_MS` (ms, clamped).

### Extreme Offline Workflow (No Network)
```bash
# On a machine with working network (prepare once)
scripts/xb web bundle create
scripts/xb web deps-update --bundle-output .xenobot/offline/frontend-offline-bundle.tar.gz

# On a restricted/offline machine
scripts/xb web doctor
scripts/xb web bootstrap --extreme --offline-bundle .xenobot/offline/frontend-offline-bundle.tar.gz
scripts/xb web bundle info
scripts/xb web bundle verify --input .xenobot/offline/frontend-offline-bundle.tar.gz
```

Design notes:
- Official npm registry remains the only online source (`registry.npmjs.org`).
- Extreme mode never attempts network and uses local `node_modules` or an offline bundle.
- Offline bundles are integrity-verified with checksum sidecar (`.sha256`) and metadata manifest sidecar (`.manifest.json`) plus internal bundle manifest.
- Rust backend workflows remain available even when frontend network bootstrap is skipped.

### Continuous Run
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api start --db-path /tmp/xenobot.db
```

### Result Checking
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- db create /tmp/xenobot.db
cargo run -p xenobot-cli --features "api,analysis" -- import <export_file_or_directory> we-chat --db-path /tmp/xenobot.db --write-db --incremental
cargo run -p xenobot-cli --features "api,analysis" -- query --db-path /tmp/xenobot.db search "<keyword>" -l 20 -f table
cargo run -p xenobot-cli --features "api,analysis" -- analyze --db-path /tmp/xenobot.db stats
cargo run -p xenobot-cli --features "api,analysis" -- source matrix --format-out json
scripts/check_platform_coverage.sh
```

### Webhook Dispatch Tuning
Webhook runtime dispatch tuning is read from `~/.config/xenobot/webhooks.json` under `dispatch`:

```json
{
  "dispatch": {
    "batchSize": 64,
    "maxConcurrency": 8,
    "requestTimeoutMs": 8000,
    "flushIntervalMs": 250,
    "retryAttempts": 3,
    "retryBaseDelayMs": 150
  }
}
```

Notes:
- dispatch flushes on either batch size or flush interval, whichever comes first.
- low-throughput streams do not wait for full batches before delivery.
- invalid/out-of-range values are clamped to safe runtime bounds.

```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch show --format json
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch set \
  --batch-size 128 --max-concurrency 16 --flush-interval-ms 100 --retry-attempts 4
```

### Troubleshooting
```bash
# 1) If cargo says "could not find Cargo.toml", run via wrapper:
cd Xenobot
scripts/xb api status
scripts/xb api status --format json

# 2) If sqlx macros require DATABASE_URL during test/build:
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline

# 3) If transient rust metadata cache error appears (missing .rmeta):
cd Xenobot
cargo clean -p xenobot-cli -p xenobot-api

# 4) If local workspace size grows too large:
cd Xenobot
scripts/xb repo hygiene
# optional cleanup actions:
scripts/xb repo hygiene --apply --remove-target
# optional full local cleanup:
scripts/xb repo hygiene --apply --remove-target --remove-node-modules --prune-empty-dirs

# 5) If macOS linker fails with xcrun / SDK license error:
sudo xcodebuild -license
# then rerun the workspace check
cargo check --workspace --all-targets --offline
```

### MCP Integration Preset Check (Claude Desktop / ChatWise / Opencode / Pencil)
```bash
cd Xenobot
# list supported integration targets
curl -s http://127.0.0.1:8081/integrations | jq

# fetch one preset from MCP server
curl -s http://127.0.0.1:8081/integrations/claude-desktop | jq
curl -s http://127.0.0.1:8081/integrations/pencil | jq

# same operation via CLI helper
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target claude-desktop --format json
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target pencil --format json
```

If the current execution environment does not expose a direct Pencil tool entry, use the `pencil` preset as the fallback integration path and apply it in a Pencil-compatible MCP host.

### Metal/MPS Benchmark Baseline (Apple Silicon)
```bash
cd Xenobot
cargo run -p xenobot-gpu --bin xenobot-gpu-bench --offline -- --size 256 --iters 8 --format json
# or wrapper
scripts/xb gpu bench --size 256 --iters 8 --format json
```

Notes:
- if Metal/MPS is unavailable in the current runtime, the benchmark still reports CPU baseline and returns a structured error field.
- use this output as a reproducible baseline for `17.x` GPU acceleration tracking.

### Performance Baseline Report (15.x)
```bash
cd Xenobot
scripts/xb perf baseline --messages 20000 --db-path /tmp/xenobot-perf.db
scripts/xb perf check --max-import-ms 90000 --max-merge-import-ms 90000 --max-query-ms 10000
```

Output:
- JSON report under `reports/perf/`
- step logs under `reports/perf/<report-name>_logs/`
- contract gate can enforce upper bounds for key steps (`db_create`, `import_incremental`, `import_merge_batch`, `query_search_json`, and total duration).

### Unified Quality Gate
```bash
cd Xenobot
scripts/xb quality gate --skip-platform --messages 600
# full gate (includes platform coverage):
scripts/xb quality gate --messages 1200
```

Notes:
- Quality gate validates documentation route/runbook consistency before test suites.
- Quality gate runs source-hygiene pre-clean and then strict check (`.DS_Store` is blocked if it still exists after cleanup).
- Quality gate runs strict MCP suite by default (`cargo test -p xenobot-mcp --offline`).
- Quality gate includes in-process API smoke contract by default (`/health` + SQL generate + SQL execute).
- Use `--skip-mcp` only for temporary MCP isolation during incident triage.
- Use `--skip-smoke` only for temporary isolation during incident triage.
