# Recent Operations and Execution Flow

This document summarizes the latest operational fixes and the recommended execution flow for Xenobot.

## 1) Latest Operations Completed

### 1.1 SQLx compile-blocker resolved
- Symptom:
  - Build failed with:
    - `set DATABASE_URL to use query macros online, or run cargo sqlx prepare to update the query cache`
- Root cause:
  - Several `sqlx::query_as!` macros in `crates/api/src/database/repository.rs` required offline cache entries that were not available in the target environment.
- Fix:
  - Replaced affected compile-time macros with runtime-bound typed queries (`sqlx::query_as::<_, T>(...).bind(...)`) for those new query sites.
  - Result: build no longer depends on missing offline cache for these specific queries.

### 1.2 API smoke behavior clarified
- `api smoke` is an in-process health smoke check.
- It validates `/health` and exits immediately by design.
- Output example:
  - `api smoke check completed`
  - `status: 200`
  - `body: OK`

### 1.3 API start behavior clarified
- `api start` is a foreground long-running process.
- It appears "stuck" because it is waiting for requests (normal behavior).
- Root route `/` is not defined as a webpage route; use `/health` for health checks.

### 1.4 Smart path launcher added (`scripts/xb`)
- Added a launcher script that auto-detects Xenobot root from any working directory.
- It prevents `Cargo.toml not found` errors caused by running `cargo` in non-project directories.
- Supports:
  - `xb root`
  - `xb cargo <args...>`
  - `xb <xenobot-cli args...>`

## 2) Current Runtime Modes (Priority/Fallback)

When starting API in restricted environments, Xenobot uses a fallback chain:

1. TCP listener (`host:port`)
2. Unix domain socket (UDS)
3. File Gateway IPC mode (no listener mode)

This preserves operability under sandbox restrictions.

## 3) Recommended Command Flow

### 3.1 Anywhere command flow (recommended)

Use the smart launcher:

```bash
xb root
xb api status
xb api smoke
xb api start --db-path /tmp/xenobot.db
```

### 3.2 Standard project flow

```bash
cd /Users/ycy/Desktop/open-resources-programs/My-program/Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis"
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

### 3.3 Live health check (second terminal)

```bash
curl -i http://127.0.0.1:5030/health
```

If API is not running, status will show:
- `state file: missing`
- `status: stopped`

## 4) Troubleshooting Quick Map

### 4.1 `could not find Cargo.toml`
- Cause: command executed outside Xenobot root.
- Fix:
  - Use `xb ...`, or
  - Use `cargo --manifest-path /abs/path/to/Xenobot/Cargo.toml ...`

### 4.2 `/` in browser returns not found
- Cause: no homepage route is defined.
- Fix: use `/health` and API endpoints (`/chat`, `/ai`, `/agent`, etc.).

### 4.3 `api status` stale/incorrect after forced interruptions
- Run:
  - `xb api stop --force`
  - `xb api status`

## 5) Privacy and Publishing Safety

The publish process must continue excluding local-private files:

- `TODOWRITE_STATUS.md`
- `Xenobot项目蓝图.txt`
- `前身项目总结.txt`
- `.env*`, `.Xenobot/`, local DB files, build artifacts

This document contains only operational information and is safe to publish.
