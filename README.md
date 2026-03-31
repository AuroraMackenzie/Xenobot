<div align="center">
  <img src="docs/assets/xenobot.png" width="260" alt="Xenobot">

  <h1>Xenobot</h1>
  <p><strong>Don’t Lose Memories, Xenobot Keeps Them</strong></p>
  <p>Rust-native chat data engineering for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.</p>

  <p>
    <a href="#english">English</a> |
    <a href="#español">Español</a> |
    <a href="#中文">中文</a>
  </p>
</div>

---

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

### Documentation

| Document | Path |
| :-- | :-- |
| API reference | `docs/API.md` |
| User guide | `docs/USER_GUIDE.md` |
| Quality gate | `docs/QUALITY_GATE.md` |
| Operations runbook | `docs/OPERATIONS_RUNBOOK.md` |

### Legal and Safe Defaults
- Authorized export files and user-accessible local directories only.
- No process-memory key extraction, decryption bypass, or offensive capability.
- Principle of least privilege and local-first processing.

---

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

---

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
`api smoke` now runs the full in-process backend workflow across `wechat`, `qq`, and `discord`: `GET /health`, `POST /chat/import`, session generation, member activity, summary persistence, memory recall, keyword search, semantic search, SQL generation, and SQL execution. It also fails if `detectedPlatform` or `payloadPlatform` drift away from the expected platform fixture.

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

---

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

---

## Español

### Alcance
Xenobot es un proyecto de ingeniería de datos de chat, nativo en Rust, orientado a exportaciones autorizadas, ingestión incremental en base de datos, analítica y flujos de trabajo asistidos por LLM.

### Documentación

| Documento | Ruta |
| :-- | :-- |
| Referencia de API | `docs/API.md` |
| Guía de uso | `docs/USER_GUIDE.md` |
| Puerta de calidad | `docs/QUALITY_GATE.md` |
| Manual operativo | `docs/OPERATIONS_RUNBOOK.md` |

### Valores legales y seguros por defecto
- Solo archivos de exportación autorizados y directorios locales accesibles por el usuario.
- Sin extracción de claves desde memoria de procesos, sin evasión de cifrado y sin capacidades ofensivas.
- Principio de mínimo privilegio y procesamiento local como base.

---

### Capacidades actuales
- Registro de parsers multicanal y detección de formato.
- 17 crates adaptadores de plataforma en modo legal-safe con verificación automática de cobertura.
- Semántica incremental de checkpoints con escritura de fallos.
- Importación por lotes en modos `separate` y `merged`.
- Bases de Axum HTTP API, clap CLI y ratatui TUI.
- Runtime MCP con transportes HTTP/SSE/WS + JSON-RPC (`xenobot-mcp`).
- Ruta de ejecución LLM con validación de provider/model/baseUrl y fallback local seguro cuando el upstream falla.
- Endpoints de canal multimedia en memoria para procesamiento autorizado:
  - `POST /media/decrypt/dat`
  - `POST /media/transcode/audio/mp3`
  - la transcodificación de audio acepta `ffmpegPath` en la petición (o `XENOBOT_FFMPEG_PATH` como fallback por entorno).
- Base compatible con Apple Silicon y andamiaje para integración Metal/MPS.

### Resiliencia en tiempo de ejecución
Si una ruta de arranque queda bloqueada por el entorno, Xenobot cambia automáticamente a otra ruta local segura y sigue funcionando.

La superficie de configuración del frontend ahora expone:
- sondas locales del runtime (`/api/health`, `/api/status`, `/api/`)
- recomendación de transporte para sandbox (`TCP` / `UDS` / `file-gateway`)
- presets de integración MCP, incluido `pencil`

### Límite legal-safe
- Sin extracción de claves desde memoria de procesos ni lógica de evasión de cifrado.
- Solo flujos con exportaciones autorizadas y datos locales accesibles por el usuario.
- Los adaptadores de plataforma mantienen explícito el comportamiento legal-safe en sus respuestas de runtime.

---

### Modo de coexistencia con sandbox (sin requerir TCP/UDS)
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api start --force-file-gateway --file-gateway-dir /tmp/xenobot-file-gateway --db-path /tmp/xenobot.db
```

Este modo usa IPC por archivos locales de forma directa y es adecuado para entornos restringidos o en sandbox.

Diagnóstico del runtime (recomienda automáticamente el modo de arranque):
```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api sandbox-doctor --format json
```

Última evidencia de ejecución (2026-03-05 UTC en el entorno actual):
- `tcp.allowed=false` con `Operation not permitted (os error 1)`
- `uds.allowed=false` con `Operation not permitted (os error 1)`
- `fileGateway.writable=true`
- modo recomendado: `file-gateway`

### Inicio rápido
```bash
git clone https://github.com/AuroraMackenzie/Xenobot.git
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```
`api smoke` ejecuta ahora el flujo completo del backend en proceso para `wechat`, `qq` y `discord`: `GET /health`, `POST /chat/import`, generación de sesión, actividad de miembros, persistencia de resúmenes, recuperación de memoria, búsqueda por palabra clave, búsqueda semántica, generación de SQL y ejecución de SQL. También falla si `detectedPlatform` o `payloadPlatform` se desvían de la plataforma esperada del fixture.

### Comandos wrapper (recomendados)
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

### Bootstrap del frontend (solo registro npm oficial)
```bash
cd Xenobot
scripts/xb web doctor
scripts/xb web doctor --fix-dns
scripts/xb web doctor --fix-dns-sudo
scripts/frontend_bootstrap.sh
scripts/frontend_bootstrap.sh --with-typecheck
```

Si DNS o la red no están disponibles, el script sale de forma segura con un mensaje claro y no bloquea el desarrollo del backend en Rust.
`--fix-dns` es opt-in explícito y solo actualiza el DNS del sistema cuando tú lo pides.
`--fix-dns-sudo` es un modo administrativo explícito cuando los cambios de DNS requieren permisos elevados.

### Notas del runtime LLM
- `POST /llm/chat` y `POST /llm/chat-stream` intentan llamadas reales a providers OpenAI-compatible y Gemini.
- Si el upstream no está disponible o es inválido, el runtime vuelve a una respuesta local determinista y segura.
- el timeout de petición puede ajustarse con `XENOBOT_LLM_TIMEOUT_MS` (ms, con límites).

### Flujo extremo sin red
```bash
# En una máquina con red funcional (preparar una vez)
scripts/xb web bundle create
scripts/xb web deps-update --bundle-output .xenobot/offline/frontend-offline-bundle.tar.gz

# En una máquina restringida u offline
scripts/xb web doctor
scripts/xb web bootstrap --extreme --offline-bundle .xenobot/offline/frontend-offline-bundle.tar.gz
scripts/xb web bundle info
scripts/xb web bundle verify --input .xenobot/offline/frontend-offline-bundle.tar.gz
```

Notas de diseño:
- El registro npm oficial sigue siendo la única fuente online (`registry.npmjs.org`).
- El modo extremo nunca intenta acceder a la red y usa `node_modules` local o un bundle offline.
- Los bundles offline se verifican por integridad con sidecar de checksum (`.sha256`) y sidecar de manifiesto (`.manifest.json`) más el manifiesto interno del bundle.
- Los flujos del backend en Rust siguen disponibles aunque se omita el bootstrap de frontend por red.

### Ejecución continua
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api start --db-path /tmp/xenobot.db
```

### Verificación de resultados
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- db create /tmp/xenobot.db
cargo run -p xenobot-cli --features "api,analysis" -- import <export_file_or_directory> we-chat --db-path /tmp/xenobot.db --write-db --incremental
cargo run -p xenobot-cli --features "api,analysis" -- query --db-path /tmp/xenobot.db search "<keyword>" -l 20 -f table
cargo run -p xenobot-cli --features "api,analysis" -- analyze --db-path /tmp/xenobot.db stats
cargo run -p xenobot-cli --features "api,analysis" -- source matrix --format-out json
scripts/check_platform_coverage.sh
```

### Ajuste del despacho de webhooks
La configuración de despacho en runtime se lee desde `~/.config/xenobot/webhooks.json` bajo `dispatch`:

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

Notas:
- el despacho vacía la cola por tamaño de lote o por intervalo de flush, lo que ocurra primero.
- los flujos de bajo volumen no esperan a lotes completos antes de entregar.
- los valores inválidos o fuera de rango se ajustan a límites seguros de runtime.

```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch show --format json
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch set \
  --batch-size 128 --max-concurrency 16 --flush-interval-ms 100 --retry-attempts 4
```

### Solución de problemas
```bash
# 1) Si cargo dice "could not find Cargo.toml", ejecútalo mediante el wrapper:
cd Xenobot
scripts/xb api status
scripts/xb api status --format json

# 2) Si sqlx macros requiere DATABASE_URL durante test/build:
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline

# 3) Si aparece un error transitorio del metadata cache de Rust (missing .rmeta):
cd Xenobot
cargo clean -p xenobot-cli -p xenobot-api

# 4) Si el workspace local crece demasiado:
cd Xenobot
scripts/xb repo hygiene
# acciones opcionales de limpieza:
scripts/xb repo hygiene --apply --remove-target
# limpieza local completa opcional:
scripts/xb repo hygiene --apply --remove-target --remove-node-modules --prune-empty-dirs

# 5) Si el linker de macOS falla con xcrun / SDK license error:
sudo xcodebuild -license
# luego vuelve a ejecutar la comprobación del workspace
cargo check --workspace --all-targets --offline
```

### Verificación de presets MCP (Claude Desktop / ChatWise / Opencode / Pencil)
```bash
cd Xenobot
# listar objetivos de integración compatibles
curl -s http://127.0.0.1:8081/integrations | jq

# obtener un preset desde el MCP server
curl -s http://127.0.0.1:8081/integrations/claude-desktop | jq
curl -s http://127.0.0.1:8081/integrations/pencil | jq

# la misma operación mediante el helper CLI
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target claude-desktop --format json
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target pencil --format json
```

Si el entorno de ejecución actual no expone una entrada directa para Pencil, usa el preset `pencil` como ruta de integración de respaldo y aplícalo dentro de un host MCP compatible con Pencil.

---

### Línea base de benchmark Metal/MPS (Apple Silicon)
```bash
cd Xenobot
cargo run -p xenobot-gpu --bin xenobot-gpu-bench --offline -- --size 256 --iters 8 --format json
# o mediante wrapper
scripts/xb gpu bench --size 256 --iters 8 --format json
```

Notas:
- si Metal/MPS no está disponible en el runtime actual, el benchmark sigue informando la línea base de CPU y devuelve un campo de error estructurado.
- usa esta salida como línea base reproducible para el seguimiento de aceleración GPU en `17.x`.

### Informe de línea base de rendimiento (15.x)
```bash
cd Xenobot
scripts/xb perf baseline --messages 20000 --db-path /tmp/xenobot-perf.db
scripts/xb perf check --max-import-ms 90000 --max-merge-import-ms 90000 --max-query-ms 10000
```

Salida:
- informe JSON en `reports/perf/`
- logs de pasos en `reports/perf/<report-name>_logs/`
- la puerta contractual puede imponer límites superiores para pasos clave (`db_create`, `import_incremental`, `import_merge_batch`, `query_search_json` y duración total).

### Puerta de calidad unificada
```bash
cd Xenobot
scripts/xb quality gate --skip-platform --messages 600
# puerta completa (incluye cobertura de plataforma):
scripts/xb quality gate --messages 1200
```

Notas:
- la puerta de calidad valida la coherencia entre documentación, rutas y runbook antes de los test suites.
- la puerta de calidad ejecuta limpieza previa de source-hygiene y luego verificación estricta (`.DS_Store` se bloquea si sigue existiendo tras la limpieza).
- la puerta de calidad ejecuta por defecto la suite MCP estricta (`cargo test -p xenobot-mcp --offline`).
- la puerta de calidad incluye por defecto el contrato smoke de API en proceso (`/health` + SQL generate + SQL execute).
- usa `--skip-mcp` solo para aislamiento temporal de MCP durante triage de incidentes.
- usa `--skip-smoke` solo para aislamiento temporal durante triage de incidentes.

---

## 中文

### 项目范围
Xenobot 是一个以 Rust 为底层的聊天数据工程项目，面向授权导出、增量数据库导入、分析能力以及 LLM 辅助工作流。

### 文档

| 文档 | 路径 |
| :-- | :-- |
| API 参考 | `docs/API.md` |
| 用户指南 | `docs/USER_GUIDE.md` |
| 质量门禁 | `docs/QUALITY_GATE.md` |
| 运维手册 | `docs/OPERATIONS_RUNBOOK.md` |

### 默认合法安全边界
- 仅处理授权导出文件和用户可访问的本地目录。
- 不包含进程内存取钥、解密绕过或任何攻击性能力。
- 遵循最小权限原则与本地优先处理原则。

---

### 当前能力
- 多平台解析器注册表与格式嗅探。
- 17 个 legal-safe 平台适配 crate，并带自动覆盖校验。
- 带失败回写的增量 checkpoint 语义。
- `separate` 与 `merged` 两种批量导入模式。
- Axum HTTP API、clap CLI 与 ratatui TUI 基础设施。
- 支持 HTTP/SSE/WS + JSON-RPC 传输的 MCP 运行时（`xenobot-mcp`）。
- LLM 运行路径已具备 provider/model/baseUrl 校验，并在上游失败时自动回退到本地安全响应。
- 面向授权处理的内存态多媒体流水线端点：
  - `POST /media/decrypt/dat`
  - `POST /media/transcode/audio/mp3`
  - 音频转码支持请求内 `ffmpegPath` 覆盖，或 `XENOBOT_FFMPEG_PATH` 环境变量回退。
- 兼容 Apple Silicon，并已铺好 Metal/MPS 集成骨架。

### 运行时韧性
如果某一种启动路径被当前环境阻断，Xenobot 会自动切换到另一条本地安全路径继续运行。

前端设置界面当前可以直接展示：
- 本地 runtime 探针（`/api/health`、`/api/status`、`/api/`）
- sandbox 传输推荐（`TCP` / `UDS` / `file-gateway`）
- MCP 集成预设，包括 `pencil`

### Legal-Safe 边界
- 不包含进程内存取钥或解密绕过逻辑。
- 只允许授权导出与本地用户可访问数据流程。
- 平台适配层会在运行时响应里明确保持 legal-safe 行为。

---

### Sandbox 共生模式（不依赖 TCP/UDS）
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- \
  api start --force-file-gateway --file-gateway-dir /tmp/xenobot-file-gateway --db-path /tmp/xenobot.db
```

该模式直接使用本地文件 IPC，适合受限或沙箱环境。

运行时诊断（自动推荐启动模式）：
```bash
cargo run -p xenobot-cli --features "api,analysis" -- \
  api sandbox-doctor --format json
```

当前环境中的最近执行证据（2026-03-05 UTC）：
- `tcp.allowed=false`，错误为 `Operation not permitted (os error 1)`
- `uds.allowed=false`，错误为 `Operation not permitted (os error 1)`
- `fileGateway.writable=true`
- 推荐模式：`file-gateway`

### 快速开始
```bash
git clone https://github.com/AuroraMackenzie/Xenobot.git
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```
`api smoke` 现在会在进程内完整跑通 `wechat`、`qq`、`discord` 三个平台的后端主流程：`GET /health`、`POST /chat/import`、session 生成、成员活跃度、summary 持久化、memory 召回、关键词搜索、语义搜索、SQL 生成与 SQL 执行。如果 `detectedPlatform` 或 `payloadPlatform` 偏离了预期 fixture 平台，也会直接失败。

### Wrapper 命令（推荐）
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

### 前端引导（仅官方 npm Registry）
```bash
cd Xenobot
scripts/xb web doctor
scripts/xb web doctor --fix-dns
scripts/xb web doctor --fix-dns-sudo
scripts/frontend_bootstrap.sh
scripts/frontend_bootstrap.sh --with-typecheck
```

如果 DNS 或网络不可用，脚本会安全退出并给出清晰提示，不会阻断 Rust 后端开发。
`--fix-dns` 为显式 opt-in，只会在你主动要求时修改系统 DNS。
`--fix-dns-sudo` 为显式管理员模式，用于系统 DNS 修改必须提升权限的场景。

### LLM Runtime 说明
- `POST /llm/chat` 与 `POST /llm/chat-stream` 会尝试真实调用 OpenAI-compatible provider 和 Gemini。
- 如果上游不可用或配置无效，运行时会回退到确定性的本地安全响应。
- 请求超时可通过 `XENOBOT_LLM_TIMEOUT_MS` 调整（毫秒，带边界收敛）。

### 极限离线工作流（无网络）
```bash
# 在有网络的机器上准备一次
scripts/xb web bundle create
scripts/xb web deps-update --bundle-output .xenobot/offline/frontend-offline-bundle.tar.gz

# 在受限或离线机器上
scripts/xb web doctor
scripts/xb web bootstrap --extreme --offline-bundle .xenobot/offline/frontend-offline-bundle.tar.gz
scripts/xb web bundle info
scripts/xb web bundle verify --input .xenobot/offline/frontend-offline-bundle.tar.gz
```

设计说明：
- 在线模式下只使用官方 npm 源（`registry.npmjs.org`）。
- Extreme 模式绝不尝试联网，只使用本地 `node_modules` 或离线 bundle。
- 离线 bundle 带有 checksum sidecar（`.sha256`）、manifest sidecar（`.manifest.json`）以及内部 bundle manifest 用于完整性校验。
- 即使跳过前端联网引导，Rust 后端工作流依然可用。

### 持续运行
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- api start --db-path /tmp/xenobot.db
```

### 结果检查
```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- db create /tmp/xenobot.db
cargo run -p xenobot-cli --features "api,analysis" -- import <export_file_or_directory> we-chat --db-path /tmp/xenobot.db --write-db --incremental
cargo run -p xenobot-cli --features "api,analysis" -- query --db-path /tmp/xenobot.db search "<keyword>" -l 20 -f table
cargo run -p xenobot-cli --features "api,analysis" -- analyze --db-path /tmp/xenobot.db stats
cargo run -p xenobot-cli --features "api,analysis" -- source matrix --format-out json
scripts/check_platform_coverage.sh
```

### Webhook 派发调优
Webhook 运行时派发配置读取自 `~/.config/xenobot/webhooks.json` 中的 `dispatch`：

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

说明：
- 派发会在达到 batch size 或 flush interval 任一条件时触发。
- 低吞吐流不会为了等满批次而阻塞交付。
- 非法或超界值会被收敛到安全的运行时范围。

```bash
cd Xenobot
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch show --format json
cargo run -p xenobot-cli --features "api,analysis" -- webhook dispatch set \
  --batch-size 128 --max-concurrency 16 --flush-interval-ms 100 --retry-attempts 4
```

### 故障排查
```bash
# 1) 如果 cargo 提示 "could not find Cargo.toml"，优先通过 wrapper 执行：
cd Xenobot
scripts/xb api status
scripts/xb api status --format json

# 2) 如果 sqlx macros 在 test/build 阶段要求 DATABASE_URL：
cd Xenobot
export DATABASE_URL="sqlite://$(pwd)/test.db"
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline

# 3) 如果出现 Rust metadata cache 的瞬时错误（缺失 .rmeta）：
cd Xenobot
cargo clean -p xenobot-cli -p xenobot-api

# 4) 如果本地 workspace 体积过大：
cd Xenobot
scripts/xb repo hygiene
# 可选清理动作：
scripts/xb repo hygiene --apply --remove-target
# 可选完整本地清理：
scripts/xb repo hygiene --apply --remove-target --remove-node-modules --prune-empty-dirs

# 5) 如果 macOS linker 因 xcrun / SDK license 报错：
sudo xcodebuild -license
# 然后重新执行 workspace 检查
cargo check --workspace --all-targets --offline
```

### MCP 集成预设检查（Claude Desktop / ChatWise / Opencode / Pencil）
```bash
cd Xenobot
# 列出支持的 integration target
curl -s http://127.0.0.1:8081/integrations | jq

# 从 MCP server 拉取单个 preset
curl -s http://127.0.0.1:8081/integrations/claude-desktop | jq
curl -s http://127.0.0.1:8081/integrations/pencil | jq

# 通过 CLI helper 完成同样操作
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target claude-desktop --format json
cargo run -p xenobot-cli --features "api,analysis" -- \
  api mcp-preset --url http://127.0.0.1:8081 --target pencil --format json
```

如果当前执行环境没有直接暴露 Pencil 工具入口，请使用 `pencil` preset 作为回退集成路径，并在兼容 Pencil 的 MCP host 中加载。

---

### Metal/MPS Benchmark 基线（Apple Silicon）
```bash
cd Xenobot
cargo run -p xenobot-gpu --bin xenobot-gpu-bench --offline -- --size 256 --iters 8 --format json
# 或使用 wrapper
scripts/xb gpu bench --size 256 --iters 8 --format json
```

说明：
- 如果当前 runtime 不支持 Metal/MPS，benchmark 仍会返回 CPU baseline，并附带结构化错误字段。
- 这份输出可作为 `17.x` GPU 加速跟踪的可复现实验基线。

### 性能基线报告（15.x）
```bash
cd Xenobot
scripts/xb perf baseline --messages 20000 --db-path /tmp/xenobot-perf.db
scripts/xb perf check --max-import-ms 90000 --max-merge-import-ms 90000 --max-query-ms 10000
```

输出：
- `reports/perf/` 下的 JSON 报告
- `reports/perf/<report-name>_logs/` 下的步骤日志
- 合同门禁可为关键步骤设置上限（`db_create`、`import_incremental`、`import_merge_batch`、`query_search_json` 以及总时长）。

### 统一质量门禁
```bash
cd Xenobot
scripts/xb quality gate --skip-platform --messages 600
# 完整门禁（包含平台覆盖）：
scripts/xb quality gate --messages 1200
```

说明：
- 质量门禁会在测试前先校验文档、路由与 runbook 的一致性。
- 质量门禁会先执行 source-hygiene 预清理，再执行严格检查（如果清理后仍存在 `.DS_Store` 会被阻断）。
- 质量门禁默认运行严格 MCP 套件（`cargo test -p xenobot-mcp --offline`）。
- 质量门禁默认包含进程内 API smoke 合同（`/health` + SQL generate + SQL execute`）。
- `--skip-mcp` 仅应用于事故排查时的 MCP 临时隔离。
- `--skip-smoke` 仅应用于事故排查时的 smoke 临时隔离。
