use crate::Error;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for Xenobot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XenobotConfig {
    /// Path to configuration directory.
    pub config_dir: PathBuf,

    /// Path to data directory.
    pub data_dir: PathBuf,

    /// Path to cache directory.
    pub cache_dir: PathBuf,

    /// Path to logs directory.
    pub logs_dir: PathBuf,

    /// HTTP server configuration.
    pub http: HttpConfig,

    /// Database configuration.
    pub database: DatabaseConfig,

    /// MCP server configuration.
    pub mcp: McpConfig,

    /// GPU acceleration configuration.
    pub gpu: GpuConfig,

    /// Platform-specific configuration.
    pub platform: PlatformConfig,

    /// Logging configuration.
    pub logging: LoggingConfig,

    /// Feature flags.
    pub features: FeatureConfig,
}

/// HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Host to bind to.
    pub host: String,

    /// Port to bind to.
    pub port: u16,

    /// API base path.
    pub api_base_path: String,

    /// Enable CORS.
    pub enable_cors: bool,

    /// Request timeout in seconds.
    pub request_timeout: u64,

    /// Enable request logging.
    pub enable_request_logging: bool,
}

/// Database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to SQLite database file.
    pub sqlite_path: PathBuf,

    /// Maximum number of connections in pool.
    pub max_connections: u32,

    /// Connection timeout in seconds.
    pub connection_timeout: u64,

    /// Enable WAL mode.
    pub enable_wal: bool,

    /// Enable foreign keys.
    pub enable_foreign_keys: bool,

    /// Auto-vacuum mode.
    pub auto_vacuum: AutoVacuumMode,
}

/// Auto-vacuum mode for SQLite.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AutoVacuumMode {
    /// No auto-vacuum.
    None,

    /// Full auto-vacuum.
    Full,

    /// Incremental auto-vacuum.
    Incremental,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Enable MCP server.
    pub enabled: bool,

    /// MCP transport mode.
    pub transport: McpTransport,

    /// MCP server port.
    pub port: u16,

    /// List of MCP tools to expose.
    pub tools: Vec<McpToolConfig>,

    /// Enable SSE streaming.
    pub enable_sse: bool,
}

/// MCP transport mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpTransport {
    /// STDIO transport.
    Stdio,

    /// HTTP transport.
    Http,

    /// SSE transport.
    Sse,
}

/// MCP tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    /// Tool name.
    pub name: String,

    /// Tool description.
    pub description: String,

    /// Tool schema (JSON Schema).
    pub schema: serde_json::Value,

    /// Enable tool.
    pub enabled: bool,
}

/// GPU acceleration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// Enable GPU acceleration.
    pub enabled: bool,

    /// GPU backend to use.
    pub backend: GpuBackend,

    /// Device index to use.
    pub device_index: u32,

    /// Memory limit in MB.
    pub memory_limit_mb: u64,

    /// Enable Metal MPS acceleration (macOS).
    pub enable_metal_mps: bool,
}

/// GPU backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuBackend {
    /// CPU fallback.
    Cpu,

    /// CUDA acceleration.
    Cuda,

    /// Metal acceleration (macOS).
    Metal,

    /// Vulkan acceleration.
    Vulkan,

    /// OpenCL acceleration.
    OpenCl,
}

/// Platform-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// macOS-specific configuration.
    #[cfg(target_os = "macos")]
    pub macos: MacOsConfig,

    /// Windows-specific configuration.
    #[cfg(target_os = "windows")]
    pub windows: WindowsConfig,

    /// Linux-specific configuration.
    #[cfg(target_os = "linux")]
    pub linux: LinuxConfig,

    /// Platform name.
    pub platform_name: String,

    /// Architecture.
    pub architecture: String,
}

/// macOS-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOsConfig {
    /// Enable SIP bypass (requires root).
    pub enable_sip_bypass: bool,

    /// Use Apple Silicon GPU.
    pub use_apple_silicon_gpu: bool,

    /// Enable Metal Performance Shaders.
    pub enable_metal_mps: bool,

    /// Process access permissions.
    pub process_access: ProcessAccessConfig,
}

/// Windows-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsConfig {
    /// Enable process debugging.
    pub enable_process_debugging: bool,

    /// Use DirectX GPU.
    pub use_directx_gpu: bool,

    /// Enable Windows Subsystem for Linux integration.
    pub enable_wsl_integration: bool,
}

/// Linux-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxConfig {
    /// Use Wayland display server.
    pub use_wayland: bool,

    /// Use NVIDIA GPU.
    pub use_nvidia_gpu: bool,

    /// Enable systemd integration.
    pub enable_systemd_integration: bool,
}

/// Process access configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAccessConfig {
    /// Enable process memory reading.
    pub enable_memory_reading: bool,

    /// Enable process injection.
    pub enable_process_injection: bool,

    /// Require root/admin privileges.
    pub require_root_privileges: bool,
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level.
    pub level: LogLevel,

    /// Log format.
    pub format: LogFormat,

    /// Enable file logging.
    pub enable_file_logging: bool,

    /// Enable console logging.
    pub enable_console_logging: bool,

    /// Maximum log file size in MB.
    pub max_file_size_mb: u64,

    /// Maximum number of log files to keep.
    pub max_log_files: u32,
}

/// Log level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogLevel {
    /// Error level.
    Error,

    /// Warning level.
    Warn,

    /// Info level.
    Info,

    /// Debug level.
    Debug,

    /// Trace level.
    Trace,
}

/// Log format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogFormat {
    /// JSON format.
    Json,

    /// Text format.
    Text,

    /// Pretty format.
    Pretty,
}

/// Feature configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Enable real-time data extraction.
    pub enable_real_time_extraction: bool,

    /// Enable database decryption.
    pub enable_database_decryption: bool,

    /// Enable TUI interface.
    pub enable_tui: bool,

    /// Enable HTTP API.
    pub enable_http_api: bool,

    /// Enable MCP server.
    pub enable_mcp_server: bool,

    /// Enable Webhook support.
    pub enable_webhook: bool,

    /// Enable GPU acceleration.
    pub enable_gpu_acceleration: bool,

    /// Enable multi-account management.
    pub enable_multi_account: bool,

    /// Enable file monitoring.
    pub enable_file_monitoring: bool,
}

impl XenobotConfig {
    /// Create default configuration.
    pub fn default() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Cannot find config directory".to_string()))?
            .join("xenobot");

        let data_dir = dirs::data_dir()
            .ok_or_else(|| Error::Config("Cannot find data directory".to_string()))?
            .join("xenobot");

        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| Error::Config("Cannot find cache directory".to_string()))?
            .join("xenobot");

        let logs_dir = config_dir.join("logs");

        Ok(Self {
            config_dir,
            data_dir,
            cache_dir,
            logs_dir,
            http: HttpConfig::default(),
            database: DatabaseConfig::default(),
            mcp: McpConfig::default(),
            gpu: GpuConfig::default(),
            platform: PlatformConfig::default(),
            logging: LoggingConfig::default(),
            features: FeatureConfig::default(),
        })
    }

    /// Load configuration from file.
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::FileSystem(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content).map_err(|e| Error::Parse(format!("Failed to parse config: {}", e)))
    }

    /// Save configuration to file.
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Parse(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| Error::FileSystem(format!("Failed to write config file: {}", e)))
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5030,
            api_base_path: "/api/v1".to_string(),
            enable_cors: true,
            request_timeout: 30,
            enable_request_logging: true,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            sqlite_path: PathBuf::from("xenobot.db"),
            max_connections: 10,
            connection_timeout: 10,
            enable_wal: true,
            enable_foreign_keys: true,
            auto_vacuum: AutoVacuumMode::Incremental,
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transport: McpTransport::Http,
            port: 5031,
            tools: vec![McpToolConfig {
                name: "query_chat_log".to_string(),
                description: "Query chat logs".to_string(),
                schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "number" }
                    }
                }),
                enabled: true,
            }],
            enable_sse: true,
        }
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: GpuBackend::Cpu,
            device_index: 0,
            memory_limit_mb: 1024,
            enable_metal_mps: true,
        }
    }
}

impl Default for PlatformConfig {
    fn default() -> Self {
        let platform_name = std::env::consts::OS.to_string();
        let architecture = std::env::consts::ARCH.to_string();

        Self {
            #[cfg(target_os = "macos")]
            macos: MacOsConfig::default(),
            #[cfg(target_os = "windows")]
            windows: WindowsConfig::default(),
            #[cfg(target_os = "linux")]
            linux: LinuxConfig::default(),
            platform_name,
            architecture,
        }
    }
}

impl Default for MacOsConfig {
    fn default() -> Self {
        Self {
            enable_sip_bypass: false,
            use_apple_silicon_gpu: true,
            enable_metal_mps: true,
            process_access: ProcessAccessConfig::default(),
        }
    }
}

impl Default for ProcessAccessConfig {
    fn default() -> Self {
        Self {
            enable_memory_reading: true,
            enable_process_injection: false,
            require_root_privileges: false,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Text,
            enable_file_logging: true,
            enable_console_logging: true,
            max_file_size_mb: 10,
            max_log_files: 5,
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            enable_real_time_extraction: true,
            enable_database_decryption: true,
            enable_tui: true,
            enable_http_api: true,
            enable_mcp_server: true,
            enable_webhook: true,
            enable_gpu_acceleration: false,
            enable_multi_account: true,
            enable_file_monitoring: true,
        }
    }
}
