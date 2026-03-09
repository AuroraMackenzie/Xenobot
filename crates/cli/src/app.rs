//! CLI application entry point and configuration.
//!
//! This module provides the main CLI application logic, including argument parsing,
//! configuration loading, and command dispatch.

use crate::commands::{
    AccountCommand, AdvancedAnalysis, AnalysisType, Cli, Commands, DecryptArgs, ExportArgs,
    ExportFormat, ImportArgs, KeyArgs, MonitorArgs, OutputFormat, PlatformFormat, QueryArgs,
    QueryType, SourceArgs, SourceCommand, TimeGranularity, WebhookArgs, WebhookCommand,
    WebhookDispatchCommand,
};
use crate::error::{CliError, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(feature = "api")]
use std::collections::VecDeque;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use xenobot_core::webhook::{
    WebhookDeadLetterEntry, overwrite_dead_letter_entries, read_dead_letter_entries,
};
#[cfg(all(feature = "analysis", feature = "api"))]
use xenobot_core::webhook::{
    WebhookDispatchStats, WebhookMessageCreatedEvent, WebhookRule, append_dead_letter_entry,
    build_dead_letter_entry, merge_webhook_dispatch_stats, webhook_rule_matches_event,
};
use xenobot_core::{
    Platform as RuntimePlatform, SourceCandidate, discover_sources_for_all_platforms,
    discover_sources_for_platform, legal_safe_runtime_platforms, platform_id as core_platform_id,
};

/// Configuration for the CLI application.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Path to configuration file.
    pub config_path: Option<PathBuf>,
    /// Logging verbosity level.
    pub verbosity: u8,
    /// Working directory for file operations.
    pub work_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_path: None,
            verbosity: 0,
            work_dir: PathBuf::from("./.xenobot/work"),
        }
    }
}

/// Main CLI application.
#[derive(Debug)]
pub struct App {
    /// Application configuration.
    pub config: AppConfig,
    /// Parsed CLI arguments.
    pub cli: Cli,
}

impl App {
    /// Create a new application instance from command line arguments.
    pub fn new() -> Result<Self> {
        let cli = Cli::parse();
        let config = Self::load_config(&cli)?;
        Ok(Self { config, cli })
    }

    /// Load configuration from file and environment.
    fn load_config(cli: &Cli) -> Result<AppConfig> {
        let mut config = AppConfig {
            verbosity: cli.verbose,
            ..AppConfig::default()
        };

        // Load configuration file if specified
        if let Some(config_path) = &cli.config {
            if config_path.exists() {
                let file_config = read_cli_config_file(config_path)?;
                if let Some(verbosity) = file_config.verbosity {
                    config.verbosity = verbosity;
                }
                if let Some(work_dir) = file_config.work_dir {
                    config.work_dir = work_dir;
                }
                config.config_path = Some(config_path.clone());
            } else {
                return Err(CliError::Config(format!(
                    "Configuration file not found: {}",
                    config_path.display()
                )));
            }
        }

        // Override with environment variables
        if let Ok(work_dir) = std::env::var("XENOBOT_WORK_DIR") {
            config.work_dir = PathBuf::from(work_dir);
        }

        Ok(config)
    }

    /// Run the application.
    pub fn run(self) -> Result<()> {
        // Set up logging based on verbosity
        self.setup_logging();

        // Dispatch command
        match &self.cli.command {
            Commands::Key(args) => self.handle_key(args),
            Commands::Decrypt(args) => self.handle_decrypt(args),
            Commands::Monitor(args) => self.handle_monitor(args),
            Commands::Source(args) => self.handle_source(args),
            Commands::Api(args) => self.handle_api(args),
            Commands::Analyze(args) => self.handle_analyze(args),
            Commands::Import(args) => self.handle_import(args),
            Commands::Export(args) => self.handle_export(args),
            Commands::Query(args) => self.handle_query(args),
            Commands::Account(args) => self.handle_account(args),
            Commands::Webhook(args) => self.handle_webhook(args),
            Commands::Db(args) => self.handle_db(args),
        }
    }

    /// Set up logging based on verbosity level.
    fn setup_logging(&self) {
        let level = match self.config.verbosity {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };

        env_logger::Builder::new()
            .filter_level(level)
            .format_module_path(false)
            .format_target(false)
            .format_timestamp(None)
            .try_init()
            .ok(); // Ignore errors if logger already initialized
    }

    fn handle_key(&self, args: &KeyArgs) -> Result<()> {
        let profile = normalize_profile_name(&args.profile)?;
        let mut store = read_key_store()?;

        if args.show {
            let Some(saved) = store.profiles.get(&profile) else {
                return Err(CliError::Command(format!(
                    "key profile '{}' not found",
                    profile
                )));
            };
            print_key_profile(saved, &profile, &args.format)?;
            return Ok(());
        }

        let data_key = normalize_hex_key(
            args.data_key
                .as_deref()
                .ok_or_else(|| CliError::Argument("missing --data-key".to_string()))?,
            64,
            "data key",
        )?;
        let image_key = normalize_hex_key(
            args.image_key
                .as_deref()
                .ok_or_else(|| CliError::Argument("missing --image-key".to_string()))?,
            32,
            "image key",
        )?;

        if store.profiles.contains_key(&profile) && !args.force {
            return Err(CliError::Command(format!(
                "key profile '{}' already exists, use --force to overwrite",
                profile
            )));
        }

        let saved = StoredKeyProfile {
            data_key,
            image_key: image_key.clone(),
            version: args.wechat_version.to_string(),
            platform: args.platform.to_string(),
            pid: args.pid,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        store.profiles.insert(profile.clone(), saved.clone());
        write_key_store(&store)?;

        match args.format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "profile": profile,
                        "updatedAt": saved.updated_at,
                        "version": saved.version,
                        "platform": saved.platform,
                        "pid": saved.pid,
                        "dataKeyMasked": mask_secret(&saved.data_key),
                        "imageKeyMasked": mask_secret(&saved.image_key),
                        "xorKey": if args.xor_key { Some(mask_secret(&saved.image_key)) } else { None },
                    }))
                    .map_err(|e| CliError::Parse(e.to_string()))?
                );
            }
            _ => {
                println!("saved key profile: {}", profile);
                println!("version: {}", saved.version);
                println!("platform: {}", saved.platform);
                println!("data key: {}", mask_secret(&saved.data_key));
                println!("image key: {}", mask_secret(&saved.image_key));
                if args.xor_key {
                    println!("xor key hint: {}", mask_secret(&saved.image_key));
                }
            }
        }
        Ok(())
    }

    fn handle_decrypt(&self, args: &DecryptArgs) -> Result<()> {
        let runtime_platform = runtime_platform_from_format(args.format);
        if runtime_platform != RuntimePlatform::WeChat {
            return self.handle_non_wechat_decrypt(runtime_platform, args);
        }

        let (data_key, image_key) = resolve_keys_for_runtime(
            args.data_key.as_deref(),
            args.image_key.as_deref(),
            Some("default"),
        )?;
        let _ = normalize_hex_key(&data_key, 64, "data key")?;
        let _ = normalize_hex_key(&image_key, 32, "image key")?;

        let target_data_dir = args
            .data_dir
            .as_ref()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(default_wechat_data_dir()));

        if target_data_dir.as_os_str().is_empty() {
            return Err(CliError::Argument(
                "wechat data dir is empty; provide --data-dir explicitly".to_string(),
            ));
        }

        println!("decrypt plan generated");
        println!("data dir: {}", target_data_dir.to_string_lossy());
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("threads: {}", args.threads);
        println!("overwrite: {}", args.overwrite);
        println!("platform format: {}", platform_format_id(args.format));
        println!("data key: {}", mask_secret(&data_key));
        println!("image key: {}", mask_secret(&image_key));
        println!("mode: legal-safe authorized export staging");

        #[cfg(feature = "analysis")]
        {
            return run_legal_safe_decrypt_stage(
                &target_data_dir,
                &args.work_dir,
                args.overwrite,
                args.format,
            );
        }

        #[cfg(not(feature = "analysis"))]
        {
            println!(
                "decrypt staging runtime is disabled in this build; enable with --features analysis"
            );
            Ok(())
        }
    }

    fn handle_monitor(&self, args: &MonitorArgs) -> Result<()> {
        let runtime_platform = runtime_platform_from_format(args.format);
        if runtime_platform != RuntimePlatform::WeChat {
            return self.handle_non_wechat_monitor(runtime_platform, args);
        }

        let (data_key, image_key) = resolve_keys_for_runtime(
            args.data_key.as_deref(),
            args.image_key.as_deref(),
            Some("default"),
        )?;
        let _ = normalize_hex_key(&data_key, 64, "data key")?;
        let _ = normalize_hex_key(&image_key, 32, "image key")?;

        let target_data_dir = args
            .data_dir
            .as_ref()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(default_wechat_data_dir()));

        if target_data_dir.as_os_str().is_empty() {
            return Err(CliError::Argument(
                "wechat watch dir is empty; provide --data-dir explicitly".to_string(),
            ));
        }

        println!("monitor plan generated");
        println!("watch dir: {}", target_data_dir.to_string_lossy());
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("interval seconds: {}", args.interval);
        println!("start immediately: {}", args.start);
        println!("write_db: {}", args.write_db);
        if let Some(path) = args.db_path.as_ref() {
            println!("db path: {}", path.display());
        }
        println!("platform format: {}", platform_format_id(args.format));
        println!("data key: {}", mask_secret(&data_key));
        println!("image key: {}", mask_secret(&image_key));
        println!("mode: legal-safe incremental monitor");

        if !args.start {
            return Ok(());
        }

        #[cfg(feature = "analysis")]
        {
            println!("monitor loop started (Ctrl+C to stop)");
            return run_legal_safe_monitor_loop(
                &runtime_platform,
                &target_data_dir,
                args.interval,
                args.write_db,
                args.db_path.clone(),
                args.format,
            );
        }

        #[cfg(not(feature = "analysis"))]
        {
            println!("monitor runtime is disabled in this build; enable with --features analysis");
            Ok(())
        }
    }

    fn handle_non_wechat_decrypt(
        &self,
        runtime_platform: RuntimePlatform,
        args: &DecryptArgs,
    ) -> Result<()> {
        let hints = discover_sources_for_platform(&runtime_platform);
        let selected = args
            .data_dir
            .as_ref()
            .cloned()
            .or_else(|| first_existing_path(&hints))
            .unwrap_or_else(|| PathBuf::from("."));

        println!("decrypt plan generated (legal-safe mode)");
        println!("platform format: {}", platform_format_id(args.format));
        println!("data dir: {}", selected.to_string_lossy());
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("threads: {}", args.threads);
        println!("overwrite: {}", args.overwrite);
        println!(
            "note: non-WeChat platforms rely on authorized exports and parser import, not key-based DB decryption"
        );

        print_source_candidates(&hints, false, &OutputFormat::Text)?;
        Ok(())
    }

    fn handle_non_wechat_monitor(
        &self,
        runtime_platform: RuntimePlatform,
        args: &MonitorArgs,
    ) -> Result<()> {
        let hints = discover_sources_for_platform(&runtime_platform);
        let selected = args
            .data_dir
            .as_ref()
            .cloned()
            .or_else(|| first_existing_path(&hints))
            .unwrap_or_else(|| PathBuf::from("."));

        println!("monitor plan generated (legal-safe mode)");
        println!("platform format: {}", platform_format_id(args.format));
        println!("watch dir: {}", selected.to_string_lossy());
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("interval seconds: {}", args.interval);
        println!("start immediately: {}", args.start);
        println!("write_db: {}", args.write_db);
        if let Some(path) = args.db_path.as_ref() {
            println!("db path: {}", path.display());
        }
        println!(
            "note: non-WeChat monitoring follows export folder updates and incremental parser import"
        );

        print_source_candidates(&hints, false, &OutputFormat::Text)?;

        if !args.start {
            return Ok(());
        }

        #[cfg(feature = "analysis")]
        {
            println!("monitor loop started (Ctrl+C to stop)");
            return run_legal_safe_monitor_loop(
                &runtime_platform,
                &selected,
                args.interval,
                args.write_db,
                args.db_path.clone(),
                args.format,
            );
        }

        #[cfg(not(feature = "analysis"))]
        {
            println!(
                "monitor runtime is disabled in current build; enable with --features analysis"
            );
            Ok(())
        }
    }

    fn handle_source(&self, args: &SourceArgs) -> Result<()> {
        match &args.command {
            SourceCommand::Scan {
                format,
                existing_only,
                format_out,
            } => {
                let items = if let Some(fmt) = format {
                    let platform = runtime_platform_from_format(*fmt);
                    discover_sources_for_platform(&platform)
                } else {
                    discover_sources_for_all_platforms()
                };
                print_source_candidates(&items, *existing_only, format_out)
            }
            SourceCommand::Matrix { format_out } => print_source_platform_matrix(format_out),
        }
    }

    fn handle_api(&self, args: &crate::commands::ApiArgs) -> Result<()> {
        #[cfg(feature = "api")]
        {
            use crate::commands::ApiCommand;

            match &args.command {
                ApiCommand::Start {
                    host,
                    port,
                    unix_socket,
                    unix_socket_mode,
                    file_gateway_dir,
                    file_gateway_poll_ms,
                    file_gateway_response_ttl_seconds,
                    force_file_gateway,
                    db_path,
                    cors,
                    websocket,
                } => start_api_server_foreground(
                    host.trim(),
                    *port,
                    unix_socket.clone(),
                    unix_socket_mode.as_str(),
                    file_gateway_dir.clone(),
                    *file_gateway_poll_ms,
                    *file_gateway_response_ttl_seconds,
                    *force_file_gateway,
                    db_path.clone(),
                    *cors,
                    *websocket,
                ),
                ApiCommand::Status { format } => print_api_server_status(format),
                ApiCommand::Stop { force } => stop_api_server(*force),
                ApiCommand::Restart { force } => restart_api_server(*force),
                ApiCommand::Smoke { db_path } => run_api_smoke_check(db_path.clone()),
                ApiCommand::GatewayStress {
                    file_gateway_dir,
                    requests,
                    concurrency,
                    timeout_ms,
                    method,
                    path,
                } => run_api_file_gateway_stress(
                    file_gateway_dir.clone(),
                    *requests,
                    *concurrency,
                    *timeout_ms,
                    method.clone(),
                    path.clone(),
                ),
                ApiCommand::GatewayCall {
                    file_gateway_dir,
                    request_id,
                    method,
                    path,
                    body_json,
                    timeout_ms,
                    format,
                } => run_api_file_gateway_call(
                    file_gateway_dir.clone(),
                    request_id.clone(),
                    method.clone(),
                    path.clone(),
                    body_json.clone(),
                    *timeout_ms,
                    format.clone(),
                ),
                ApiCommand::SandboxDoctor {
                    file_gateway_dir,
                    format,
                } => run_api_sandbox_doctor(file_gateway_dir.clone(), format.clone()),
                ApiCommand::McpSmoke { url, timeout_ms } => {
                    run_mcp_smoke_check(url.clone(), *timeout_ms)
                }
                ApiCommand::McpPreset {
                    url,
                    target,
                    format,
                    timeout_ms,
                } => run_mcp_integration_preset_fetch(
                    url.clone(),
                    target.clone(),
                    format.clone(),
                    *timeout_ms,
                ),
                ApiCommand::McpCall {
                    url,
                    mode,
                    tool,
                    args_json,
                    format,
                    timeout_ms,
                } => run_mcp_tool_call(
                    url.clone(),
                    mode.clone(),
                    tool.clone(),
                    args_json.clone(),
                    format.clone(),
                    *timeout_ms,
                ),
                ApiCommand::McpTools {
                    url,
                    mode,
                    format,
                    timeout_ms,
                } => run_mcp_tools_list(url.clone(), mode.clone(), format.clone(), *timeout_ms),
                ApiCommand::McpResources {
                    url,
                    mode,
                    format,
                    timeout_ms,
                } => run_mcp_resources_list(url.clone(), mode.clone(), format.clone(), *timeout_ms),
                ApiCommand::McpResource {
                    url,
                    mode,
                    uri,
                    format,
                    timeout_ms,
                } => run_mcp_resource_read(
                    url.clone(),
                    mode.clone(),
                    uri.clone(),
                    format.clone(),
                    *timeout_ms,
                ),
            }
        }

        #[cfg(not(feature = "api"))]
        {
            let _ = args;
            println!("API command requires CLI build with --features api");
            Ok(())
        }
    }

    fn handle_analyze(&self, args: &crate::commands::AnalyzeArgs) -> Result<()> {
        let conn = open_sqlite_read_connection(&args.db_path)?;

        match &args.analysis {
            AnalysisType::Stats {
                start_date,
                end_date,
                member_id,
            } => {
                let start_ts = parse_optional_date_start(start_date.as_deref())?;
                let end_ts = parse_optional_date_end(end_date.as_deref())?;
                let member_filter = parse_optional_member_id(member_id.as_deref())?;

                let (total_messages, unique_senders, min_ts, max_ts): (
                    i64,
                    i64,
                    Option<i64>,
                    Option<i64>,
                ) = conn
                    .query_row(
                        r#"
                        SELECT
                            COUNT(*) AS total_messages,
                            COUNT(DISTINCT sender_id) AS unique_senders,
                            MIN(ts) AS min_ts,
                            MAX(ts) AS max_ts
                        FROM message
                        WHERE (?1 IS NULL OR ts >= ?1)
                          AND (?2 IS NULL OR ts <= ?2)
                          AND (?3 IS NULL OR sender_id = ?3)
                        "#,
                        rusqlite::params![start_ts, end_ts, member_filter],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get::<_, Option<i64>>(2)?,
                                row.get::<_, Option<i64>>(3)?,
                            ))
                        },
                    )
                    .map_err(|e| CliError::Database(e.to_string()))?;

                let mut stmt = conn
                    .prepare(
                        r#"
                        SELECT
                            msg.sender_id,
                            COALESCE(m.account_name, m.group_nickname, m.platform_id, printf('member_%d', msg.sender_id)),
                            COUNT(*) AS message_count
                        FROM message msg
                        LEFT JOIN member m ON m.id = msg.sender_id
                        WHERE (?1 IS NULL OR msg.ts >= ?1)
                          AND (?2 IS NULL OR msg.ts <= ?2)
                          AND (?3 IS NULL OR msg.sender_id = ?3)
                        GROUP BY msg.sender_id, 2
                        ORDER BY message_count DESC
                        LIMIT 20
                        "#,
                    )
                    .map_err(|e| CliError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map(rusqlite::params![start_ts, end_ts, member_filter], |row| {
                        Ok(serde_json::json!({
                            "senderId": row.get::<_, i64>(0)?,
                            "senderName": row.get::<_, String>(1)?,
                            "messageCount": row.get::<_, i64>(2)?,
                        }))
                    })
                    .map_err(|e| CliError::Database(e.to_string()))?;
                let mut top_members = Vec::new();
                for row in rows {
                    top_members.push(row.map_err(|e| CliError::Database(e.to_string()))?);
                }

                let payload = serde_json::json!({
                    "analysis": "stats",
                    "filters": {
                        "startDate": start_date,
                        "endDate": end_date,
                        "memberId": member_id,
                    },
                    "totalMessages": total_messages,
                    "uniqueSenders": unique_senders,
                    "timeRange": {
                        "minTs": min_ts,
                        "maxTs": max_ts,
                    },
                    "topMembers": top_members,
                });
                print_analysis_result(&payload, &OutputFormat::Text)?;
            }
            AnalysisType::Advanced { analysis, format } => {
                let payload = run_advanced_analysis(&conn, analysis)?;
                print_analysis_result(&payload, format)?;
            }
            AnalysisType::TimeDistribution {
                granularity,
                format,
            } => {
                let payload = run_time_distribution_analysis(&conn, granularity)?;
                print_analysis_result(&payload, format)?;
            }
        }

        Ok(())
    }

    fn handle_import(&self, args: &ImportArgs) -> Result<()> {
        #[cfg(feature = "analysis")]
        {
            use xenobot_analysis::parsers::ParserRegistry;

            if !args.input.exists() {
                return Err(CliError::Argument(format!(
                    "input path not found: {}",
                    args.input.display()
                )));
            }

            let registry = ParserRegistry::new();
            let mut total = 0usize;
            let mut parsed_ok = 0usize;
            let mut parse_failed = 0usize;
            let mut parsed_chats = Vec::new();

            let candidates = if args.input.is_file() {
                vec![args.input.clone()]
            } else {
                collect_candidate_chat_files(&args.input)?
            };

            for path in &candidates {
                total += 1;
                match registry.detect_and_parse(path) {
                    Ok(chat) => {
                        parsed_ok += 1;
                        println!(
                            "[ok] {} -> platform={} chat={} messages={}",
                            path.to_string_lossy(),
                            chat.platform,
                            chat.chat_name,
                            chat.messages.len()
                        );
                        parsed_chats.push((path.clone(), chat));
                    }
                    Err(err) => {
                        parse_failed += 1;
                        println!("[skip] {} -> {}", path.to_string_lossy(), err);
                    }
                }
            }

            if args.write_db {
                #[cfg(feature = "api")]
                {
                    use xenobot_api::database::{
                        self, ChatMeta, ImportProgress, ImportSourceCheckpoint, Message, Repository,
                    };
                    let mut db_config = xenobot_core::config::DatabaseConfig::default();
                    if let Some(path) = &args.db_path {
                        db_config.sqlite_path = path.clone();
                    }

                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| CliError::Internal(e.to_string()))?;

                    let parsed_chats_for_write = parsed_chats.clone();
                    let session_name_override = args.session_name.clone();
                    let format_hint = args.format;
                    let incremental = args.incremental;
                    let merge = args.merge;
                    let webhook_store = read_webhook_store()?;
                    let webhook_rules: Vec<WebhookRule> = webhook_store
                        .items
                        .iter()
                        .map(webhook_item_to_rule)
                        .collect();
                    let webhook_dispatch =
                        resolve_webhook_dispatch_settings(&webhook_store.dispatch);
                    let import_input = args.input.to_string_lossy().to_string();
                    let total_messages = parsed_chats_for_write
                        .iter()
                        .map(|(_, chat)| chat.messages.len() as i64)
                        .sum::<i64>()
                        .min(i64::from(i32::MAX)) as i32;

                    let (
                        payloads_processed,
                        session_targets,
                        inserted_messages,
                        skipped_duplicates,
                        source_checkpoints_skipped,
                        source_checkpoints_updated,
                        processed_messages,
                        import_progress_id,
                        webhook_attempted,
                        webhook_delivered,
                        webhook_failed,
                        webhook_filtered,
                    ) = runtime.block_on(async move {
                        database::init_database_with_config(&db_config)
                            .await
                            .map_err(|e| CliError::Database(e.to_string()))?;
                        let pool = database::get_pool()
                            .await
                            .map_err(|e| CliError::Database(e.to_string()))?;
                        let repo = Repository::new(pool);
                        let progress = ImportProgress {
                            id: 0,
                            file_path: import_input,
                            total_messages: Some(total_messages),
                            processed_messages: Some(0),
                            status: Some("pending".to_string()),
                            started_at: Some(current_unix_ts()),
                            completed_at: None,
                            error_message: None,
                        };
                        let progress_id = repo
                            .create_import_progress(&progress)
                            .await
                            .map_err(|e| CliError::Database(e.to_string()))?;
                        repo.update_progress(progress_id, 0, "importing")
                            .await
                            .map_err(|e| CliError::Database(e.to_string()))?;

                        let mut payloads_processed = 0usize;
                        let mut inserted_messages = 0usize;
                        let mut skipped_duplicates = 0usize;
                        let mut source_checkpoints_skipped = 0usize;
                        let mut source_checkpoints_updated = 0usize;
                        let mut processed_messages = 0i32;
                        let mut webhook_attempted = 0usize;
                        let mut webhook_delivered = 0usize;
                        let mut webhook_failed = 0usize;
                        let mut webhook_filtered = 0usize;
                        #[derive(Debug, Clone)]
                        struct ActiveSourceContext {
                            source_path: String,
                            source_fingerprint: SourceFileFingerprint,
                            platform: String,
                            chat_name: String,
                            meta_id: Option<i64>,
                        }
                        let mut active_source_context: Option<ActiveSourceContext> = None;
                        let webhook_worker = if webhook_rules.is_empty() {
                            None
                        } else {
                            let client = reqwest::Client::builder()
                                .timeout(std::time::Duration::from_millis(
                                    webhook_dispatch.request_timeout_ms,
                                ))
                                .build()
                                .map_err(|e| CliError::Network(e.to_string()))?;
                            Some(spawn_webhook_dispatch_worker(
                                client,
                                webhook_rules.clone(),
                                webhook_dispatch,
                            ))
                        };
                        let mut run_scope_session_ids: std::collections::HashMap<String, i64> =
                            std::collections::HashMap::new();
                        let mut platform_chat_meta_cache: std::collections::HashMap<
                            String,
                            std::collections::HashMap<String, i64>,
                        > = std::collections::HashMap::new();

                        let write_result = async {
                            for (path, chat) in parsed_chats_for_write {
                                let platform = if chat.platform.trim().is_empty() {
                                    platform_format_id(format_hint).to_string()
                                } else {
                                    chat.platform.trim().to_ascii_lowercase()
                                };
                                let chat_name = if merge {
                                    session_name_override
                                        .clone()
                                        .unwrap_or_else(|| "Merged Import".to_string())
                                } else {
                                    session_name_override
                                        .clone()
                                        .unwrap_or_else(|| chat.chat_name.clone())
                                };
                                let session_key = format!("{}::{}", platform, chat_name);
                                let source_path = path.to_string_lossy().to_string();
                                let source_fingerprint = build_source_file_fingerprint(&path)?;
                                active_source_context = Some(ActiveSourceContext {
                                    source_path: source_path.clone(),
                                    source_fingerprint: source_fingerprint.clone(),
                                    platform: platform.clone(),
                                    chat_name: chat_name.clone(),
                                    meta_id: None,
                                });
                                let existing_checkpoint = repo
                                    .get_import_source_checkpoint("import", &source_path)
                                    .await
                                    .map_err(|e| CliError::Database(e.to_string()))?;
                                if incremental {
                                    if let Some(checkpoint) = existing_checkpoint.as_ref() {
                                        if checkpoint.fingerprint == source_fingerprint.fingerprint
                                            && checkpoint.status == "completed"
                                        {
                                            source_checkpoints_skipped =
                                                source_checkpoints_skipped.saturating_add(1);
                                            processed_messages = processed_messages.saturating_add(
                                                chat.messages.len().min(i32::MAX as usize) as i32,
                                            );
                                            repo.update_progress(
                                                progress_id,
                                                processed_messages,
                                                "importing",
                                            )
                                            .await
                                            .map_err(|e| CliError::Database(e.to_string()))?;
                                            println!(
                                                "[incremental-skip] {} -> unchanged fingerprint={}",
                                                source_path, source_fingerprint.fingerprint
                                            );
                                            continue;
                                        }
                                    }
                                }

                                let existing_meta_id =
                                    if let Some(id) = run_scope_session_ids.get(&session_key) {
                                        Some(*id)
                                    } else if incremental {
                                        if !platform_chat_meta_cache.contains_key(&platform) {
                                            let candidates = repo
                                                .list_chats(Some(&platform), 10_000, 0)
                                                .await
                                                .map_err(|e| CliError::Database(e.to_string()))?;
                                            let mut name_to_meta = std::collections::HashMap::new();
                                            for meta in candidates {
                                                name_to_meta.insert(meta.name, meta.id);
                                            }
                                            platform_chat_meta_cache
                                                .insert(platform.clone(), name_to_meta);
                                        }
                                        platform_chat_meta_cache
                                            .get(&platform)
                                            .and_then(|name_to_meta| name_to_meta.get(&chat_name))
                                            .copied()
                                    } else {
                                        None
                                    };

                                let meta_id = if let Some(id) = existing_meta_id {
                                    id
                                } else {
                                    let chat_type = match chat.chat_type {
                                        xenobot_analysis::parsers::ChatType::Private => {
                                            "private".to_string()
                                        }
                                        xenobot_analysis::parsers::ChatType::Group => {
                                            "group".to_string()
                                        }
                                    };
                                    let meta = ChatMeta {
                                        id: 0,
                                        name: chat_name.clone(),
                                        platform: platform.clone(),
                                        chat_type,
                                        imported_at: current_unix_ts(),
                                        group_id: None,
                                        group_avatar: None,
                                        owner_id: None,
                                        schema_version: 3,
                                        session_gap_threshold: 1800,
                                    };
                                    repo.create_chat(&meta)
                                        .await
                                        .map_err(|e| CliError::Database(e.to_string()))?
                                };
                                if let Some(ctx) = active_source_context.as_mut() {
                                    ctx.meta_id = Some(meta_id);
                                }
                                run_scope_session_ids.insert(session_key, meta_id);
                                platform_chat_meta_cache
                                    .entry(platform.clone())
                                    .or_default()
                                    .insert(chat_name.clone(), meta_id);

                                payloads_processed += 1;
                                let inserted_before = inserted_messages;
                                let duplicates_before = skipped_duplicates;
                                let mut dedup_in_batch: std::collections::HashSet<String> =
                                    std::collections::HashSet::with_capacity(
                                        chat.messages.len().saturating_mul(2).min(262_144),
                                    );

                                for msg in chat.messages {
                                    processed_messages = processed_messages.saturating_add(1);
                                    if msg.timestamp <= 0 {
                                        continue;
                                    }
                                    let sender_platform_id = if msg.sender.trim().is_empty() {
                                        format!("{}:unknown", platform)
                                    } else {
                                        format!("{}:{}", platform, msg.sender.trim())
                                    };
                                    let sender_name = msg
                                        .sender_name
                                        .clone()
                                        .or_else(|| Some(msg.sender.clone()));
                                    let member_id = repo
                                        .get_or_create_member(
                                            &sender_platform_id,
                                            sender_name.as_deref(),
                                        )
                                        .await
                                        .map_err(|e| CliError::Database(e.to_string()))?;

                                    let msg_type_code = parser_message_type_to_code(&msg.msg_type);
                                    let normalized_content = normalize_content(msg.content);
                                    let dedup_sig = format!(
                                        "{}:{}:{}:{}",
                                        member_id,
                                        msg.timestamp,
                                        msg_type_code,
                                        normalized_content.as_deref().unwrap_or_default()
                                    );
                                    if !dedup_in_batch.insert(dedup_sig) {
                                        skipped_duplicates += 1;
                                        continue;
                                    }

                                    if incremental {
                                        let exists = repo
                                            .message_exists(
                                                meta_id,
                                                member_id,
                                                msg.timestamp,
                                                msg_type_code,
                                                normalized_content.as_deref(),
                                            )
                                            .await
                                            .map_err(|e| CliError::Database(e.to_string()))?;
                                        if exists {
                                            skipped_duplicates += 1;
                                            continue;
                                        }
                                    }

                                    let row = Message {
                                        id: 0,
                                        sender_id: member_id,
                                        sender_account_name: sender_name.clone(),
                                        sender_group_nickname: None,
                                        ts: msg.timestamp,
                                        msg_type: msg_type_code,
                                        content: normalized_content.clone(),
                                        reply_to_message_id: None,
                                        platform_message_id: None,
                                        meta_id,
                                    };
                                    let inserted_message_id = repo
                                        .create_message(&row)
                                        .await
                                        .map_err(|e| CliError::Database(e.to_string()))?;
                                    inserted_messages += 1;

                                    if let Some(worker) = webhook_worker.as_ref() {
                                        let event = WebhookMessageCreatedEvent {
                                            event_type: "message.created".to_string(),
                                            platform: platform.clone(),
                                            chat_name: chat_name.clone(),
                                            meta_id,
                                            message_id: inserted_message_id,
                                            sender_id: member_id,
                                            sender_name: sender_name.clone(),
                                            ts: msg.timestamp,
                                            msg_type: msg_type_code,
                                            content: normalized_content.clone(),
                                        };
                                        if worker.send(event).await.is_err() {
                                            webhook_failed = webhook_failed.saturating_add(1);
                                        }
                                    }
                                }

                                let inserted_delta =
                                    inserted_messages.saturating_sub(inserted_before);
                                let duplicate_delta =
                                    skipped_duplicates.saturating_sub(duplicates_before);
                                repo.upsert_import_source_checkpoint(&ImportSourceCheckpoint {
                                    id: existing_checkpoint.as_ref().map(|v| v.id).unwrap_or(0),
                                    source_kind: "import".to_string(),
                                    source_path: source_path.clone(),
                                    fingerprint: source_fingerprint.fingerprint.clone(),
                                    file_size: source_fingerprint.file_size,
                                    modified_at: source_fingerprint.modified_at,
                                    platform: Some(platform.clone()),
                                    chat_name: Some(chat_name.clone()),
                                    meta_id: Some(meta_id),
                                    last_processed_at: current_unix_ts(),
                                    last_inserted_messages: inserted_delta as i64,
                                    last_duplicate_messages: duplicate_delta as i64,
                                    status: "completed".to_string(),
                                    error_message: None,
                                })
                                .await
                                .map_err(|e| CliError::Database(e.to_string()))?;
                                source_checkpoints_updated =
                                    source_checkpoints_updated.saturating_add(1);
                                active_source_context = None;

                                repo.update_progress(progress_id, processed_messages, "importing")
                                    .await
                                    .map_err(|e| CliError::Database(e.to_string()))?;
                            }

                            if let Some(worker) = webhook_worker {
                                let stats = worker.close_and_wait().await;
                                webhook_attempted += stats.attempted;
                                webhook_delivered += stats.delivered;
                                webhook_failed += stats.failed;
                                webhook_filtered += stats.filtered;
                            }
                            Ok::<(), CliError>(())
                        }
                        .await;

                        match write_result {
                            Ok(()) => {
                                repo.update_progress(progress_id, processed_messages, "importing")
                                    .await
                                    .map_err(|e| CliError::Database(e.to_string()))?;
                                repo.complete_import(progress_id, current_unix_ts())
                                    .await
                                    .map_err(|e| CliError::Database(e.to_string()))?;
                            }
                            Err(err) => {
                                if let Some(ctx) = active_source_context.take() {
                                    let _ = repo
                                        .upsert_import_source_checkpoint(&ImportSourceCheckpoint {
                                            id: 0,
                                            source_kind: "import".to_string(),
                                            source_path: ctx.source_path,
                                            fingerprint: ctx.source_fingerprint.fingerprint,
                                            file_size: ctx.source_fingerprint.file_size,
                                            modified_at: ctx.source_fingerprint.modified_at,
                                            platform: Some(ctx.platform),
                                            chat_name: Some(ctx.chat_name),
                                            meta_id: ctx.meta_id,
                                            last_processed_at: current_unix_ts(),
                                            last_inserted_messages: 0,
                                            last_duplicate_messages: 0,
                                            status: "failed".to_string(),
                                            error_message: Some(err.to_string()),
                                        })
                                        .await;
                                }
                                let _ = repo
                                    .update_progress(progress_id, processed_messages, "failed")
                                    .await;
                                let _ = repo.fail_import(progress_id, &err.to_string()).await;
                                return Err(err);
                            }
                        };

                        Ok::<
                            (
                                usize,
                                usize,
                                usize,
                                usize,
                                usize,
                                usize,
                                i32,
                                i64,
                                usize,
                                usize,
                                usize,
                                usize,
                            ),
                            CliError,
                        >((
                            payloads_processed,
                            run_scope_session_ids.len(),
                            inserted_messages,
                            skipped_duplicates,
                            source_checkpoints_skipped,
                            source_checkpoints_updated,
                            processed_messages,
                            progress_id,
                            webhook_attempted,
                            webhook_delivered,
                            webhook_failed,
                            webhook_filtered,
                        ))
                    })?;
                    println!("database write summary");
                    println!("import_progress_id: {}", import_progress_id);
                    println!("chat payloads processed: {}", payloads_processed);
                    println!("session targets touched: {}", session_targets);
                    println!("messages processed: {}", processed_messages);
                    println!("messages inserted: {}", inserted_messages);
                    println!("duplicates skipped: {}", skipped_duplicates);
                    println!(
                        "source checkpoints skipped(unchanged): {}",
                        source_checkpoints_skipped
                    );
                    println!("source checkpoints updated: {}", source_checkpoints_updated);
                    println!("webhooks attempted: {}", webhook_attempted);
                    println!("webhooks delivered: {}", webhook_delivered);
                    println!("webhooks failed: {}", webhook_failed);
                    println!("webhooks filtered/skipped: {}", webhook_filtered);
                    println!(
                        "database path: {}",
                        args.db_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| xenobot_api::database::get_db_path()
                                .to_string_lossy()
                                .to_string())
                    );
                }

                #[cfg(not(feature = "api"))]
                {
                    println!("write-db requested but CLI is not built with `api` feature");
                    println!(
                        "try: cargo run -p xenobot-cli --features api,analysis -- import ... --write-db"
                    );
                }
            }

            println!("import parse summary");
            println!("requested format: {}", platform_format_id(args.format));
            println!("input: {}", args.input.to_string_lossy());
            println!("incremental: {}", args.incremental);
            println!("merge: {}", args.merge);
            println!("stream: {}", args.stream);
            println!("write_db: {}", args.write_db);
            println!("candidate files: {}", total);
            println!("parsed successfully: {}", parsed_ok);
            println!("parse failed/skipped: {}", parse_failed);
            if args.write_db {
                println!(
                    "note: basic parser-to-db write path is enabled; advanced normalization/dedicated incremental planners are still in progress"
                );
            } else {
                println!("note: parser preview is completed");
            }
            return Ok(());
        }

        #[cfg(not(feature = "analysis"))]
        {
            println!("import command needs CLI built with analysis feature");
            println!("try: cargo run -p xenobot-cli --features analysis -- import ...");
            println!("requested format: {}", platform_format_id(args.format));
            Ok(())
        }
    }

    fn handle_export(&self, args: &ExportArgs) -> Result<()> {
        let conn = open_sqlite_read_connection(&args.db_path)?;
        let member_filter = parse_optional_member_id(args.member_id.as_deref())?;
        let start_ts = parse_optional_date_start(args.start_date.as_deref())?;
        let end_ts = parse_optional_date_end(args.end_date.as_deref())?;
        let rows = run_export_query(&conn, start_ts, end_ts, member_filter)?;

        let output_path = resolve_export_output_path(&args.output, args.format.clone());
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        write_export_rows(&output_path, args.format.clone(), &rows)?;
        println!("export completed");
        println!("format: {:?}", args.format);
        println!("rows: {}", rows.len());
        println!("output: {}", output_path.to_string_lossy());
        Ok(())
    }

    fn handle_query(&self, args: &QueryArgs) -> Result<()> {
        let conn = open_sqlite_read_connection(&args.db_path)?;

        match &args.query {
            QueryType::Search {
                keyword,
                start_date,
                end_date,
                member_id,
                limit,
                format,
            } => {
                let member_filter = parse_optional_member_id(member_id.as_deref())?;
                let start_ts = parse_optional_date_start(start_date.as_deref())?;
                let end_ts = parse_optional_date_end(end_date.as_deref())?;
                let rows = run_message_search(
                    &conn,
                    keyword,
                    start_ts,
                    end_ts,
                    member_filter,
                    *limit as i64,
                )?;
                print_search_rows(&rows, format)?;
            }
            QueryType::Sql { sql, format } => {
                let (headers, rows) = execute_safe_select_sql(&conn, sql)?;
                print_sql_rows(&headers, &rows, format)?;
            }
            QueryType::Semantic {
                query,
                threshold,
                limit,
                format,
            } => {
                let rows = run_semantic_search(&conn, query, *threshold, *limit as i64)?;
                print_semantic_rows(&rows, format)?;
            }
        }
        Ok(())
    }

    fn handle_account(&self, args: &crate::commands::AccountArgs) -> Result<()> {
        match &args.command {
            AccountCommand::List { details, format } => {
                let store = read_account_store()?;
                let source_items = discover_sources_for_all_platforms();
                let mut grouped: HashMap<String, (usize, usize)> = HashMap::new();
                for item in source_items {
                    let entry = grouped.entry(item.platform_id).or_insert((0, 0));
                    entry.0 += 1;
                    if item.exists && item.readable {
                        entry.1 += 1;
                    }
                }

                if *details {
                    return print_account_details_view(&store, &grouped, format);
                }

                match format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "activeAccountId": store.active_account_id,
                                "registeredCount": store.items.len(),
                                "accounts": store.items,
                            }))
                            .map_err(|e| CliError::Parse(e.to_string()))?
                        );
                    }
                    _ => {
                        println!("registered accounts: {}", store.items.len());
                        println!(
                            "active account: {}",
                            store.active_account_id.as_deref().unwrap_or("none")
                        );
                        for item in &store.items {
                            println!(
                                "- {} | {} | platform={} | data_dir={}",
                                item.id,
                                item.name,
                                item.platform,
                                item.data_dir
                                    .as_ref()
                                    .map(|v| v.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "-".to_string())
                            );
                        }
                    }
                }
                Ok(())
            }
            AccountCommand::Switch { account_id } => {
                let mut store = read_account_store()?;
                let target = account_id.trim();
                if target.is_empty() {
                    return Err(CliError::Argument("account id cannot be empty".to_string()));
                }

                let Some(item) = store.items.iter_mut().find(|item| item.id == target) else {
                    let known = store
                        .items
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(CliError::Argument(format!(
                        "account id not found: {} (known: [{}])",
                        target, known
                    )));
                };

                item.updated_at = chrono::Utc::now().to_rfc3339();
                let switched_id = item.id.clone();
                let switched_name = item.name.clone();
                let switched_platform = item.platform.clone();
                store.active_account_id = Some(switched_id.clone());
                write_account_store(&store)?;

                println!("active account switched");
                println!("id: {}", switched_id);
                println!("name: {}", switched_name);
                println!("platform: {}", switched_platform);
                Ok(())
            }
            AccountCommand::Add {
                name,
                data_dir,
                format,
                wechat_version,
            } => {
                let mut store = read_account_store()?;
                let runtime_platform = runtime_platform_from_format(*format);
                let platform = platform_format_id(*format).to_string();
                let existing_ids = store
                    .items
                    .iter()
                    .map(|item| item.id.clone())
                    .collect::<std::collections::HashSet<_>>();
                let account_id = allocate_account_id(&platform, name, &existing_ids)?;
                let resolved_data_dir = data_dir.as_ref().cloned().or_else(|| {
                    first_existing_path(&discover_sources_for_platform(&runtime_platform))
                });
                let now = chrono::Utc::now().to_rfc3339();
                let profile = StoredAccountProfile {
                    id: account_id.clone(),
                    name: name.trim().to_string(),
                    platform: platform.clone(),
                    data_dir: resolved_data_dir,
                    wechat_version: wechat_version.to_string(),
                    created_at: now.clone(),
                    updated_at: now,
                };

                store.items.push(profile.clone());
                if store.active_account_id.is_none() {
                    store.active_account_id = Some(account_id.clone());
                }
                write_account_store(&store)?;

                println!("account registered");
                println!("id: {}", profile.id);
                println!("name: {}", profile.name);
                println!("platform: {}", profile.platform);
                println!("version hint: {}", profile.wechat_version);
                println!(
                    "data dir: {}",
                    profile
                        .data_dir
                        .as_ref()
                        .map(|v| v.to_string_lossy().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "active account: {}",
                    store.active_account_id.as_deref().unwrap_or("none")
                );
                Ok(())
            }
        }
    }

    fn handle_webhook(&self, args: &WebhookArgs) -> Result<()> {
        match &args.command {
            WebhookCommand::Add {
                url,
                event_type,
                platform,
                chat_name,
                meta_id,
                sender,
                keyword,
            } => {
                let normalized_url = url.trim();
                let parsed_url = reqwest::Url::parse(normalized_url)
                    .map_err(|e| CliError::Argument(format!("invalid webhook url: {}", e)))?;
                if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
                    return Err(CliError::Argument(
                        "webhook url must use http or https".to_string(),
                    ));
                }
                if meta_id.is_some_and(|id| id <= 0) {
                    return Err(CliError::Argument(
                        "meta-id must be a positive integer".to_string(),
                    ));
                }

                let normalize_filter = |input: &Option<String>| {
                    input
                        .as_ref()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                };
                let normalized_platform = platform
                    .as_ref()
                    .map(|v| v.trim().to_ascii_lowercase())
                    .filter(|v| !v.is_empty());
                let normalized_chat_name = normalize_filter(chat_name);

                let mut store = read_webhook_store()?;
                let id = format!(
                    "wh_{}_{}",
                    chrono::Utc::now().timestamp(),
                    store.items.len() + 1
                );
                let item = WebhookItem {
                    id: id.clone(),
                    url: normalized_url.to_string(),
                    event_type: normalize_filter(event_type),
                    platform: normalized_platform,
                    chat_name: normalized_chat_name,
                    meta_id: *meta_id,
                    sender: normalize_filter(sender),
                    keyword: normalize_filter(keyword),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                store.items.push(item.clone());
                write_webhook_store(&store)?;
                println!("webhook added");
                println!("id: {}", id);
                println!("url: {}", item.url);
                println!(
                    "filters: event={} platform={} chat={} meta_id={} sender={} keyword={}",
                    item.event_type.as_deref().unwrap_or("-"),
                    item.platform.as_deref().unwrap_or("-"),
                    item.chat_name.as_deref().unwrap_or("-"),
                    item.meta_id
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    item.sender.as_deref().unwrap_or("-"),
                    item.keyword.as_deref().unwrap_or("-"),
                );
                Ok(())
            }
            WebhookCommand::List { format } => {
                let store = read_webhook_store()?;
                match format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&store.items)
                                .map_err(|e| CliError::Parse(e.to_string()))?
                        );
                    }
                    OutputFormat::Csv => {
                        println!(
                            "id,url,event_type,platform,chat_name,meta_id,sender,keyword,created_at"
                        );
                        for item in store.items {
                            println!(
                                "{},{},{},{},{},{},{},{},{}",
                                csv_escape(&item.id),
                                csv_escape(&item.url),
                                csv_escape(item.event_type.as_deref().unwrap_or_default()),
                                csv_escape(item.platform.as_deref().unwrap_or_default()),
                                csv_escape(item.chat_name.as_deref().unwrap_or_default()),
                                item.meta_id.map(|v| v.to_string()).unwrap_or_default(),
                                csv_escape(item.sender.as_deref().unwrap_or_default()),
                                csv_escape(item.keyword.as_deref().unwrap_or_default()),
                                csv_escape(&item.created_at)
                            );
                        }
                    }
                    _ => {
                        if store.items.is_empty() {
                            println!("no webhook configured");
                            return Ok(());
                        }
                        println!("configured webhooks");
                        for item in store.items {
                            println!(
                                "- {} | {} | event={} platform={} chat={} meta_id={} sender={} keyword={} created_at={}",
                                item.id,
                                item.url,
                                item.event_type.unwrap_or_else(|| "-".to_string()),
                                item.platform.unwrap_or_else(|| "-".to_string()),
                                item.chat_name.unwrap_or_else(|| "-".to_string()),
                                item.meta_id
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                item.sender.unwrap_or_else(|| "-".to_string()),
                                item.keyword.unwrap_or_else(|| "-".to_string()),
                                item.created_at
                            );
                        }
                    }
                }
                Ok(())
            }
            WebhookCommand::Remove { webhook_id } => {
                let mut store = read_webhook_store()?;
                let before = store.items.len();
                store.items.retain(|item| item.id != *webhook_id);
                if store.items.len() == before {
                    return Err(CliError::Argument(format!(
                        "webhook id not found: {}",
                        webhook_id
                    )));
                }
                write_webhook_store(&store)?;
                println!("webhook removed: {}", webhook_id);
                Ok(())
            }
            WebhookCommand::ListFailed { format } => {
                let entries =
                    read_dead_letter_entries().map_err(|e| CliError::FileSystem(e.to_string()))?;
                match format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&entries)
                                .map_err(|e| CliError::Parse(e.to_string()))?
                        );
                    }
                    OutputFormat::Csv => {
                        println!(
                            "id,webhook_id,webhook_url,attempts,first_failed_at,last_failed_at,last_error,event_type,platform,chat_name,message_id,sender_id,ts,msg_type,content"
                        );
                        for entry in entries {
                            println!(
                                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                                csv_escape(&entry.id),
                                csv_escape(&entry.webhook_id),
                                csv_escape(&entry.webhook_url),
                                entry.attempts,
                                entry.first_failed_at,
                                entry.last_failed_at,
                                csv_escape(&entry.last_error),
                                csv_escape(&entry.event.event_type),
                                csv_escape(&entry.event.platform),
                                csv_escape(&entry.event.chat_name),
                                entry.event.message_id,
                                entry.event.sender_id,
                                entry.event.ts,
                                entry.event.msg_type,
                                csv_escape(entry.event.content.as_deref().unwrap_or_default())
                            );
                        }
                    }
                    _ => {
                        if entries.is_empty() {
                            println!("no webhook dead-letter entries");
                            return Ok(());
                        }
                        println!("webhook dead-letter entries");
                        for entry in entries {
                            println!(
                                "- {} | webhook={}({}) | attempts={} | last_error={} | event={} platform={} chat={} message_id={}",
                                entry.id,
                                entry.webhook_id,
                                entry.webhook_url,
                                entry.attempts,
                                entry.last_error,
                                entry.event.event_type,
                                entry.event.platform,
                                entry.event.chat_name,
                                entry.event.message_id
                            );
                        }
                    }
                }
                Ok(())
            }
            WebhookCommand::RetryFailed { limit } => {
                let entries =
                    read_dead_letter_entries().map_err(|e| CliError::FileSystem(e.to_string()))?;
                if entries.is_empty() {
                    println!("no webhook dead-letter entries");
                    return Ok(());
                }
                let webhook_dispatch =
                    resolve_webhook_dispatch_settings(&read_webhook_store()?.dispatch);

                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| CliError::Internal(e.to_string()))?;

                let (remaining, retried, delivered, failed) = runtime.block_on(async move {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_millis(
                            webhook_dispatch.request_timeout_ms,
                        ))
                        .build()
                        .map_err(|e| CliError::Network(e.to_string()))?;

                    let mut remaining: Vec<WebhookDeadLetterEntry> = Vec::new();
                    let mut retried = 0usize;
                    let mut delivered = 0usize;
                    let mut failed = 0usize;
                    let retry_attempts = webhook_dispatch.retry_attempts.max(1);
                    let retry_base_delay_ms = webhook_dispatch.retry_base_delay_ms.max(1);

                    for mut entry in entries {
                        if retried >= *limit {
                            remaining.push(entry);
                            continue;
                        }
                        retried += 1;

                        let mut ok = false;
                        let mut last_error = String::new();
                        for attempt in 0..retry_attempts {
                            let resp = client
                                .post(&entry.webhook_url)
                                .header("X-Xenobot-Event", &entry.event.event_type)
                                .header("X-Xenobot-Webhook-Id", &entry.webhook_id)
                                .json(&entry.event)
                                .send()
                                .await;
                            match resp {
                                Ok(r) if r.status().is_success() => {
                                    ok = true;
                                    break;
                                }
                                Ok(r) => {
                                    last_error = format!("http status {}", r.status());
                                    if attempt.saturating_add(1) < retry_attempts {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            retry_base_delay_ms
                                                .saturating_mul(1_u64 << attempt.min(10)),
                                        ))
                                        .await;
                                    }
                                }
                                Err(err) => {
                                    last_error = err.to_string();
                                    if attempt.saturating_add(1) < retry_attempts {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            retry_base_delay_ms
                                                .saturating_mul(1_u64 << attempt.min(10)),
                                        ))
                                        .await;
                                    }
                                }
                            }
                        }

                        if ok {
                            delivered += 1;
                        } else {
                            failed += 1;
                            entry.attempts = entry.attempts.saturating_add(1);
                            entry.last_failed_at = chrono::Utc::now().timestamp();
                            if !last_error.is_empty() {
                                entry.last_error = last_error;
                            }
                            remaining.push(entry);
                        }
                    }

                    Ok::<(Vec<WebhookDeadLetterEntry>, usize, usize, usize), CliError>((
                        remaining, retried, delivered, failed,
                    ))
                })?;

                overwrite_dead_letter_entries(&remaining)
                    .map_err(|e| CliError::FileSystem(e.to_string()))?;
                println!("webhook dead-letter retry completed");
                println!("retried: {}", retried);
                println!("delivered: {}", delivered);
                println!("failed: {}", failed);
                println!("remaining: {}", remaining.len());
                Ok(())
            }
            WebhookCommand::ClearFailed => {
                let entries =
                    read_dead_letter_entries().map_err(|e| CliError::FileSystem(e.to_string()))?;
                let count = entries.len();
                overwrite_dead_letter_entries(&[])
                    .map_err(|e| CliError::FileSystem(e.to_string()))?;
                println!("webhook dead-letter queue cleared");
                println!("removed entries: {}", count);
                Ok(())
            }
            WebhookCommand::Dispatch { command } => match command {
                WebhookDispatchCommand::Show { format } => {
                    let store = read_webhook_store()?;
                    let effective = resolve_webhook_dispatch_settings(&store.dispatch);
                    print_webhook_dispatch_settings(&store.dispatch, effective, format)
                }
                WebhookDispatchCommand::Set {
                    reset,
                    batch_size,
                    max_concurrency,
                    request_timeout_ms,
                    flush_interval_ms,
                    retry_attempts,
                    retry_base_delay_ms,
                    format,
                } => {
                    let mut store = read_webhook_store()?;
                    apply_webhook_dispatch_update(
                        &mut store.dispatch,
                        WebhookDispatchUpdate {
                            reset: *reset,
                            batch_size: *batch_size,
                            max_concurrency: *max_concurrency,
                            request_timeout_ms: *request_timeout_ms,
                            flush_interval_ms: *flush_interval_ms,
                            retry_attempts: *retry_attempts,
                            retry_base_delay_ms: *retry_base_delay_ms,
                        },
                    );

                    write_webhook_store(&store)?;
                    let effective = resolve_webhook_dispatch_settings(&store.dispatch);
                    print_webhook_dispatch_settings(&store.dispatch, effective, format)
                }
            },
        }
    }

    fn handle_db(&self, args: &crate::commands::DbArgs) -> Result<()> {
        use crate::commands::DbCommand;

        match &args.command {
            DbCommand::Create {
                path,
                schema_version,
            } => {
                let target = Some(i64::from(*schema_version));
                let applied = apply_migrations_to_path(path, target, true)?;
                println!("database created");
                println!("path: {}", path.to_string_lossy());
                println!("target schema version: {}", schema_version);
                println!("migrations applied: {}", applied);
                Ok(())
            }
            DbCommand::Migrate {
                path,
                target_version,
            } => {
                let target = target_version.map(i64::from);
                let applied = apply_migrations_to_path(path, target, false)?;
                println!("database migration completed");
                println!("path: {}", path.to_string_lossy());
                println!(
                    "target: {}",
                    target
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "latest".to_string())
                );
                println!("migrations applied: {}", applied);
                Ok(())
            }
            DbCommand::Info { path, format } => {
                let conn = open_sqlite_read_connection(path)?;
                let info = collect_db_info(path, &conn)?;
                print_db_info(&info, format)
            }
            DbCommand::Optimize { path } => {
                let conn = open_sqlite_rw_connection(path, false)?;
                conn.execute_batch("PRAGMA optimize; ANALYZE; VACUUM;")
                    .map_err(|e| CliError::Database(e.to_string()))?;
                println!("database optimize completed");
                println!("path: {}", path.to_string_lossy());
                Ok(())
            }
            DbCommand::Verify {
                path,
                format,
                strict,
            } => {
                let conn = open_sqlite_read_connection(path)?;
                let report = collect_db_verification(path, &conn)?;
                print_db_verification(&report, format)?;
                if *strict && !report.ok {
                    return Err(CliError::Command(format!(
                        "database verification failed: missing required checks ({})",
                        report.missing_required
                    )));
                }
                Ok(())
            }
            DbCommand::Checkpoints {
                path,
                source_kind,
                status,
                limit,
                format,
            } => {
                let conn = open_sqlite_read_connection(path)?;
                let report = collect_db_checkpoints(
                    path,
                    &conn,
                    source_kind.as_deref(),
                    status.as_deref(),
                    *limit,
                )?;
                print_db_checkpoints(&report, format)
            }
            DbCommand::Schema {
                path,
                include_row_count,
                format,
            } => {
                let conn = open_sqlite_read_connection(path)?;
                let report = collect_db_schema(path, &conn, *include_row_count)?;
                print_db_schema(&report, format)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct CliConfigFile {
    verbosity: Option<u8>,
    work_dir: Option<PathBuf>,
}

fn read_cli_config_file(path: &Path) -> Result<CliConfigFile> {
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(CliConfigFile {
            verbosity: None,
            work_dir: None,
        });
    }
    serde_json::from_str(&raw).map_err(|e| {
        CliError::Config(format!(
            "failed to parse config file {} as JSON: {}",
            path.display(),
            e
        ))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKeyProfile {
    data_key: String,
    image_key: String,
    version: String,
    platform: String,
    pid: Option<u32>,
    updated_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct KeyStore {
    profiles: HashMap<String, StoredKeyProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebhookItem {
    id: String,
    url: String,
    #[serde(default, alias = "eventType")]
    event_type: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default, alias = "chatName")]
    chat_name: Option<String>,
    #[serde(default, alias = "metaId")]
    meta_id: Option<i64>,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default, alias = "createdAt")]
    created_at: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct WebhookDispatchSettings {
    #[serde(default, alias = "batchSize")]
    batch_size: Option<usize>,
    #[serde(default, alias = "maxConcurrency")]
    max_concurrency: Option<usize>,
    #[serde(default, alias = "requestTimeoutMs")]
    request_timeout_ms: Option<u64>,
    #[serde(default, alias = "flushIntervalMs")]
    flush_interval_ms: Option<u64>,
    #[serde(default, alias = "retryAttempts")]
    retry_attempts: Option<u32>,
    #[serde(default, alias = "retryBaseDelayMs")]
    retry_base_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedWebhookDispatchSettings {
    batch_size: usize,
    max_concurrency: usize,
    queue_capacity: usize,
    request_timeout_ms: u64,
    flush_interval_ms: u64,
    retry_attempts: u32,
    retry_base_delay_ms: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct WebhookDispatchUpdate {
    reset: bool,
    batch_size: Option<usize>,
    max_concurrency: Option<usize>,
    request_timeout_ms: Option<u64>,
    flush_interval_ms: Option<u64>,
    retry_attempts: Option<u32>,
    retry_base_delay_ms: Option<u64>,
}

fn apply_webhook_dispatch_update(
    target: &mut WebhookDispatchSettings,
    update: WebhookDispatchUpdate,
) {
    if update.reset {
        *target = WebhookDispatchSettings::default();
    }

    if let Some(value) = update.batch_size {
        target.batch_size = Some(value);
    }
    if let Some(value) = update.max_concurrency {
        target.max_concurrency = Some(value);
    }
    if let Some(value) = update.request_timeout_ms {
        target.request_timeout_ms = Some(value);
    }
    if let Some(value) = update.flush_interval_ms {
        target.flush_interval_ms = Some(value);
    }
    if let Some(value) = update.retry_attempts {
        target.retry_attempts = Some(value);
    }
    if let Some(value) = update.retry_base_delay_ms {
        target.retry_base_delay_ms = Some(value);
    }
}

fn resolve_webhook_dispatch_settings(
    settings: &WebhookDispatchSettings,
) -> ResolvedWebhookDispatchSettings {
    let batch_size = settings.batch_size.unwrap_or(64).clamp(1, 512);
    let max_concurrency = settings.max_concurrency.unwrap_or(8).clamp(1, 64);
    let request_timeout_ms = settings
        .request_timeout_ms
        .unwrap_or(8_000)
        .clamp(500, 120_000);
    let flush_interval_ms = settings.flush_interval_ms.unwrap_or(250).clamp(10, 10_000);
    let retry_attempts = settings.retry_attempts.unwrap_or(3).clamp(1, 8);
    let retry_base_delay_ms = settings.retry_base_delay_ms.unwrap_or(150).clamp(10, 5_000);
    let queue_capacity = batch_size
        .saturating_mul(max_concurrency)
        .saturating_mul(4)
        .clamp(32, 8192);

    ResolvedWebhookDispatchSettings {
        batch_size,
        max_concurrency,
        queue_capacity,
        request_timeout_ms,
        flush_interval_ms,
        retry_attempts,
        retry_base_delay_ms,
    }
}

fn print_webhook_dispatch_settings(
    raw: &WebhookDispatchSettings,
    effective: ResolvedWebhookDispatchSettings,
    format: &OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "raw": {
                        "batchSize": raw.batch_size,
                        "maxConcurrency": raw.max_concurrency,
                        "requestTimeoutMs": raw.request_timeout_ms,
                        "flushIntervalMs": raw.flush_interval_ms,
                        "retryAttempts": raw.retry_attempts,
                        "retryBaseDelayMs": raw.retry_base_delay_ms
                    },
                    "effective": {
                        "batchSize": effective.batch_size,
                        "maxConcurrency": effective.max_concurrency,
                        "queueCapacity": effective.queue_capacity,
                        "requestTimeoutMs": effective.request_timeout_ms,
                        "flushIntervalMs": effective.flush_interval_ms,
                        "retryAttempts": effective.retry_attempts,
                        "retryBaseDelayMs": effective.retry_base_delay_ms
                    }
                }))
                .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "batch_size,max_concurrency,queue_capacity,request_timeout_ms,flush_interval_ms,retry_attempts,retry_base_delay_ms"
            );
            println!(
                "{},{},{},{},{},{},{}",
                effective.batch_size,
                effective.max_concurrency,
                effective.queue_capacity,
                effective.request_timeout_ms,
                effective.flush_interval_ms,
                effective.retry_attempts,
                effective.retry_base_delay_ms
            );
        }
        _ => {
            println!("webhook dispatch settings");
            println!("batch size: {}", effective.batch_size);
            println!("max concurrency: {}", effective.max_concurrency);
            println!("queue capacity: {}", effective.queue_capacity);
            println!("request timeout(ms): {}", effective.request_timeout_ms);
            println!("flush interval(ms): {}", effective.flush_interval_ms);
            println!("retry attempts: {}", effective.retry_attempts);
            println!("retry base delay(ms): {}", effective.retry_base_delay_ms);
        }
    }
    Ok(())
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WebhookStore {
    #[serde(default)]
    items: Vec<WebhookItem>,
    #[serde(default)]
    dispatch: WebhookDispatchSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccountProfile {
    id: String,
    name: String,
    platform: String,
    data_dir: Option<PathBuf>,
    wechat_version: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountStore {
    active_account_id: Option<String>,
    items: Vec<StoredAccountProfile>,
}

fn key_store_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("cli_keys.json"))
}

fn read_key_store() -> Result<KeyStore> {
    let path = key_store_path()?;
    if !path.exists() {
        return Ok(KeyStore::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(KeyStore::default());
    }
    serde_json::from_str(&raw).map_err(|e| CliError::Parse(e.to_string()))
}

fn write_key_store(store: &KeyStore) -> Result<()> {
    let path = key_store_path()?;
    let raw = serde_json::to_string_pretty(store).map_err(|e| CliError::Parse(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn account_store_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("accounts.json"))
}

fn read_account_store() -> Result<AccountStore> {
    let path = account_store_path()?;
    if !path.exists() {
        return Ok(AccountStore::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(AccountStore::default());
    }
    serde_json::from_str(&raw).map_err(|e| CliError::Parse(e.to_string()))
}

fn write_account_store(store: &AccountStore) -> Result<()> {
    let path = account_store_path()?;
    let raw = serde_json::to_string_pretty(store).map_err(|e| CliError::Parse(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn webhook_store_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("webhooks.json"))
}

fn read_webhook_store() -> Result<WebhookStore> {
    let path = webhook_store_path()?;
    if !path.exists() {
        return Ok(WebhookStore::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(WebhookStore::default());
    }
    serde_json::from_str(&raw).map_err(|e| CliError::Parse(e.to_string()))
}

fn write_webhook_store(store: &WebhookStore) -> Result<()> {
    let path = webhook_store_path()?;
    let raw = serde_json::to_string_pretty(store).map_err(|e| CliError::Parse(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn slugify_identifier(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            out.push(lowered);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn allocate_account_id(
    platform: &str,
    name: &str,
    existing_ids: &std::collections::HashSet<String>,
) -> Result<String> {
    let platform_slug = slugify_identifier(platform);
    let name_slug = slugify_identifier(name);
    if platform_slug.is_empty() || name_slug.is_empty() {
        return Err(CliError::Argument(
            "account id generation failed: platform/name are empty after normalization".to_string(),
        ));
    }

    let base = format!("{}-{}", platform_slug, name_slug);
    if !existing_ids.contains(&base) {
        return Ok(base);
    }

    for suffix in 2usize..=10_000usize {
        let candidate = format!("{}-{}", base, suffix);
        if !existing_ids.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err(CliError::Internal(
        "unable to allocate unique account id".to_string(),
    ))
}

fn print_account_details_view(
    store: &AccountStore,
    grouped: &HashMap<String, (usize, usize)>,
    format: &OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let mut platform_rows = Vec::new();
            for (platform, (candidates, available)) in grouped {
                platform_rows.push(serde_json::json!({
                    "platform": platform,
                    "candidates": candidates,
                    "available": available,
                }));
            }
            platform_rows.sort_by(|a, b| a["platform"].as_str().cmp(&b["platform"].as_str()));

            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "activeAccountId": store.active_account_id,
                    "accounts": store.items,
                    "platformSourceSummary": platform_rows,
                }))
                .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        _ => {
            println!("account registry details");
            println!(
                "active account: {}",
                store.active_account_id.as_deref().unwrap_or("none")
            );
            if store.items.is_empty() {
                println!("registered accounts: none");
            } else {
                println!("registered accounts");
                for item in &store.items {
                    println!(
                        "- {} | {} | platform={} | version={} | data_dir={} | updated_at={}",
                        item.id,
                        item.name,
                        item.platform,
                        item.wechat_version,
                        item.data_dir
                            .as_ref()
                            .map(|v| v.to_string_lossy().to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        item.updated_at
                    );
                }
            }

            println!("platform source summary");
            let mut rows: Vec<_> = grouped.iter().collect();
            rows.sort_by(|a, b| a.0.cmp(b.0));
            for (platform, (candidates, available)) in rows {
                println!(
                    "- {}: {} available / {} candidates",
                    platform, available, candidates
                );
            }
        }
    }
    Ok(())
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn webhook_item_to_rule(item: &WebhookItem) -> WebhookRule {
    WebhookRule {
        id: item.id.clone(),
        url: item.url.clone(),
        event_type: item.event_type.clone(),
        platform: item.platform.clone(),
        chat_name: item.chat_name.clone(),
        meta_id: item.meta_id,
        sender: item.sender.clone(),
        keyword: item.keyword.clone(),
        created_at: if item.created_at.trim().is_empty() {
            None
        } else {
            Some(item.created_at.clone())
        },
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
struct WebhookDispatchWorker {
    sender: tokio::sync::mpsc::Sender<WebhookMessageCreatedEvent>,
    join_handle: tokio::task::JoinHandle<WebhookDispatchStats>,
}

#[cfg(all(feature = "analysis", feature = "api"))]
impl WebhookDispatchWorker {
    async fn send(&self, event: WebhookMessageCreatedEvent) -> std::result::Result<(), ()> {
        self.sender.send(event).await.map_err(|_| ())
    }

    async fn close_and_wait(self) -> WebhookDispatchStats {
        drop(self.sender);
        match self.join_handle.await {
            Ok(stats) => stats,
            Err(_) => WebhookDispatchStats {
                failed: 1,
                ..WebhookDispatchStats::default()
            },
        }
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn spawn_webhook_dispatch_worker(
    client: reqwest::Client,
    items: Vec<WebhookRule>,
    dispatch: ResolvedWebhookDispatchSettings,
) -> WebhookDispatchWorker {
    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<WebhookMessageCreatedEvent>(dispatch.queue_capacity.max(1));
    let join_handle = tokio::spawn(async move {
        let mut total = WebhookDispatchStats::default();
        let mut buffer = Vec::new();
        let flush_interval = std::time::Duration::from_millis(dispatch.flush_interval_ms.max(1));

        loop {
            match tokio::time::timeout(flush_interval, receiver.recv()).await {
                Ok(Some(event)) => {
                    buffer.push(event);
                    if buffer.len() >= dispatch.batch_size.max(1) {
                        let stats = flush_webhook_queue(
                            &client,
                            items.as_slice(),
                            &mut buffer,
                            dispatch.max_concurrency,
                            dispatch.retry_attempts,
                            dispatch.retry_base_delay_ms,
                        )
                        .await;
                        merge_webhook_dispatch_stats(&mut total, &stats);
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    if !buffer.is_empty() {
                        let stats = flush_webhook_queue(
                            &client,
                            items.as_slice(),
                            &mut buffer,
                            dispatch.max_concurrency,
                            dispatch.retry_attempts,
                            dispatch.retry_base_delay_ms,
                        )
                        .await;
                        merge_webhook_dispatch_stats(&mut total, &stats);
                    }
                }
            }
        }

        if !buffer.is_empty() {
            let stats = flush_webhook_queue(
                &client,
                items.as_slice(),
                &mut buffer,
                dispatch.max_concurrency,
                dispatch.retry_attempts,
                dispatch.retry_base_delay_ms,
            )
            .await;
            merge_webhook_dispatch_stats(&mut total, &stats);
        }

        total
    });

    WebhookDispatchWorker {
        sender,
        join_handle,
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
async fn dispatch_webhook_message_created(
    client: &reqwest::Client,
    items: &[WebhookRule],
    event: &WebhookMessageCreatedEvent,
    retry_attempts: u32,
    retry_base_delay_ms: u64,
) -> WebhookDispatchStats {
    let mut stats = WebhookDispatchStats::default();
    let attempts = retry_attempts.max(1);
    for item in items {
        if !webhook_rule_matches_event(item, event) {
            stats.filtered += 1;
            continue;
        }
        stats.attempted += 1;

        let mut delivered = false;
        let mut attempts_used = 0u32;
        let mut last_error = "unknown delivery failure".to_string();
        for attempt in 0..attempts {
            attempts_used = attempt.saturating_add(1);
            let send_result = client
                .post(&item.url)
                .header("X-Xenobot-Event", &event.event_type)
                .header("X-Xenobot-Webhook-Id", &item.id)
                .json(event)
                .send()
                .await;

            match send_result {
                Ok(resp) if resp.status().is_success() => {
                    stats.delivered += 1;
                    delivered = true;
                    break;
                }
                Ok(resp) => {
                    last_error = format!("http status {}", resp.status());
                    if attempt.saturating_add(1) < attempts {
                        let wait_ms = retry_base_delay_ms.saturating_mul(1_u64 << attempt.min(10));
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                    }
                }
                Err(err) => {
                    last_error = err.to_string();
                    if attempt.saturating_add(1) < attempts {
                        let wait_ms = retry_base_delay_ms.saturating_mul(1_u64 << attempt.min(10));
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                    }
                }
            }
        }

        if !delivered {
            stats.failed += 1;
            let entry = build_dead_letter_entry(item, event, attempts_used, last_error);
            if let Err(err) = append_dead_letter_entry(&entry) {
                eprintln!(
                    "failed to persist webhook dead-letter entry {}: {}",
                    entry.id, err
                );
            }
        }
    }
    stats
}

#[cfg(all(feature = "analysis", feature = "api"))]
async fn flush_webhook_queue(
    client: &reqwest::Client,
    items: &[WebhookRule],
    queue: &mut Vec<WebhookMessageCreatedEvent>,
    max_concurrency: usize,
    retry_attempts: u32,
    retry_base_delay_ms: u64,
) -> WebhookDispatchStats {
    if queue.is_empty() {
        return WebhookDispatchStats::default();
    }

    let mut set = tokio::task::JoinSet::new();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrency.max(1)));
    let shared_items = std::sync::Arc::new(items.to_vec());

    for event in queue.drain(..) {
        let client_clone = client.clone();
        let items_clone = shared_items.clone();
        let semaphore_clone = semaphore.clone();
        let attempts = retry_attempts.max(1);
        let base_delay_ms = retry_base_delay_ms.max(1);
        set.spawn(async move {
            let _permit = semaphore_clone.acquire_owned().await.ok();
            dispatch_webhook_message_created(
                &client_clone,
                items_clone.as_slice(),
                &event,
                attempts,
                base_delay_ms,
            )
            .await
        });
    }

    let mut total = WebhookDispatchStats::default();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(stats) => merge_webhook_dispatch_stats(&mut total, &stats),
            Err(_) => {
                total.failed = total.failed.saturating_add(1);
            }
        }
    }
    total
}

fn normalize_profile_name(profile: &str) -> Result<String> {
    let p = profile.trim();
    if p.is_empty() {
        return Err(CliError::Argument("profile cannot be empty".to_string()));
    }
    Ok(p.to_string())
}

fn normalize_hex_key(raw: &str, expected_len: usize, label: &str) -> Result<String> {
    let mut value = raw.trim().to_ascii_lowercase();
    if let Some(rest) = value.strip_prefix("0x") {
        value = rest.to_string();
    }
    if value.len() != expected_len {
        return Err(CliError::Argument(format!(
            "{} must be {} hex chars, got {}",
            label,
            expected_len,
            value.len()
        )));
    }
    if !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CliError::Argument(format!(
            "{} contains non-hex characters",
            label
        )));
    }
    Ok(value)
}

fn mask_secret(secret: &str) -> String {
    if secret.len() <= 8 {
        return "*".repeat(secret.len());
    }
    format!("{}...{}", &secret[..4], &secret[secret.len() - 4..])
}

fn print_key_profile(
    profile: &StoredKeyProfile,
    profile_name: &str,
    format: &OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "profile": profile_name,
                    "version": profile.version,
                    "platform": profile.platform,
                    "pid": profile.pid,
                    "updatedAt": profile.updated_at,
                    "dataKeyMasked": mask_secret(&profile.data_key),
                    "imageKeyMasked": mask_secret(&profile.image_key),
                }))
                .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        _ => {
            println!("profile: {}", profile_name);
            println!("version: {}", profile.version);
            println!("platform: {}", profile.platform);
            println!(
                "pid: {}",
                profile
                    .pid
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!("updated at: {}", profile.updated_at);
            println!("data key: {}", mask_secret(&profile.data_key));
            println!("image key: {}", mask_secret(&profile.image_key));
        }
    }
    Ok(())
}

fn resolve_keys_for_runtime(
    data_key: Option<&str>,
    image_key: Option<&str>,
    default_profile: Option<&str>,
) -> Result<(String, String)> {
    match (data_key, image_key) {
        (Some(data), Some(image)) => {
            return Ok((data.trim().to_string(), image.trim().to_string()));
        }
        (None, None) => {}
        _ => {
            return Err(CliError::Argument(
                "data_key and image_key must be provided together".to_string(),
            ));
        }
    }

    let profile_name = default_profile.unwrap_or("default");
    let store = read_key_store()?;
    let Some(saved) = store.profiles.get(profile_name) else {
        return Err(CliError::Argument(format!(
            "no runtime keys provided and key profile '{}' not found",
            profile_name
        )));
    };
    Ok((saved.data_key.clone(), saved.image_key.clone()))
}

fn default_wechat_data_dir() -> String {
    if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|home| {
                home.join("Library")
                    .join("Containers")
                    .join("com.tencent.xinWeChat")
                    .join("Data")
                    .join("Library")
                    .join("Application Support")
                    .join("com.tencent.xinWeChat")
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn runtime_platform_from_format(format: PlatformFormat) -> RuntimePlatform {
    match format {
        PlatformFormat::WeChat => RuntimePlatform::WeChat,
        PlatformFormat::WhatsApp => RuntimePlatform::WhatsApp,
        PlatformFormat::Line => RuntimePlatform::Line,
        PlatformFormat::Qq => RuntimePlatform::Qq,
        PlatformFormat::Discord => RuntimePlatform::Discord,
        PlatformFormat::Telegram => RuntimePlatform::Telegram,
        PlatformFormat::Instagram => RuntimePlatform::Instagram,
        PlatformFormat::IMessage => RuntimePlatform::IMessage,
        PlatformFormat::Messenger => RuntimePlatform::Messenger,
        PlatformFormat::KakaoTalk => RuntimePlatform::KakaoTalk,
        PlatformFormat::Slack => RuntimePlatform::Slack,
        PlatformFormat::Teams => RuntimePlatform::Teams,
        PlatformFormat::Signal => RuntimePlatform::Signal,
        PlatformFormat::Skype => RuntimePlatform::Custom("skype".to_string()),
        PlatformFormat::GoogleChat => RuntimePlatform::Custom("googlechat".to_string()),
        PlatformFormat::Zoom => RuntimePlatform::Custom("zoom".to_string()),
        PlatformFormat::Viber => RuntimePlatform::Custom("viber".to_string()),
        PlatformFormat::Xenobot => RuntimePlatform::Custom("xenobot".to_string()),
    }
}

fn platform_format_id(format: PlatformFormat) -> &'static str {
    match format {
        PlatformFormat::WeChat => "wechat",
        PlatformFormat::WhatsApp => "whatsapp",
        PlatformFormat::Line => "line",
        PlatformFormat::Qq => "qq",
        PlatformFormat::Discord => "discord",
        PlatformFormat::Telegram => "telegram",
        PlatformFormat::Instagram => "instagram",
        PlatformFormat::IMessage => "imessage",
        PlatformFormat::Messenger => "messenger",
        PlatformFormat::KakaoTalk => "kakaotalk",
        PlatformFormat::Slack => "slack",
        PlatformFormat::Teams => "teams",
        PlatformFormat::Signal => "signal",
        PlatformFormat::Skype => "skype",
        PlatformFormat::GoogleChat => "googlechat",
        PlatformFormat::Zoom => "zoom",
        PlatformFormat::Viber => "viber",
        PlatformFormat::Xenobot => "xenobot",
    }
}

fn first_existing_path(candidates: &[SourceCandidate]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|item| item.exists && item.readable)
        .map(|item| item.path.clone())
}

fn print_source_candidates(
    candidates: &[SourceCandidate],
    existing_only: bool,
    format: &OutputFormat,
) -> Result<()> {
    let filtered: Vec<&SourceCandidate> = candidates
        .iter()
        .filter(|item| !existing_only || item.exists)
        .collect();

    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&filtered)
                    .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        _ => {
            if filtered.is_empty() {
                println!("no source candidate matched current filter");
                return Ok(());
            }
            println!("source scan result");
            for item in filtered {
                println!(
                    "- [{}] {} | kind={} | exists={} readable={} | {}",
                    core_platform_id(&item.platform),
                    item.label,
                    serde_json::to_string(&item.kind).unwrap_or_else(|_| "\"unknown\"".to_string()),
                    item.exists,
                    item.readable,
                    item.path.to_string_lossy()
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct SourceCoverageRow {
    platform_id: String,
    total_candidates: usize,
    existing_candidates: usize,
    readable_candidates: usize,
}

fn build_source_platform_matrix_rows() -> Vec<SourceCoverageRow> {
    let mut rows = Vec::new();
    for platform in legal_safe_runtime_platforms() {
        let candidates = discover_sources_for_platform(&platform);
        let existing = candidates.iter().filter(|item| item.exists).count();
        let readable = candidates
            .iter()
            .filter(|item| item.exists && item.readable)
            .count();

        rows.push(SourceCoverageRow {
            platform_id: core_platform_id(&platform).to_string(),
            total_candidates: candidates.len(),
            existing_candidates: existing,
            readable_candidates: readable,
        });
    }
    rows.sort_by(|a, b| a.platform_id.cmp(&b.platform_id));
    rows
}

fn print_source_platform_matrix(format: &OutputFormat) -> Result<()> {
    let rows = build_source_platform_matrix_rows();

    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        _ => {
            println!("source coverage matrix (legal-safe runtime platforms)");
            for row in rows {
                println!(
                    "- {} | total={} existing={} readable={}",
                    row.platform_id,
                    row.total_candidates,
                    row.existing_candidates,
                    row.readable_candidates
                );
            }
        }
    }

    Ok(())
}

#[cfg(feature = "analysis")]
fn collect_candidate_chat_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_supported_chat_file(&path) {
                out.push(path);
            }
        }
    }

    Ok(out)
}

#[cfg(feature = "analysis")]
fn run_legal_safe_decrypt_stage(
    input_path: &Path,
    work_dir: &Path,
    overwrite: bool,
    format_hint: PlatformFormat,
) -> Result<()> {
    use xenobot_analysis::parsers::ParserRegistry;

    if !input_path.exists() {
        return Err(CliError::Argument(format!(
            "data path not found: {}",
            input_path.display()
        )));
    }

    let mut candidates = if input_path.is_file() {
        vec![input_path.to_path_buf()]
    } else {
        collect_candidate_chat_files(input_path)?
    };
    candidates.sort();

    if candidates.is_empty() {
        println!(
            "no candidate chat files found under {}",
            input_path.to_string_lossy()
        );
        return Ok(());
    }

    let registry = ParserRegistry::new();
    let stage_root = work_dir
        .join("stage")
        .join(platform_format_id(format_hint).to_string());
    fs::create_dir_all(&stage_root)?;

    let mut processed = 0usize;
    let mut staged = 0usize;
    let mut parse_failed = 0usize;
    let mut skipped_existing = 0usize;
    let mut skipped_platform = 0usize;
    let expected_platform = platform_format_id(format_hint);

    for path in candidates {
        processed = processed.saturating_add(1);
        match registry.detect_and_parse(&path) {
            Ok(chat) => {
                let parsed_platform = chat.platform.trim().to_ascii_lowercase();
                if expected_platform != "xenobot" && parsed_platform != expected_platform {
                    skipped_platform = skipped_platform.saturating_add(1);
                    println!(
                        "[skip] {} -> parsed platform={} expected={}",
                        path.display(),
                        parsed_platform,
                        expected_platform
                    );
                    continue;
                }

                let path_hash = short_path_hash(&path);
                let file_name = format!(
                    "{}_{}.json",
                    sanitize_file_component(&chat.chat_name),
                    path_hash
                );
                let output_path = stage_root.join(file_name);
                if output_path.exists() && !overwrite {
                    skipped_existing = skipped_existing.saturating_add(1);
                    println!(
                        "[skip] {} -> staged file exists (use --overwrite): {}",
                        path.display(),
                        output_path.display()
                    );
                    continue;
                }

                let payload = serde_json::json!({
                    "sourcePath": path.to_string_lossy().to_string(),
                    "sourcePlatformHint": expected_platform,
                    "parsedPlatform": chat.platform,
                    "chatName": chat.chat_name,
                    "chatType": chat.chat_type,
                    "members": chat.members,
                    "messages": chat.messages,
                    "stagedAt": chrono::Utc::now().to_rfc3339(),
                });
                let raw = serde_json::to_vec_pretty(&payload)
                    .map_err(|e| CliError::Parse(e.to_string()))?;
                fs::write(&output_path, raw)?;
                staged = staged.saturating_add(1);
                println!(
                    "[ok] {} -> staged {}",
                    path.display(),
                    output_path.display()
                );
            }
            Err(err) => {
                parse_failed = parse_failed.saturating_add(1);
                println!("[skip] {} -> {}", path.display(), err);
            }
        }
    }

    println!("decrypt staging summary");
    println!("processed files: {}", processed);
    println!("staged files: {}", staged);
    println!("parse failed: {}", parse_failed);
    println!("platform skipped: {}", skipped_platform);
    println!("existing skipped: {}", skipped_existing);
    println!("stage dir: {}", stage_root.display());
    Ok(())
}

#[cfg(feature = "analysis")]
fn sanitize_file_component(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let collapsed = out.trim_matches('-');
    if collapsed.is_empty() {
        "chat".to_string()
    } else {
        collapsed.to_string()
    }
}

#[cfg(feature = "analysis")]
fn short_path_hash(path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    let value = hasher.finish();
    format!("{:08x}", (value & 0xffff_ffff) as u32)
}

#[cfg(feature = "analysis")]
fn run_legal_safe_monitor_loop(
    runtime_platform: &RuntimePlatform,
    watch_path: &Path,
    interval_seconds: u64,
    write_db: bool,
    db_path: Option<PathBuf>,
    format_hint: PlatformFormat,
) -> Result<()> {
    use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use xenobot_analysis::parsers::ParserRegistry;

    fn file_state(path: &Path) -> Result<(u64, u64)> {
        let meta = std::fs::metadata(path)?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|ts: SystemTime| ts.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok((meta.len(), modified))
    }

    fn resolve_watch_target(path: &Path) -> Option<(PathBuf, RecursiveMode)> {
        if path.is_file() {
            return path
                .parent()
                .map(|parent| (parent.to_path_buf(), RecursiveMode::NonRecursive));
        }
        if path.is_dir() {
            return Some((path.to_path_buf(), RecursiveMode::Recursive));
        }
        let mut ancestor = path.to_path_buf();
        while !ancestor.exists() {
            if !ancestor.pop() {
                return None;
            }
        }
        if ancestor.parent().is_none() {
            return None;
        }
        let mode = if ancestor.is_file() {
            RecursiveMode::NonRecursive
        } else {
            RecursiveMode::Recursive
        };
        Some((ancestor, mode))
    }

    fn event_kind_requires_rescan(kind: &EventKind) -> bool {
        matches!(
            kind,
            EventKind::Any
                | EventKind::Create(_)
                | EventKind::Modify(_)
                | EventKind::Remove(_)
                | EventKind::Other
        )
    }

    let mut state_map: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    let parser_registry = ParserRegistry::new();
    let target_platform = core_platform_id(runtime_platform).to_string();
    let mut announced_empty = false;
    let scan_interval = Duration::from_secs(interval_seconds.max(1));
    #[cfg(all(feature = "analysis", feature = "api"))]
    let checkpoint_db_path = if write_db {
        Some(
            db_path
                .clone()
                .unwrap_or_else(xenobot_api::database::get_db_path),
        )
    } else {
        None
    };

    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher: Option<RecommendedWatcher> = None;
    if let Some((target, mode)) = resolve_watch_target(watch_path) {
        match notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        }) {
            Ok(mut watch_impl) => match watch_impl.watch(&target, mode) {
                Ok(()) => {
                    println!(
                        "[monitor] watcher enabled: target={} mode={:?}",
                        target.display(),
                        mode
                    );
                    watcher = Some(watch_impl);
                }
                Err(err) => {
                    println!(
                        "[monitor] watcher setup failed for {}: {}, using polling fallback",
                        target.display(),
                        err
                    );
                }
            },
            Err(err) => {
                println!(
                    "[monitor] watcher initialization failed: {}, using polling fallback",
                    err
                );
            }
        }
    } else {
        println!(
            "[monitor] no valid watcher target for {}, using polling fallback",
            watch_path.display()
        );
    }

    loop {
        let mut should_scan = watcher.is_none();
        if let Some(rx) = watcher.as_ref().map(|_| &event_rx) {
            match rx.recv_timeout(scan_interval) {
                Ok(Ok(event)) => {
                    let mut batch_count = 1usize;
                    let mut path_count = event.paths.len();
                    let mut requires_rescan = event_kind_requires_rescan(&event.kind);

                    while let Ok(next) = rx.try_recv() {
                        batch_count = batch_count.saturating_add(1);
                        match next {
                            Ok(ev) => {
                                path_count = path_count.saturating_add(ev.paths.len());
                                if event_kind_requires_rescan(&ev.kind) {
                                    requires_rescan = true;
                                }
                            }
                            Err(err) => {
                                println!("[monitor] watcher event error: {}", err);
                                requires_rescan = true;
                            }
                        }
                        if batch_count >= 256 {
                            break;
                        }
                    }

                    if requires_rescan {
                        println!(
                            "[monitor] fs events received: events={} paths={} -> scanning",
                            batch_count, path_count
                        );
                        should_scan = true;
                    }
                }
                Ok(Err(err)) => {
                    println!("[monitor] watcher event error: {}", err);
                    should_scan = true;
                }
                Err(RecvTimeoutError::Timeout) => {
                    should_scan = true;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    println!(
                        "[monitor] watcher channel disconnected, switching to polling fallback"
                    );
                    watcher = None;
                    should_scan = true;
                }
            }
        } else {
            std::thread::sleep(scan_interval);
        }

        if !should_scan {
            continue;
        }

        let mut candidates = Vec::new();
        if watch_path.is_file() {
            candidates.push(watch_path.to_path_buf());
        } else if watch_path.exists() {
            candidates = collect_candidate_chat_files(watch_path)?;
        }

        let candidate_set: std::collections::HashSet<PathBuf> =
            candidates.iter().cloned().collect();
        state_map.retain(|path, _| candidate_set.contains(path));

        if candidates.is_empty() {
            if !announced_empty {
                println!(
                    "[monitor] no candidate chat files under {}",
                    watch_path.display()
                );
                announced_empty = true;
            }
        } else {
            announced_empty = false;
        }

        candidates.sort();
        for path in candidates {
            let state = match file_state(&path) {
                Ok(v) => v,
                Err(err) => {
                    println!("[skip] {} -> {}", path.display(), err);
                    continue;
                }
            };

            let changed = state_map.get(&path).copied() != Some(state);
            if !changed {
                continue;
            }
            state_map.insert(path.clone(), state);

            #[cfg(all(feature = "analysis", feature = "api"))]
            if write_db {
                if let Some(db_path) = checkpoint_db_path.as_ref() {
                    if let Ok(source_fp) = build_source_file_fingerprint(&path) {
                        if monitor_source_checkpoint_unchanged(
                            db_path,
                            &path,
                            &source_fp.fingerprint,
                        ) {
                            println!(
                                "[skip] {} -> unchanged checkpoint fingerprint={}",
                                path.display(),
                                source_fp.fingerprint
                            );
                            continue;
                        }
                    }
                }
            }

            match parser_registry.detect_and_parse(&path) {
                Ok(chat) => {
                    let parsed_platform = chat.platform.to_ascii_lowercase();
                    if parsed_platform != target_platform {
                        println!(
                            "[skip] {} -> parsed platform={} differs from monitor target={}",
                            path.display(),
                            parsed_platform,
                            target_platform
                        );
                        continue;
                    }

                    println!(
                        "[update] {} -> platform={} chat={} messages={}",
                        path.display(),
                        chat.platform,
                        chat.chat_name,
                        chat.messages.len()
                    );

                    if write_db {
                        #[cfg(feature = "api")]
                        {
                            let summary = persist_monitor_chat_to_db(
                                &path,
                                chat,
                                db_path.as_ref(),
                                format_hint,
                            )?;
                            println!(
                                "[db] {} -> meta_id={} processed={} inserted={} duplicates={} checkpoint_skipped={} webhooks(delivered/failed/filtered)={}/{}/{}",
                                path.display(),
                                summary.meta_id,
                                summary.processed_messages,
                                summary.inserted_messages,
                                summary.skipped_duplicates,
                                summary.source_checkpoint_skipped,
                                summary.webhook_delivered,
                                summary.webhook_failed,
                                summary.webhook_filtered
                            );
                        }

                        #[cfg(not(feature = "api"))]
                        {
                            println!(
                                "[db] {} -> skipped (CLI not built with --features api)",
                                path.display()
                            );
                        }
                    }
                }
                Err(err) => {
                    println!("[skip] {} -> {}", path.display(), err);
                }
            }
        }
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
#[derive(Debug, Clone, Default)]
struct MonitorDbWriteSummary {
    meta_id: i64,
    processed_messages: usize,
    inserted_messages: usize,
    skipped_duplicates: usize,
    source_checkpoint_skipped: bool,
    webhook_attempted: usize,
    webhook_delivered: usize,
    webhook_failed: usize,
    webhook_filtered: usize,
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn persist_monitor_chat_to_db(
    source_path: &Path,
    chat: xenobot_analysis::parsers::ParsedChat,
    db_path: Option<&PathBuf>,
    format_hint: PlatformFormat,
) -> Result<MonitorDbWriteSummary> {
    use xenobot_api::database::{self, ChatMeta, ImportSourceCheckpoint, Message, Repository};

    let mut db_config = xenobot_core::config::DatabaseConfig::default();
    if let Some(path) = db_path {
        db_config.sqlite_path = path.clone();
    }

    let webhook_store = read_webhook_store()?;
    let webhook_rules: Vec<WebhookRule> = webhook_store
        .items
        .iter()
        .map(webhook_item_to_rule)
        .collect();
    let webhook_dispatch = resolve_webhook_dispatch_settings(&webhook_store.dispatch);

    let source_hint = source_path.to_string_lossy().to_string();
    let source_fingerprint = build_source_file_fingerprint(source_path)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    runtime.block_on(async move {
        database::init_database_with_config(&db_config)
            .await
            .map_err(|e| CliError::Database(e.to_string()))?;
        let pool = database::get_pool()
            .await
            .map_err(|e| CliError::Database(e.to_string()))?;
        let repo = Repository::new(pool);

        let platform = if chat.platform.trim().is_empty() {
            platform_format_id(format_hint).to_string()
        } else {
            chat.platform.trim().to_ascii_lowercase()
        };
        let chat_name = if chat.chat_name.trim().is_empty() {
            Path::new(&source_hint)
                .file_stem()
                .and_then(|v| v.to_str())
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Monitored Chat".to_string())
        } else {
            chat.chat_name.trim().to_string()
        };

        if let Some(checkpoint) = repo
            .get_import_source_checkpoint("monitor", &source_hint)
            .await
            .map_err(|e| CliError::Database(e.to_string()))?
        {
            if checkpoint.fingerprint == source_fingerprint.fingerprint
                && checkpoint.status == "completed"
            {
                return Ok::<MonitorDbWriteSummary, CliError>(MonitorDbWriteSummary {
                    meta_id: checkpoint.meta_id.unwrap_or_default(),
                    source_checkpoint_skipped: true,
                    ..Default::default()
                });
            }
        }

        let existing_meta = repo
            .list_chats(Some(&platform), 10_000, 0)
            .await
            .map_err(|e| CliError::Database(e.to_string()))?
            .into_iter()
            .find(|meta| meta.name == chat_name)
            .map(|meta| meta.id);

        let meta_id = if let Some(id) = existing_meta {
            id
        } else {
            let chat_type = match chat.chat_type {
                xenobot_analysis::parsers::ChatType::Private => "private".to_string(),
                xenobot_analysis::parsers::ChatType::Group => "group".to_string(),
            };
            let meta = ChatMeta {
                id: 0,
                name: chat_name.clone(),
                platform: platform.clone(),
                chat_type,
                imported_at: current_unix_ts(),
                group_id: None,
                group_avatar: None,
                owner_id: None,
                schema_version: 3,
                session_gap_threshold: 1800,
            };
            repo.create_chat(&meta)
                .await
                .map_err(|e| CliError::Database(e.to_string()))?
        };

        let webhook_worker = if webhook_rules.is_empty() {
            None
        } else {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(
                    webhook_dispatch.request_timeout_ms,
                ))
                .build()
                .map_err(|e| CliError::Network(e.to_string()))?;
            Some(spawn_webhook_dispatch_worker(
                client,
                webhook_rules,
                webhook_dispatch,
            ))
        };

        let mut summary = MonitorDbWriteSummary {
            meta_id,
            ..Default::default()
        };
        let mut dedup_in_batch: std::collections::HashSet<String> =
            std::collections::HashSet::with_capacity(
                chat.messages.len().saturating_mul(2).min(262_144),
            );
        let mut worker = webhook_worker;

        for msg in chat.messages {
            summary.processed_messages = summary.processed_messages.saturating_add(1);
            if msg.timestamp <= 0 {
                continue;
            }
            let sender_platform_id = if msg.sender.trim().is_empty() {
                format!("{}:unknown", platform)
            } else {
                format!("{}:{}", platform, msg.sender.trim())
            };
            let sender_name = msg.sender_name.clone().or_else(|| Some(msg.sender.clone()));
            let member_id = repo
                .get_or_create_member(&sender_platform_id, sender_name.as_deref())
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;

            let msg_type_code = parser_message_type_to_code(&msg.msg_type);
            let normalized_content = normalize_content(msg.content);
            let dedup_sig = format!(
                "{}:{}:{}:{}",
                member_id,
                msg.timestamp,
                msg_type_code,
                normalized_content.as_deref().unwrap_or_default()
            );
            if !dedup_in_batch.insert(dedup_sig) {
                summary.skipped_duplicates = summary.skipped_duplicates.saturating_add(1);
                continue;
            }

            let exists = repo
                .message_exists(
                    meta_id,
                    member_id,
                    msg.timestamp,
                    msg_type_code,
                    normalized_content.as_deref(),
                )
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;
            if exists {
                summary.skipped_duplicates = summary.skipped_duplicates.saturating_add(1);
                continue;
            }

            let row = Message {
                id: 0,
                sender_id: member_id,
                sender_account_name: sender_name.clone(),
                sender_group_nickname: None,
                ts: msg.timestamp,
                msg_type: msg_type_code,
                content: normalized_content.clone(),
                reply_to_message_id: None,
                platform_message_id: None,
                meta_id,
            };
            let inserted_message_id = repo
                .create_message(&row)
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;
            summary.inserted_messages = summary.inserted_messages.saturating_add(1);

            if let Some(worker_ref) = worker.as_ref() {
                let event = WebhookMessageCreatedEvent {
                    event_type: "message.created".to_string(),
                    platform: platform.clone(),
                    chat_name: chat_name.clone(),
                    meta_id,
                    message_id: inserted_message_id,
                    sender_id: member_id,
                    sender_name: sender_name.clone(),
                    ts: msg.timestamp,
                    msg_type: msg_type_code,
                    content: normalized_content.clone(),
                };
                if worker_ref.send(event).await.is_err() {
                    summary.webhook_failed = summary.webhook_failed.saturating_add(1);
                }
            }
        }

        if let Some(worker_ref) = worker.take() {
            let stats = worker_ref.close_and_wait().await;
            summary.webhook_attempted = summary.webhook_attempted.saturating_add(stats.attempted);
            summary.webhook_delivered = summary.webhook_delivered.saturating_add(stats.delivered);
            summary.webhook_failed = summary.webhook_failed.saturating_add(stats.failed);
            summary.webhook_filtered = summary.webhook_filtered.saturating_add(stats.filtered);
        }

        repo.upsert_import_source_checkpoint(&ImportSourceCheckpoint {
            id: 0,
            source_kind: "monitor".to_string(),
            source_path: source_hint.clone(),
            fingerprint: source_fingerprint.fingerprint.clone(),
            file_size: source_fingerprint.file_size,
            modified_at: source_fingerprint.modified_at,
            platform: Some(platform.clone()),
            chat_name: Some(chat_name.clone()),
            meta_id: Some(meta_id),
            last_processed_at: current_unix_ts(),
            last_inserted_messages: summary.inserted_messages as i64,
            last_duplicate_messages: summary.skipped_duplicates as i64,
            status: "completed".to_string(),
            error_message: None,
        })
        .await
        .map_err(|e| CliError::Database(e.to_string()))?;

        Ok::<MonitorDbWriteSummary, CliError>(summary)
    })
}

#[cfg(feature = "analysis")]
fn is_supported_chat_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase());
    matches!(
        ext.as_deref(),
        Some("txt")
            | Some("json")
            | Some("jsonl")
            | Some("csv")
            | Some("md")
            | Some("html")
            | Some("xml")
    )
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn normalize_content(content: String) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn parser_message_type_to_code(msg_type: &xenobot_analysis::parsers::MessageType) -> i64 {
    match msg_type {
        xenobot_analysis::parsers::MessageType::Text => 0,
        xenobot_analysis::parsers::MessageType::Image => 1,
        xenobot_analysis::parsers::MessageType::Audio => 2,
        xenobot_analysis::parsers::MessageType::Video => 3,
        xenobot_analysis::parsers::MessageType::File => 4,
        xenobot_analysis::parsers::MessageType::Sticker => 5,
        xenobot_analysis::parsers::MessageType::Location => 6,
        xenobot_analysis::parsers::MessageType::System => 7,
        xenobot_analysis::parsers::MessageType::Link => 8,
    }
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn current_unix_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(all(feature = "analysis", feature = "api"))]
#[derive(Debug, Clone)]
struct SourceFileFingerprint {
    file_size: i64,
    modified_at: i64,
    fingerprint: String,
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn build_source_file_fingerprint(path: &Path) -> Result<SourceFileFingerprint> {
    use std::time::UNIX_EPOCH;

    let meta = std::fs::metadata(path)?;
    let file_size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
    let modified = meta
        .modified()
        .ok()
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok());
    let modified_at = modified.map(|v| v.as_secs() as i64).unwrap_or(0);
    let modified_nanos = modified.map(|v| v.subsec_nanos()).unwrap_or(0);

    // Build a stable stream hash over full file content to avoid false-positive
    // incremental skips when only mtime/size metadata is reused.
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut hash_state: u64 = 0xcbf29ce484222325;
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash_state ^= u64::from(*byte);
            hash_state = hash_state.wrapping_mul(0x100000001b3);
        }
    }
    let content_hash = format!("{:016x}", hash_state);
    let fingerprint = format!(
        "v2:{}:{}:{}:{}",
        file_size, modified_at, modified_nanos, content_hash
    );
    Ok(SourceFileFingerprint {
        file_size,
        modified_at,
        fingerprint,
    })
}

#[cfg(all(feature = "analysis", feature = "api"))]
fn monitor_source_checkpoint_unchanged(
    db_path: &Path,
    source_path: &Path,
    fingerprint: &str,
) -> bool {
    if !db_path.exists() {
        return false;
    }
    let conn = match open_sqlite_read_connection(db_path) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let source = source_path.to_string_lossy().to_string();
    let sql = r#"
        SELECT COUNT(*)
        FROM import_source_checkpoint
        WHERE source_kind = ?1
          AND source_path = ?2
          AND fingerprint = ?3
          AND status = 'completed'
    "#;
    conn.query_row(
        sql,
        rusqlite::params!["monitor", source, fingerprint],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
struct QueryMessageRow {
    message_id: i64,
    meta_id: i64,
    platform: String,
    chat_name: String,
    sender_id: i64,
    sender_name: String,
    ts: i64,
    msg_type: i64,
    content: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SemanticMessageRow {
    message_id: i64,
    meta_id: i64,
    platform: String,
    chat_name: String,
    sender_id: i64,
    sender_name: String,
    ts: i64,
    msg_type: i64,
    content: Option<String>,
    similarity: f32,
}

fn open_sqlite_read_connection(path: &Path) -> Result<rusqlite::Connection> {
    if !path.exists() {
        return Err(CliError::Argument(format!(
            "database path not found: {}",
            path.display()
        )));
    }
    rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| CliError::Database(e.to_string()))
}

fn open_sqlite_rw_connection(path: &Path, create_if_missing: bool) -> Result<rusqlite::Connection> {
    if !create_if_missing && !path.exists() {
        return Err(CliError::Argument(format!(
            "database path not found: {}",
            path.display()
        )));
    }
    if create_if_missing {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }
    let mut flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
        | rusqlite::OpenFlags::SQLITE_OPEN_URI
        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create_if_missing {
        flags |= rusqlite::OpenFlags::SQLITE_OPEN_CREATE;
    }
    rusqlite::Connection::open_with_flags(path, flags)
        .map_err(|e| CliError::Database(e.to_string()))
}

#[derive(Debug, Clone)]
struct MigrationFile {
    version: i64,
    path: PathBuf,
}

fn migration_dir_path() -> PathBuf {
    PathBuf::from("crates").join("api").join("migrations")
}

fn collect_migration_files() -> Result<Vec<MigrationFile>> {
    let dir = migration_dir_path();
    if !dir.exists() {
        return Err(CliError::FileSystem(format!(
            "migrations directory not found: {}",
            dir.display()
        )));
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("sql") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let Some(version) = parse_migration_version(file_name) else {
            continue;
        };
        out.push(MigrationFile { version, path });
    }
    out.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(out)
}

fn parse_migration_version(file_name: &str) -> Option<i64> {
    let prefix = file_name.split('_').next()?;
    prefix.parse::<i64>().ok()
}

fn ensure_schema_migrations_table(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .map_err(|e| CliError::Database(e.to_string()))
}

fn get_applied_versions(conn: &rusqlite::Connection) -> Result<std::collections::HashSet<i64>> {
    let mut stmt = conn
        .prepare("SELECT version FROM schema_migrations")
        .map_err(|e| CliError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mut set = std::collections::HashSet::new();
    for row in rows {
        set.insert(row.map_err(|e| CliError::Database(e.to_string()))?);
    }
    Ok(set)
}

fn apply_migrations_to_path(
    db_path: &Path,
    target_version: Option<i64>,
    create_if_missing: bool,
) -> Result<usize> {
    let mut conn = open_sqlite_rw_connection(db_path, create_if_missing)?;
    ensure_schema_migrations_table(&conn)?;
    let migration_files = collect_migration_files()?;
    let mut applied_versions = get_applied_versions(&conn)?;
    let tx = conn
        .transaction()
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut applied_count = 0usize;
    for migration in migration_files {
        if let Some(target) = target_version {
            if migration.version > target {
                continue;
            }
        }
        if applied_versions.contains(&migration.version) {
            continue;
        }
        let sql = std::fs::read_to_string(&migration.path)?;
        tx.execute_batch(&sql)
            .map_err(|e| CliError::Database(format!("{}: {}", migration.path.display(), e)))?;
        tx.execute(
            "INSERT INTO schema_migrations(version) VALUES (?1)",
            [migration.version],
        )
        .map_err(|e| CliError::Database(e.to_string()))?;
        applied_versions.insert(migration.version);
        applied_count += 1;
    }

    tx.commit().map_err(|e| CliError::Database(e.to_string()))?;
    Ok(applied_count)
}

#[derive(Debug, Serialize)]
struct DbInfoRow {
    path: String,
    size_bytes: u64,
    table_count: i64,
    message_count: i64,
    member_count: i64,
    chat_count: i64,
    migration_versions: Vec<i64>,
}

fn collect_db_info(path: &Path, conn: &rusqlite::Connection) -> Result<DbInfoRow> {
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| CliError::Database(e.to_string()))?;
    let message_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM message", [], |row| row.get(0))
        .unwrap_or(0);
    let member_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM member", [], |row| row.get(0))
        .unwrap_or(0);
    let chat_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM meta", [], |row| row.get(0))
        .unwrap_or(0);

    let mut versions = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
    {
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| CliError::Database(e.to_string()))?;
        for row in rows {
            versions.push(row.map_err(|e| CliError::Database(e.to_string()))?);
        }
    }

    Ok(DbInfoRow {
        path: path.to_string_lossy().to_string(),
        size_bytes,
        table_count,
        message_count,
        member_count,
        chat_count,
        migration_versions: versions,
    })
}

fn print_db_info(info: &DbInfoRow, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(info).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "path,size_bytes,table_count,message_count,member_count,chat_count,migration_versions"
            );
            println!(
                "{},{},{},{},{},{},{}",
                csv_escape(&info.path),
                info.size_bytes,
                info.table_count,
                info.message_count,
                info.member_count,
                info.chat_count,
                csv_escape(
                    &info
                        .migration_versions
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join("|"),
                )
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(info).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            println!("database info");
            println!("path: {}", info.path);
            println!("size bytes: {}", info.size_bytes);
            println!("table count: {}", info.table_count);
            println!("chat count: {}", info.chat_count);
            println!("member count: {}", info.member_count);
            println!("message count: {}", info.message_count);
            println!(
                "migration versions: {}",
                if info.migration_versions.is_empty() {
                    "-".to_string()
                } else {
                    info.migration_versions
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                }
            );
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct DbVerificationCheck {
    kind: String,
    name: String,
    required: bool,
    exists: bool,
}

#[derive(Debug, Serialize)]
struct DbVerificationReport {
    path: String,
    ok: bool,
    required_total: usize,
    optional_total: usize,
    missing_required: usize,
    missing_optional: usize,
    checks: Vec<DbVerificationCheck>,
}

fn sqlite_object_exists(
    conn: &rusqlite::Connection,
    object_type: &str,
    name: &str,
) -> Result<bool> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
            rusqlite::params![object_type, name],
            |row| row.get(0),
        )
        .map_err(|e| CliError::Database(e.to_string()))?;
    Ok(exists > 0)
}

fn collect_db_verification(
    path: &Path,
    conn: &rusqlite::Connection,
) -> Result<DbVerificationReport> {
    // Required schema objects for API/analysis hot paths.
    let required_tables = [
        "meta",
        "member",
        "message",
        "sessions",
        "message_context",
        "member_name_history",
    ];
    let required_indexes = [
        "idx_message_meta_ts_id",
        "idx_message_meta_sender_ts_id",
        "idx_sessions_meta_created_at",
        "idx_member_name_history_member_start_ts",
        "idx_message_context_session_message",
        "idx_chat_session_meta_start_ts_id",
    ];

    // Optional but expected for import/incremental diagnostics.
    let optional_tables = ["import_progress", "import_source_checkpoint"];
    let optional_indexes = ["idx_message_dedup_lookup", "idx_meta_platform_name"];

    let mut checks = Vec::new();

    for table in required_tables {
        checks.push(DbVerificationCheck {
            kind: "table".to_string(),
            name: table.to_string(),
            required: true,
            exists: sqlite_object_exists(conn, "table", table)?,
        });
    }
    for index in required_indexes {
        checks.push(DbVerificationCheck {
            kind: "index".to_string(),
            name: index.to_string(),
            required: true,
            exists: sqlite_object_exists(conn, "index", index)?,
        });
    }
    for table in optional_tables {
        checks.push(DbVerificationCheck {
            kind: "table".to_string(),
            name: table.to_string(),
            required: false,
            exists: sqlite_object_exists(conn, "table", table)?,
        });
    }
    for index in optional_indexes {
        checks.push(DbVerificationCheck {
            kind: "index".to_string(),
            name: index.to_string(),
            required: false,
            exists: sqlite_object_exists(conn, "index", index)?,
        });
    }

    let required_total = checks.iter().filter(|row| row.required).count();
    let optional_total = checks.iter().filter(|row| !row.required).count();
    let missing_required = checks
        .iter()
        .filter(|row| row.required && !row.exists)
        .count();
    let missing_optional = checks
        .iter()
        .filter(|row| !row.required && !row.exists)
        .count();

    Ok(DbVerificationReport {
        path: path.to_string_lossy().to_string(),
        ok: missing_required == 0,
        required_total,
        optional_total,
        missing_required,
        missing_optional,
        checks,
    })
}

fn print_db_verification(report: &DbVerificationReport, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!("path,ok,required_total,optional_total,missing_required,missing_optional");
            println!(
                "{},{},{},{},{},{}",
                csv_escape(&report.path),
                report.ok,
                report.required_total,
                report.optional_total,
                report.missing_required,
                report.missing_optional
            );
            println!("kind,name,required,exists");
            for row in &report.checks {
                println!(
                    "{},{},{},{}",
                    csv_escape(&row.kind),
                    csv_escape(&row.name),
                    row.required,
                    row.exists
                );
            }
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            println!("database verification");
            println!("path: {}", report.path);
            println!("ok: {}", report.ok);
            println!("required checks: {}", report.required_total);
            println!("optional checks: {}", report.optional_total);
            println!("missing required: {}", report.missing_required);
            println!("missing optional: {}", report.missing_optional);
            if report.missing_required > 0 || report.missing_optional > 0 {
                println!("missing objects");
                for row in &report.checks {
                    if !row.exists {
                        println!(
                            "- [{}] {} {}",
                            if row.required { "required" } else { "optional" },
                            row.kind,
                            row.name
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct DbCheckpointRow {
    id: i64,
    source_kind: String,
    source_path: String,
    fingerprint: String,
    file_size: i64,
    modified_at: i64,
    platform: Option<String>,
    chat_name: Option<String>,
    meta_id: Option<i64>,
    last_processed_at: i64,
    last_inserted_messages: i64,
    last_duplicate_messages: i64,
    status: String,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct DbCheckpointReport {
    path: String,
    table_present: bool,
    source_kind_filter: Option<String>,
    status_filter: Option<String>,
    limit: usize,
    total_rows: usize,
    returned_rows: usize,
    rows: Vec<DbCheckpointRow>,
}

fn collect_db_checkpoints(
    path: &Path,
    conn: &rusqlite::Connection,
    source_kind_filter: Option<&str>,
    status_filter: Option<&str>,
    limit: usize,
) -> Result<DbCheckpointReport> {
    let source_kind = source_kind_filter
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let status = status_filter
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let capped_limit = limit.clamp(1, 10_000);

    if !sqlite_object_exists(conn, "table", "import_source_checkpoint")? {
        return Ok(DbCheckpointReport {
            path: path.to_string_lossy().to_string(),
            table_present: false,
            source_kind_filter: source_kind,
            status_filter: status,
            limit: capped_limit,
            total_rows: 0,
            returned_rows: 0,
            rows: Vec::new(),
        });
    }

    let mut where_parts = Vec::new();
    let mut args = Vec::<String>::new();
    if let Some(value) = source_kind.as_ref() {
        where_parts.push("source_kind = ?".to_string());
        args.push(value.clone());
    }
    if let Some(value) = status.as_ref() {
        where_parts.push("status = ?".to_string());
        args.push(value.clone());
    }

    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_parts.join(" AND "))
    };

    let total_sql = format!("SELECT COUNT(*) FROM import_source_checkpoint{}", where_sql);
    let total_rows: i64 = conn
        .query_row(
            &total_sql,
            rusqlite::params_from_iter(args.iter().map(|s| s.as_str())),
            |row| row.get(0),
        )
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut row_args = args.clone();
    row_args.push(capped_limit.to_string());

    let rows_sql = format!(
        r#"
        SELECT
            id, source_kind, source_path, fingerprint, file_size, modified_at,
            platform, chat_name, meta_id, last_processed_at,
            last_inserted_messages, last_duplicate_messages, status, error_message
        FROM import_source_checkpoint
        {}
        ORDER BY last_processed_at DESC, id DESC
        LIMIT ?
        "#,
        where_sql
    );

    let mut stmt = conn
        .prepare(&rows_sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mapped = stmt
        .query_map(
            rusqlite::params_from_iter(row_args.iter().map(|s| s.as_str())),
            |row| {
                Ok(DbCheckpointRow {
                    id: row.get(0)?,
                    source_kind: row.get(1)?,
                    source_path: row.get(2)?,
                    fingerprint: row.get(3)?,
                    file_size: row.get(4)?,
                    modified_at: row.get(5)?,
                    platform: row.get(6)?,
                    chat_name: row.get(7)?,
                    meta_id: row.get(8)?,
                    last_processed_at: row.get(9)?,
                    last_inserted_messages: row.get(10)?,
                    last_duplicate_messages: row.get(11)?,
                    status: row.get(12)?,
                    error_message: row.get(13)?,
                })
            },
        )
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| CliError::Database(e.to_string()))?);
    }

    Ok(DbCheckpointReport {
        path: path.to_string_lossy().to_string(),
        table_present: true,
        source_kind_filter: source_kind,
        status_filter: status,
        limit: capped_limit,
        total_rows: total_rows.max(0) as usize,
        returned_rows: rows.len(),
        rows,
    })
}

fn print_db_checkpoints(report: &DbCheckpointReport, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "path,table_present,source_kind_filter,status_filter,limit,total_rows,returned_rows"
            );
            println!(
                "{},{},{},{},{},{},{}",
                csv_escape(&report.path),
                report.table_present,
                csv_escape(report.source_kind_filter.as_deref().unwrap_or_default()),
                csv_escape(report.status_filter.as_deref().unwrap_or_default()),
                report.limit,
                report.total_rows,
                report.returned_rows
            );
            println!(
                "id,source_kind,source_path,status,platform,chat_name,meta_id,last_processed_at,last_inserted,last_duplicate,file_size,modified_at,fingerprint,error_message"
            );
            for row in &report.rows {
                println!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    row.id,
                    csv_escape(&row.source_kind),
                    csv_escape(&row.source_path),
                    csv_escape(&row.status),
                    csv_escape(row.platform.as_deref().unwrap_or_default()),
                    csv_escape(row.chat_name.as_deref().unwrap_or_default()),
                    row.meta_id
                        .map(|v| v.to_string())
                        .unwrap_or_else(String::new),
                    row.last_processed_at,
                    row.last_inserted_messages,
                    row.last_duplicate_messages,
                    row.file_size,
                    row.modified_at,
                    csv_escape(&row.fingerprint),
                    csv_escape(row.error_message.as_deref().unwrap_or_default()),
                );
            }
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            println!("database checkpoints");
            println!("path: {}", report.path);
            println!("table present: {}", report.table_present);
            println!(
                "filters: source_kind={} status={}",
                report
                    .source_kind_filter
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .unwrap_or("-"),
                report
                    .status_filter
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .unwrap_or("-")
            );
            println!("limit: {}", report.limit);
            println!("total rows: {}", report.total_rows);
            println!("returned rows: {}", report.returned_rows);
            if !report.table_present {
                println!("note: import_source_checkpoint table does not exist in this database");
                return Ok(());
            }
            for row in &report.rows {
                println!(
                    "- [{}] kind={} path={} meta_id={} inserted={} duplicate={} ts={} error={}",
                    row.status,
                    row.source_kind,
                    row.source_path,
                    row.meta_id
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    row.last_inserted_messages,
                    row.last_duplicate_messages,
                    row.last_processed_at,
                    row.error_message.as_deref().unwrap_or("-")
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct DbSchemaColumn {
    cid: i64,
    name: String,
    #[serde(rename = "type")]
    column_type: String,
    notnull: bool,
    default_value: Option<String>,
    primary_key: bool,
}

#[derive(Debug, Serialize)]
struct DbSchemaIndex {
    name: String,
    unique: bool,
    origin: String,
    partial: bool,
    columns: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DbSchemaForeignKey {
    id: i64,
    seq: i64,
    table: String,
    from: String,
    to: String,
    on_update: String,
    on_delete: String,
    match_rule: String,
}

#[derive(Debug, Serialize)]
struct DbSchemaTable {
    name: String,
    columns: Vec<DbSchemaColumn>,
    indexes: Vec<DbSchemaIndex>,
    foreign_keys: Vec<DbSchemaForeignKey>,
    row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DbSchemaSummary {
    table_count: usize,
    column_count: usize,
    index_count: usize,
    foreign_key_count: usize,
}

#[derive(Debug, Serialize)]
struct DbSchemaReport {
    path: String,
    include_row_count: bool,
    summary: DbSchemaSummary,
    tables: Vec<DbSchemaTable>,
}

fn escape_sqlite_literal_value(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_sqlite_identifier_name(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn collect_db_schema(
    path: &Path,
    conn: &rusqlite::Connection,
    include_row_count: bool,
) -> Result<DbSchemaReport> {
    let mut tables_stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|e| CliError::Database(e.to_string()))?;
    let table_iter = tables_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mut table_names = Vec::new();
    for table_name in table_iter {
        table_names.push(table_name.map_err(|e| CliError::Database(e.to_string()))?);
    }

    let mut tables = Vec::new();
    let mut summary = DbSchemaSummary {
        table_count: 0,
        column_count: 0,
        index_count: 0,
        foreign_key_count: 0,
    };

    for table_name in table_names {
        let escaped_table_literal = escape_sqlite_literal_value(&table_name);

        let mut columns_stmt = conn
            .prepare(&format!("PRAGMA table_info('{escaped_table_literal}')"))
            .map_err(|e| CliError::Database(e.to_string()))?;
        let column_rows = columns_stmt
            .query_map([], |row| {
                Ok(DbSchemaColumn {
                    cid: row.get::<_, i64>(0)?,
                    name: row.get::<_, String>(1)?,
                    column_type: row.get::<_, String>(2).unwrap_or_default(),
                    notnull: row.get::<_, i64>(3).unwrap_or_default() == 1,
                    default_value: row.get::<_, Option<String>>(4).unwrap_or(None),
                    primary_key: row.get::<_, i64>(5).unwrap_or_default() == 1,
                })
            })
            .map_err(|e| CliError::Database(e.to_string()))?;
        let mut columns = Vec::new();
        for col in column_rows {
            columns.push(col.map_err(|e| CliError::Database(e.to_string()))?);
        }
        summary.column_count += columns.len();

        let mut index_list_stmt = conn
            .prepare(&format!("PRAGMA index_list('{escaped_table_literal}')"))
            .map_err(|e| CliError::Database(e.to_string()))?;
        let index_list_rows = index_list_stmt
            .query_map([], |row| {
                let index_name: String = row.get(1)?;
                let unique: i64 = row.get(2)?;
                let origin: String = row.get::<_, String>(3).unwrap_or_else(|_| "c".to_string());
                let partial: i64 = row.get::<_, i64>(4).unwrap_or_default();
                Ok((index_name, unique == 1, origin, partial == 1))
            })
            .map_err(|e| CliError::Database(e.to_string()))?;

        let mut indexes = Vec::new();
        for entry in index_list_rows {
            let (index_name, unique, origin, partial) =
                entry.map_err(|e| CliError::Database(e.to_string()))?;
            if index_name.trim().is_empty() {
                continue;
            }
            let escaped_index_literal = escape_sqlite_literal_value(&index_name);
            let mut index_info_stmt = conn
                .prepare(&format!("PRAGMA index_info('{escaped_index_literal}')"))
                .map_err(|e| CliError::Database(e.to_string()))?;
            let index_col_rows = index_info_stmt
                .query_map([], |row| row.get::<_, String>(2))
                .map_err(|e| CliError::Database(e.to_string()))?;
            let mut index_columns = Vec::new();
            for col in index_col_rows {
                index_columns.push(col.map_err(|e| CliError::Database(e.to_string()))?);
            }
            indexes.push(DbSchemaIndex {
                name: index_name,
                unique,
                origin,
                partial,
                columns: index_columns,
            });
        }
        summary.index_count += indexes.len();

        let mut fk_stmt = conn
            .prepare(&format!(
                "PRAGMA foreign_key_list('{escaped_table_literal}')"
            ))
            .map_err(|e| CliError::Database(e.to_string()))?;
        let fk_rows = fk_stmt
            .query_map([], |row| {
                Ok(DbSchemaForeignKey {
                    id: row.get::<_, i64>(0)?,
                    seq: row.get::<_, i64>(1)?,
                    table: row.get::<_, String>(2).unwrap_or_default(),
                    from: row.get::<_, String>(3).unwrap_or_default(),
                    to: row.get::<_, String>(4).unwrap_or_default(),
                    on_update: row.get::<_, String>(5).unwrap_or_default(),
                    on_delete: row.get::<_, String>(6).unwrap_or_default(),
                    match_rule: row.get::<_, String>(7).unwrap_or_default(),
                })
            })
            .map_err(|e| CliError::Database(e.to_string()))?;
        let mut foreign_keys = Vec::new();
        for fk in fk_rows {
            foreign_keys.push(fk.map_err(|e| CliError::Database(e.to_string()))?);
        }
        summary.foreign_key_count += foreign_keys.len();

        let row_count = if include_row_count {
            let sql = format!(
                "SELECT COUNT(*) FROM {}",
                escape_sqlite_identifier_name(&table_name)
            );
            Some(
                conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
                    .map_err(|e| CliError::Database(e.to_string()))?,
            )
        } else {
            None
        };

        tables.push(DbSchemaTable {
            name: table_name,
            columns,
            indexes,
            foreign_keys,
            row_count,
        });
    }

    summary.table_count = tables.len();

    Ok(DbSchemaReport {
        path: path.to_string_lossy().to_string(),
        include_row_count,
        summary,
        tables,
    })
}

fn print_db_schema(report: &DbSchemaReport, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "path,include_row_count,table_count,column_count,index_count,foreign_key_count"
            );
            println!(
                "{},{},{},{},{},{}",
                csv_escape(&report.path),
                report.include_row_count,
                report.summary.table_count,
                report.summary.column_count,
                report.summary.index_count,
                report.summary.foreign_key_count
            );
            println!(
                "table_name,column_count,index_count,foreign_key_count,row_count,column_names,index_names"
            );
            for table in &report.tables {
                let column_names = table
                    .columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join("|");
                let index_names = table
                    .indexes
                    .iter()
                    .map(|idx| idx.name.as_str())
                    .collect::<Vec<_>>()
                    .join("|");
                println!(
                    "{},{},{},{},{},{},{}",
                    csv_escape(&table.name),
                    table.columns.len(),
                    table.indexes.len(),
                    table.foreign_keys.len(),
                    table
                        .row_count
                        .map(|v| v.to_string())
                        .unwrap_or_else(String::new),
                    csv_escape(&column_names),
                    csv_escape(&index_names),
                );
            }
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            println!("database schema");
            println!("path: {}", report.path);
            println!("include row count: {}", report.include_row_count);
            println!(
                "summary: tables={} columns={} indexes={} foreign_keys={}",
                report.summary.table_count,
                report.summary.column_count,
                report.summary.index_count,
                report.summary.foreign_key_count
            );
            for table in &report.tables {
                println!(
                    "- table={} columns={} indexes={} foreign_keys={} row_count={}",
                    table.name,
                    table.columns.len(),
                    table.indexes.len(),
                    table.foreign_keys.len(),
                    table
                        .row_count
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
        }
    }
    Ok(())
}

fn parse_optional_member_id(raw: Option<&str>) -> Result<Option<i64>> {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|_| CliError::Argument(format!("member_id must be integer, got '{}'", v))),
    }
}

fn parse_optional_date_start(raw: Option<&str>) -> Result<Option<i64>> {
    parse_optional_date(raw, true)
}

fn parse_optional_date_end(raw: Option<&str>) -> Result<Option<i64>> {
    parse_optional_date(raw, false)
}

fn parse_optional_date(raw: Option<&str>, start_of_day: bool) -> Result<Option<i64>> {
    let Some(raw_value) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let date = chrono::NaiveDate::parse_from_str(raw_value, "%Y-%m-%d").map_err(|e| {
        CliError::Argument(format!(
            "invalid date '{}', expected YYYY-MM-DD ({})",
            raw_value, e
        ))
    })?;
    let Some(naive_dt) = (if start_of_day {
        date.and_hms_opt(0, 0, 0)
    } else {
        date.and_hms_opt(23, 59, 59)
    }) else {
        return Err(CliError::Argument(format!(
            "failed to build timestamp for date '{}'",
            raw_value
        )));
    };
    Ok(Some(naive_dt.and_utc().timestamp()))
}

fn print_analysis_result(payload: &serde_json::Value, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json | OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(payload)
                    .map_err(|e| CliError::Parse(e.to_string()))?
            );
            if matches!(format, OutputFormat::Yaml) {
                println!("note: yaml renderer is not wired in cli; json is printed instead");
            }
        }
        OutputFormat::Csv => {
            let rows = payload
                .get("rows")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if rows.is_empty() {
                println!("analysis,rows");
                println!(
                    "{},0",
                    csv_escape(
                        payload
                            .get("analysis")
                            .and_then(|v| v.as_str())
                            .unwrap_or("analysis")
                    )
                );
                return Ok(());
            }

            let headers = rows[0]
                .as_object()
                .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            if headers.is_empty() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(payload)
                        .map_err(|e| CliError::Parse(e.to_string()))?
                );
                return Ok(());
            }

            println!("{}", headers.join(","));
            for row in rows {
                let obj = row.as_object();
                let cols = headers
                    .iter()
                    .map(|key| {
                        let value = obj.and_then(|o| o.get(key)).cloned().unwrap_or_default();
                        match value {
                            serde_json::Value::Null => "".to_string(),
                            serde_json::Value::String(s) => csv_escape(&s),
                            serde_json::Value::Bool(v) => v.to_string(),
                            serde_json::Value::Number(v) => v.to_string(),
                            _ => csv_escape(&value.to_string()),
                        }
                    })
                    .collect::<Vec<_>>();
                println!("{}", cols.join(","));
            }
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!(
                "{}",
                serde_json::to_string_pretty(payload)
                    .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
    }
    Ok(())
}

fn run_time_distribution_analysis(
    conn: &rusqlite::Connection,
    granularity: &TimeGranularity,
) -> Result<serde_json::Value> {
    let (bucket_expr, label) = match granularity {
        TimeGranularity::Hourly => ("%H", "hourly"),
        TimeGranularity::Daily => ("%Y-%m-%d", "daily"),
        TimeGranularity::Weekly => ("%Y-W%W", "weekly"),
        TimeGranularity::Monthly => ("%Y-%m", "monthly"),
        TimeGranularity::Yearly => ("%Y", "yearly"),
    };

    let sql = format!(
        r#"
        SELECT strftime('{bucket}', datetime(ts, 'unixepoch')) AS bucket, COUNT(*) AS count
        FROM message
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
        bucket = bucket_expr
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "bucket": row.get::<_, Option<String>>(0)?.unwrap_or_else(|| "unknown".to_string()),
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| CliError::Database(e.to_string()))?);
    }

    Ok(serde_json::json!({
        "analysis": format!("time_distribution_{}", label),
        "rows": items,
    }))
}

fn run_advanced_analysis(
    conn: &rusqlite::Connection,
    analysis: &AdvancedAnalysis,
) -> Result<serde_json::Value> {
    match analysis {
        AdvancedAnalysis::NightOwl => run_ranked_sender_analysis(
            conn,
            "night_owl",
            "CAST(strftime('%H', datetime(msg.ts, 'unixepoch')) AS INTEGER) BETWEEN 0 AND 5",
            true,
            20,
        ),
        AdvancedAnalysis::DragonKing => {
            run_ranked_sender_analysis(conn, "dragon_king", "1=1", true, 20)
        }
        AdvancedAnalysis::Diving => run_ranked_sender_analysis(conn, "diving", "1=1", false, 20),
        AdvancedAnalysis::Mention => run_ranked_sender_analysis(
            conn,
            "mention",
            "COALESCE(msg.content,'') LIKE '%@%'",
            true,
            20,
        ),
        AdvancedAnalysis::MemeBattle => run_ranked_sender_analysis(
            conn,
            "meme_battle",
            "(msg.msg_type IN (5, 6) OR LOWER(COALESCE(msg.content,'')) LIKE '%sticker%' OR COALESCE(msg.content,'') LIKE '%[表情]%')",
            true,
            20,
        ),
        AdvancedAnalysis::Laugh => run_ranked_sender_analysis(
            conn,
            "laugh",
            "(COALESCE(msg.content,'') LIKE '%哈哈%' OR LOWER(COALESCE(msg.content,'')) LIKE '%lol%' OR LOWER(COALESCE(msg.content,'')) LIKE '%haha%' OR COALESCE(msg.content,'') LIKE '%😂%')",
            true,
            20,
        ),
        AdvancedAnalysis::CheckIn => {
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT
                        msg.sender_id,
                        COALESCE(m.account_name, m.group_nickname, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name,
                        COUNT(DISTINCT strftime('%Y-%m-%d', datetime(msg.ts, 'unixepoch'))) AS active_days,
                        COUNT(*) AS message_count
                    FROM message msg
                    LEFT JOIN member m ON m.id = msg.sender_id
                    GROUP BY msg.sender_id, sender_name
                    ORDER BY active_days DESC, message_count DESC
                    LIMIT 20
                    "#,
                )
                .map_err(|e| CliError::Database(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "senderId": row.get::<_, i64>(0)?,
                        "senderName": row.get::<_, String>(1)?,
                        "activeDays": row.get::<_, i64>(2)?,
                        "messageCount": row.get::<_, i64>(3)?,
                    }))
                })
                .map_err(|e| CliError::Database(e.to_string()))?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| CliError::Database(e.to_string()))?);
            }
            Ok(serde_json::json!({
                "analysis": "check_in",
                "rows": items,
            }))
        }
        AdvancedAnalysis::Repeat | AdvancedAnalysis::Catchphrase => {
            let analysis_name = if matches!(analysis, AdvancedAnalysis::Repeat) {
                "repeat"
            } else {
                "catchphrase"
            };
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT
                        TRIM(LOWER(COALESCE(msg.content, ''))) AS phrase,
                        COUNT(*) AS repeat_count
                    FROM message msg
                    WHERE LENGTH(TRIM(COALESCE(msg.content, ''))) BETWEEN 2 AND 80
                    GROUP BY phrase
                    HAVING repeat_count >= 2
                    ORDER BY repeat_count DESC, phrase ASC
                    LIMIT 50
                    "#,
                )
                .map_err(|e| CliError::Database(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "phrase": row.get::<_, String>(0)?,
                        "repeatCount": row.get::<_, i64>(1)?,
                    }))
                })
                .map_err(|e| CliError::Database(e.to_string()))?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| CliError::Database(e.to_string()))?);
            }
            Ok(serde_json::json!({
                "analysis": analysis_name,
                "rows": items,
            }))
        }
        AdvancedAnalysis::Cluster => run_ranked_sender_analysis(
            conn,
            "cluster_proxy",
            "COALESCE(msg.content,'') LIKE '%@%'",
            true,
            30,
        ),
    }
}

fn run_ranked_sender_analysis(
    conn: &rusqlite::Connection,
    analysis_name: &str,
    predicate_sql: &str,
    desc: bool,
    limit: usize,
) -> Result<serde_json::Value> {
    let order = if desc { "DESC" } else { "ASC" };
    let sql = format!(
        r#"
        SELECT
            msg.sender_id,
            COALESCE(m.account_name, m.group_nickname, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name,
            COUNT(*) AS count
        FROM message msg
        LEFT JOIN member m ON m.id = msg.sender_id
        WHERE {predicate}
        GROUP BY msg.sender_id, sender_name
        ORDER BY count {order}, msg.sender_id ASC
        LIMIT {limit}
        "#,
        predicate = predicate_sql,
        order = order,
        limit = limit
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "senderId": row.get::<_, i64>(0)?,
                "senderName": row.get::<_, String>(1)?,
                "count": row.get::<_, i64>(2)?,
            }))
        })
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| CliError::Database(e.to_string()))?);
    }
    Ok(serde_json::json!({
        "analysis": analysis_name,
        "rows": items,
    }))
}

fn run_message_search(
    conn: &rusqlite::Connection,
    keyword: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    member_id: Option<i64>,
    limit: i64,
) -> Result<Vec<QueryMessageRow>> {
    if keyword.trim().is_empty() {
        return Err(CliError::Argument("keyword cannot be empty".to_string()));
    }

    let mut sql = String::from(
        r#"
        SELECT
            msg.id,
            msg.meta_id,
            meta.platform,
            meta.name,
            msg.sender_id,
            COALESCE(msg.sender_account_name, member.account_name, ''),
            msg.ts,
            msg.msg_type,
            msg.content
        FROM message msg
        JOIN meta ON meta.id = msg.meta_id
        LEFT JOIN member ON member.id = msg.sender_id
        WHERE COALESCE(msg.content, '') LIKE ?
        "#,
    );

    let mut params = vec![rusqlite::types::Value::Text(format!(
        "%{}%",
        keyword.trim()
    ))];
    if let Some(start) = start_ts {
        sql.push_str(" AND msg.ts >= ?");
        params.push(rusqlite::types::Value::Integer(start));
    }
    if let Some(end) = end_ts {
        sql.push_str(" AND msg.ts <= ?");
        params.push(rusqlite::types::Value::Integer(end));
    }
    if let Some(member) = member_id {
        sql.push_str(" AND msg.sender_id = ?");
        params.push(rusqlite::types::Value::Integer(member));
    }
    sql.push_str(" ORDER BY msg.ts DESC LIMIT ?");
    params.push(rusqlite::types::Value::Integer(limit.max(1)));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mapped = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(QueryMessageRow {
                message_id: row.get(0)?,
                meta_id: row.get(1)?,
                platform: row.get(2)?,
                chat_name: row.get(3)?,
                sender_id: row.get(4)?,
                sender_name: row.get::<_, String>(5).unwrap_or_default(),
                ts: row.get(6)?,
                msg_type: row.get(7)?,
                content: row.get(8)?,
            })
        })
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut out = Vec::new();
    for row in mapped {
        out.push(row.map_err(|e| CliError::Database(e.to_string()))?);
    }
    Ok(out)
}

const SEMANTIC_EMBEDDING_DIM: usize = 512;
const SEMANTIC_CHUNK_MAX_CHARS: usize = 240;
const SEMANTIC_CHUNK_OVERLAP_CHARS: usize = 48;

fn run_semantic_search(
    conn: &rusqlite::Connection,
    query: &str,
    threshold: f32,
    limit: i64,
) -> Result<Vec<SemanticMessageRow>> {
    let rewritten_query = rewrite_semantic_query(query);
    let query = rewritten_query.trim();
    if query.is_empty() {
        return Err(CliError::Argument("query cannot be empty".to_string()));
    }

    let candidate_limit = ((limit.max(1) as usize).saturating_mul(300)).clamp(500, 20_000) as i64;
    let sql = r#"
        SELECT
            msg.id,
            msg.meta_id,
            meta.platform,
            meta.name,
            msg.sender_id,
            COALESCE(msg.sender_account_name, member.account_name, ''),
            msg.ts,
            msg.msg_type,
            msg.content
        FROM message msg
        JOIN meta ON meta.id = msg.meta_id
        LEFT JOIN member ON member.id = msg.sender_id
        WHERE COALESCE(msg.content, '') <> ''
        ORDER BY msg.ts DESC
        LIMIT ?1
    "#;

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mapped = stmt
        .query_map(rusqlite::params![candidate_limit], |row| {
            Ok(QueryMessageRow {
                message_id: row.get(0)?,
                meta_id: row.get(1)?,
                platform: row.get(2)?,
                chat_name: row.get(3)?,
                sender_id: row.get(4)?,
                sender_name: row.get::<_, String>(5).unwrap_or_default(),
                ts: row.get(6)?,
                msg_type: row.get(7)?,
                content: row.get(8)?,
            })
        })
        .map_err(|e| CliError::Database(e.to_string()))?;

    let query_embedding = embed_text_for_semantic(query);
    let mut scored = Vec::new();
    for row in mapped {
        let row = row.map_err(|e| CliError::Database(e.to_string()))?;
        let content = row.content.as_deref().unwrap_or_default().trim();
        if content.is_empty() {
            continue;
        }
        let similarity = cosine_similarity(&query_embedding, &embed_text_for_semantic(content));
        if similarity >= threshold {
            scored.push(SemanticMessageRow {
                message_id: row.message_id,
                meta_id: row.meta_id,
                platform: row.platform,
                chat_name: row.chat_name,
                sender_id: row.sender_id,
                sender_name: row.sender_name,
                ts: row.ts,
                msg_type: row.msg_type,
                content: row.content,
                similarity,
            });
        }
    }

    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.ts.cmp(&a.ts))
    });
    scored.truncate(limit.max(1) as usize);
    Ok(scored)
}

fn rewrite_semantic_query(query: &str) -> String {
    let mut normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return String::new();
    }

    let replacements = [
        ("聊天记录", "聊天 消息 记录"),
        ("群聊", "群组 聊天"),
        ("私聊", "私人 聊天"),
        ("语音", "音频"),
        ("图片", "图像 照片"),
        ("msg", "message"),
        ("msgs", "messages"),
        ("chat_history", "chat log"),
        ("im", "instant message"),
    ];
    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }

    let mut out = String::with_capacity(normalized.len());
    let mut last_was_space = false;
    for ch in normalized.chars() {
        let mapped = if ch.is_alphanumeric() || is_cjk_char(ch) {
            ch
        } else {
            ' '
        };
        if mapped == ' ' {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(mapped);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

fn semantic_chunk_text(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<&str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let max_chars = max_chars.max(1);
    let overlap_chars = overlap_chars.min(max_chars.saturating_sub(1));
    let char_positions: Vec<usize> = trimmed.char_indices().map(|(i, _)| i).collect();
    let total_chars = char_positions.len();
    if total_chars <= max_chars {
        return vec![trimmed];
    }

    let mut out = Vec::new();
    let step = max_chars.saturating_sub(overlap_chars).max(1);
    let mut start = 0usize;
    while start < total_chars {
        let end = (start + max_chars).min(total_chars);
        let start_byte = char_positions[start];
        let end_byte = if end == total_chars {
            trimmed.len()
        } else {
            char_positions[end]
        };
        let chunk = trimmed[start_byte..end_byte].trim();
        if !chunk.is_empty() {
            out.push(chunk);
        }
        if end == total_chars {
            break;
        }
        start = start.saturating_add(step);
    }
    out
}

fn embed_text_for_semantic(text: &str) -> Vec<f32> {
    let chunks = semantic_chunk_text(text, SEMANTIC_CHUNK_MAX_CHARS, SEMANTIC_CHUNK_OVERLAP_CHARS);
    if chunks.is_empty() {
        return vec![0.0; SEMANTIC_EMBEDDING_DIM];
    }

    let mut acc = vec![0.0f32; SEMANTIC_EMBEDDING_DIM];
    for chunk in chunks {
        let tokens = semantic_tokenize(chunk);
        if tokens.is_empty() {
            continue;
        }
        let embedding = hash_embedding_from_tokens(&tokens, SEMANTIC_EMBEDDING_DIM);
        for (idx, value) in embedding.iter().enumerate() {
            acc[idx] += *value;
        }
    }
    normalize_vector(&mut acc);
    acc
}

fn semantic_tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || (!ch.is_ascii() && ch.is_alphanumeric()) {
            for lowered in ch.to_lowercase() {
                current.push(lowered);
            }
            continue;
        }
        if !current.is_empty() {
            tokens.push(current.clone());
            current.clear();
        }
        if is_cjk_char(ch) {
            tokens.push(ch.to_string());
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_cjk_char(ch: char) -> bool {
    let code = ch as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
}

fn hash_embedding_from_tokens(tokens: &[String], dim: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; dim.max(1)];
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();
        let idx = (hash as usize) % output.len();
        let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
        let weight = 1.0 + (token.chars().count() as f32).ln_1p() * 0.15;
        output[idx] += sign * weight;
    }
    normalize_vector(&mut output);
    output
}

fn normalize_vector(vec: &mut [f32]) {
    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for item in vec {
            *item /= norm;
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for idx in 0..len {
        dot += a[idx] * b[idx];
        norm_a += a[idx] * a[idx];
        norm_b += b[idx] * b[idx];
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        0.0
    } else {
        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

fn print_semantic_rows(rows: &[SemanticMessageRow], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "message_id,meta_id,platform,chat_name,sender_id,sender_name,ts,msg_type,similarity,content"
            );
            for row in rows {
                println!(
                    "{},{},{},{},{},{},{},{},{:.6},{}",
                    row.message_id,
                    row.meta_id,
                    csv_escape(&row.platform),
                    csv_escape(&row.chat_name),
                    row.sender_id,
                    csv_escape(&row.sender_name),
                    row.ts,
                    row.msg_type,
                    row.similarity,
                    csv_escape(row.content.as_deref().unwrap_or_default())
                );
            }
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            if rows.is_empty() {
                println!("no messages matched semantic query");
                return Ok(());
            }
            println!("semantic search results");
            for row in rows {
                println!(
                    "- score={:.4} [{}] {} / {} | sender={}({}) | ts={} | type={} | {}",
                    row.similarity,
                    row.message_id,
                    row.platform,
                    row.chat_name,
                    row.sender_name,
                    row.sender_id,
                    row.ts,
                    row.msg_type,
                    row.content.as_deref().unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}

fn print_search_rows(rows: &[QueryMessageRow], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "message_id,meta_id,platform,chat_name,sender_id,sender_name,ts,msg_type,content"
            );
            for row in rows {
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    row.message_id,
                    row.meta_id,
                    csv_escape(&row.platform),
                    csv_escape(&row.chat_name),
                    row.sender_id,
                    csv_escape(&row.sender_name),
                    row.ts,
                    row.msg_type,
                    csv_escape(row.content.as_deref().unwrap_or_default())
                );
            }
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            if rows.is_empty() {
                println!("no messages matched query");
                return Ok(());
            }
            println!("message search results");
            for row in rows {
                println!(
                    "- [{}] {} / {} | sender={}({}) | ts={} | type={} | {}",
                    row.message_id,
                    row.platform,
                    row.chat_name,
                    row.sender_name,
                    row.sender_id,
                    row.ts,
                    row.msg_type,
                    row.content.as_deref().unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}

fn execute_safe_select_sql(
    conn: &rusqlite::Connection,
    raw_sql: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let sql = validate_select_sql(raw_sql)?;
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let headers: Vec<String> = stmt.column_names().iter().map(|v| v.to_string()).collect();
    let col_count = stmt.column_count();

    let mut rows = stmt
        .query([])
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| CliError::Database(e.to_string()))? {
        let mut values = Vec::with_capacity(col_count);
        for idx in 0..col_count {
            let value = row
                .get_ref(idx)
                .map_err(|e| CliError::Database(e.to_string()))?;
            values.push(sql_value_ref_to_string(value));
        }
        out.push(values);
    }
    Ok((headers, out))
}

fn validate_select_sql(raw_sql: &str) -> Result<String> {
    let sql = raw_sql.trim();
    if sql.is_empty() {
        return Err(CliError::Argument("sql cannot be empty".to_string()));
    }

    let upper = sql.to_ascii_uppercase();
    if !upper.starts_with("SELECT") {
        return Err(CliError::Argument(
            "only SELECT statements are allowed".to_string(),
        ));
    }

    let trimmed_no_tail = sql.trim_end_matches(';').trim_end();
    if trimmed_no_tail.contains(';') {
        return Err(CliError::Argument(
            "multiple SQL statements are not allowed".to_string(),
        ));
    }

    Ok(trimmed_no_tail.to_string())
}

fn print_sql_rows(headers: &[String], rows: &[Vec<String>], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let mut objects = Vec::with_capacity(rows.len());
            for row in rows {
                let mut obj = serde_json::Map::new();
                for (idx, value) in row.iter().enumerate() {
                    let key = headers
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| format!("col_{}", idx + 1));
                    obj.insert(key, serde_json::Value::String(value.clone()));
                }
                objects.push(serde_json::Value::Object(obj));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&objects)
                    .map_err(|e| CliError::Parse(e.to_string()))?
            );
        }
        OutputFormat::Csv => {
            println!(
                "{}",
                headers
                    .iter()
                    .map(|h| csv_escape(h))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for row in rows {
                println!(
                    "{}",
                    row.iter()
                        .map(|v| csv_escape(v))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        OutputFormat::Yaml => {
            let mut objects = Vec::with_capacity(rows.len());
            for row in rows {
                let mut obj = serde_json::Map::new();
                for (idx, value) in row.iter().enumerate() {
                    let key = headers
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| format!("col_{}", idx + 1));
                    obj.insert(key, serde_json::Value::String(value.clone()));
                }
                objects.push(serde_json::Value::Object(obj));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&objects)
                    .map_err(|e| CliError::Parse(e.to_string()))?
            );
            println!("note: yaml renderer is not wired in cli; json is printed instead");
        }
        _ => {
            if headers.is_empty() {
                println!("query executed: 0 columns");
                return Ok(());
            }
            println!("{}", headers.join(" | "));
            for row in rows {
                println!("{}", row.join(" | "));
            }
            println!("rows: {}", rows.len());
        }
    }
    Ok(())
}

fn sql_value_ref_to_string(value: rusqlite::types::ValueRef<'_>) -> String {
    match value {
        rusqlite::types::ValueRef::Null => String::new(),
        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
        rusqlite::types::ValueRef::Real(v) => v.to_string(),
        rusqlite::types::ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        rusqlite::types::ValueRef::Blob(v) => format!("<blob:{} bytes>", v.len()),
    }
}

fn csv_escape(v: &str) -> String {
    let escaped = v.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

#[derive(Debug, Clone, Serialize)]
struct ExportMessageRow {
    message_id: i64,
    meta_id: i64,
    platform: String,
    chat_name: String,
    sender_id: i64,
    sender_name: String,
    ts: i64,
    msg_type: i64,
    content: Option<String>,
}

fn run_export_query(
    conn: &rusqlite::Connection,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    member_id: Option<i64>,
) -> Result<Vec<ExportMessageRow>> {
    let mut sql = String::from(
        r#"
        SELECT
            msg.id,
            msg.meta_id,
            meta.platform,
            meta.name,
            msg.sender_id,
            COALESCE(msg.sender_account_name, member.account_name, ''),
            msg.ts,
            msg.msg_type,
            msg.content
        FROM message msg
        JOIN meta ON meta.id = msg.meta_id
        LEFT JOIN member ON member.id = msg.sender_id
        WHERE 1 = 1
        "#,
    );

    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(start) = start_ts {
        sql.push_str(" AND msg.ts >= ?");
        params.push(rusqlite::types::Value::Integer(start));
    }
    if let Some(end) = end_ts {
        sql.push_str(" AND msg.ts <= ?");
        params.push(rusqlite::types::Value::Integer(end));
    }
    if let Some(member) = member_id {
        sql.push_str(" AND msg.sender_id = ?");
        params.push(rusqlite::types::Value::Integer(member));
    }
    sql.push_str(" ORDER BY msg.ts ASC, msg.id ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Database(e.to_string()))?;
    let mapped = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(ExportMessageRow {
                message_id: row.get(0)?,
                meta_id: row.get(1)?,
                platform: row.get(2)?,
                chat_name: row.get(3)?,
                sender_id: row.get(4)?,
                sender_name: row.get::<_, String>(5).unwrap_or_default(),
                ts: row.get(6)?,
                msg_type: row.get(7)?,
                content: row.get(8)?,
            })
        })
        .map_err(|e| CliError::Database(e.to_string()))?;

    let mut out = Vec::new();
    for row in mapped {
        out.push(row.map_err(|e| CliError::Database(e.to_string()))?);
    }
    Ok(out)
}

fn resolve_export_output_path(base: &Path, format: ExportFormat) -> PathBuf {
    if base.is_dir() {
        let file_name = match format {
            ExportFormat::Jsonl => "xenobot-export.jsonl",
            ExportFormat::Text => "xenobot-export.txt",
            ExportFormat::Csv => "xenobot-export.csv",
            ExportFormat::Json => "xenobot-export.json",
            ExportFormat::Html => "xenobot-export.html",
        };
        return base.join(file_name);
    }
    base.to_path_buf()
}

fn write_export_rows(path: &Path, format: ExportFormat, rows: &[ExportMessageRow]) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::create(path)?;
    match format {
        ExportFormat::Jsonl => {
            for row in rows {
                let line =
                    serde_json::to_string(row).map_err(|e| CliError::Parse(e.to_string()))?;
                writeln!(file, "{}", line)?;
            }
        }
        ExportFormat::Json => {
            let payload =
                serde_json::to_string_pretty(rows).map_err(|e| CliError::Parse(e.to_string()))?;
            file.write_all(payload.as_bytes())?;
        }
        ExportFormat::Csv => {
            writeln!(
                file,
                "message_id,meta_id,platform,chat_name,sender_id,sender_name,ts,msg_type,content"
            )?;
            for row in rows {
                writeln!(
                    file,
                    "{},{},{},{},{},{},{},{},{}",
                    row.message_id,
                    row.meta_id,
                    csv_escape(&row.platform),
                    csv_escape(&row.chat_name),
                    row.sender_id,
                    csv_escape(&row.sender_name),
                    row.ts,
                    row.msg_type,
                    csv_escape(row.content.as_deref().unwrap_or_default())
                )?;
            }
        }
        ExportFormat::Text => {
            for row in rows {
                writeln!(
                    file,
                    "[{}] {} / {} | sender={}({}) | type={} | {}",
                    row.ts,
                    row.platform,
                    row.chat_name,
                    row.sender_name,
                    row.sender_id,
                    row.msg_type,
                    row.content.as_deref().unwrap_or_default()
                )?;
            }
        }
        ExportFormat::Html => {
            file.write_all(
                br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Xenobot Export</title>
  <style>
    :root { color-scheme: light; }
    body { font-family: "SF Pro Text", "Segoe UI", sans-serif; margin: 24px; background: #f7fafc; color: #111827; }
    h1 { margin: 0 0 12px 0; font-size: 22px; }
    .hint { color: #4b5563; margin-bottom: 16px; }
    table { width: 100%; border-collapse: collapse; background: #fff; border: 1px solid #e5e7eb; }
    th, td { border: 1px solid #e5e7eb; padding: 8px 10px; text-align: left; font-size: 13px; vertical-align: top; }
    th { background: #f3f4f6; position: sticky; top: 0; }
    tr:nth-child(even) { background: #f9fafb; }
    code { font-family: "SF Mono", Menlo, monospace; }
  </style>
</head>
<body>
  <h1>Xenobot Message Export</h1>
  <p class="hint">Generated by xenobot-cli export</p>
  <table>
    <thead>
      <tr>
        <th>message_id</th><th>meta_id</th><th>platform</th><th>chat_name</th><th>sender_id</th><th>sender_name</th><th>ts</th><th>msg_type</th><th>content</th>
      </tr>
    </thead>
    <tbody>
"#,
            )?;
            for row in rows {
                writeln!(
                    file,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                    row.message_id,
                    row.meta_id,
                    html_escape(&row.platform),
                    html_escape(&row.chat_name),
                    row.sender_id,
                    html_escape(&row.sender_name),
                    row.ts,
                    row.msg_type,
                    html_escape(row.content.as_deref().unwrap_or_default())
                )?;
            }
            file.write_all(
                br#"    </tbody>
  </table>
</body>
</html>
"#,
            )?;
        }
    }
    Ok(())
}

fn html_escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(feature = "api")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiServerState {
    pid: i32,
    #[serde(default = "default_api_transport")]
    transport: String,
    bind_addr: String,
    unix_socket_path: Option<String>,
    #[serde(default = "default_api_unix_socket_mode")]
    unix_socket_mode: String,
    file_gateway_dir: Option<String>,
    #[serde(default = "default_file_gateway_poll_ms")]
    file_gateway_poll_ms: u64,
    #[serde(default = "default_file_gateway_response_ttl_seconds")]
    file_gateway_response_ttl_seconds: u64,
    db_path: Option<String>,
    cors_enabled: bool,
    websocket_enabled: bool,
    started_at: i64,
}

#[cfg(feature = "api")]
fn default_api_transport() -> String {
    "tcp".to_string()
}

#[cfg(feature = "api")]
fn default_api_unix_socket_mode() -> String {
    "700".to_string()
}

#[cfg(feature = "api")]
fn default_file_gateway_poll_ms() -> u64 {
    1000
}

#[cfg(feature = "api")]
fn default_file_gateway_response_ttl_seconds() -> u64 {
    300
}

#[cfg(feature = "api")]
fn api_server_state_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| CliError::Config("Cannot resolve config directory".to_string()))?;
    let dir = base.join("xenobot");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("api_server_state.json"))
}

#[cfg(feature = "api")]
fn read_api_server_state() -> Result<Option<ApiServerState>> {
    let path = api_server_state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    let parsed =
        serde_json::from_str::<ApiServerState>(&raw).map_err(|e| CliError::Parse(e.to_string()))?;
    Ok(Some(parsed))
}

#[cfg(feature = "api")]
fn write_api_server_state(state: &ApiServerState) -> Result<()> {
    let path = api_server_state_path()?;
    let payload =
        serde_json::to_string_pretty(state).map_err(|e| CliError::Parse(e.to_string()))?;
    std::fs::write(path, payload)?;
    Ok(())
}

#[cfg(feature = "api")]
fn clear_api_server_state() -> Result<()> {
    let path = api_server_state_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(feature = "api")]
fn parse_bind_addr(host: &str, port: u16) -> Result<std::net::SocketAddr> {
    format!("{}:{}", host.trim(), port)
        .parse()
        .map_err(|e: std::net::AddrParseError| {
            CliError::Argument(format!("invalid host/port: {}", e))
        })
}

#[cfg(feature = "api")]
fn parse_unix_socket_mode(raw_mode: &str) -> Result<u32> {
    let trimmed = raw_mode.trim();
    if trimmed.is_empty() {
        return Err(CliError::Argument(
            "unix socket mode cannot be empty".to_string(),
        ));
    }
    let octal = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
        .unwrap_or(trimmed);
    if !octal.chars().all(|ch| ('0'..='7').contains(&ch)) {
        return Err(CliError::Argument(format!(
            "invalid unix socket mode '{}': use octal digits only (e.g. 700 or 0o700)",
            raw_mode
        )));
    }
    let value = u32::from_str_radix(octal, 8).map_err(|e| {
        CliError::Argument(format!("invalid unix socket mode '{}': {}", raw_mode, e))
    })?;
    if value > 0o777 {
        return Err(CliError::Argument(format!(
            "unix socket mode '{}' out of range; max is 777",
            raw_mode
        )));
    }
    Ok(value)
}

#[cfg(feature = "api")]
fn build_api_config(
    bind_addr: std::net::SocketAddr,
    unix_socket: Option<PathBuf>,
    unix_socket_mode: u32,
    cors: bool,
) -> xenobot_api::config::ApiConfig {
    let mut config = xenobot_api::config::ApiConfig::default();
    config.bind_addr = bind_addr;
    config.unix_socket_path = unix_socket;
    config.unix_socket_mode = unix_socket_mode;
    config.enable_cors = cors;
    config
}

#[cfg(feature = "api")]
fn run_api_server_attempt(
    config: xenobot_api::config::ApiConfig,
    transport: &str,
    file_gateway_dir: Option<&PathBuf>,
    file_gateway_poll_ms: u64,
    file_gateway_response_ttl_seconds: u64,
    db_path: Option<PathBuf>,
    websocket: bool,
) -> Result<()> {
    let bind_addr = config.bind_addr;
    let unix_socket_path = config
        .unix_socket_path
        .as_ref()
        .map(|v| v.to_string_lossy().to_string());
    let state = ApiServerState {
        pid: std::process::id() as i32,
        transport: transport.to_string(),
        bind_addr: bind_addr.to_string(),
        unix_socket_path,
        unix_socket_mode: format!("{:o}", config.unix_socket_mode & 0o777),
        file_gateway_dir: file_gateway_dir.map(|v| v.to_string_lossy().to_string()),
        file_gateway_poll_ms: file_gateway_poll_ms.max(100),
        file_gateway_response_ttl_seconds: file_gateway_response_ttl_seconds.max(30),
        db_path: db_path.as_ref().map(|v| v.to_string_lossy().to_string()),
        cors_enabled: config.enable_cors,
        websocket_enabled: websocket,
        started_at: chrono::Utc::now().timestamp(),
    };
    write_api_server_state(&state)?;

    println!("API server start requested");
    if let Some(path) = state.unix_socket_path.as_ref() {
        println!("unix socket: {}", path);
        println!("unix socket mode: {}", state.unix_socket_mode);
    } else if let Some(dir) = state.file_gateway_dir.as_ref() {
        println!("file gateway dir: {}", dir);
        println!("file gateway poll(ms): {}", state.file_gateway_poll_ms);
        println!(
            "file gateway response ttl(s): {}",
            state.file_gateway_response_ttl_seconds
        );
    } else {
        println!("bind: {}", state.bind_addr);
    }
    println!("cors enabled: {}", state.cors_enabled);
    println!("websocket enabled: {}", state.websocket_enabled);
    if let Some(path) = db_path.as_ref() {
        println!("db path: {}", path.display());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let run_result = runtime
        .block_on(xenobot_api::start_server(config))
        .map_err(|e| CliError::Internal(e.to_string()));
    if let Err(err) = clear_api_server_state() {
        eprintln!("warn: failed to clear api server state file: {}", err);
    }
    if let Some(path) = state.unix_socket_path.as_ref() {
        let _ = std::fs::remove_file(path);
    }
    run_result
}

#[cfg(feature = "api")]
fn looks_like_bind_permission_issue(err: &CliError) -> bool {
    let text = err.to_string().to_ascii_lowercase();
    let permission_related = text.contains("operation not permitted")
        || text.contains("permission denied")
        || text.contains("os error 1")
        || text.contains("eperm");
    let bind_related = text.contains("bind")
        || text.contains("listener")
        || text.contains("socket")
        || text.contains("addr");
    permission_related && bind_related
}

#[cfg(all(feature = "api", unix))]
fn select_sandbox_safe_unix_socket_path() -> Result<PathBuf> {
    xenobot_core::sandbox::select_sandbox_safe_unix_socket_path()
        .map_err(|e| CliError::Argument(e.to_string()))
}

#[cfg(feature = "api")]
fn select_file_gateway_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    xenobot_core::sandbox::select_file_gateway_root(explicit)
        .map_err(|e| CliError::Argument(e.to_string()))
}

#[cfg(feature = "api")]
#[derive(Debug, Deserialize)]
struct FileGatewayRequest {
    id: Option<String>,
    method: Option<String>,
    path: Option<String>,
    params: Option<serde_json::Value>,
    body: Option<serde_json::Value>,
    headers: Option<HashMap<String, String>>,
    timestamp: Option<i64>,
    ttl: Option<u64>,
}

#[cfg(feature = "api")]
#[derive(Debug, Serialize)]
struct FileGatewayResponse {
    id: String,
    ok: bool,
    status: u16,
    timestamp: i64,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

#[cfg(feature = "api")]
#[derive(Debug, Clone, Default)]
struct FileGatewayBatchMetrics {
    queue_depth: usize,
    processed: usize,
    succeeded: usize,
    failed: usize,
    lock_contended: usize,
    latency_samples_ms: Vec<u64>,
}

#[cfg(feature = "api")]
#[derive(Debug, Clone, Default)]
struct FileGatewayRuntimeMetrics {
    started_at: i64,
    total_processed: u64,
    total_succeeded: u64,
    total_failed: u64,
    total_lock_contended: u64,
    last_queue_depth: usize,
    last_processed: usize,
    window_latencies_ms: VecDeque<u64>,
}

#[cfg(feature = "api")]
#[derive(Debug, Clone, Serialize)]
struct FileGatewayMetricsSnapshot {
    timestamp: i64,
    started_at: i64,
    total_processed: u64,
    total_succeeded: u64,
    total_failed: u64,
    total_lock_contended: u64,
    queue_depth: usize,
    last_tick_processed: usize,
    latency_avg_ms: f64,
    latency_p95_ms: u64,
    latency_max_ms: u64,
}

#[cfg(feature = "api")]
impl FileGatewayRuntimeMetrics {
    fn with_started_at(started_at: i64) -> Self {
        Self {
            started_at,
            ..Self::default()
        }
    }

    fn record_batch(&mut self, batch: &FileGatewayBatchMetrics) {
        const WINDOW_CAP: usize = 2048;

        self.total_processed = self
            .total_processed
            .saturating_add(batch.processed.min(u64::MAX as usize) as u64);
        self.total_succeeded = self
            .total_succeeded
            .saturating_add(batch.succeeded.min(u64::MAX as usize) as u64);
        self.total_failed = self
            .total_failed
            .saturating_add(batch.failed.min(u64::MAX as usize) as u64);
        self.total_lock_contended = self
            .total_lock_contended
            .saturating_add(batch.lock_contended.min(u64::MAX as usize) as u64);
        self.last_queue_depth = batch.queue_depth;
        self.last_processed = batch.processed;
        for latency in &batch.latency_samples_ms {
            self.window_latencies_ms.push_back(*latency);
            if self.window_latencies_ms.len() > WINDOW_CAP {
                let _ = self.window_latencies_ms.pop_front();
            }
        }
    }

    fn snapshot(&self) -> FileGatewayMetricsSnapshot {
        let mut sorted = self.window_latencies_ms.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();
        let latency_avg_ms = if sorted.is_empty() {
            0.0
        } else {
            let sum: u128 = sorted.iter().copied().map(u128::from).sum();
            sum as f64 / sorted.len() as f64
        };
        let latency_p95_ms = if sorted.is_empty() {
            0
        } else {
            let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
            sorted[idx.saturating_sub(1).min(sorted.len().saturating_sub(1))]
        };
        let latency_max_ms = sorted.last().copied().unwrap_or(0);

        FileGatewayMetricsSnapshot {
            timestamp: chrono::Utc::now().timestamp(),
            started_at: self.started_at,
            total_processed: self.total_processed,
            total_succeeded: self.total_succeeded,
            total_failed: self.total_failed,
            total_lock_contended: self.total_lock_contended,
            queue_depth: self.last_queue_depth,
            last_tick_processed: self.last_processed,
            latency_avg_ms,
            latency_p95_ms,
            latency_max_ms,
        }
    }
}

#[cfg(feature = "api")]
fn sanitize_file_gateway_id(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(80)
        .collect();
    if filtered.is_empty() {
        format!("req_{}", chrono::Utc::now().timestamp_millis())
    } else {
        filtered
    }
}

#[cfg(feature = "api")]
fn extract_request_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let raw = stem.strip_prefix("req_")?;
    let sanitized = sanitize_file_gateway_id(raw);
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(feature = "api")]
fn parse_file_gateway_http_target(
    req: &FileGatewayRequest,
) -> Result<(axum::http::Method, String, Option<serde_json::Value>)> {
    let method_raw = req.method.as_deref().unwrap_or("GET").trim();
    let upper = method_raw.to_ascii_uppercase();
    let is_http_verb = matches!(
        upper.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    );
    if is_http_verb {
        let method = axum::http::Method::from_bytes(upper.as_bytes()).map_err(|e| {
            CliError::Argument(format!("invalid HTTP method '{}': {}", method_raw, e))
        })?;
        let path = req.path.as_deref().unwrap_or("/health").trim();
        let normalized_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
        return Ok((
            method,
            normalized_path,
            req.body.clone().or(req.params.clone()),
        ));
    }

    let mapped = match method_raw {
        "health" | "health.check" => ("GET", "/health".to_string()),
        "chat.sessions.list" => ("GET", "/chat/sessions".to_string()),
        "chat.import" => ("POST", "/chat/import".to_string()),
        "ai.search_messages" | "chat.search" => ("POST", "/ai/search-messages".to_string()),
        "llm.chat" => ("POST", "/llm/chat".to_string()),
        _ => ("POST", format!("/{}", method_raw.replace('.', "/"))),
    };
    let method = axum::http::Method::from_bytes(mapped.0.as_bytes())
        .map_err(|e| CliError::Argument(format!("invalid mapped HTTP method: {}", e)))?;
    Ok((method, mapped.1, req.body.clone().or(req.params.clone())))
}

#[cfg(feature = "api")]
async fn dispatch_file_gateway_request(
    router: &axum::Router,
    req: &FileGatewayRequest,
) -> Result<axum::response::Response> {
    use tower::util::ServiceExt;

    let (method, path, body_json) = parse_file_gateway_http_target(req)?;
    let uri = path
        .parse::<axum::http::Uri>()
        .map_err(|e| CliError::Argument(format!("invalid path '{}': {}", path, e)))?;

    let mut builder = axum::http::Request::builder().method(method).uri(uri);
    if let Some(headers) = req.headers.as_ref() {
        for (name, value) in headers {
            if let (Ok(name), Ok(value)) = (
                axum::http::header::HeaderName::from_bytes(name.as_bytes()),
                axum::http::HeaderValue::from_str(value),
            ) {
                builder = builder.header(name, value);
            }
        }
    }

    let body = if let Some(json) = body_json {
        builder = builder.header(axum::http::header::CONTENT_TYPE, "application/json");
        axum::body::Body::from(
            serde_json::to_vec(&json)
                .map_err(|e| CliError::Parse(format!("failed to serialize request body: {}", e)))?,
        )
    } else {
        axum::body::Body::empty()
    };
    let request = builder
        .body(body)
        .map_err(|e| CliError::Internal(format!("failed to build file gateway request: {}", e)))?;

    let response = router
        .clone()
        .oneshot(request)
        .await
        .unwrap_or_else(|err| match err {});
    Ok(response)
}

#[cfg(feature = "api")]
fn cleanup_file_gateway_artifacts(
    root: &Path,
    response_ttl_seconds: u64,
    stale_lock_seconds: u64,
) -> Result<()> {
    let now = std::time::SystemTime::now();
    let ttl = std::time::Duration::from_secs(response_ttl_seconds.max(30));
    let stale_lock = std::time::Duration::from_secs(stale_lock_seconds.max(60));
    let entries = std::fs::read_dir(root)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !(file_name.starts_with("resp_") && file_name.ends_with(".json"))
            && !(file_name.starts_with("req_") && file_name.ends_with(".lock"))
        {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let modified = match meta.modified() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let age = match now.duration_since(modified) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if file_name.ends_with(".lock") && age > stale_lock {
            let _ = std::fs::remove_file(path);
        } else if file_name.starts_with("resp_") && age > ttl {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[cfg(feature = "api")]
async fn process_pending_file_gateway_requests(
    root: &Path,
    router: &axum::Router,
) -> Result<FileGatewayBatchMetrics> {
    let mut request_paths: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(root)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with("req_") && name.ends_with(".json")) {
            continue;
        }
        request_paths.push(path);
    }
    request_paths.sort();

    let mut metrics = FileGatewayBatchMetrics {
        queue_depth: request_paths.len(),
        ..FileGatewayBatchMetrics::default()
    };
    for req_path in request_paths {
        let started = std::time::Instant::now();
        let lock_path = req_path.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path);
        let lock_handle = match lock_file {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                metrics.lock_contended = metrics.lock_contended.saturating_add(1);
                continue;
            }
            Err(err) => {
                eprintln!(
                    "[file-api] failed to create lock for {}: {}",
                    req_path.display(),
                    err
                );
                continue;
            }
        };
        drop(lock_handle);

        let response_payload = async {
            let meta = std::fs::symlink_metadata(&req_path).map_err(CliError::from)?;
            if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                return Err(CliError::Argument(format!(
                    "request is not a regular file: {}",
                    req_path.display()
                )));
            }

            let raw = std::fs::read_to_string(&req_path)?;
            let req: FileGatewayRequest =
                serde_json::from_str(&raw).map_err(|e| CliError::Parse(e.to_string()))?;
            let req_id = req
                .id
                .as_deref()
                .map(sanitize_file_gateway_id)
                .or_else(|| extract_request_id_from_path(&req_path))
                .unwrap_or_else(|| format!("req_{}", chrono::Utc::now().timestamp_millis()));

            if let (Some(ts), Some(ttl)) = (req.timestamp, req.ttl) {
                let now = chrono::Utc::now().timestamp();
                if now.saturating_sub(ts) > ttl as i64 {
                    return Ok(FileGatewayResponse {
                        id: req_id,
                        ok: false,
                        status: 408,
                        timestamp: now,
                        result: None,
                        error: Some("request expired (ttl exceeded)".to_string()),
                    });
                }
            }

            let response = dispatch_file_gateway_request(router, &req).await?;
            let status = response.status();
            let body_bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
                .await
                .map_err(|e| CliError::Internal(format!("failed to read response body: {}", e)))?;
            let body_text = String::from_utf8_lossy(&body_bytes).to_string();
            let parsed_body = serde_json::from_slice::<serde_json::Value>(&body_bytes)
                .ok()
                .or_else(|| {
                    if body_text.trim().is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::String(body_text.clone()))
                    }
                });

            let error = if status.is_success() {
                None
            } else if let Some(serde_json::Value::Object(map)) = parsed_body.as_ref() {
                map.get("error")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
                    .or_else(|| Some(format!("request failed with status {}", status.as_u16())))
            } else {
                Some(format!("request failed with status {}", status.as_u16()))
            };

            Ok(FileGatewayResponse {
                id: req_id,
                ok: status.is_success(),
                status: status.as_u16(),
                timestamp: chrono::Utc::now().timestamp(),
                result: parsed_body,
                error,
            })
        }
        .await;

        let (response_id, response_obj) = match response_payload {
            Ok(resp) => (resp.id.clone(), resp),
            Err(err) => {
                let fallback_id = extract_request_id_from_path(&req_path)
                    .unwrap_or_else(|| format!("req_{}", chrono::Utc::now().timestamp_millis()));
                (
                    fallback_id.clone(),
                    FileGatewayResponse {
                        id: fallback_id,
                        ok: false,
                        status: 500,
                        timestamp: chrono::Utc::now().timestamp(),
                        result: None,
                        error: Some(err.to_string()),
                    },
                )
            }
        };

        let response_path = root.join(format!(
            "resp_{}.json",
            sanitize_file_gateway_id(&response_id)
        ));
        let tmp_response_path = root.join(format!(
            "resp_{}.json.tmp",
            sanitize_file_gateway_id(&response_id)
        ));
        let response_raw = serde_json::to_string_pretty(&response_obj)
            .map_err(|e| CliError::Parse(e.to_string()))?;
        std::fs::write(&tmp_response_path, response_raw)?;
        std::fs::rename(&tmp_response_path, &response_path)?;

        let _ = std::fs::remove_file(&req_path);
        let _ = std::fs::remove_file(&lock_path);
        metrics.processed = metrics.processed.saturating_add(1);
        if response_obj.ok {
            metrics.succeeded = metrics.succeeded.saturating_add(1);
        } else {
            metrics.failed = metrics.failed.saturating_add(1);
        }
        metrics
            .latency_samples_ms
            .push(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
    }

    Ok(metrics)
}

#[cfg(feature = "api")]
fn run_api_file_gateway_mode(
    gateway_root: PathBuf,
    file_gateway_poll_ms: u64,
    file_gateway_response_ttl_seconds: u64,
    db_path: Option<PathBuf>,
    cors: bool,
    websocket: bool,
    unix_socket_mode: u32,
    bind_addr: std::net::SocketAddr,
) -> Result<()> {
    std::fs::create_dir_all(&gateway_root)?;
    if let Some(path) = db_path.as_ref() {
        std::env::set_var("XENOBOT_DB_PATH", path.as_os_str());
    }

    let state = ApiServerState {
        pid: std::process::id() as i32,
        transport: "file-gateway".to_string(),
        bind_addr: bind_addr.to_string(),
        unix_socket_path: None,
        unix_socket_mode: format!("{:o}", unix_socket_mode & 0o777),
        file_gateway_dir: Some(gateway_root.to_string_lossy().to_string()),
        file_gateway_poll_ms: file_gateway_poll_ms.max(100),
        file_gateway_response_ttl_seconds: file_gateway_response_ttl_seconds.max(30),
        db_path: db_path.as_ref().map(|v| v.to_string_lossy().to_string()),
        cors_enabled: cors,
        websocket_enabled: websocket,
        started_at: chrono::Utc::now().timestamp(),
    };
    write_api_server_state(&state)?;

    println!("API server start requested");
    println!("file gateway dir: {}", gateway_root.display());
    println!("file gateway poll(ms): {}", state.file_gateway_poll_ms);
    println!(
        "file gateway response ttl(s): {}",
        state.file_gateway_response_ttl_seconds
    );
    println!("request pattern: req_<id>.json");
    println!("response pattern: resp_<id>.json");
    println!("cors enabled: {}", state.cors_enabled);
    println!("websocket enabled: {}", state.websocket_enabled);
    if let Some(path) = db_path.as_ref() {
        println!("db path: {}", path.display());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let root_for_run = gateway_root.clone();
    let run_result = runtime
        .block_on(async move {
            use notify::Watcher;

            xenobot_api::database::init_database()
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;

            let config = xenobot_api::config::ApiConfig::default();
            let router = xenobot_api::router::build_router(&config);

            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
            let mut watcher =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res {
                        let should_scan = matches!(
                            event.kind,
                            notify::EventKind::Any
                                | notify::EventKind::Create(_)
                                | notify::EventKind::Modify(_)
                                | notify::EventKind::Remove(_)
                                | notify::EventKind::Other
                        );
                        if should_scan {
                            let _ = event_tx.send(());
                        }
                    } else {
                        let _ = event_tx.send(());
                    }
                })
                .map_err(|e| {
                    CliError::Internal(format!("failed to initialize file watcher: {}", e))
                })?;
            watcher
                .watch(&root_for_run, notify::RecursiveMode::NonRecursive)
                .map_err(|e| {
                    CliError::Internal(format!("failed to watch {}: {}", root_for_run.display(), e))
                })?;

            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(
                file_gateway_poll_ms.max(100),
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let metrics_path = root_for_run.join("gateway_metrics.json");
            let mut runtime_metrics = FileGatewayRuntimeMetrics::with_started_at(
                chrono::Utc::now().timestamp(),
            );
            let mut cleanup_every = 0u64;
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        println!("file gateway mode received Ctrl+C, shutting down");
                        break;
                    }
                    _ = ticker.tick() => {}
                    maybe = event_rx.recv() => {
                        if maybe.is_none() {
                            break;
                        }
                    }
                }

                let batch =
                    process_pending_file_gateway_requests(&root_for_run, &router).await?;
                runtime_metrics.record_batch(&batch);
                if batch.processed > 0 || batch.queue_depth > 0 {
                    let snapshot = runtime_metrics.snapshot();
                    println!(
                        "[file-api] queue={} processed={} ok={} failed={} lock_contended={} latency(avg/p95/max)={:.2}/{}/{}ms",
                        snapshot.queue_depth,
                        batch.processed,
                        batch.succeeded,
                        batch.failed,
                        batch.lock_contended,
                        snapshot.latency_avg_ms,
                        snapshot.latency_p95_ms,
                        snapshot.latency_max_ms
                    );

                    let metrics_raw = serde_json::to_string_pretty(&snapshot)
                        .map_err(|e| CliError::Parse(e.to_string()))?;
                    let tmp_path = metrics_path.with_extension("json.tmp");
                    std::fs::write(&tmp_path, metrics_raw)?;
                    std::fs::rename(&tmp_path, &metrics_path)?;
                }

                cleanup_every = cleanup_every.saturating_add(1);
                if cleanup_every % 10 == 0 {
                    let _ = cleanup_file_gateway_artifacts(
                        &root_for_run,
                        file_gateway_response_ttl_seconds,
                        file_gateway_response_ttl_seconds.saturating_mul(2),
                    );
                }
            }

            Ok::<(), CliError>(())
        })
        .map_err(|e| CliError::Internal(e.to_string()));

    if let Err(err) = clear_api_server_state() {
        eprintln!("warn: failed to clear api server state file: {}", err);
    }
    run_result
}

#[cfg(feature = "api")]
fn start_api_server_foreground(
    host: &str,
    port: u16,
    unix_socket: Option<PathBuf>,
    unix_socket_mode: &str,
    file_gateway_dir: Option<PathBuf>,
    file_gateway_poll_ms: u64,
    file_gateway_response_ttl_seconds: u64,
    force_file_gateway: bool,
    db_path: Option<PathBuf>,
    cors: bool,
    websocket: bool,
) -> Result<()> {
    let bind_addr = if unix_socket.is_some() {
        "127.0.0.1:0"
            .parse()
            .map_err(|e| CliError::Internal(format!("fallback socket parse failed: {}", e)))?
    } else {
        parse_bind_addr(host, port)?
    };

    if let Some(existing) = read_api_server_state()? {
        if api_pid_is_alive(existing.pid) {
            let endpoint = if let Some(path) = existing.unix_socket_path.as_ref() {
                format!("unix://{}", path)
            } else if let Some(dir) = existing.file_gateway_dir.as_ref() {
                format!("file://{}", dir)
            } else {
                existing.bind_addr.clone()
            };
            return Err(CliError::Argument(format!(
                "API server already running: pid={} transport={} endpoint={}",
                existing.pid, existing.transport, endpoint
            )));
        }
        if let Some(path) = existing.unix_socket_path.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        let _ = clear_api_server_state();
    }

    if let Some(path) = db_path.as_ref() {
        std::env::set_var("XENOBOT_DB_PATH", path.as_os_str());
    }

    let socket_mode = parse_unix_socket_mode(unix_socket_mode)?;
    let poll_ms = file_gateway_poll_ms.max(100);
    let response_ttl_seconds = file_gateway_response_ttl_seconds.max(30);

    if force_file_gateway {
        let gateway_root = select_file_gateway_root(file_gateway_dir)?;
        println!("force file-gateway mode enabled (sandbox-coexist)");
        return run_api_file_gateway_mode(
            gateway_root,
            poll_ms,
            response_ttl_seconds,
            db_path,
            cors,
            websocket,
            socket_mode,
            bind_addr,
        );
    }

    let primary_config = build_api_config(bind_addr, unix_socket.clone(), socket_mode, cors);
    match run_api_server_attempt(
        primary_config,
        if unix_socket.is_some() { "unix" } else { "tcp" },
        file_gateway_dir.as_ref(),
        poll_ms,
        response_ttl_seconds,
        db_path.clone(),
        websocket,
    ) {
        Ok(()) => Ok(()),
        Err(primary_err) => {
            if unix_socket.is_some() || !looks_like_bind_permission_issue(&primary_err) {
                return Err(primary_err);
            }

            #[cfg(unix)]
            {
                let fallback_socket = select_sandbox_safe_unix_socket_path()?;
                println!(
                    "tcp bind denied by sandbox, retrying with unix socket: {}",
                    fallback_socket.display()
                );
                let fallback_bind_addr: std::net::SocketAddr =
                    "127.0.0.1:0".parse().map_err(|e| {
                        CliError::Internal(format!("fallback socket parse failed: {}", e))
                    })?;
                let fallback_config =
                    build_api_config(fallback_bind_addr, Some(fallback_socket), socket_mode, cors);
                match run_api_server_attempt(
                    fallback_config,
                    "unix",
                    file_gateway_dir.as_ref(),
                    poll_ms,
                    response_ttl_seconds,
                    db_path.clone(),
                    websocket,
                ) {
                    Ok(()) => Ok(()),
                    Err(fallback_err) => {
                        if looks_like_bind_permission_issue(&fallback_err) {
                            let gateway_root = select_file_gateway_root(file_gateway_dir)?;
                            println!(
                                "API listener blocked by sandbox for TCP+UDS, switching to file gateway IPC mode"
                            );
                            return run_api_file_gateway_mode(
                                gateway_root,
                                poll_ms,
                                response_ttl_seconds,
                                db_path,
                                cors,
                                websocket,
                                socket_mode,
                                bind_addr,
                            );
                        }
                        Err(fallback_err)
                    }
                }
            }
            #[cfg(not(unix))]
            {
                if looks_like_bind_permission_issue(&primary_err) {
                    let gateway_root = select_file_gateway_root(file_gateway_dir)?;
                    println!(
                        "API listener blocked by sandbox for TCP, switching to file gateway IPC mode"
                    );
                    return run_api_file_gateway_mode(
                        gateway_root,
                        poll_ms,
                        response_ttl_seconds,
                        db_path,
                        cors,
                        websocket,
                        socket_mode,
                        bind_addr,
                    );
                }
                Err(primary_err)
            }
        }
    }
}

#[cfg(feature = "api")]
fn print_api_server_status(format: &OutputFormat) -> Result<()> {
    use std::net::SocketAddr;
    use std::net::TcpStream;
    use std::time::Duration;

    if let Some(state) = read_api_server_state()? {
        let pid_alive = api_pid_is_alive(state.pid);
        let transport = state.transport.to_ascii_lowercase();
        let endpoint_alive = if transport == "unix" {
            state
                .unix_socket_path
                .as_ref()
                .map(|path| std::path::Path::new(path).exists())
                .unwrap_or(false)
        } else if transport == "file-gateway" {
            state
                .file_gateway_dir
                .as_ref()
                .map(|dir| {
                    let path = std::path::Path::new(dir);
                    path.exists() && path.is_dir()
                })
                .unwrap_or(false)
        } else {
            state
                .bind_addr
                .parse::<SocketAddr>()
                .ok()
                .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok())
                .unwrap_or(false)
        };
        let status_snapshot =
            if transport == "tcp" && endpoint_alive {
                state.bind_addr.parse::<SocketAddr>().ok().and_then(|addr| {
                    fetch_status_snapshot_via_tcp(addr, Duration::from_millis(800))
                })
            } else {
                None
            };
        let gateway_metrics = if transport == "file-gateway" {
            state.file_gateway_dir.as_ref().and_then(|dir| {
                let metrics_path = std::path::Path::new(dir).join("gateway_metrics.json");
                std::fs::read_to_string(&metrics_path)
                    .ok()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            })
        } else {
            None
        };
        let status = if transport == "file-gateway" {
            if endpoint_alive && pid_alive {
                "running"
            } else if endpoint_alive {
                "running_or_restricted"
            } else if pid_alive {
                "starting_or_unhealthy"
            } else {
                "stopped"
            }
        } else if pid_alive && endpoint_alive {
            "running"
        } else if pid_alive {
            "starting_or_unhealthy"
        } else {
            "stopped"
        };

        let mut report = serde_json::json!({
            "transport": transport,
            "apiAddr": state.bind_addr,
            "pid": state.pid,
            "stateFile": "present",
            "status": status,
            "pidAlive": pid_alive,
            "endpointAlive": endpoint_alive,
            "corsEnabled": state.cors_enabled,
            "websocketEnabled": state.websocket_enabled,
            "dbPath": state.db_path,
            "unixSocketPath": state.unix_socket_path,
            "unixSocketMode": state.unix_socket_mode,
            "fileGatewayDir": state.file_gateway_dir,
            "fileGatewayPollMs": state.file_gateway_poll_ms,
            "fileGatewayResponseTtlSeconds": state.file_gateway_response_ttl_seconds,
            "gatewayMetrics": gateway_metrics,
        });
        if let Some(snapshot) = status_snapshot.clone() {
            if let Some(map) = report.as_object_mut() {
                map.insert("statusSnapshot".to_string(), snapshot);
            }
        }

        if matches!(
            format,
            OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv
        ) {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .map_err(|e| CliError::Internal(format!("format api status failed: {}", e)))?
            );
        } else {
            if transport == "unix" {
                let path = state.unix_socket_path.as_deref().unwrap_or_default();
                println!("api transport: unix");
                println!("api socket: {}", path);
                println!("api socket mode: {}", state.unix_socket_mode);
            } else if transport == "file-gateway" {
                let dir = state.file_gateway_dir.as_deref().unwrap_or_default();
                println!("api transport: file-gateway");
                println!("api gateway dir: {}", dir);
                println!("api gateway poll(ms): {}", state.file_gateway_poll_ms);
                println!(
                    "api gateway response ttl(s): {}",
                    state.file_gateway_response_ttl_seconds
                );
                let metrics_path = std::path::Path::new(dir).join("gateway_metrics.json");
                if let Some(metrics) = gateway_metrics {
                    let total_processed = metrics
                        .get("total_processed")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let latency_p95 = metrics
                        .get("latency_p95_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let queue_depth = metrics
                        .get("queue_depth")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    println!("api gateway metrics file: {}", metrics_path.display());
                    println!("api gateway metrics total processed: {}", total_processed);
                    println!("api gateway metrics queue depth: {}", queue_depth);
                    println!("api gateway metrics latency p95(ms): {}", latency_p95);
                }
            } else {
                println!("api transport: tcp");
                println!("api addr: {}", state.bind_addr);
                if let Some(snapshot) = status_snapshot {
                    print_status_snapshot(&snapshot);
                } else if endpoint_alive {
                    println!("api status endpoint: unavailable_or_non_json");
                }
            }
            println!("pid: {}", state.pid);
            println!("state file: present");
            println!("status: {}", status);
            println!("cors enabled: {}", state.cors_enabled);
            println!("websocket enabled: {}", state.websocket_enabled);
            if let Some(path) = state.db_path {
                println!("db path: {}", path);
            }
        }
        return Ok(());
    }

    let target = std::env::var("XENOBOT_API_ADDR").unwrap_or_else(|_| "127.0.0.1:5030".to_string());
    let parsed = target
        .parse::<SocketAddr>()
        .map_err(|e| CliError::Argument(format!("invalid XENOBOT_API_ADDR '{}': {}", target, e)))?;
    let alive = TcpStream::connect_timeout(&parsed, Duration::from_millis(400)).is_ok();
    let status_snapshot = if alive {
        fetch_status_snapshot_via_tcp(parsed, Duration::from_millis(800))
    } else {
        None
    };
    let mut report = serde_json::json!({
        "transport": "tcp",
        "apiAddr": parsed.to_string(),
        "stateFile": "missing",
        "status": if alive { "running" } else { "stopped" },
        "endpointAlive": alive,
    });
    if let Some(snapshot) = status_snapshot.clone() {
        if let Some(map) = report.as_object_mut() {
            map.insert("statusSnapshot".to_string(), snapshot);
        }
    }

    if matches!(
        format,
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv
    ) {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| CliError::Internal(format!("format api status failed: {}", e)))?
        );
    } else {
        println!("api addr: {}", parsed);
        if let Some(snapshot) = status_snapshot {
            print_status_snapshot(&snapshot);
        } else if alive {
            println!("api status endpoint: unavailable_or_non_json");
        }
        println!("state file: missing");
        println!("status: {}", if alive { "running" } else { "stopped" });
    }
    Ok(())
}

#[cfg(feature = "api")]
fn fetch_status_snapshot_via_tcp(
    addr: std::net::SocketAddr,
    timeout: std::time::Duration,
) -> Option<serde_json::Value> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let request = format!(
        "GET /status HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    parse_http_json_response_body(&raw)
}

#[cfg(feature = "api")]
fn parse_http_json_response_body(raw: &[u8]) -> Option<serde_json::Value> {
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let body = raw.get(header_end + 4..)?;
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice::<serde_json::Value>(body).ok()
}

#[cfg(feature = "api")]
fn print_status_snapshot(snapshot: &serde_json::Value) {
    println!("api status endpoint: /status");
    if let Some(service) = snapshot.get("service").and_then(|v| v.as_str()) {
        println!("api service: {}", service);
    }
    if let Some(version) = snapshot.get("version").and_then(|v| v.as_str()) {
        println!("api version: {}", version);
    }
    if let Some(runtime_arch) = snapshot
        .get("runtime")
        .and_then(|v| v.get("arch"))
        .and_then(|v| v.as_str())
    {
        println!("api runtime arch: {}", runtime_arch);
    }
    if let Some(features) = snapshot.get("features").and_then(|v| v.as_object()) {
        let enabled = features
            .values()
            .filter(|value| value.as_bool().unwrap_or(false))
            .count();
        println!("api features enabled: {}/{}", enabled, features.len());
    }
}

#[cfg(feature = "api")]
fn stop_api_server(force: bool) -> Result<()> {
    let Some(state) = read_api_server_state()? else {
        println!("API server is not running (state file not found)");
        return Ok(());
    };

    if !api_pid_is_alive(state.pid) {
        if let Some(path) = state.unix_socket_path.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        let _ = clear_api_server_state();
        println!("API server is not running (stale state cleared)");
        return Ok(());
    }

    send_stop_signal(state.pid, force)?;
    let deadline = std::time::Instant::now()
        + if force {
            std::time::Duration::from_secs(2)
        } else {
            std::time::Duration::from_secs(8)
        };
    while std::time::Instant::now() < deadline {
        if !api_pid_is_alive(state.pid) {
            if let Some(path) = state.unix_socket_path.as_ref() {
                let _ = std::fs::remove_file(path);
            }
            let _ = clear_api_server_state();
            println!("API server stopped: pid={}", state.pid);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    if force {
        return Err(CliError::Internal(format!(
            "failed to stop API server pid={} within timeout",
            state.pid
        )));
    }

    println!(
        "API server did not stop within timeout (pid={}), try --force",
        state.pid
    );
    Ok(())
}

#[cfg(feature = "api")]
fn restart_api_server(force: bool) -> Result<()> {
    let previous = read_api_server_state()?;
    stop_api_server(force)?;

    let (
        host,
        port,
        unix_socket,
        unix_socket_mode,
        file_gateway_dir,
        file_gateway_poll_ms,
        file_gateway_response_ttl_seconds,
        force_file_gateway,
        db_path,
        cors_enabled,
        websocket_enabled,
    ) = if let Some(state) = previous {
        let addr = state
            .bind_addr
            .parse::<std::net::SocketAddr>()
            .map_err(|e| CliError::Parse(format!("invalid saved bind addr: {}", e)))?;
        (
            addr.ip().to_string(),
            addr.port(),
            state.unix_socket_path.map(PathBuf::from),
            state.unix_socket_mode,
            state.file_gateway_dir.map(PathBuf::from),
            state.file_gateway_poll_ms,
            state.file_gateway_response_ttl_seconds,
            state.transport.eq_ignore_ascii_case("file-gateway"),
            state.db_path.map(PathBuf::from),
            state.cors_enabled,
            state.websocket_enabled,
        )
    } else {
        (
            "127.0.0.1".to_string(),
            5030,
            None,
            "700".to_string(),
            None,
            1000,
            300,
            false,
            None,
            false,
            true,
        )
    };

    println!("restarting API server...");
    start_api_server_foreground(
        host.as_str(),
        port,
        unix_socket,
        unix_socket_mode.as_str(),
        file_gateway_dir,
        file_gateway_poll_ms,
        file_gateway_response_ttl_seconds,
        force_file_gateway,
        db_path,
        cors_enabled,
        websocket_enabled,
    )
}

#[cfg(feature = "api")]
fn api_pid_is_alive(pid: i32) -> bool {
    use std::process::Stdio;

    let kill_alive = std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if kill_alive {
        return true;
    }

    std::process::Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("pid=")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(feature = "api")]
fn send_stop_signal(pid: i32, force: bool) -> Result<()> {
    use std::process::Stdio;

    let signal = if force { "-KILL" } else { "-TERM" };
    let status = std::process::Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| CliError::Internal(format!("failed to invoke kill command: {}", e)))?;
    if status.success() {
        return Ok(());
    }
    Err(CliError::Internal(format!(
        "failed to signal pid {} via kill {}",
        pid, signal
    )))
}

#[cfg(feature = "api")]
fn run_api_sandbox_doctor(file_gateway_dir: Option<PathBuf>, format: OutputFormat) -> Result<()> {
    let now = chrono::Utc::now();
    let report = xenobot_core::sandbox::diagnose_sandbox(file_gateway_dir)
        .map_err(|e| CliError::Internal(e.to_string()))?;
    let recommended_mode = report.recommended.mode.clone();
    let recommended_command = report.recommended.command.clone();

    let payload = serde_json::json!({
        "timestamp": now.to_rfc3339(),
        "tcp": report.tcp,
        "uds": report.uds,
        "fileGateway": report.file_gateway,
        "recommended": report.recommended,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).map_err(|e| CliError::Internal(format!(
                    "format sandbox doctor output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("sandbox doctor");
            println!("tcp allowed: {}", report.tcp.allowed);
            if let Some(err) = report.tcp.error.as_ref() {
                println!("tcp error: {}", err);
            }
            println!("uds supported: {}", report.uds.supported);
            println!("uds allowed: {}", report.uds.allowed);
            if let Some(path) = report.uds.path.as_ref() {
                println!("uds path probe: {}", path);
            }
            if let Some(err) = report.uds.error.as_ref() {
                println!("uds error: {}", err);
            }
            println!("file gateway dir: {}", report.file_gateway.dir);
            println!("file gateway writable: {}", report.file_gateway.writable);
            if let Some(err) = report.file_gateway.error.as_ref() {
                println!("file gateway error: {}", err);
            }
            println!("recommended mode: {}", recommended_mode);
            println!("recommended command: {}", recommended_command);
        }
    }

    if recommended_mode == "file-gateway" && !report.file_gateway.writable {
        return Err(CliError::Internal(
            "sandbox doctor detected listener restrictions and non-writable file gateway path"
                .to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn parse_optional_json_body(raw: Option<String>) -> Result<Option<serde_json::Value>> {
    let Some(raw_value) = raw else {
        return Ok(None);
    };
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| CliError::Argument(format!("invalid --body-json: {}", e)))?;
    Ok(Some(parsed))
}

#[cfg(feature = "api")]
fn run_api_file_gateway_call(
    file_gateway_dir: Option<PathBuf>,
    request_id: Option<String>,
    method: String,
    path: Option<String>,
    body_json: Option<String>,
    timeout_ms: u64,
    format: OutputFormat,
) -> Result<()> {
    let gateway_root = select_file_gateway_root(file_gateway_dir)?;
    std::fs::create_dir_all(&gateway_root)?;
    let method_trimmed = method.trim().to_string();
    if method_trimmed.is_empty() {
        return Err(CliError::Argument(
            "gateway-call method must not be empty".to_string(),
        ));
    }
    let body_value = parse_optional_json_body(body_json)?;
    let req_id = sanitize_file_gateway_id(request_id.as_deref().unwrap_or(&format!(
        "manual_{}_{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    )));
    let per_request_timeout = timeout_ms.max(100);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let gateway_root_for_task = gateway_root.clone();
    let req_id_for_task = req_id.clone();
    let method_for_task = method_trimmed.clone();
    let path_for_task = path.clone();
    let body_for_task = body_value.clone();
    let response_json = runtime.block_on(async move {
        let req_path = gateway_root_for_task.join(format!("req_{}.json", req_id_for_task));
        let req_tmp_path = gateway_root_for_task.join(format!("req_{}.json.tmp", req_id_for_task));
        let resp_path = gateway_root_for_task.join(format!("resp_{}.json", req_id_for_task));

        let request_payload = serde_json::json!({
            "id": req_id_for_task,
            "method": method_for_task,
            "path": path_for_task,
            "body": body_for_task,
            "timestamp": chrono::Utc::now().timestamp(),
            "ttl": ((per_request_timeout / 1000).max(1) + 5),
        });

        tokio::fs::write(
            &req_tmp_path,
            serde_json::to_vec(&request_payload).map_err(|e| {
                CliError::Internal(format!("serialize gateway request failed: {}", e))
            })?,
        )
        .await
        .map_err(|e| CliError::Internal(format!("write gateway request failed: {}", e)))?;
        tokio::fs::rename(&req_tmp_path, &req_path)
            .await
            .map_err(|e| CliError::Internal(format!("commit gateway request failed: {}", e)))?;

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(per_request_timeout);
        loop {
            if tokio::fs::metadata(&resp_path).await.is_ok() {
                let response_raw = tokio::fs::read_to_string(&resp_path).await.map_err(|e| {
                    CliError::Internal(format!("read gateway response failed: {}", e))
                })?;
                let _ = tokio::fs::remove_file(&resp_path).await;
                let parsed =
                    serde_json::from_str::<serde_json::Value>(&response_raw).map_err(|e| {
                        CliError::Internal(format!("decode gateway response json failed: {}", e))
                    })?;
                let _ = tokio::fs::remove_file(&req_path).await;
                return Ok::<serde_json::Value, CliError>(parsed);
            }
            if std::time::Instant::now() >= deadline {
                let _ = tokio::fs::remove_file(&req_path).await;
                return Err(CliError::Internal(
                    "timeout waiting for file gateway response".to_string(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })?;

    let status = response_json
        .get("status")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ok = response_json
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let output = serde_json::json!({
        "transport": "file-gateway",
        "gatewayDir": gateway_root,
        "requestId": req_id,
        "method": method_trimmed,
        "path": path,
        "status": status,
        "ok": ok,
        "response": response_json,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| CliError::Internal(format!(
                    "format gateway-call output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("file gateway call");
            println!(
                "request id: {}",
                output["requestId"].as_str().unwrap_or_default()
            );
            println!("method: {}", output["method"].as_str().unwrap_or_default());
            if let Some(p) = output.get("path").and_then(|v| v.as_str()) {
                if !p.is_empty() {
                    println!("path: {}", p);
                }
            }
            println!("status: {}", status);
            println!("ok: {}", ok);
            println!("response:");
            println!(
                "{}",
                serde_json::to_string_pretty(
                    output.get("response").unwrap_or(&serde_json::Value::Null)
                )
                .map_err(|e| CliError::Internal(format!(
                    "format gateway response failed: {}",
                    e
                )))?
            );
        }
    }

    if !ok || status >= 400 {
        return Err(CliError::Internal(format!(
            "file gateway request failed (status={}, ok={})",
            status, ok
        )));
    }

    Ok(())
}

#[cfg(feature = "api")]
#[derive(Debug, Clone)]
struct FileGatewayStressOutcome {
    latency_ms: u64,
    success: bool,
    timed_out: bool,
    status: Option<u16>,
    error: Option<String>,
}

#[cfg(feature = "api")]
fn run_api_file_gateway_stress(
    file_gateway_dir: Option<PathBuf>,
    requests: usize,
    concurrency: usize,
    timeout_ms: u64,
    method: String,
    path: Option<String>,
) -> Result<()> {
    let gateway_root = select_file_gateway_root(file_gateway_dir)?;
    std::fs::create_dir_all(&gateway_root)?;
    let total_requests = requests.max(1);
    let max_concurrency = concurrency.max(1).min(total_requests);
    let per_request_timeout = timeout_ms.max(100);
    let method_trimmed = method.trim().to_string();
    if method_trimmed.is_empty() {
        return Err(CliError::Argument(
            "stress method must not be empty".to_string(),
        ));
    }

    println!("file gateway stress test");
    println!("gateway dir: {}", gateway_root.display());
    println!("requests: {}", total_requests);
    println!("concurrency: {}", max_concurrency);
    println!("timeout(ms): {}", per_request_timeout);
    println!("method: {}", method_trimmed);
    if let Some(path_value) = path.as_ref() {
        println!("path override: {}", path_value);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let started = std::time::Instant::now();
    let outcomes = runtime.block_on(async move {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let semaphore = Arc::new(Semaphore::new(max_concurrency));
        let mut set = tokio::task::JoinSet::new();

        for idx in 0..total_requests {
            let permit = semaphore.clone().acquire_owned().await.map_err(|e| {
                CliError::Internal(format!("stress semaphore acquire failed: {}", e))
            })?;
            let gateway_root_for_task = gateway_root.clone();
            let method_for_task = method_trimmed.clone();
            let path_for_task = path.clone();
            set.spawn(async move {
                let _permit = permit;
                let req_id = format!(
                    "stress_{}_{}_{}",
                    std::process::id(),
                    idx,
                    chrono::Utc::now().timestamp_micros()
                );
                let req_path = gateway_root_for_task.join(format!("req_{}.json", req_id));
                let req_tmp_path = gateway_root_for_task.join(format!("req_{}.json.tmp", req_id));
                let resp_path = gateway_root_for_task.join(format!("resp_{}.json", req_id));

                let mut payload = serde_json::json!({
                    "id": req_id,
                    "method": method_for_task,
                    "timestamp": chrono::Utc::now().timestamp(),
                    "ttl": ((per_request_timeout / 1000).max(1) + 5),
                });
                if let Some(path_value) = path_for_task {
                    payload["path"] = serde_json::Value::String(path_value);
                }

                let raw = match serde_json::to_vec(&payload) {
                    Ok(v) => v,
                    Err(e) => {
                        return FileGatewayStressOutcome {
                            latency_ms: 0,
                            success: false,
                            timed_out: false,
                            status: None,
                            error: Some(format!("serialize request failed: {}", e)),
                        };
                    }
                };
                if let Err(e) = tokio::fs::write(&req_tmp_path, raw).await {
                    return FileGatewayStressOutcome {
                        latency_ms: 0,
                        success: false,
                        timed_out: false,
                        status: None,
                        error: Some(format!("write request failed: {}", e)),
                    };
                }
                if let Err(e) = tokio::fs::rename(&req_tmp_path, &req_path).await {
                    let _ = tokio::fs::remove_file(&req_tmp_path).await;
                    return FileGatewayStressOutcome {
                        latency_ms: 0,
                        success: false,
                        timed_out: false,
                        status: None,
                        error: Some(format!("commit request failed: {}", e)),
                    };
                }

                let req_started = std::time::Instant::now();
                let deadline = req_started + std::time::Duration::from_millis(per_request_timeout);
                loop {
                    if tokio::fs::metadata(&resp_path).await.is_ok() {
                        let response_raw = tokio::fs::read_to_string(&resp_path)
                            .await
                            .unwrap_or_else(|_| "{}".to_string());
                        let _ = tokio::fs::remove_file(&resp_path).await;
                        let response_json =
                            serde_json::from_str::<serde_json::Value>(&response_raw)
                                .unwrap_or_else(|_| serde_json::json!({}));
                        let ok = response_json
                            .get("ok")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let status = response_json
                            .get("status")
                            .and_then(|v| v.as_u64())
                            .map(|v| v.min(u16::MAX as u64) as u16);
                        let error = response_json
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|v| v.to_string());
                        let latency_ms =
                            req_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        return FileGatewayStressOutcome {
                            latency_ms,
                            success: ok,
                            timed_out: false,
                            status,
                            error,
                        };
                    }

                    if std::time::Instant::now() >= deadline {
                        let _ = tokio::fs::remove_file(&req_path).await;
                        return FileGatewayStressOutcome {
                            latency_ms: per_request_timeout,
                            success: false,
                            timed_out: true,
                            status: None,
                            error: Some("timeout waiting for file gateway response".to_string()),
                        };
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            });
        }

        let mut out = Vec::with_capacity(total_requests);
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(result) => out.push(result),
                Err(err) => out.push(FileGatewayStressOutcome {
                    latency_ms: 0,
                    success: false,
                    timed_out: false,
                    status: None,
                    error: Some(format!("task join error: {}", err)),
                }),
            }
        }
        Ok::<Vec<FileGatewayStressOutcome>, CliError>(out)
    })?;

    let elapsed = started.elapsed();
    let mut latencies = outcomes.iter().map(|v| v.latency_ms).collect::<Vec<_>>();
    latencies.sort_unstable();
    let total = outcomes.len();
    let success = outcomes.iter().filter(|v| v.success).count();
    let failed = total.saturating_sub(success);
    let timed_out = outcomes.iter().filter(|v| v.timed_out).count();
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        total as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let avg_latency = if latencies.is_empty() {
        0.0
    } else {
        let sum: u128 = latencies.iter().copied().map(u128::from).sum();
        sum as f64 / latencies.len() as f64
    };
    let percentile = |p: f64| -> u64 {
        if latencies.is_empty() {
            return 0;
        }
        let idx = ((latencies.len() as f64) * p).ceil() as usize;
        latencies[idx.saturating_sub(1).min(latencies.len().saturating_sub(1))]
    };
    let status_2xx = outcomes
        .iter()
        .filter(|v| v.status.map(|s| (200..300).contains(&s)).unwrap_or(false))
        .count();
    let status_4xx_5xx = outcomes
        .iter()
        .filter(|v| v.status.map(|s| s >= 400).unwrap_or(false))
        .count();
    let first_error = outcomes
        .iter()
        .find_map(|v| v.error.as_ref().map(|e| e.to_string()))
        .unwrap_or_else(|| "none".to_string());

    println!("file gateway stress summary");
    println!("elapsed(s): {:.3}", elapsed.as_secs_f64());
    println!("total: {}", total);
    println!("success: {}", success);
    println!("failed: {}", failed);
    println!("timeout: {}", timed_out);
    println!("status 2xx: {}", status_2xx);
    println!("status >=400: {}", status_4xx_5xx);
    println!("throughput(req/s): {:.2}", throughput);
    println!("latency avg(ms): {:.2}", avg_latency);
    println!("latency p50(ms): {}", percentile(0.50));
    println!("latency p95(ms): {}", percentile(0.95));
    println!("latency p99(ms): {}", percentile(0.99));
    println!(
        "latency max(ms): {}",
        latencies.last().copied().unwrap_or(0)
    );
    println!("first error: {}", first_error);

    Ok(())
}

#[cfg(feature = "api")]
fn run_api_smoke_check(db_path: Option<PathBuf>) -> Result<()> {
    use tower::util::ServiceExt;
    use xenobot_api::database::repository::{ChatMeta, Repository};

    if let Some(path) = db_path.as_ref() {
        std::env::set_var("XENOBOT_DB_PATH", path.as_os_str());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let (health_status, health_body, generate_status, generated_sql, execute_status, execute_body) =
        runtime.block_on(async move {
            xenobot_api::database::init_database()
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;

            let pool = xenobot_api::database::get_pool()
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;
            let repo = Repository::new(pool);
            let session_id = repo
                .create_chat(&ChatMeta {
                    id: 0,
                    name: "api_smoke_session".to_string(),
                    platform: "wechat".to_string(),
                    chat_type: "group".to_string(),
                    imported_at: 1_700_000_000,
                    group_id: None,
                    group_avatar: None,
                    owner_id: None,
                    schema_version: 3,
                    session_gap_threshold: 1800,
                })
                .await
                .map_err(|e| CliError::Database(e.to_string()))?;

            let config = xenobot_api::config::ApiConfig::default();
            let app = xenobot_api::router::build_router(&config);
            let health_request: axum::http::Request<axum::body::Body> =
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .map_err(|e| CliError::Internal(format!("failed to build request: {}", e)))?;
            let health_response: axum::response::Response = app
                .clone()
                .oneshot(health_request)
                .await
                .unwrap_or_else(|err| match err {});
            let health_status: axum::http::StatusCode = health_response.status();
            let health_body_bytes = axum::body::to_bytes(health_response.into_body(), 1024 * 1024)
                .await
                .map_err(|e| CliError::Internal(format!("failed to read response body: {}", e)))?;
            let health_body = String::from_utf8_lossy(&health_body_bytes).to_string();

            let generate_endpoint = format!("/chat/sessions/{}/generate-sql", session_id);
            let generate_request: axum::http::Request<axum::body::Body> =
                axum::http::Request::builder()
                    .method("POST")
                    .uri(&generate_endpoint)
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({
                            "prompt": "最近消息",
                            "maxRows": 5
                        })
                        .to_string(),
                    ))
                    .map_err(|e| {
                        CliError::Internal(format!("failed to build generate-sql request: {}", e))
                    })?;
            let generate_response: axum::response::Response = app
                .clone()
                .oneshot(generate_request)
                .await
                .unwrap_or_else(|err| match err {});
            let generate_status: axum::http::StatusCode = generate_response.status();
            let generate_body_bytes =
                axum::body::to_bytes(generate_response.into_body(), 2 * 1024 * 1024)
                    .await
                    .map_err(|e| {
                        CliError::Internal(format!(
                            "failed to read generate-sql response body: {}",
                            e
                        ))
                    })?;
            if generate_status != axum::http::StatusCode::OK {
                let generate_body_text = String::from_utf8_lossy(&generate_body_bytes).to_string();
                return Err(CliError::Internal(format!(
                    "smoke check failed: generate-sql expected 200, got {} with body {}",
                    generate_status, generate_body_text
                )));
            }
            if generate_body_bytes.is_empty() {
                return Err(CliError::Internal(
                    "smoke check failed: generate-sql returned empty body".to_string(),
                ));
            }
            let generate_body_json: serde_json::Value =
                serde_json::from_slice(&generate_body_bytes).map_err(|e| {
                    CliError::Internal(format!("failed to parse generate-sql response body: {}", e))
                })?;
            let generated_sql = generate_body_json
                .get("sql")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let expected_scope = format!("msg.meta_id = {}", session_id);
            if !generated_sql.contains(&expected_scope) {
                return Err(CliError::Internal(format!(
                    "smoke check failed: generated SQL missing session scope '{}'",
                    expected_scope
                )));
            }

            let execute_endpoint = format!("/chat/sessions/{}/execute-sql", session_id);
            let execute_request: axum::http::Request<axum::body::Body> =
                axum::http::Request::builder()
                    .method("POST")
                    .uri(&execute_endpoint)
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({
                            "sql": generated_sql
                        })
                        .to_string(),
                    ))
                    .map_err(|e| {
                        CliError::Internal(format!("failed to build execute-sql request: {}", e))
                    })?;
            let execute_response: axum::response::Response = app
                .clone()
                .oneshot(execute_request)
                .await
                .unwrap_or_else(|err| match err {});
            let execute_status: axum::http::StatusCode = execute_response.status();
            let execute_body_bytes =
                axum::body::to_bytes(execute_response.into_body(), 2 * 1024 * 1024)
                    .await
                    .map_err(|e| {
                        CliError::Internal(format!(
                            "failed to read execute-sql response body: {}",
                            e
                        ))
                    })?;
            let execute_body = String::from_utf8_lossy(&execute_body_bytes).to_string();

            Ok::<
                (
                    axum::http::StatusCode,
                    String,
                    axum::http::StatusCode,
                    String,
                    axum::http::StatusCode,
                    String,
                ),
                CliError,
            >((
                health_status,
                health_body,
                generate_status,
                generated_sql,
                execute_status,
                execute_body,
            ))
        })?;

    println!("api smoke check completed");
    println!("route: GET /health");
    println!("status: {}", health_status.as_u16());
    println!("body: {}", health_body);
    println!("route: POST /chat/sessions/:session_id/generate-sql");
    println!("status: {}", generate_status.as_u16());
    println!("contract: generated SQL includes session scope filter");
    println!(
        "sql preview: {}",
        generated_sql.chars().take(120).collect::<String>()
    );
    println!("route: POST /chat/sessions/:session_id/execute-sql");
    println!("status: {}", execute_status.as_u16());
    if health_status != axum::http::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "smoke check failed: expected 200, got {}",
            health_status
        )));
    }
    if !health_body.trim().eq_ignore_ascii_case("ok") {
        return Err(CliError::Internal(format!(
            "smoke check failed: expected body 'OK', got '{}'",
            health_body.trim()
        )));
    }
    if execute_status != axum::http::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "smoke check failed: execute-sql expected 200, got {} with body {}",
            execute_status, execute_body
        )));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn percent_encode_path_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", byte));
            }
        }
    }
    out
}

#[cfg(feature = "api")]
fn validate_mcp_integration_preset_contract(
    target: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let expected_id = target.trim();
    let payload_id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if payload_id != expected_id {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: integration preset '{}' returned mismatched id '{}'",
            expected_id, payload_id
        )));
    }

    let transport = payload.get("transport").and_then(|v| v.as_object());
    let has_sse = transport
        .and_then(|obj| obj.get("sse"))
        .and_then(|v| v.as_str())
        .is_some();
    let has_websocket = transport
        .and_then(|obj| obj.get("websocket"))
        .and_then(|v| v.as_str())
        .is_some();
    let has_tools = transport
        .and_then(|obj| obj.get("tools"))
        .and_then(|v| v.as_str())
        .is_some();
    if !has_sse || !has_websocket || !has_tools {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: integration preset '{}' missing transport.sse/websocket/tools",
            expected_id
        )));
    }

    if !payload
        .get("notes")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| !arr.is_empty())
    {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: integration preset '{}' missing notes",
            expected_id
        )));
    }

    match expected_id {
        "claude-desktop" => {
            let command = payload
                .get("configuration")
                .and_then(|v| v.get("mcpServers"))
                .and_then(|v| v.get("xenobot"))
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str());
            let args = payload
                .get("configuration")
                .and_then(|v| v.get("mcpServers"))
                .and_then(|v| v.get("xenobot"))
                .and_then(|v| v.get("args"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let has_remote = args.iter().any(|v| v.as_str() == Some("mcp-remote"));
            if command != Some("pnpm") || !has_remote {
                return Err(CliError::Internal(
                    "mcp smoke failed: claude-desktop preset requires pnpm + mcp-remote bridge"
                        .to_string(),
                ));
            }
        }
        "chatwise" => {
            let first = payload
                .get("configuration")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());
            let name = first
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let transport_value = first
                .and_then(|v| v.get("transport"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let url = first.and_then(|v| v.get("url")).and_then(|v| v.as_str());
            if name != "xenobot" || transport_value != "sse" || url.is_none() {
                return Err(CliError::Internal(
                    "mcp smoke failed: chatwise preset missing servers[0] contract".to_string(),
                ));
            }
        }
        "opencode" => {
            let first = payload
                .get("configuration")
                .and_then(|v| v.get("mcpServers"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());
            let name = first
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let transport_value = first
                .and_then(|v| v.get("transport"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let url = first.and_then(|v| v.get("url")).and_then(|v| v.as_str());
            if name != "xenobot" || transport_value != "sse" || url.is_none() {
                return Err(CliError::Internal(
                    "mcp smoke failed: opencode preset missing mcpServers[0] contract".to_string(),
                ));
            }
        }
        "pencil" => {
            let first = payload
                .get("configuration")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());
            let name = first
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let transport_value = first
                .and_then(|v| v.get("transport"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let url = first.and_then(|v| v.get("url")).and_then(|v| v.as_str());
            if name != "xenobot" || transport_value != "sse" || url.is_none() {
                return Err(CliError::Internal(
                    "mcp smoke failed: pencil preset missing servers[0] contract".to_string(),
                ));
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(feature = "api")]
fn run_mcp_smoke_check(base_url: String, timeout_ms: u64) -> Result<()> {
    let base = base_url.trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp smoke requires a non-empty --url".to_string(),
        ));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let base_for_requests = base.clone();
    let summary = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let health_url = format!("{}/health", base_for_requests);
        let tools_url = format!("{}/tools", base_for_requests);
        let resources_url = format!("{}/resources", base_for_requests);
        let integrations_url = format!("{}/integrations", base_for_requests);
        let sse_url = format!("{}/sse", base_for_requests);
        let ws_url = format!("{}/ws", base_for_requests);
        let mcp_url = format!("{}/mcp", base_for_requests);

        let health_resp = client
            .get(&health_url)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp health request failed: {}", e)))?;
        let health_status = health_resp.status();
        let health_body = health_resp
            .text()
            .await
            .map_err(|e| CliError::Internal(format!("mcp health read failed: {}", e)))?;

        let tools_resp = client
            .get(&tools_url)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools request failed: {}", e)))?;
        let tools_status = tools_resp.status();
        let tools_json: serde_json::Value = tools_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools decode failed: {}", e)))?;
        let tools = tools_json
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let tool_names = tools
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect::<std::collections::HashSet<_>>();
        let tool_specs = tools_json
            .get("toolSpecs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let tool_spec_names = tool_specs
            .iter()
            .filter_map(|item| item.get("name").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect::<std::collections::HashSet<_>>();
        let tool_specs_with_schema = tool_specs
            .iter()
            .filter(|item| item.get("inputSchema").is_some_and(|v| v.is_object()))
            .count();

        let resources_resp = client
            .get(&resources_url)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources request failed: {}", e)))?;
        let resources_status = resources_resp.status();
        let resources_json: serde_json::Value = resources_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources decode failed: {}", e)))?;
        let resource_uris = resources_json
            .get("resources")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                item.get("uri")
                    .and_then(|uri| uri.as_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        let first_resource_uri = resource_uris.first().cloned().ok_or_else(|| {
            CliError::Internal("mcp smoke failed: /resources returned no resource URIs".to_string())
        })?;
        let resource_read_url = format!(
            "{}/resources/{}",
            base_for_requests,
            percent_encode_path_component(&first_resource_uri)
        );
        let resource_read_resp =
            client.get(&resource_read_url).send().await.map_err(|e| {
                CliError::Internal(format!("mcp resource read request failed: {}", e))
            })?;
        let resource_read_status = resource_read_resp.status();
        let resource_read_json: serde_json::Value = resource_read_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resource read decode failed: {}", e)))?;

        let mcp_initialize_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-init",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05"
                }
            }))
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp initialize request failed: {}", e)))?;
        let mcp_initialize_status = mcp_initialize_resp.status();
        let mcp_initialize_json: serde_json::Value = mcp_initialize_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp initialize decode failed: {}", e)))?;

        let mcp_list_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-tools-list",
                "method": "tools/list"
            }))
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools/list request failed: {}", e)))?;
        let mcp_list_status = mcp_list_resp.status();
        let mcp_list_json: serde_json::Value = mcp_list_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools/list decode failed: {}", e)))?;
        let mcp_tool_items = mcp_list_json
            .get("result")
            .and_then(|v| v.get("tools"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mcp_tool_names = mcp_tool_items
            .iter()
            .cloned()
            .into_iter()
            .filter_map(|item| {
                item.get("name")
                    .and_then(|name| name.as_str())
                    .or_else(|| item.as_str())
                    .map(|name| name.to_string())
            })
            .collect::<std::collections::HashSet<_>>();
        let mcp_tools_with_schema = mcp_tool_items
            .iter()
            .filter(|item| item.get("inputSchema").is_some_and(|v| v.is_object()))
            .count();

        let mcp_resources_list_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-resources-list",
                "method": "resources/list"
            }))
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources/list request failed: {}", e)))?;
        let mcp_resources_list_status = mcp_resources_list_resp.status();
        let mcp_resources_list_json: serde_json::Value = mcp_resources_list_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources/list decode failed: {}", e)))?;
        let mcp_resource_uris = mcp_resources_list_json
            .get("result")
            .and_then(|v| v.get("resources"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                item.get("uri")
                    .and_then(|uri| uri.as_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        let first_mcp_resource_uri = mcp_resource_uris.first().cloned().ok_or_else(|| {
            CliError::Internal(
                "mcp smoke failed: /mcp resources/list returned no resource URIs".to_string(),
            )
        })?;

        let mcp_resources_read_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-resources-read",
                "method": "resources/read",
                "params": {
                    "uri": first_mcp_resource_uri
                }
            }))
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources/read request failed: {}", e)))?;
        let mcp_resources_read_status = mcp_resources_read_resp.status();
        let mcp_resources_read_json: serde_json::Value = mcp_resources_read_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources/read decode failed: {}", e)))?;

        let mcp_alias_tool_list_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-tool-list-alias",
                "method": "tool/list"
            }))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!("mcp tool/list alias request failed: {}", e))
            })?;
        let mcp_alias_tool_list_status = mcp_alias_tool_list_resp.status();
        let mcp_alias_tool_list_json: serde_json::Value = mcp_alias_tool_list_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tool/list alias decode failed: {}", e)))?;
        let mcp_alias_tool_count = mcp_alias_tool_list_json
            .get("result")
            .and_then(|v| v.get("tools"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        let mcp_alias_resource_list_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-resource-list-alias",
                "method": "resource/list"
            }))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!("mcp resource/list alias request failed: {}", e))
            })?;
        let mcp_alias_resource_list_status = mcp_alias_resource_list_resp.status();
        let mcp_alias_resource_list_json: serde_json::Value =
            mcp_alias_resource_list_resp.json().await.map_err(|e| {
                CliError::Internal(format!("mcp resource/list alias decode failed: {}", e))
            })?;
        let mcp_alias_resource_count = mcp_alias_resource_list_json
            .get("result")
            .and_then(|v| v.get("resources"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        let mcp_call_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-tools-call-current-time",
                "method": "tools/call",
                "params": {
                    "name": "get_current_time",
                    "arguments": {}
                }
            }))
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools/call request failed: {}", e)))?;
        let mcp_call_status = mcp_call_resp.status();
        let mcp_call_json: serde_json::Value = mcp_call_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools/call decode failed: {}", e)))?;

        let mcp_call_missing_arg_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-tools-call-chat-records-missing-session",
                "method": "tools/call",
                "params": {
                    "name": "chat_records",
                    "arguments": {}
                }
            }))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!(
                    "mcp tools/call chat_records missing-arg request failed: {}",
                    e
                ))
            })?;
        let mcp_call_missing_arg_status = mcp_call_missing_arg_resp.status();
        let mcp_call_missing_arg_json: serde_json::Value =
            mcp_call_missing_arg_resp.json().await.map_err(|e| {
                CliError::Internal(format!(
                    "mcp tools/call chat_records missing-arg decode failed: {}",
                    e
                ))
            })?;

        let mcp_call_unknown_tool_resp = client
            .post(&mcp_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "mcp-smoke-tools-call-unknown",
                "method": "tools/call",
                "params": {
                    "name": "totally_unknown_tool",
                    "arguments": {}
                }
            }))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!("mcp tools/call unknown-tool request failed: {}", e))
            })?;
        let mcp_call_unknown_tool_status = mcp_call_unknown_tool_resp.status();
        let mcp_call_unknown_tool_json: serde_json::Value =
            mcp_call_unknown_tool_resp.json().await.map_err(|e| {
                CliError::Internal(format!("mcp tools/call unknown-tool decode failed: {}", e))
            })?;

        let http_tool_not_found_url =
            format!("{}/tools/{}", base_for_requests, "totally_unknown_tool");
        let http_tool_not_found_resp = client
            .post(&http_tool_not_found_url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!("mcp http tool_not_found request failed: {}", e))
            })?;
        let http_tool_not_found_status = http_tool_not_found_resp.status();
        let http_tool_not_found_json: serde_json::Value =
            http_tool_not_found_resp.json().await.map_err(|e| {
                CliError::Internal(format!("mcp http tool_not_found decode failed: {}", e))
            })?;

        let http_tool_error_url = format!("{}/tools/{}", base_for_requests, "chat_records");
        let http_tool_error_resp = client
            .post(&http_tool_error_url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| {
                CliError::Internal(format!("mcp http tool_error request failed: {}", e))
            })?;
        let http_tool_error_status = http_tool_error_resp.status();
        let http_tool_error_json: serde_json::Value = http_tool_error_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp http tool_error decode failed: {}", e)))?;

        let integrations_resp =
            client.get(&integrations_url).send().await.map_err(|e| {
                CliError::Internal(format!("mcp integrations request failed: {}", e))
            })?;
        let integrations_status = integrations_resp.status();
        let integrations_json: serde_json::Value = integrations_resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp integrations decode failed: {}", e)))?;
        let integration_ids = integrations_json
            .get("integrations")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .collect::<std::collections::HashSet<_>>();
        let mut integration_preset_statuses = Vec::new();
        for target in ["claude-desktop", "chatwise", "opencode"] {
            let preset_url = format!("{}/integrations/{}", base_for_requests, target);
            let preset_resp = client.get(&preset_url).send().await.map_err(|e| {
                CliError::Internal(format!(
                    "mcp integration preset '{}' request failed: {}",
                    target, e
                ))
            })?;
            let preset_status = preset_resp.status();
            let preset_json: serde_json::Value = preset_resp.json().await.map_err(|e| {
                CliError::Internal(format!(
                    "mcp integration preset '{}' decode failed: {}",
                    target, e
                ))
            })?;
            if preset_status != reqwest::StatusCode::OK {
                return Err(CliError::Internal(format!(
                    "mcp smoke failed: /integrations/{} expected 200, got {}",
                    target, preset_status
                )));
            }
            validate_mcp_integration_preset_contract(target, &preset_json)?;
            integration_preset_statuses.push((target.to_string(), preset_status));
        }

        let sse_resp = client
            .get(&sse_url)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp sse route request failed: {}", e)))?;
        let sse_status = sse_resp.status();
        let sse_content_type = sse_resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let ws_resp = client.get(&ws_url).send().await.map_err(|e| {
            CliError::Internal(format!("mcp websocket route request failed: {}", e))
        })?;
        let ws_status = ws_resp.status();

        Ok::<_, CliError>((
            health_status,
            health_body,
            tools_status,
            tools.len(),
            tool_names,
            tool_specs.len(),
            tool_spec_names,
            tool_specs_with_schema,
            resources_status,
            resource_uris.len(),
            resource_read_status,
            resource_read_json,
            mcp_initialize_status,
            mcp_initialize_json,
            mcp_list_status,
            mcp_tool_names,
            mcp_tool_items.len(),
            mcp_tools_with_schema,
            mcp_resources_list_status,
            mcp_resource_uris.len(),
            mcp_resources_read_status,
            mcp_resources_read_json,
            mcp_alias_tool_list_status,
            mcp_alias_tool_count,
            mcp_alias_resource_list_status,
            mcp_alias_resource_count,
            mcp_call_status,
            mcp_call_json,
            mcp_call_missing_arg_status,
            mcp_call_missing_arg_json,
            mcp_call_unknown_tool_status,
            mcp_call_unknown_tool_json,
            http_tool_not_found_status,
            http_tool_not_found_json,
            http_tool_error_status,
            http_tool_error_json,
            integrations_status,
            integration_ids,
            integration_preset_statuses,
            sse_status,
            sse_content_type,
            ws_status,
        ))
    })?;

    let (
        health_status,
        health_body,
        tools_status,
        tool_count,
        tool_names,
        tool_spec_count,
        tool_spec_names,
        tool_specs_with_schema,
        resources_status,
        resource_count,
        resource_read_status,
        resource_read_json,
        mcp_initialize_status,
        mcp_initialize_json,
        mcp_list_status,
        mcp_tool_names,
        mcp_tool_count,
        mcp_tools_with_schema,
        mcp_resources_list_status,
        mcp_resource_count,
        mcp_resources_read_status,
        mcp_resources_read_json,
        mcp_alias_tool_list_status,
        mcp_alias_tool_count,
        mcp_alias_resource_list_status,
        mcp_alias_resource_count,
        mcp_call_status,
        mcp_call_json,
        mcp_call_missing_arg_status,
        mcp_call_missing_arg_json,
        mcp_call_unknown_tool_status,
        mcp_call_unknown_tool_json,
        http_tool_not_found_status,
        http_tool_not_found_json,
        http_tool_error_status,
        http_tool_error_json,
        integrations_status,
        integration_ids,
        integration_preset_statuses,
        sse_status,
        sse_content_type,
        ws_status,
    ) = summary;

    println!("mcp smoke check completed");
    println!("base: {}", base);
    println!(
        "GET /health -> {} {}",
        health_status.as_u16(),
        health_body.trim()
    );
    println!(
        "GET /tools -> {} (count={}, specs={}, specs_with_schema={})",
        tools_status.as_u16(),
        tool_count,
        tool_spec_count,
        tool_specs_with_schema
    );
    println!(
        "GET /resources -> {} (count={})",
        resources_status.as_u16(),
        resource_count
    );
    println!("GET /resources/*uri -> {}", resource_read_status.as_u16());
    println!("POST /mcp initialize -> {}", mcp_initialize_status.as_u16());
    println!(
        "POST /mcp tools/list -> {} (count={}, with_schema={})",
        mcp_list_status.as_u16(),
        mcp_tool_count,
        mcp_tools_with_schema
    );
    println!(
        "POST /mcp resources/list -> {} (count={})",
        mcp_resources_list_status.as_u16(),
        mcp_resource_count
    );
    println!(
        "POST /mcp resources/read -> {}",
        mcp_resources_read_status.as_u16()
    );
    println!(
        "POST /mcp tool/list(alias) -> {} (count={})",
        mcp_alias_tool_list_status.as_u16(),
        mcp_alias_tool_count
    );
    println!(
        "POST /mcp resource/list(alias) -> {} (count={})",
        mcp_alias_resource_list_status.as_u16(),
        mcp_alias_resource_count
    );
    println!(
        "POST /mcp tools/call(get_current_time) -> {}",
        mcp_call_status.as_u16()
    );
    println!(
        "POST /mcp tools/call(chat_records missing session_id) -> {}",
        mcp_call_missing_arg_status.as_u16()
    );
    println!(
        "POST /mcp tools/call(unknown tool) -> {}",
        mcp_call_unknown_tool_status.as_u16()
    );
    println!(
        "POST /tools/totally_unknown_tool -> {}",
        http_tool_not_found_status.as_u16()
    );
    println!(
        "POST /tools/chat_records({{}}) -> {}",
        http_tool_error_status.as_u16()
    );
    println!(
        "GET /integrations -> {} (count={})",
        integrations_status.as_u16(),
        integration_ids.len()
    );
    for (target, status) in &integration_preset_statuses {
        println!("GET /integrations/{} -> {}", target, status.as_u16());
    }
    println!("GET /sse -> {} ({})", sse_status.as_u16(), sse_content_type);
    println!("GET /ws -> {}", ws_status.as_u16());

    if health_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /health expected 200, got {}",
            health_status
        )));
    }
    if tools_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /tools expected 200, got {}",
            tools_status
        )));
    }
    if resources_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /resources expected 200, got {}",
            resources_status
        )));
    }
    if resource_read_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /resources/*uri expected 200, got {}",
            resource_read_status
        )));
    }
    if mcp_initialize_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp initialize expected 200, got {}",
            mcp_initialize_status
        )));
    }
    if mcp_list_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp tools/list expected 200, got {}",
            mcp_list_status
        )));
    }
    if tool_spec_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools missing toolSpecs contract".to_string(),
        ));
    }
    if tool_specs_with_schema != tool_spec_count {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools toolSpecs missing inputSchema object".to_string(),
        ));
    }
    if mcp_tool_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/list returned empty tools".to_string(),
        ));
    }
    if mcp_tools_with_schema != mcp_tool_count {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/list tools missing inputSchema object".to_string(),
        ));
    }
    if tool_names != tool_spec_names {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools tools and toolSpecs are out of sync".to_string(),
        ));
    }
    if tool_spec_names != mcp_tool_names {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools toolSpecs and /mcp tools/list are out of sync".to_string(),
        ));
    }
    if mcp_resources_list_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp resources/list expected 200, got {}",
            mcp_resources_list_status
        )));
    }
    if mcp_resources_read_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp resources/read expected 200, got {}",
            mcp_resources_read_status
        )));
    }
    if mcp_alias_tool_list_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp tool/list alias expected 200, got {}",
            mcp_alias_tool_list_status
        )));
    }
    if mcp_alias_resource_list_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp resource/list alias expected 200, got {}",
            mcp_alias_resource_list_status
        )));
    }
    if mcp_call_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp tools/call expected 200, got {}",
            mcp_call_status
        )));
    }
    if integrations_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /integrations expected 200, got {}",
            integrations_status
        )));
    }
    if sse_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /sse expected 200, got {}",
            sse_status
        )));
    }
    if !sse_content_type.starts_with("text/event-stream") {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /sse expected text/event-stream content-type, got '{}'",
            sse_content_type
        )));
    }
    if ws_status != reqwest::StatusCode::UPGRADE_REQUIRED
        && ws_status != reqwest::StatusCode::BAD_REQUEST
    {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /ws expected 426 or 400 without websocket upgrade headers, got {}",
            ws_status
        )));
    }
    if !mcp_initialize_json
        .get("result")
        .and_then(|v| v.get("protocolVersion"))
        .and_then(|v| v.as_str())
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp initialize missing result.protocolVersion".to_string(),
        ));
    }
    if mcp_call_json
        .get("result")
        .and_then(|v| v.get("tool"))
        .and_then(|v| v.as_str())
        != Some("get_current_time")
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/call missing result.tool=get_current_time".to_string(),
        ));
    }
    if !mcp_call_json
        .get("result")
        .and_then(|v| v.get("structuredContent"))
        .and_then(|v| v.get("unix"))
        .is_some_and(|v| v.is_number())
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/call missing structuredContent.unix".to_string(),
        ));
    }
    if mcp_call_missing_arg_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp tools/call(chat_records missing session_id) expected 200, got {}",
            mcp_call_missing_arg_status
        )));
    }
    if mcp_call_missing_arg_json
        .get("error")
        .and_then(|v| v.get("code"))
        .and_then(|v| v.as_i64())
        != Some(-32002)
        || mcp_call_missing_arg_json
            .get("error")
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str())
            != Some("tool_error")
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/call(chat_records missing session_id) expected error.code=-32002 message=tool_error".to_string(),
        ));
    }
    if mcp_call_unknown_tool_status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /mcp tools/call(unknown tool) expected 200, got {}",
            mcp_call_unknown_tool_status
        )));
    }
    if mcp_call_unknown_tool_json
        .get("error")
        .and_then(|v| v.get("code"))
        .and_then(|v| v.as_i64())
        != Some(-32001)
        || mcp_call_unknown_tool_json
            .get("error")
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str())
            != Some("tool_not_found")
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tools/call(unknown tool) expected error.code=-32001 message=tool_not_found".to_string(),
        ));
    }
    if http_tool_not_found_status != reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /tools/totally_unknown_tool expected 404, got {}",
            http_tool_not_found_status
        )));
    }
    if http_tool_not_found_json
        .get("code")
        .and_then(|v| v.as_str())
        != Some("tool_not_found")
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools/totally_unknown_tool expected code=tool_not_found"
                .to_string(),
        ));
    }
    if http_tool_error_status != reqwest::StatusCode::INTERNAL_SERVER_ERROR {
        return Err(CliError::Internal(format!(
            "mcp smoke failed: /tools/chat_records({{}}) expected 500, got {}",
            http_tool_error_status
        )));
    }
    if http_tool_error_json.get("code").and_then(|v| v.as_str()) != Some("tool_error") {
        return Err(CliError::Internal(
            "mcp smoke failed: /tools/chat_records({}) expected code=tool_error".to_string(),
        ));
    }
    if resource_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /resources returned empty list".to_string(),
        ));
    }
    if !resource_read_json
        .get("uri")
        .and_then(|v| v.as_str())
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /resources/*uri missing uri field".to_string(),
        ));
    }
    if !resource_read_json
        .get("content")
        .is_some_and(|v| v.is_array())
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /resources/*uri missing content array".to_string(),
        ));
    }
    if mcp_resource_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp resources/list returned empty list".to_string(),
        ));
    }
    if mcp_alias_tool_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp tool/list alias returned empty list".to_string(),
        ));
    }
    if mcp_alias_resource_count == 0 {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp resource/list alias returned empty list".to_string(),
        ));
    }
    if !mcp_resources_read_json
        .get("result")
        .and_then(|v| v.get("uri"))
        .and_then(|v| v.as_str())
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp resources/read missing result.uri".to_string(),
        ));
    }
    if !mcp_resources_read_json
        .get("result")
        .and_then(|v| v.get("content"))
        .is_some_and(|v| v.is_array())
    {
        return Err(CliError::Internal(
            "mcp smoke failed: /mcp resources/read missing result.content array".to_string(),
        ));
    }

    for required in [
        "current_time",
        "get_current_time",
        "query_contacts",
        "query_groups",
        "recent_sessions",
        "query_chats",
        "chat_records",
        "chat_history",
    ] {
        if !tool_names.contains(required) {
            return Err(CliError::Internal(format!(
                "mcp smoke failed: missing required tool '{}'",
                required
            )));
        }
        if !mcp_tool_names.contains(required) {
            return Err(CliError::Internal(format!(
                "mcp smoke failed: /mcp tools/list missing required tool '{}'",
                required
            )));
        }
    }

    for required in ["claude-desktop", "chatwise", "opencode"] {
        if !integration_ids.contains(required) {
            return Err(CliError::Internal(format!(
                "mcp smoke failed: missing integration preset '{}'",
                required
            )));
        }
    }
    if integration_preset_statuses.len() != 3 {
        return Err(CliError::Internal(
            "mcp smoke failed: expected 3 integration preset checks".to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn normalize_mcp_preset_target(target: &str) -> Result<String> {
    let normalized = target.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(CliError::Argument(
            "mcp preset requires a non-empty --target".to_string(),
        ));
    }

    match normalized.as_str() {
        "claude-desktop" | "claude_desktop" | "claude" => Ok("claude-desktop".to_string()),
        "chatwise" => Ok("chatwise".to_string()),
        "opencode" => Ok("opencode".to_string()),
        "pencil" => Ok("pencil".to_string()),
        _ => Err(CliError::Argument(format!(
            "mcp preset target '{}' is not supported; use one of: claude-desktop, chatwise, opencode, pencil",
            target.trim()
        ))),
    }
}

#[cfg(feature = "api")]
fn run_mcp_integration_preset_fetch(
    base_url: String,
    target: String,
    format: OutputFormat,
    timeout_ms: u64,
) -> Result<()> {
    let base = base_url.trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp preset requires a non-empty --url".to_string(),
        ));
    }
    let target = normalize_mcp_preset_target(&target)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let target_for_request = target.clone();
    let response = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;
        let url = format!("{}/integrations/{}", base, target_for_request);
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp preset request failed: {}", e)))?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp preset decode failed: {}", e)))?;
        Ok::<(reqwest::StatusCode, serde_json::Value), CliError>((status, json))
    })?;

    let (status, json) = response;
    if status == reqwest::StatusCode::NOT_FOUND {
        let supported = json
            .get("supported")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CliError::Internal(format!(
            "mcp preset not found for target '{}'; supported: [{}]",
            target, supported
        )));
    }
    if status != reqwest::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "mcp preset request failed with status {}: {}",
            status, json
        )));
    }

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json)
                    .map_err(|e| CliError::Internal(format!("format preset json failed: {}", e)))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("mcp integration preset");
            println!(
                "id: {}",
                json.get("id").and_then(|v| v.as_str()).unwrap_or_default()
            );
            println!(
                "name: {}",
                json.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            );
            println!(
                "description: {}",
                json.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            );
            println!(
                "transport.sse: {}",
                json.get("transport")
                    .and_then(|v| v.get("sse"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            );
            println!("configuration:");
            println!(
                "{}",
                serde_json::to_string_pretty(
                    json.get("configuration")
                        .unwrap_or(&serde_json::Value::Object(serde_json::Map::new()))
                )
                .map_err(|e| CliError::Internal(format!("format configuration failed: {}", e)))?
            );
        }
    }

    Ok(())
}

#[cfg(feature = "api")]
fn parse_mcp_tool_args_json(args_json: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    let parsed: serde_json::Value = serde_json::from_str(args_json)
        .map_err(|e| CliError::Argument(format!("invalid --args-json: {}", e)))?;
    parsed
        .as_object()
        .cloned()
        .ok_or_else(|| CliError::Argument("--args-json must be a JSON object".to_string()))
}

#[cfg(feature = "api")]
fn run_mcp_tool_call(
    base_url: String,
    mode: crate::commands::McpCallMode,
    tool: String,
    args_json: String,
    format: OutputFormat,
    timeout_ms: u64,
) -> Result<()> {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp call requires a non-empty --url".to_string(),
        ));
    }
    let tool = tool.trim().to_string();
    if tool.is_empty() {
        return Err(CliError::Argument(
            "mcp call requires a non-empty --tool".to_string(),
        ));
    }
    let args = parse_mcp_tool_args_json(&args_json)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let mode_for_request = mode.clone();
    let tool_for_request = tool.clone();
    let args_for_request = args.clone();
    let response = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let (url, body) = match mode_for_request {
            crate::commands::McpCallMode::Rpc => (
                format!("{}/mcp", base),
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "xenobot-mcp-call",
                    "method": "tools/call",
                    "params": {
                        "name": tool_for_request,
                        "arguments": args_for_request
                    }
                }),
            ),
            crate::commands::McpCallMode::Http => (
                format!("{}/tools/{}", base, tool_for_request),
                serde_json::Value::Object(args_for_request),
            ),
        };

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp call request failed: {}", e)))?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp call decode failed: {}", e)))?;
        Ok::<(reqwest::StatusCode, serde_json::Value), CliError>((status, json))
    })?;

    let (status, payload) = response;
    let output = serde_json::json!({
        "mode": mode.to_string(),
        "tool": tool,
        "status": status.as_u16(),
        "ok": status.is_success(),
        "response": payload,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| CliError::Internal(format!(
                    "format mcp call output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("mcp tool call");
            println!("mode: {}", mode);
            println!("tool: {}", output["tool"].as_str().unwrap_or_default());
            println!("status: {}", output["status"].as_u64().unwrap_or_default());
            println!("ok: {}", output["ok"].as_bool().unwrap_or(false));
            println!("response:");
            println!(
                "{}",
                serde_json::to_string_pretty(
                    output.get("response").unwrap_or(&serde_json::Value::Null)
                )
                .map_err(|e| CliError::Internal(format!("format mcp response failed: {}", e)))?
            );
        }
    }

    if !status.is_success() {
        return Err(CliError::Internal(format!(
            "mcp call failed with status {}",
            status
        )));
    }

    if output
        .get("response")
        .and_then(|v| v.get("error"))
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp call returned JSON-RPC error payload".to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn run_mcp_tools_list(
    base_url: String,
    mode: crate::commands::McpCallMode,
    format: OutputFormat,
    timeout_ms: u64,
) -> Result<()> {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp tools requires a non-empty --url".to_string(),
        ));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let mode_for_request = mode.clone();
    let response = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let request = match mode_for_request {
            crate::commands::McpCallMode::Rpc => {
                client
                    .post(format!("{}/mcp", base))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "xenobot-mcp-tools-list",
                        "method": "tools/list"
                    }))
            }
            crate::commands::McpCallMode::Http => client.get(format!("{}/tools", base)),
        };

        let resp = request
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools request failed: {}", e)))?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp tools decode failed: {}", e)))?;
        Ok::<(reqwest::StatusCode, serde_json::Value), CliError>((status, json))
    })?;

    let (status, payload) = response;

    let (tool_names, tools_with_schema) = match mode {
        crate::commands::McpCallMode::Rpc => {
            let items = payload
                .get("result")
                .and_then(|v| v.get("tools"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let names = items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(|v| v.as_str())
                        .or_else(|| item.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>();
            let with_schema = items
                .iter()
                .filter(|item| item.get("inputSchema").is_some_and(|v| v.is_object()))
                .count();
            (names, with_schema)
        }
        crate::commands::McpCallMode::Http => {
            let names = payload
                .get("tools")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>();
            let with_schema = payload
                .get("toolSpecs")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item.get("inputSchema").is_some_and(|v| v.is_object()))
                        .count()
                })
                .unwrap_or(0);
            (names, with_schema)
        }
    };

    let output = serde_json::json!({
        "mode": mode.to_string(),
        "status": status.as_u16(),
        "ok": status.is_success(),
        "toolCount": tool_names.len(),
        "toolsWithSchema": tools_with_schema,
        "tools": tool_names,
        "response": payload,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| CliError::Internal(format!(
                    "format mcp tools output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("mcp tools");
            println!("mode: {}", mode);
            println!("status: {}", output["status"].as_u64().unwrap_or_default());
            println!("ok: {}", output["ok"].as_bool().unwrap_or(false));
            println!(
                "tool count: {}",
                output["toolCount"].as_u64().unwrap_or_default()
            );
            println!(
                "tools with schema: {}",
                output["toolsWithSchema"].as_u64().unwrap_or_default()
            );
            println!("tools:");
            for name in output["tools"].as_array().cloned().unwrap_or_default() {
                if let Some(tool_name) = name.as_str() {
                    println!("- {}", tool_name);
                }
            }
        }
    }

    if !status.is_success() {
        return Err(CliError::Internal(format!(
            "mcp tools failed with status {}",
            status
        )));
    }

    if output
        .get("response")
        .and_then(|value| value.get("error"))
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp tools returned JSON-RPC error payload".to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn encode_http_path_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

#[cfg(feature = "api")]
fn run_mcp_resources_list(
    base_url: String,
    mode: crate::commands::McpCallMode,
    format: OutputFormat,
    timeout_ms: u64,
) -> Result<()> {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp resources requires a non-empty --url".to_string(),
        ));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let mode_for_request = mode.clone();
    let response = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let request = match mode_for_request {
            crate::commands::McpCallMode::Rpc => {
                client
                    .post(format!("{}/mcp", base))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "xenobot-mcp-resources-list",
                        "method": "resources/list"
                    }))
            }
            crate::commands::McpCallMode::Http => client.get(format!("{}/resources", base)),
        };

        let resp = request
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources request failed: {}", e)))?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resources decode failed: {}", e)))?;
        Ok::<(reqwest::StatusCode, serde_json::Value), CliError>((status, json))
    })?;

    let (status, payload) = response;
    let output = serde_json::json!({
        "mode": mode.to_string(),
        "status": status.as_u16(),
        "ok": status.is_success(),
        "response": payload,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| CliError::Internal(format!(
                    "format mcp resources output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("mcp resources");
            println!("mode: {}", mode);
            println!("status: {}", output["status"].as_u64().unwrap_or_default());
            println!("ok: {}", output["ok"].as_bool().unwrap_or(false));
            println!("response:");
            println!(
                "{}",
                serde_json::to_string_pretty(
                    output.get("response").unwrap_or(&serde_json::Value::Null)
                )
                .map_err(|e| CliError::Internal(format!("format mcp response failed: {}", e)))?
            );
        }
    }

    if !status.is_success() {
        return Err(CliError::Internal(format!(
            "mcp resources failed with status {}",
            status
        )));
    }

    if output
        .get("response")
        .and_then(|value| value.get("error"))
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp resources returned JSON-RPC error payload".to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "api")]
fn run_mcp_resource_read(
    base_url: String,
    mode: crate::commands::McpCallMode,
    uri: String,
    format: OutputFormat,
    timeout_ms: u64,
) -> Result<()> {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(CliError::Argument(
            "mcp resource requires a non-empty --url".to_string(),
        ));
    }
    let uri = uri.trim().to_string();
    if uri.is_empty() {
        return Err(CliError::Argument(
            "mcp resource requires a non-empty --uri".to_string(),
        ));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let mode_for_request = mode.clone();
    let uri_for_request = uri.clone();
    let response = runtime.block_on(async move {
        let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let request = match mode_for_request {
            crate::commands::McpCallMode::Rpc => {
                client
                    .post(format!("{}/mcp", base))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "xenobot-mcp-resource-read",
                        "method": "resources/read",
                        "params": {
                            "uri": uri_for_request
                        }
                    }))
            }
            crate::commands::McpCallMode::Http => client.get(format!(
                "{}/resources/{}",
                base,
                encode_http_path_component(&uri_for_request)
            )),
        };

        let resp = request
            .send()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resource request failed: {}", e)))?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CliError::Internal(format!("mcp resource decode failed: {}", e)))?;
        Ok::<(reqwest::StatusCode, serde_json::Value), CliError>((status, json))
    })?;

    let (status, payload) = response;
    let output = serde_json::json!({
        "mode": mode.to_string(),
        "uri": uri,
        "status": status.as_u16(),
        "ok": status.is_success(),
        "response": payload,
    });

    match format {
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Csv => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| CliError::Internal(format!(
                    "format mcp resource output failed: {}",
                    e
                )))?
            );
        }
        OutputFormat::Text | OutputFormat::Table => {
            println!("mcp resource");
            println!("mode: {}", mode);
            println!("uri: {}", output["uri"].as_str().unwrap_or_default());
            println!("status: {}", output["status"].as_u64().unwrap_or_default());
            println!("ok: {}", output["ok"].as_bool().unwrap_or(false));
            println!("response:");
            println!(
                "{}",
                serde_json::to_string_pretty(
                    output.get("response").unwrap_or(&serde_json::Value::Null)
                )
                .map_err(|e| CliError::Internal(format!("format mcp response failed: {}", e)))?
            );
        }
    }

    if !status.is_success() {
        return Err(CliError::Internal(format!(
            "mcp resource failed with status {}",
            status
        )));
    }

    if output
        .get("response")
        .and_then(|value| value.get("error"))
        .is_some()
    {
        return Err(CliError::Internal(
            "mcp resource returned JSON-RPC error payload".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_select_sql_accepts_select_statement() {
        let sql = "SELECT id, content FROM message LIMIT 10;";
        let normalized = validate_select_sql(sql).expect("select should be accepted");
        assert_eq!(normalized, "SELECT id, content FROM message LIMIT 10");
    }

    #[test]
    fn validate_select_sql_rejects_non_select_statement() {
        let sql = "DELETE FROM message";
        let err = validate_select_sql(sql).expect_err("non-select must be rejected");
        assert!(
            err.to_string()
                .contains("only SELECT statements are allowed")
        );
    }

    #[test]
    fn validate_select_sql_rejects_multiple_statements() {
        let sql = "SELECT 1; SELECT 2;";
        let err = validate_select_sql(sql).expect_err("multiple statements must be rejected");
        assert!(
            err.to_string()
                .contains("multiple SQL statements are not allowed")
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_mcp_tool_args_json_accepts_object_payload() {
        let parsed = parse_mcp_tool_args_json(r#"{"session_id":1,"limit":10}"#)
            .expect("object json should be accepted");
        assert_eq!(parsed.get("session_id").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(parsed.get("limit").and_then(|v| v.as_i64()), Some(10));
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_mcp_tool_args_json_rejects_non_object_payload() {
        let err =
            parse_mcp_tool_args_json(r#"[1,2,3]"#).expect_err("array json should be rejected");
        assert!(
            err.to_string()
                .contains("--args-json must be a JSON object")
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_optional_json_body_accepts_valid_payload() {
        let parsed =
            parse_optional_json_body(Some(r#"{"hello":"world"}"#.to_string())).expect("valid json");
        assert_eq!(
            parsed
                .as_ref()
                .and_then(|v| v.get("hello"))
                .and_then(|v| v.as_str()),
            Some("world")
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_optional_json_body_rejects_invalid_payload() {
        let err = parse_optional_json_body(Some("{not-json}".to_string()))
            .expect_err("invalid json should be rejected");
        assert!(err.to_string().contains("invalid --body-json"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_http_json_response_body_accepts_valid_http_json_payload() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"service\":\"xenobot-api\",\"version\":\"0.1.0\"}";
        let parsed = parse_http_json_response_body(raw).expect("json body should parse");
        assert_eq!(parsed["service"], "xenobot-api");
        assert_eq!(parsed["version"], "0.1.0");
    }

    #[cfg(feature = "api")]
    #[test]
    fn parse_http_json_response_body_rejects_non_json_payload() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nOK";
        assert!(
            parse_http_json_response_body(raw).is_none(),
            "non-json payload should be rejected"
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_tool_call_rejects_empty_url_before_network() {
        let err = run_mcp_tool_call(
            "   ".to_string(),
            crate::commands::McpCallMode::Rpc,
            "get_current_time".to_string(),
            "{}".to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty url must fail before network execution");
        assert!(err.to_string().contains("non-empty --url"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_tool_call_rejects_empty_tool_before_network() {
        let err = run_mcp_tool_call(
            "http://127.0.0.1:5030".to_string(),
            crate::commands::McpCallMode::Rpc,
            "   ".to_string(),
            "{}".to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty tool must fail before network execution");
        assert!(err.to_string().contains("non-empty --tool"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_tool_call_rejects_non_object_args_before_network() {
        let err = run_mcp_tool_call(
            "http://127.0.0.1:5030".to_string(),
            crate::commands::McpCallMode::Http,
            "chat_records".to_string(),
            r#"["invalid"]"#.to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("non-object args should fail before network execution");
        assert!(
            err.to_string()
                .contains("--args-json must be a JSON object")
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_tools_list_rejects_empty_url_before_network() {
        let err = run_mcp_tools_list(
            "   ".to_string(),
            crate::commands::McpCallMode::Rpc,
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty url must fail before network execution");
        assert!(err.to_string().contains("non-empty --url"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn encode_http_path_component_percent_encodes_reserved_chars() {
        let encoded = encode_http_path_component("xenobot://server/info name");
        assert_eq!(encoded, "xenobot%3A%2F%2Fserver%2Finfo%20name");
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_resources_list_rejects_empty_url_before_network() {
        let err = run_mcp_resources_list(
            "   ".to_string(),
            crate::commands::McpCallMode::Rpc,
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty url must fail before network execution");
        assert!(err.to_string().contains("non-empty --url"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_resource_read_rejects_empty_url_before_network() {
        let err = run_mcp_resource_read(
            " ".to_string(),
            crate::commands::McpCallMode::Http,
            "xenobot://server/info".to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty url must fail before network execution");
        assert!(err.to_string().contains("non-empty --url"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_resource_read_rejects_empty_uri_before_network() {
        let err = run_mcp_resource_read(
            "http://127.0.0.1:5030".to_string(),
            crate::commands::McpCallMode::Rpc,
            "   ".to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("empty uri must fail before network execution");
        assert!(err.to_string().contains("non-empty --uri"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_api_file_gateway_call_rejects_empty_method() {
        let err = run_api_file_gateway_call(
            Some(std::env::temp_dir().join("xenobot-gateway-call-test")),
            Some("req_test".to_string()),
            "   ".to_string(),
            Some("/health".to_string()),
            None,
            1000,
            OutputFormat::Json,
        )
        .expect_err("empty method should fail before request creation");
        assert!(err.to_string().contains("method must not be empty"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_api_smoke_check_validates_health_and_sql_contracts() {
        let temp_db = std::env::temp_dir().join(format!(
            "xenobot-cli-api-smoke-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        let result = run_api_smoke_check(Some(temp_db.clone()));
        let _ = std::fs::remove_file(&temp_db);
        assert!(
            result.is_ok(),
            "api smoke should pass health + sql contracts: {:?}",
            result.err()
        );
    }

    #[test]
    fn resolve_webhook_dispatch_settings_applies_defaults() {
        let settings = WebhookDispatchSettings::default();
        let resolved = resolve_webhook_dispatch_settings(&settings);
        assert_eq!(resolved.batch_size, 64);
        assert_eq!(resolved.max_concurrency, 8);
        assert_eq!(resolved.request_timeout_ms, 8_000);
        assert_eq!(resolved.flush_interval_ms, 250);
        assert_eq!(resolved.retry_attempts, 3);
        assert_eq!(resolved.retry_base_delay_ms, 150);
        assert!(resolved.queue_capacity >= 32);
    }

    #[test]
    fn apply_webhook_dispatch_update_resets_then_applies_fields() {
        let mut settings = WebhookDispatchSettings {
            batch_size: Some(12),
            max_concurrency: Some(3),
            request_timeout_ms: Some(4_000),
            flush_interval_ms: Some(500),
            retry_attempts: Some(2),
            retry_base_delay_ms: Some(90),
        };

        apply_webhook_dispatch_update(
            &mut settings,
            WebhookDispatchUpdate {
                reset: true,
                batch_size: Some(256),
                max_concurrency: None,
                request_timeout_ms: Some(20_000),
                flush_interval_ms: None,
                retry_attempts: Some(5),
                retry_base_delay_ms: None,
            },
        );

        assert_eq!(settings.batch_size, Some(256));
        assert_eq!(settings.max_concurrency, None);
        assert_eq!(settings.request_timeout_ms, Some(20_000));
        assert_eq!(settings.flush_interval_ms, None);
        assert_eq!(settings.retry_attempts, Some(5));
        assert_eq!(settings.retry_base_delay_ms, None);
    }

    #[test]
    fn resolve_webhook_dispatch_settings_clamps_out_of_range_values() {
        let settings = WebhookDispatchSettings {
            batch_size: Some(0),
            max_concurrency: Some(999),
            request_timeout_ms: Some(100),
            flush_interval_ms: Some(50_000),
            retry_attempts: Some(999),
            retry_base_delay_ms: Some(0),
        };
        let resolved = resolve_webhook_dispatch_settings(&settings);
        assert_eq!(resolved.batch_size, 1);
        assert_eq!(resolved.max_concurrency, 64);
        assert_eq!(resolved.request_timeout_ms, 500);
        assert_eq!(resolved.flush_interval_ms, 10_000);
        assert_eq!(resolved.retry_attempts, 8);
        assert_eq!(resolved.retry_base_delay_ms, 10);
        assert!((32..=8192).contains(&resolved.queue_capacity));
    }

    #[cfg(all(feature = "analysis", feature = "api"))]
    #[test]
    fn webhook_rule_matches_event_filters_by_event_sender_keyword() {
        let item = WebhookItem {
            id: "wh_1".to_string(),
            url: "http://127.0.0.1:65535/hook".to_string(),
            event_type: Some("message.created".to_string()),
            platform: None,
            chat_name: None,
            meta_id: None,
            sender: Some("alice".to_string()),
            keyword: Some("urgent".to_string()),
            created_at: "2026-02-23T00:00:00Z".to_string(),
        };
        let rule = webhook_item_to_rule(&item);
        let event_ok = WebhookMessageCreatedEvent {
            event_type: "message.created".to_string(),
            platform: "whatsapp".to_string(),
            chat_name: "Team Chat".to_string(),
            meta_id: 1,
            message_id: 10,
            sender_id: 7,
            sender_name: Some("Alice".to_string()),
            ts: 1_771_800_000,
            msg_type: 0,
            content: Some("urgent: please review".to_string()),
        };
        let event_bad_keyword = WebhookMessageCreatedEvent {
            content: Some("normal message".to_string()),
            ..event_ok.clone()
        };
        assert!(webhook_rule_matches_event(&rule, &event_ok));
        assert!(!webhook_rule_matches_event(&rule, &event_bad_keyword));
    }

    #[cfg(all(feature = "analysis", feature = "api"))]
    #[test]
    fn webhook_rule_matches_event_filters_by_platform_chat_and_meta_id() {
        let item = WebhookItem {
            id: "wh_2".to_string(),
            url: "http://127.0.0.1:65535/hook".to_string(),
            event_type: Some("message.created".to_string()),
            platform: Some("whatsapp".to_string()),
            chat_name: Some("Team Chat".to_string()),
            meta_id: Some(42),
            sender: None,
            keyword: None,
            created_at: "2026-03-05T00:00:00Z".to_string(),
        };
        let rule = webhook_item_to_rule(&item);
        let event_ok = WebhookMessageCreatedEvent {
            event_type: "message.created".to_string(),
            platform: "whatsapp".to_string(),
            chat_name: "Team Chat".to_string(),
            meta_id: 42,
            message_id: 100,
            sender_id: 1,
            sender_name: Some("Alice".to_string()),
            ts: 1_772_000_000,
            msg_type: 0,
            content: Some("hello".to_string()),
        };
        let event_bad_platform = WebhookMessageCreatedEvent {
            platform: "telegram".to_string(),
            ..event_ok.clone()
        };
        let event_bad_chat = WebhookMessageCreatedEvent {
            chat_name: "Another Chat".to_string(),
            ..event_ok.clone()
        };
        let event_bad_meta = WebhookMessageCreatedEvent {
            meta_id: 7,
            ..event_ok.clone()
        };

        assert!(webhook_rule_matches_event(&rule, &event_ok));
        assert!(!webhook_rule_matches_event(&rule, &event_bad_platform));
        assert!(!webhook_rule_matches_event(&rule, &event_bad_chat));
        assert!(!webhook_rule_matches_event(&rule, &event_bad_meta));
    }

    #[cfg(all(feature = "analysis", feature = "api"))]
    #[test]
    fn source_file_fingerprint_changes_with_content() {
        let temp_path = std::env::temp_dir().join(format!(
            "xenobot-fp-{}-{}.txt",
            std::process::id(),
            chrono::Utc::now().timestamp_micros()
        ));
        std::fs::write(&temp_path, "hello").expect("write initial temp file");
        let fp1 = build_source_file_fingerprint(&temp_path).expect("build fingerprint v1");
        assert!(fp1.fingerprint.starts_with("v2:"));

        std::fs::write(&temp_path, "hello world").expect("rewrite temp file");
        let fp2 = build_source_file_fingerprint(&temp_path).expect("build fingerprint v2");
        assert_ne!(fp1.fingerprint, fp2.fingerprint);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[cfg(all(feature = "analysis", feature = "api"))]
    #[test]
    fn monitor_checkpoint_helper_matches_completed_fingerprint() {
        let temp_db = std::env::temp_dir().join(format!(
            "xenobot-checkpoint-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_micros()
        ));
        let conn = rusqlite::Connection::open(&temp_db).expect("open temp db");
        conn.execute_batch(
            r#"
            CREATE TABLE import_source_checkpoint (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_kind TEXT NOT NULL,
                source_path TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                modified_at INTEGER NOT NULL DEFAULT 0,
                platform TEXT,
                chat_name TEXT,
                meta_id INTEGER,
                last_processed_at INTEGER NOT NULL,
                last_inserted_messages INTEGER NOT NULL DEFAULT 0,
                last_duplicate_messages INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'completed',
                error_message TEXT
            );
            "#,
        )
        .expect("create checkpoint table");
        conn.execute(
            r#"
            INSERT INTO import_source_checkpoint(
                source_kind, source_path, fingerprint, file_size, modified_at,
                last_processed_at, last_inserted_messages, last_duplicate_messages, status
            ) VALUES (?1, ?2, ?3, 1, 1, 1, 0, 0, 'completed')
            "#,
            rusqlite::params!["monitor", "/tmp/chat.txt", "v2:test-fingerprint"],
        )
        .expect("insert checkpoint");

        let matched = monitor_source_checkpoint_unchanged(
            &temp_db,
            std::path::Path::new("/tmp/chat.txt"),
            "v2:test-fingerprint",
        );
        let mismatch = monitor_source_checkpoint_unchanged(
            &temp_db,
            std::path::Path::new("/tmp/chat.txt"),
            "v2:another",
        );
        assert!(matched);
        assert!(!mismatch);

        let _ = std::fs::remove_file(&temp_db);
    }

    #[cfg(feature = "api")]
    #[test]
    fn file_gateway_metrics_snapshot_contains_latency_and_queue_signals() {
        let mut metrics = FileGatewayRuntimeMetrics::with_started_at(1_700_000_000);
        metrics.record_batch(&FileGatewayBatchMetrics {
            queue_depth: 12,
            processed: 4,
            succeeded: 3,
            failed: 1,
            lock_contended: 2,
            latency_samples_ms: vec![10, 20, 30, 40],
        });
        metrics.record_batch(&FileGatewayBatchMetrics {
            queue_depth: 3,
            processed: 2,
            succeeded: 2,
            failed: 0,
            lock_contended: 0,
            latency_samples_ms: vec![50, 60],
        });

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_processed, 6);
        assert_eq!(snapshot.total_succeeded, 5);
        assert_eq!(snapshot.total_failed, 1);
        assert_eq!(snapshot.total_lock_contended, 2);
        assert_eq!(snapshot.queue_depth, 3);
        assert_eq!(snapshot.last_tick_processed, 2);
        assert!(snapshot.latency_avg_ms > 0.0);
        assert!(snapshot.latency_p95_ms >= 50);
        assert_eq!(snapshot.latency_max_ms, 60);
    }

    #[test]
    fn semantic_chunk_text_splits_long_text_with_overlap() {
        let input = "0123456789abcdefghijKLMNOPQRSTuvwxyz";
        let chunks = semantic_chunk_text(input, 12, 4);
        assert!(chunks.len() >= 3);
        assert_eq!(chunks[0], "0123456789ab");
        assert!(chunks[1].starts_with("89ab"));
    }

    #[test]
    fn semantic_embedding_similarity_prefers_related_content() {
        let query = embed_text_for_semantic("database migration checkpoint incremental import");
        let related =
            embed_text_for_semantic("incremental import checkpoint for database migration");
        let unrelated = embed_text_for_semantic("sunny beach holiday music and mountain hiking");

        let related_score = cosine_similarity(&query, &related);
        let unrelated_score = cosine_similarity(&query, &unrelated);
        assert!(related_score > unrelated_score);
        assert!(related_score > 0.15);
    }

    #[test]
    fn semantic_query_rewrite_normalizes_phrases() {
        let rewritten = rewrite_semantic_query("  聊天记录 msg 语音!!!  ");
        assert!(rewritten.contains("聊天"));
        assert!(rewritten.contains("message"));
        assert!(rewritten.contains("音频"));
    }

    #[test]
    fn slugify_identifier_normalizes_name() {
        let slug = slugify_identifier("  Team Account #1 (Primary) ");
        assert_eq!(slug, "team-account-1-primary");
    }

    #[test]
    fn allocate_account_id_adds_suffix_when_conflicted() {
        let existing = ["wechat-team", "wechat-team-2"]
            .into_iter()
            .map(|v| v.to_string())
            .collect::<std::collections::HashSet<_>>();
        let id = allocate_account_id("wechat", "Team", &existing)
            .expect("should allocate a unique account id");
        assert_eq!(id, "wechat-team-3");
    }

    #[test]
    fn allocate_account_id_rejects_empty_after_normalization() {
        let existing = std::collections::HashSet::new();
        let err = allocate_account_id("???", "   ", &existing)
            .expect_err("empty normalized identifier must fail");
        assert!(err.to_string().contains("account id generation failed"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn sanitize_file_component_falls_back_when_empty() {
        assert_eq!(sanitize_file_component(""), "chat");
        assert_eq!(sanitize_file_component("   "), "chat");
        assert_eq!(
            sanitize_file_component("Team Discussion #1"),
            "team-discussion-1"
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn short_path_hash_is_stable_for_same_path() {
        let path = std::path::Path::new("/tmp/xenobot/chat.json");
        let first = short_path_hash(path);
        let second = short_path_hash(path);
        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
    }

    #[test]
    fn run_time_distribution_analysis_returns_rows() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE message (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id INTEGER NOT NULL,
                ts INTEGER NOT NULL,
                msg_type INTEGER NOT NULL,
                content TEXT
            );
            INSERT INTO message(sender_id, ts, msg_type, content) VALUES
              (1, 1704067200, 0, 'a'),
              (1, 1704067260, 0, 'b'),
              (2, 1704153600, 0, 'c');
            "#,
        )
        .expect("seed message table");

        let payload = run_time_distribution_analysis(&conn, &TimeGranularity::Daily)
            .expect("time distribution should succeed");
        assert_eq!(
            payload["analysis"].as_str().unwrap_or_default(),
            "time_distribution_daily"
        );
        assert!(
            payload["rows"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
    }

    #[test]
    fn run_advanced_analysis_dragon_king_returns_ranked_rows() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE member (
                id INTEGER PRIMARY KEY,
                account_name TEXT,
                group_nickname TEXT,
                platform_id TEXT
            );
            CREATE TABLE message (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id INTEGER NOT NULL,
                ts INTEGER NOT NULL,
                msg_type INTEGER NOT NULL,
                content TEXT
            );
            INSERT INTO member(id, account_name, group_nickname, platform_id) VALUES
              (1, 'alice', NULL, 'wechat:alice'),
              (2, 'bob', NULL, 'wechat:bob');
            INSERT INTO message(sender_id, ts, msg_type, content) VALUES
              (1, 1704067200, 0, 'm1'),
              (1, 1704067260, 0, 'm2'),
              (2, 1704067320, 0, 'm3');
            "#,
        )
        .expect("seed analysis tables");

        let payload = run_advanced_analysis(&conn, &AdvancedAnalysis::DragonKing)
            .expect("advanced analysis should succeed");
        assert_eq!(
            payload["analysis"].as_str().unwrap_or_default(),
            "dragon_king"
        );
        let rows = payload["rows"]
            .as_array()
            .expect("rows should be an array")
            .to_vec();
        assert!(!rows.is_empty());
        assert_eq!(rows[0]["senderName"], "alice");
    }

    #[test]
    fn source_platform_matrix_rows_cover_expected_legal_safe_set() {
        let rows = build_source_platform_matrix_rows();
        assert_eq!(rows.len(), 17, "expected 17 legal-safe runtime platforms");

        let ids = rows
            .iter()
            .map(|row| row.platform_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        for required in [
            "wechat",
            "whatsapp",
            "line",
            "qq",
            "discord",
            "instagram",
            "telegram",
            "imessage",
            "messenger",
            "kakaotalk",
            "slack",
            "teams",
            "signal",
            "skype",
            "googlechat",
            "zoom",
            "viber",
        ] {
            assert!(ids.contains(required), "missing platform id: {required}");
        }
    }

    #[test]
    fn collect_db_verification_reports_ok_when_required_objects_exist() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE meta (id INTEGER PRIMARY KEY);
            CREATE TABLE member (id INTEGER PRIMARY KEY, platform_id TEXT);
            CREATE TABLE message (id INTEGER PRIMARY KEY, sender_id INTEGER, meta_id INTEGER, ts INTEGER);
            CREATE TABLE sessions (id INTEGER PRIMARY KEY, meta_id INTEGER, created_at INTEGER);
            CREATE TABLE message_context (id INTEGER PRIMARY KEY, session_id INTEGER, message_id INTEGER);
            CREATE TABLE member_name_history (
                id INTEGER PRIMARY KEY,
                member_id INTEGER,
                start_ts INTEGER
            );
            CREATE TABLE import_progress (id INTEGER PRIMARY KEY);
            CREATE TABLE import_source_checkpoint (id INTEGER PRIMARY KEY);

            CREATE INDEX idx_message_meta_ts_id ON message(meta_id, ts, id);
            CREATE INDEX idx_message_meta_sender_ts_id ON message(meta_id, sender_id, ts, id);
            CREATE INDEX idx_sessions_meta_created_at ON sessions(meta_id, created_at);
            CREATE INDEX idx_member_name_history_member_start_ts ON member_name_history(member_id, start_ts);
            CREATE INDEX idx_message_context_session_message ON message_context(session_id, message_id);
            CREATE INDEX idx_chat_session_meta_start_ts_id ON sessions(meta_id, created_at, id);
            "#,
        )
        .expect("seed required schema");

        let report =
            collect_db_verification(std::path::Path::new(":memory:"), &conn).expect("verify db");
        assert!(report.ok);
        assert_eq!(report.missing_required, 0);
    }

    #[test]
    fn collect_db_verification_reports_missing_required_objects() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE meta (id INTEGER PRIMARY KEY);
            CREATE TABLE member (id INTEGER PRIMARY KEY);
            CREATE TABLE message (id INTEGER PRIMARY KEY, meta_id INTEGER, ts INTEGER);
            "#,
        )
        .expect("seed partial schema");

        let report =
            collect_db_verification(std::path::Path::new(":memory:"), &conn).expect("verify db");
        assert!(!report.ok);
        assert!(report.missing_required > 0);
        assert!(
            report
                .checks
                .iter()
                .any(|row| row.required && row.name == "sessions" && !row.exists)
        );
    }

    #[test]
    fn collect_db_verification_tracks_optional_indexes_from_migrations() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE meta (
                id INTEGER PRIMARY KEY,
                platform TEXT,
                name TEXT
            );
            CREATE TABLE member (id INTEGER PRIMARY KEY, platform_id TEXT);
            CREATE TABLE message (
                id INTEGER PRIMARY KEY,
                sender_id INTEGER,
                meta_id INTEGER,
                ts INTEGER,
                msg_type INTEGER,
                content TEXT
            );
            CREATE TABLE sessions (id INTEGER PRIMARY KEY, meta_id INTEGER, created_at INTEGER);
            CREATE TABLE message_context (id INTEGER PRIMARY KEY, session_id INTEGER, message_id INTEGER);
            CREATE TABLE member_name_history (
                id INTEGER PRIMARY KEY,
                member_id INTEGER,
                start_ts INTEGER
            );
            CREATE TABLE import_progress (id INTEGER PRIMARY KEY);
            CREATE TABLE import_source_checkpoint (id INTEGER PRIMARY KEY);

            CREATE INDEX idx_message_meta_ts_id ON message(meta_id, ts, id);
            CREATE INDEX idx_message_meta_sender_ts_id ON message(meta_id, sender_id, ts, id);
            CREATE INDEX idx_sessions_meta_created_at ON sessions(meta_id, created_at);
            CREATE INDEX idx_member_name_history_member_start_ts ON member_name_history(member_id, start_ts);
            CREATE INDEX idx_message_context_session_message ON message_context(session_id, message_id);
            CREATE INDEX idx_chat_session_meta_start_ts_id ON sessions(meta_id, created_at, id);
            CREATE INDEX idx_message_dedup_lookup ON message(meta_id, sender_id, ts, msg_type, content);
            CREATE INDEX idx_meta_platform_name ON meta(platform, name);
            "#,
        )
        .expect("seed schema including optional indexes");

        let report =
            collect_db_verification(std::path::Path::new(":memory:"), &conn).expect("verify db");
        assert!(report.ok);
        assert_eq!(report.missing_optional, 0);
        assert!(
            report
                .checks
                .iter()
                .any(|row| !row.required && row.name == "idx_message_dedup_lookup" && row.exists)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|row| !row.required && row.name == "idx_meta_platform_name" && row.exists)
        );
    }

    #[test]
    fn collect_db_checkpoints_handles_missing_table() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        let report = collect_db_checkpoints(
            std::path::Path::new(":memory:"),
            &conn,
            Some("monitor"),
            Some("completed"),
            10,
        )
        .expect("collect checkpoints");
        assert!(!report.table_present);
        assert_eq!(report.total_rows, 0);
        assert_eq!(report.returned_rows, 0);
        assert!(report.rows.is_empty());
    }

    #[test]
    fn collect_db_checkpoints_applies_filters_and_limit() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE import_source_checkpoint (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_kind TEXT NOT NULL,
                source_path TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                modified_at INTEGER NOT NULL DEFAULT 0,
                platform TEXT,
                chat_name TEXT,
                meta_id INTEGER,
                last_processed_at INTEGER NOT NULL,
                last_inserted_messages INTEGER NOT NULL DEFAULT 0,
                last_duplicate_messages INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'completed',
                error_message TEXT
            );
            INSERT INTO import_source_checkpoint (
                source_kind, source_path, fingerprint, file_size, modified_at,
                platform, chat_name, meta_id, last_processed_at,
                last_inserted_messages, last_duplicate_messages, status, error_message
            ) VALUES
                ('monitor', '/tmp/a.json', 'fp-a', 10, 100, 'wechat', 'alpha', 1, 200, 2, 0, 'completed', NULL),
                ('monitor', '/tmp/b.json', 'fp-b', 11, 101, 'wechat', 'beta', 2, 300, 1, 1, 'failed', 'parse error'),
                ('api-import', '/tmp/c.json', 'fp-c', 12, 102, 'telegram', 'gamma', 3, 400, 3, 0, 'completed', NULL);
            "#,
        )
        .expect("seed checkpoint table");

        let report = collect_db_checkpoints(
            std::path::Path::new(":memory:"),
            &conn,
            Some("monitor"),
            None,
            1,
        )
        .expect("collect checkpoints");
        assert!(report.table_present);
        assert_eq!(report.total_rows, 2);
        assert_eq!(report.returned_rows, 1);
        assert_eq!(report.rows[0].source_kind, "monitor");
        assert_eq!(report.rows[0].source_path, "/tmp/b.json");
    }

    #[test]
    fn collect_db_schema_reports_tables_columns_indexes_and_foreign_keys() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE parent (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE child (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                content TEXT,
                FOREIGN KEY(parent_id) REFERENCES parent(id) ON DELETE CASCADE
            );
            CREATE INDEX idx_child_parent_id ON child(parent_id);
            "#,
        )
        .expect("seed schema");

        let report = collect_db_schema(std::path::Path::new(":memory:"), &conn, false)
            .expect("collect schema");
        assert_eq!(report.summary.table_count, 2);
        assert!(report.summary.column_count >= 5);
        assert!(report.summary.index_count >= 1);
        assert!(report.summary.foreign_key_count >= 1);

        let child = report
            .tables
            .iter()
            .find(|t| t.name == "child")
            .expect("child table");
        assert!(
            child
                .columns
                .iter()
                .any(|c| c.name == "id" && c.primary_key)
        );
        assert!(
            child
                .indexes
                .iter()
                .any(|idx| idx.name == "idx_child_parent_id")
        );
        assert!(child.foreign_keys.iter().any(|fk| fk.table == "parent"));
        assert!(child.row_count.is_none());
    }

    #[test]
    fn collect_db_schema_can_include_row_count() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE sample (
                id INTEGER PRIMARY KEY,
                content TEXT
            );
            INSERT INTO sample(id, content) VALUES (1, 'a'), (2, 'b'), (3, 'c');
            "#,
        )
        .expect("seed schema");

        let report = collect_db_schema(std::path::Path::new(":memory:"), &conn, true)
            .expect("collect schema");
        let sample = report
            .tables
            .iter()
            .find(|t| t.name == "sample")
            .expect("sample table");
        assert_eq!(sample.row_count, Some(3));
    }

    #[cfg(feature = "api")]
    #[test]
    fn normalize_mcp_preset_target_accepts_aliases() {
        assert_eq!(
            normalize_mcp_preset_target("claude").expect("claude alias should normalize"),
            "claude-desktop"
        );
        assert_eq!(
            normalize_mcp_preset_target("claude_desktop")
                .expect("claude_desktop alias should normalize"),
            "claude-desktop"
        );
        assert_eq!(
            normalize_mcp_preset_target("chatwise").expect("chatwise should normalize"),
            "chatwise"
        );
        assert_eq!(
            normalize_mcp_preset_target("opencode").expect("opencode should normalize"),
            "opencode"
        );
        assert_eq!(
            normalize_mcp_preset_target("pencil").expect("pencil should normalize"),
            "pencil"
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn run_mcp_preset_rejects_unknown_target_before_network() {
        let err = run_mcp_integration_preset_fetch(
            "http://127.0.0.1:65535".to_string(),
            "unknown-target".to_string(),
            OutputFormat::Json,
            1500,
        )
        .expect_err("unknown target should fail before network execution");
        assert!(err.to_string().contains("is not supported"));
    }

    #[cfg(feature = "api")]
    #[test]
    fn validate_mcp_integration_preset_contract_accepts_known_targets() {
        let claude = serde_json::json!({
            "id": "claude-desktop",
            "transport": { "sse": "http://127.0.0.1:8081/sse", "websocket": "ws://127.0.0.1:8081/ws", "tools": "http://127.0.0.1:8081/tools" },
            "configuration": {
                "mcpServers": { "xenobot": { "command": "pnpm", "args": ["dlx", "mcp-remote", "http://127.0.0.1:8081/sse"] } }
            },
            "notes": ["n1"]
        });
        validate_mcp_integration_preset_contract("claude-desktop", &claude)
            .expect("claude preset should validate");

        let chatwise = serde_json::json!({
            "id": "chatwise",
            "transport": { "sse": "http://127.0.0.1:8081/sse", "websocket": "ws://127.0.0.1:8081/ws", "tools": "http://127.0.0.1:8081/tools" },
            "configuration": { "servers": [{ "name": "xenobot", "transport": "sse", "url": "http://127.0.0.1:8081/sse" }] },
            "notes": ["n1"]
        });
        validate_mcp_integration_preset_contract("chatwise", &chatwise)
            .expect("chatwise preset should validate");

        let opencode = serde_json::json!({
            "id": "opencode",
            "transport": { "sse": "http://127.0.0.1:8081/sse", "websocket": "ws://127.0.0.1:8081/ws", "tools": "http://127.0.0.1:8081/tools" },
            "configuration": { "mcpServers": [{ "name": "xenobot", "transport": "sse", "url": "http://127.0.0.1:8081/sse" }] },
            "notes": ["n1"]
        });
        validate_mcp_integration_preset_contract("opencode", &opencode)
            .expect("opencode preset should validate");

        let pencil = serde_json::json!({
            "id": "pencil",
            "transport": { "sse": "http://127.0.0.1:8081/sse", "websocket": "ws://127.0.0.1:8081/ws", "tools": "http://127.0.0.1:8081/tools" },
            "configuration": { "servers": [{ "name": "xenobot", "transport": "sse", "url": "http://127.0.0.1:8081/sse", "toolsUrl": "http://127.0.0.1:8081/tools" }] },
            "notes": ["n1"]
        });
        validate_mcp_integration_preset_contract("pencil", &pencil)
            .expect("pencil preset should validate");
    }

    #[cfg(feature = "api")]
    #[test]
    fn validate_mcp_integration_preset_contract_rejects_invalid_payload() {
        let invalid = serde_json::json!({
            "id": "claude-desktop",
            "transport": { "sse": "http://127.0.0.1:8081/sse", "websocket": "ws://127.0.0.1:8081/ws", "tools": "http://127.0.0.1:8081/tools" },
            "configuration": { "mcpServers": { "xenobot": { "command": "node", "args": ["run"] } } },
            "notes": ["n1"]
        });
        let err = validate_mcp_integration_preset_contract("claude-desktop", &invalid)
            .expect_err("invalid preset should fail");
        let msg = err.to_string();
        assert!(msg.contains("mcp-remote"));
    }
}

/// Parse command line arguments and run the application.
pub fn run() -> Result<()> {
    let app = App::new()?;
    app.run()
}
