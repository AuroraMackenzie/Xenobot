//! CLI command definitions for Xenobot.
//!
//! Provides command-line interface for multi-platform legal-safe extraction,
//! monitoring, analysis, and API management.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Main CLI application.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Logging verbosity
    #[arg(short, long, default_value_t = 0, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Configuration file path
    #[arg(short, long, env = "XENOBOT_CONFIG")]
    pub config: Option<PathBuf>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Register WeChat decryption keys from authorized user input
    Key(KeyArgs),

    /// Decrypt WeChat database files
    Decrypt(DecryptArgs),

    /// Monitor WeChat data directory for changes
    Monitor(MonitorArgs),

    /// Discover legal-safe data sources for supported platforms
    Source(SourceArgs),

    /// Manage API server
    Api(ApiArgs),

    /// Analyze chat data
    Analyze(AnalyzeArgs),

    /// Import chat data from various platforms
    Import(ImportArgs),

    /// Export data to various formats
    Export(ExportArgs),

    /// Query chat data
    Query(QueryArgs),

    /// Manage accounts
    Account(AccountArgs),

    /// Webhook management
    Webhook(WebhookArgs),

    /// Database operations
    Db(DbArgs),
}

/// Key registration arguments.
#[derive(Args, Debug)]
pub struct KeyArgs {
    /// WeChat process ID (optional metadata only)
    #[arg(short, long)]
    pub pid: Option<u32>,

    /// Data key (hex string, 32 bytes / 64 hex chars)
    #[arg(long, env = "WECHAT_DATA_KEY")]
    pub data_key: Option<String>,

    /// Image key (hex string, 16 bytes / 32 hex chars)
    #[arg(long, env = "WECHAT_IMAGE_KEY")]
    pub image_key: Option<String>,

    /// Key profile name
    #[arg(long, default_value = "default")]
    pub profile: String,

    /// WeChat version (v3, v4, auto)
    #[arg(long = "wechat-version", default_value_t = WeChatVersion::Auto)]
    pub wechat_version: WeChatVersion,

    /// Platform (auto, darwin, windows)
    #[arg(long, default_value_t = Platform::Auto)]
    pub platform: Platform,

    /// Force key overwrite even if keys already exist
    #[arg(long, default_value_t = false)]
    pub force: bool,

    /// Show XOR key for v4 image decryption
    #[arg(long, default_value_t = false)]
    pub xor_key: bool,

    /// Show existing key metadata for this profile
    #[arg(long, default_value_t = false)]
    pub show: bool,

    /// Output format
    #[arg(short, long, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

/// Database decryption arguments.
#[derive(Args, Debug)]
pub struct DecryptArgs {
    /// Chat platform to process
    #[arg(long, value_enum, default_value_t = PlatformFormat::WeChat)]
    pub format: PlatformFormat,

    /// Data key (hex string)
    #[arg(long, env = "WECHAT_DATA_KEY")]
    pub data_key: Option<String>,

    /// Image key for v4 (hex string)
    #[arg(long, env = "WECHAT_IMAGE_KEY")]
    pub image_key: Option<String>,

    /// WeChat data directory path
    #[arg(long, env = "WECHAT_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Output working directory
    #[arg(short, long, default_value = "./.xenobot/work")]
    pub work_dir: PathBuf,

    /// WeChat version
    #[arg(long = "wechat-version", default_value_t = WeChatVersion::Auto)]
    pub wechat_version: WeChatVersion,

    /// Platform
    #[arg(long, default_value_t = Platform::Auto)]
    pub platform: Platform,

    /// Overwrite existing decrypted files
    #[arg(long, default_value_t = false)]
    pub overwrite: bool,

    /// Parallel decryption threads
    #[arg(long, default_value_t = 4)]
    pub threads: usize,
}

/// File monitoring arguments.
#[derive(Args, Debug)]
pub struct MonitorArgs {
    /// Chat platform to monitor
    #[arg(long, value_enum, default_value_t = PlatformFormat::WeChat)]
    pub format: PlatformFormat,

    /// WeChat data directory to monitor
    #[arg(long, env = "WECHAT_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Data key for auto-decryption
    #[arg(long, env = "WECHAT_DATA_KEY")]
    pub data_key: Option<String>,

    /// Image key for v4
    #[arg(long, env = "WECHAT_IMAGE_KEY")]
    pub image_key: Option<String>,

    /// Output directory for decrypted files
    #[arg(short, long, default_value = "./.xenobot/work")]
    pub work_dir: PathBuf,

    /// WeChat version
    #[arg(long = "wechat-version", default_value_t = WeChatVersion::Auto)]
    pub wechat_version: WeChatVersion,

    /// Start monitoring immediately
    #[arg(long, default_value_t = true)]
    pub start: bool,

    /// Watch interval in seconds
    #[arg(long, default_value_t = 5)]
    pub interval: u64,

    /// Persist parsed updates into SQLite while monitoring (requires --features api,analysis)
    #[arg(long, default_value_t = false)]
    pub write_db: bool,

    /// Database path override when --write-db is enabled
    #[arg(long, env = "XENOBOT_DB_PATH")]
    pub db_path: Option<PathBuf>,
}

/// Source discovery arguments.
#[derive(Args, Debug)]
pub struct SourceArgs {
    /// Subcommand
    #[command(subcommand)]
    pub command: SourceCommand,
}

/// Source discovery subcommands.
#[derive(Subcommand, Debug)]
pub enum SourceCommand {
    /// Scan default local source candidates
    Scan {
        /// Optional platform filter
        #[arg(long, value_enum)]
        format: Option<PlatformFormat>,

        /// Show only existing paths
        #[arg(long, default_value_t = false)]
        existing_only: bool,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format_out: OutputFormat,
    },
}

/// API server management arguments.
#[derive(Args, Debug)]
pub struct ApiArgs {
    /// Subcommand
    #[command(subcommand)]
    pub command: ApiCommand,
}

/// API server subcommands.
#[derive(Subcommand, Debug)]
pub enum ApiCommand {
    /// Start API server
    Start {
        /// Listen address
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Listen port
        #[arg(long, default_value_t = 5030)]
        port: u16,

        /// Unix domain socket path (preferred in sandboxed environments)
        #[arg(long)]
        unix_socket: Option<PathBuf>,

        /// Unix socket file mode in octal form (`700`, `750`, `777`, or `0o700`)
        #[arg(long, default_value = "700")]
        unix_socket_mode: String,

        /// File gateway root directory for no-listener IPC fallback
        #[arg(long, env = "XENOBOT_FILE_API_DIR")]
        file_gateway_dir: Option<PathBuf>,

        /// File gateway polling interval in milliseconds
        #[arg(long, default_value_t = 1000)]
        file_gateway_poll_ms: u64,

        /// File gateway response retention in seconds
        #[arg(long, default_value_t = 300)]
        file_gateway_response_ttl_seconds: u64,

        /// Database path
        #[arg(long, env = "XENOBOT_DB_PATH")]
        db_path: Option<PathBuf>,

        /// Enable CORS
        #[arg(long, default_value_t = false)]
        cors: bool,

        /// Enable WebSocket
        #[arg(long, default_value_t = true)]
        websocket: bool,
    },

    /// Stop API server
    Stop {
        /// Force stop
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Restart API server
    Restart {
        /// Force restart
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Get API server status
    Status,

    /// Run API in-process smoke checks without binding network/socket listeners
    Smoke {
        /// Database path override for smoke initialization
        #[arg(long, env = "XENOBOT_DB_PATH")]
        db_path: Option<PathBuf>,
    },

    /// Run file-gateway concurrency stress test (no socket listeners required)
    GatewayStress {
        /// File gateway root directory
        #[arg(long, env = "XENOBOT_FILE_API_DIR")]
        file_gateway_dir: Option<PathBuf>,

        /// Total requests to send
        #[arg(long, default_value_t = 1000)]
        requests: usize,

        /// Max concurrent in-flight requests
        #[arg(long, default_value_t = 64)]
        concurrency: usize,

        /// Per-request timeout in milliseconds
        #[arg(long, default_value_t = 15_000)]
        timeout_ms: u64,

        /// Request method name for file gateway routing
        #[arg(long, default_value = "health.check")]
        method: String,

        /// Optional HTTP path override (used when method is an HTTP verb)
        #[arg(long)]
        path: Option<String>,
    },
}

/// Chat data analysis arguments.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Database path
    #[arg(short, long, env = "XENOBOT_DB_PATH")]
    pub db_path: PathBuf,

    /// Analysis type
    #[command(subcommand)]
    pub analysis: AnalysisType,
}

/// Analysis types.
#[derive(Subcommand, Debug)]
pub enum AnalysisType {
    /// Basic statistics
    Stats {
        /// Time range start (YYYY-MM-DD)
        #[arg(long)]
        start_date: Option<String>,

        /// Time range end (YYYY-MM-DD)
        #[arg(long)]
        end_date: Option<String>,

        /// Member ID filter
        #[arg(long)]
        member_id: Option<String>,
    },

    /// Advanced analysis
    Advanced {
        /// Analysis type
        #[arg(value_enum, default_value_t = AdvancedAnalysis::NightOwl)]
        analysis: AdvancedAnalysis,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },

    /// Time distribution
    TimeDistribution {
        /// Granularity
        #[arg(value_enum, default_value_t = TimeGranularity::Hourly)]
        granularity: TimeGranularity,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Csv)]
        format: OutputFormat,
    },
}

/// Chat data import arguments.
#[derive(Args, Debug)]
pub struct ImportArgs {
    /// Input file or directory
    #[arg(required = true)]
    pub input: PathBuf,

    /// Platform format
    #[arg(value_enum)]
    pub format: PlatformFormat,

    /// Database path (creates new if not exists)
    #[arg(short, long, env = "XENOBOT_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Session name
    #[arg(long)]
    pub session_name: Option<String>,

    /// Import as incremental update
    #[arg(long, default_value_t = false)]
    pub incremental: bool,

    /// Enable streaming for large files
    #[arg(long, default_value_t = true)]
    pub stream: bool,

    /// Persist parsed records into SQLite database (requires --features api,analysis)
    #[arg(long, default_value_t = false)]
    pub write_db: bool,

    /// Merge multi-file import into a single session name per platform when writing DB
    #[arg(long, default_value_t = false)]
    pub merge: bool,
}

/// Data export arguments.
#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Database path
    #[arg(short, long, env = "XENOBOT_DB_PATH")]
    pub db_path: PathBuf,

    /// Export format
    #[arg(value_enum, default_value_t = ExportFormat::Jsonl)]
    pub format: ExportFormat,

    /// Output file or directory
    #[arg(short, long)]
    pub output: PathBuf,

    /// Time range start
    #[arg(long)]
    pub start_date: Option<String>,

    /// Time range end
    #[arg(long)]
    pub end_date: Option<String>,

    /// Member ID filter
    #[arg(long)]
    pub member_id: Option<String>,
}

/// Data query arguments.
#[derive(Args, Debug)]
pub struct QueryArgs {
    /// Database path
    #[arg(short, long, env = "XENOBOT_DB_PATH")]
    pub db_path: PathBuf,

    /// Query type
    #[command(subcommand)]
    pub query: QueryType,
}

/// Query types.
#[derive(Subcommand, Debug)]
pub enum QueryType {
    /// Search messages
    Search {
        /// Search keyword
        #[arg(required = true)]
        keyword: String,

        /// Time range start
        #[arg(long)]
        start_date: Option<String>,

        /// Time range end
        #[arg(long)]
        end_date: Option<String>,

        /// Member ID filter
        #[arg(long)]
        member_id: Option<String>,

        /// Limit results
        #[arg(short, long, default_value_t = 100)]
        limit: usize,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Run SQL query
    Sql {
        /// SQL query
        #[arg(required = true)]
        sql: String,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Semantic search
    Semantic {
        /// Query text
        #[arg(required = true)]
        query: String,

        /// Similarity threshold
        #[arg(long, default_value_t = 0.7)]
        threshold: f32,

        /// Limit results
        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

/// Account management arguments.
#[derive(Args, Debug)]
pub struct AccountArgs {
    /// Subcommand
    #[command(subcommand)]
    pub command: AccountCommand,
}

/// Account subcommands.
#[derive(Subcommand, Debug)]
pub enum AccountCommand {
    /// List available accounts
    List {
        /// Show details
        #[arg(long, default_value_t = false)]
        details: bool,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Switch active account
    Switch {
        /// Account ID
        #[arg(required = true)]
        account_id: String,
    },

    /// Add new account
    Add {
        /// Account name
        #[arg(required = true)]
        name: String,

        /// Platform data/export directory
        #[arg(long)]
        data_dir: Option<PathBuf>,

        /// Platform format
        #[arg(long, value_enum, default_value_t = PlatformFormat::WeChat)]
        format: PlatformFormat,

        /// WeChat data version hint
        #[arg(long = "wechat-version", default_value_t = WeChatVersion::Auto)]
        wechat_version: WeChatVersion,
    },
}

/// Webhook management arguments.
#[derive(Args, Debug)]
pub struct WebhookArgs {
    /// Subcommand
    #[command(subcommand)]
    pub command: WebhookCommand,
}

/// Webhook subcommands.
#[derive(Subcommand, Debug)]
pub enum WebhookCommand {
    /// Add webhook
    Add {
        /// Webhook URL
        #[arg(required = true)]
        url: String,

        /// Event type filter
        #[arg(long)]
        event_type: Option<String>,

        /// Sender filter
        #[arg(long)]
        sender: Option<String>,

        /// Keyword filter
        #[arg(long)]
        keyword: Option<String>,
    },

    /// List webhooks
    List {
        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Remove webhook
    Remove {
        /// Webhook ID
        #[arg(required = true)]
        webhook_id: String,
    },

    /// List failed webhook deliveries (dead-letter queue)
    ListFailed {
        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Retry failed webhook deliveries
    RetryFailed {
        /// Max dead-letter entries to retry in this run
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },

    /// Clear all failed webhook deliveries
    ClearFailed,
}

/// Database operations arguments.
#[derive(Args, Debug)]
pub struct DbArgs {
    /// Subcommand
    #[command(subcommand)]
    pub command: DbCommand,
}

/// Database subcommands.
#[derive(Subcommand, Debug)]
pub enum DbCommand {
    /// Create new database
    Create {
        /// Database path
        #[arg(required = true)]
        path: PathBuf,

        /// Schema version
        #[arg(long, default_value_t = 1)]
        schema_version: u32,
    },

    /// Migrate database schema
    Migrate {
        /// Database path
        #[arg(required = true)]
        path: PathBuf,

        /// Target version
        #[arg(long)]
        target_version: Option<u32>,
    },

    /// Database info
    Info {
        /// Database path
        #[arg(required = true)]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    /// Optimize database
    Optimize {
        /// Database path
        #[arg(required = true)]
        path: PathBuf,
    },
}

/// WeChat version.
#[derive(Debug, Clone, ValueEnum)]
pub enum WeChatVersion {
    /// Auto-detect
    Auto,
    /// WeChat v3.x
    V3,
    /// WeChat v4.x
    V4,
}

impl std::fmt::Display for WeChatVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeChatVersion::Auto => write!(f, "auto"),
            WeChatVersion::V3 => write!(f, "v3"),
            WeChatVersion::V4 => write!(f, "v4"),
        }
    }
}

/// Platform.
#[derive(Debug, Clone, ValueEnum)]
pub enum Platform {
    /// Auto-detect
    Auto,
    /// macOS (Darwin)
    Darwin,
    /// Windows
    Windows,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Auto => write!(f, "auto"),
            Platform::Darwin => write!(f, "darwin"),
            Platform::Windows => write!(f, "windows"),
        }
    }
}

/// Output format.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Plain text
    Text,
    /// JSON format
    Json,
    /// CSV format
    Csv,
    /// Table format
    Table,
    /// YAML format
    Yaml,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Yaml => write!(f, "yaml"),
        }
    }
}

/// Platform format for import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PlatformFormat {
    /// WeChat (WeFlow format)
    #[value(alias = "wechat", alias = "weixin", alias = "wx")]
    WeChat,
    /// WhatsApp
    #[value(alias = "whatsapp", alias = "wa")]
    WhatsApp,
    /// LINE
    Line,
    /// QQ
    Qq,
    /// Discord
    Discord,
    /// Telegram
    #[value(alias = "telegram", alias = "tg")]
    Telegram,
    /// Instagram
    Instagram,
    /// iMessage
    #[value(alias = "imessage")]
    IMessage,
    /// Facebook Messenger
    #[value(alias = "facebook", alias = "fb-messenger")]
    Messenger,
    /// KakaoTalk
    #[value(alias = "kakaotalk", alias = "kakao")]
    KakaoTalk,
    /// Slack
    Slack,
    /// Microsoft Teams
    Teams,
    /// Signal
    Signal,
    /// Skype
    Skype,
    /// Google Chat
    #[value(alias = "googlechat", alias = "hangouts")]
    GoogleChat,
    /// Zoom
    Zoom,
    /// Viber
    Viber,
    /// Xenobot JSON
    Xenobot,
}

/// Export format.
#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat {
    /// Xenobot JSONL
    Jsonl,
    /// Plain text
    Text,
    /// CSV
    Csv,
    /// JSON
    Json,
    /// HTML
    Html,
}

/// Advanced analysis types.
#[derive(Debug, Clone, ValueEnum)]
pub enum AdvancedAnalysis {
    /// Night owl analysis
    NightOwl,
    /// Dragon king analysis
    DragonKing,
    /// Diving analysis
    Diving,
    /// Check-in analysis
    CheckIn,
    /// Meme battle analysis
    MemeBattle,
    /// Mention analysis
    Mention,
    /// Repeat analysis
    Repeat,
    /// Catchphrase analysis
    Catchphrase,
    /// Laugh analysis
    Laugh,
    /// Cluster analysis
    Cluster,
}

/// Time granularity for analysis.
#[derive(Debug, Clone, ValueEnum)]
pub enum TimeGranularity {
    /// Hourly distribution
    Hourly,
    /// Daily distribution
    Daily,
    /// Weekly distribution
    Weekly,
    /// Monthly distribution
    Monthly,
    /// Yearly distribution
    Yearly,
}
