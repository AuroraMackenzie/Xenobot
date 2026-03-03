# Xenobot

Xenobot is a Rust-first, privacy-preserving, multi-platform chat data workspace for authorized exports, incremental import, analytics, and AI-assisted exploration on Apple Silicon.

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

### Legal and Safe Defaults
- Authorized export files and user-accessible local directories only.
- No process-memory key extraction, decryption bypass, or offensive capability.
- Principle of least privilege and local-first processing.

### Current Capabilities
- Multi-platform parser registry and format sniffing.
- Incremental checkpoint semantics with failure writeback.
- Batch import in `separate` and `merged` modes.
- Axum HTTP API, clap CLI, and ratatui TUI foundations.
- Apple Silicon compatible path with Metal/MPS integration scaffolding.

### Runtime Resilience
If one startup path is blocked by the environment, Xenobot automatically switches to another safe local path and keeps running.

### Quick Start
```bash
git clone https://github.com/AuroraMackenzie/Xenobot.git
cd Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis"
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

### Run from Any Directory (Recommended)
```bash
/Users/ycy/Desktop/open-resources-programs/My-program/Xenobot/scripts/xb api status
/Users/ycy/Desktop/open-resources-programs/My-program/Xenobot/scripts/xb api start --db-path /tmp/xenobot.db
```

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
```
