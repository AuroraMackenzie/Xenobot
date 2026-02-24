# Xenobot

Xenobot is a Rust-first, privacy-preserving, multi-platform chat data workspace for authorized exports, incremental import, analytics, and AI-assisted exploration on Apple Silicon.

## 中文

### 项目定位
Xenobot 是一个以 Rust 为底层的聊天数据工程平台，面向“用户授权导出数据”的解析、增量入库、统计分析与 AI 检索。

### 合法安全边界
- 仅支持用户授权导出文件与本地可访问数据目录。
- 不包含进程内存密钥提取、绕过加密保护等攻击性能力。
- 默认遵循最小权限原则与本地优先处理。

### 核心能力（当前）
- 17 平台解析器注册与格式识别。
- 增量导入检查点（checkpoint）与失败回写。
- 批量导入：separate / merged 两种模式。
- HTTP API（Axum）+ CLI（clap）+ TUI（ratatui）基础框架。
- Apple Silicon 兼容路径与 Metal/MPS 相关模块骨架。

### Apple Silicon 快速开始
```bash
cd /Users/ycy/Desktop/open-resources-programs/My-program/Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

## English

### Scope
Xenobot is a Rust-native chat data engineering project for authorized exports, incremental database ingestion, analytics, and LLM-assisted workflows.

### Legal and Safe Defaults
- Authorized export files and user-accessible local directories only.
- No process-memory key extraction, decryption bypass, or offensive capability.
- Principle of least privilege and local-first processing.

### Current Capabilities
- 17-platform parser registry and format sniffing.
- Incremental checkpoint semantics with failure writeback.
- Batch import in `separate` and `merged` modes.
- Axum HTTP API, clap CLI, and ratatui TUI foundations.
- Apple Silicon compatible path with Metal/MPS integration scaffolding.

### Quick Start (Apple Silicon)
```bash
cd /Users/ycy/Desktop/open-resources-programs/My-program/Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

## Español

### Alcance
Xenobot es un proyecto en Rust para procesar exportaciones autorizadas de chats, con importación incremental, analítica y flujos asistidos por IA.

### Límites legales y de seguridad
- Solo datos exportados por el usuario y rutas locales autorizadas.
- Sin extracción de claves desde memoria ni evasión de cifrado.
- Procesamiento local y privilegios mínimos por defecto.

### Capacidades actuales
- Registro de analizadores para 17 plataformas y detección de formato.
- Checkpoints de importación incremental con trazabilidad de fallos.
- Importación por lotes en modo `separate` y `merged`.
- Base de API HTTP (Axum), CLI (clap) y TUI (ratatui).
- Ruta compatible con Apple Silicon y módulos de Metal/MPS.

### Inicio rápido
```bash
cd /Users/ycy/Desktop/open-resources-programs/My-program/Xenobot
cargo test -p xenobot-api -p xenobot-cli --features "api,analysis" --offline
cargo run -p xenobot-cli --features "api,analysis" -- api smoke
```

## License

`AGPL-3.0-only`. See `/Users/ycy/Desktop/open-resources-programs/My-program/Xenobot/LICENSE`.
