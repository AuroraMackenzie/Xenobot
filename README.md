# Xenobot [50%~60%+ - developing]

Xenobot is a Rust-first, privacy-preserving, multi-platform chat data workspace for authorized exports, incremental import, analytics, and AI-assisted exploration on Apple Silicon.

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

### Legal and Safe Defaults
- Authorized export files and user-accessible local directories only.
- No process-memory key extraction, decryption bypass, or offensive capability.
- Principle of least privilege and local-first processing.

### Current Capabilities
- 20-over-platform parser registry and format sniffing.
- Incremental checkpoint semantics with failure writeback.
- Batch import in `separate` and `merged` modes.
- Axum HTTP API, clap CLI, and ratatui TUI foundations.
- Apple Silicon compatible path with Metal/MPS integration scaffolding.

### Quick Start
```bash
cd Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis"
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

### Continuously Running
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api start --db-path /tmp/xenobot.db
```

### Result Checking
```
cargo run -p xenobot-cli --features "api,analysis" -- db create /tmp/xenobot.db
cargo run -p xenobot-cli --features "api,analysis" -- import <你的导出文件或目录> we-chat --db-path /tmp/xenobot.db --write-db --incremental
cargo run -p xenobot-cli --features "api,analysis" -- query --db-path /tmp/xenobot.db search "关键词" -l 20 -f table
cargo run -p xenobot-cli --features "api,analysis" -- analyze --db-path /tmp/xenobot.db stats
```


