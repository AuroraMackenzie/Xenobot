# Xenobot

## Don’t Lose Moments, Xenobot Keeps Them

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

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
- In-memory media pipeline endpoints for authorized processing:
  - `POST /media/decrypt/dat`
  - `POST /media/transcode/audio/mp3`
- Apple Silicon compatible path with Metal/MPS integration scaffolding.

### Runtime Resilience
If one startup path is blocked by the environment, Xenobot automatically switches to another safe local path and keeps running.

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

### Quick Start
```bash
git clone --recursive https://github.com/AuroraMackenzie/Xenobot.git
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

### Run from Any Directory (Recommended)
```bash
ROOT="$(git -C Xenobot rev-parse --show-toplevel)"
"$ROOT/scripts/xb" api status
"$ROOT/scripts/xb" api start --db-path /tmp/xenobot.db
"$ROOT/scripts/xb" api sandbox-doctor
"$ROOT/scripts/xb" api sandbox-up --db-path /tmp/xenobot.db
"$ROOT/scripts/xb" api sandbox-health
"$ROOT/scripts/xb" mcp start --db-path /tmp/xenobot.db
"$ROOT/scripts/xb" mcp smoke --url http://127.0.0.1:8081
"$ROOT/scripts/xb" web bootstrap --with-typecheck
```

### Frontend Bootstrap (Official npm Registry Only)
```bash
cd Xenobot
scripts/xb web doctor
scripts/xb web doctor --fix-dns
scripts/frontend_bootstrap.sh
scripts/frontend_bootstrap.sh --with-typecheck
```

If DNS/network is unavailable, the script exits safely with a clear message and does not block Rust backend development.
`--fix-dns` is explicit opt-in and only updates system DNS when you request it.

### Extreme Offline Workflow (No Network)
```bash
# On a machine with working network (prepare once)
scripts/xb web bundle create
scripts/xb web deps-update --bundle-output .xenobot/offline/frontend-offline-bundle.tar.gz

# On a restricted/offline machine
scripts/xb web doctor
scripts/xb web bootstrap --extreme --offline-bundle .xenobot/offline/frontend-offline-bundle.tar.gz
scripts/xb web bundle info
```

Design notes:
- Official npm registry remains the only online source (`registry.npmjs.org`).
- Extreme mode never attempts network and uses local `node_modules` or an offline bundle.
- Offline bundles are checksum-verified (`.sha256`) and manifest-validated during restore.
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
