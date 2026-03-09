# Xenobot Operations Runbook (Single Entry)

This runbook is the single operational entry for daily execution, sandbox mode, quality gate, and incident handling.

## 1) Choose Runtime Mode

1. Run environment diagnosis first:
```bash
cd Xenobot
scripts/xb api sandbox-doctor
```

2. Start mode based on recommendation:
- Standard mode (listener available):
```bash
scripts/xb api start --db-path /tmp/xenobot.db
```
- Sandbox coexist mode (no listener required):
```bash
scripts/xb api sandbox-up --db-path /tmp/xenobot.db
```

3. Health check in current mode:
```bash
scripts/xb api sandbox-health
```

## 2) In-Process API Contract Check

Use this check when you need deterministic verification without TCP/UDS binding dependency.

```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api smoke --db-path /tmp/xenobot-smoke.db
```

Validated contracts:
- `GET /health`
- `POST /chat/sessions/:session_id/generate-sql` (must include session scope filter)
- `POST /chat/sessions/:session_id/execute-sql`

## 3) Unified Quality Gate

Fast local gate:
```bash
cd Xenobot
scripts/xb quality gate --skip-platform --messages 600
```

Full block-closure gate:
```bash
cd Xenobot
scripts/xb quality gate --messages 1200
```

Common options:
- `--skip-smoke`: skip API smoke contract temporarily
- `--skip-mcp`: skip strict MCP suite temporarily
- `--skip-perf`: skip perf baseline/contract
- `--smoke-db-path <path>`: custom smoke db path

## 4) Frontend DNS Incident Handling

Diagnosis:
```bash
cd Xenobot
scripts/xb web doctor
```

Explicit opt-in DNS fix:
```bash
scripts/xb web doctor --fix-dns
scripts/xb web doctor --fix-dns-sudo
```

Offline fallback:
```bash
scripts/xb web bootstrap --extreme --offline-bundle .xenobot/offline/frontend-offline-bundle.tar.gz
```

## 5) Rust Metadata Cache Incident (`.rmeta` missing)

Symptom example:
- `extern location for xenobot_api does not exist ... .rmeta`

Recovery:
```bash
cd Xenobot
cargo clean -p xenobot-cli -p xenobot-api
```

Then rerun your previous `cargo test` or `cargo run` command.

## 6) Publishing Hygiene

## 6) Local Workspace Hygiene (Disk/Artifact Control)

Audit-only (safe):
```bash
cd Xenobot
scripts/xb repo hygiene
```

Strict source check (CI-safe, no deletion):
```bash
cd Xenobot
scripts/xb repo hygiene --strict-source
```

Clean build artifacts:
```bash
cd Xenobot
scripts/xb repo hygiene --apply --remove-target
```

Optional broader local cleanup:
```bash
cd Xenobot
scripts/xb repo hygiene --apply --remove-target --remove-node-modules --prune-empty-dirs
```

## 7) Publishing Hygiene

- Modify source only in:
  - `/Users/ycy/Desktop/open-resources-programs/My-program/Xenobot`
- Mirror to publish workspace only after a combined block is complete.
- Never copy private planning/reference files into publish workspace.
