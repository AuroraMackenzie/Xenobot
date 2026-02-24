//! CLI application entry point and configuration.
//!
//! This module provides the main CLI application logic, including argument parsing,
//! configuration loading, and command dispatch.

use crate::commands::{
    AccountCommand, Cli, Commands, DecryptArgs, ExportArgs, ExportFormat, ImportArgs, KeyArgs,
    MonitorArgs, OutputFormat, PlatformFormat, QueryArgs, QueryType, SourceArgs, SourceCommand,
    WebhookArgs, WebhookCommand,
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
#[cfg(all(feature = "analysis", feature = "api"))]
use xenobot_core::webhook::{
    append_dead_letter_entry, build_dead_letter_entry, merge_webhook_dispatch_stats,
    webhook_rule_matches_event, WebhookDispatchStats, WebhookMessageCreatedEvent, WebhookRule,
};
use xenobot_core::webhook::{
    overwrite_dead_letter_entries, read_dead_letter_entries, WebhookDeadLetterEntry,
};
use xenobot_core::{
    discover_sources_for_all_platforms, discover_sources_for_platform,
    platform_id as core_platform_id, Platform as RuntimePlatform, SourceCandidate,
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
        let mut config = AppConfig::default();
        config.verbosity = cli.verbose;

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
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(default_wechat_data_dir);

        println!("decrypt plan generated");
        println!("data dir: {}", target_data_dir);
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("threads: {}", args.threads);
        println!("overwrite: {}", args.overwrite);
        println!("platform format: {}", platform_format_id(args.format));
        println!("data key: {}", mask_secret(&data_key));
        println!("image key: {}", mask_secret(&image_key));
        println!("note: full decrypt execution pipeline is still in progress");
        Ok(())
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
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(default_wechat_data_dir);

        println!("monitor plan generated");
        println!("watch dir: {}", target_data_dir);
        println!("work dir: {}", args.work_dir.to_string_lossy());
        println!("interval seconds: {}", args.interval);
        println!("start immediately: {}", args.start);
        println!("platform format: {}", platform_format_id(args.format));
        println!("data key: {}", mask_secret(&data_key));
        println!("image key: {}", mask_secret(&image_key));
        println!("note: full monitor execution pipeline is still in progress");
        Ok(())
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
                    db_path.clone(),
                    *cors,
                    *websocket,
                ),
                ApiCommand::Status => print_api_server_status(),
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
            }
        }

        #[cfg(not(feature = "api"))]
        {
            let _ = args;
            println!("API command requires CLI build with --features api");
            Ok(())
        }
    }

    fn handle_analyze(&self, _args: &crate::commands::AnalyzeArgs) -> Result<()> {
        println!("Analysis command not yet implemented");
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
                    let webhook_rules: Vec<WebhookRule> = read_webhook_store()?
                        .items
                        .iter()
                        .map(webhook_item_to_rule)
                        .collect();
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
                                .timeout(std::time::Duration::from_secs(8))
                                .build()
                                .map_err(|e| CliError::Network(e.to_string()))?;
                            Some(spawn_webhook_dispatch_worker(
                                client,
                                webhook_rules.clone(),
                                64,
                                8,
                                512,
                            ))
                        };
                        let mut run_scope_session_ids: std::collections::HashMap<String, i64> =
                            std::collections::HashMap::new();

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
                                        let candidates = repo
                                            .list_chats(Some(&platform), 10_000, 0)
                                            .await
                                            .map_err(|e| CliError::Database(e.to_string()))?;
                                        candidates
                                            .into_iter()
                                            .find(|meta| meta.name == chat_name)
                                            .map(|meta| meta.id)
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

                                payloads_processed += 1;
                                let inserted_before = inserted_messages;
                                let duplicates_before = skipped_duplicates;
                                let mut dedup_in_batch: std::collections::HashSet<String> =
                                    std::collections::HashSet::new();

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
                let items = discover_sources_for_all_platforms();
                if *details {
                    return print_source_candidates(&items, false, format);
                }

                let mut grouped: HashMap<String, (usize, usize)> = HashMap::new();
                for item in items {
                    let entry = grouped.entry(item.platform_id).or_insert((0, 0));
                    entry.0 += 1;
                    if item.exists && item.readable {
                        entry.1 += 1;
                    }
                }
                match format {
                    OutputFormat::Json => {
                        let mut rows = Vec::new();
                        for (platform, (candidates, available)) in grouped {
                            rows.push(serde_json::json!({
                                "platform": platform,
                                "candidates": candidates,
                                "available": available,
                            }));
                        }
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&rows)
                                .map_err(|e| CliError::Parse(e.to_string()))?
                        );
                    }
                    _ => {
                        println!("platform source summary");
                        let mut platforms: Vec<_> = grouped.into_iter().collect();
                        platforms.sort_by(|a, b| a.0.cmp(&b.0));
                        for (platform, (candidates, available)) in platforms {
                            println!(
                                "- {}: {} available / {} candidates",
                                platform, available, candidates
                            );
                        }
                    }
                }
                Ok(())
            }
            AccountCommand::Switch { account_id } => {
                println!("active account switch requested: {}", account_id);
                println!("note: persistent account switching is not yet wired");
                Ok(())
            }
            AccountCommand::Add {
                name,
                data_dir,
                format,
                wechat_version,
            } => {
                println!("account registration plan generated");
                println!("name: {}", name);
                println!("platform format: {}", platform_format_id(*format));
                println!("version hint: {}", wechat_version);
                println!(
                    "data dir: {}",
                    data_dir
                        .as_ref()
                        .map(|v| v.to_string_lossy().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("note: persistent account registry implementation is in progress");
                Ok(())
            }
        }
    }

    fn handle_webhook(&self, args: &WebhookArgs) -> Result<()> {
        match &args.command {
            WebhookCommand::Add {
                url,
                event_type,
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
                let mut store = read_webhook_store()?;
                let id = format!(
                    "wh_{}_{}",
                    chrono::Utc::now().timestamp(),
                    store.items.len() + 1
                );
                let item = WebhookItem {
                    id: id.clone(),
                    url: normalized_url.to_string(),
                    event_type: event_type.as_ref().map(|v| v.trim().to_string()),
                    sender: sender.as_ref().map(|v| v.trim().to_string()),
                    keyword: keyword.as_ref().map(|v| v.trim().to_string()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                store.items.push(item.clone());
                write_webhook_store(&store)?;
                println!("webhook added");
                println!("id: {}", id);
                println!("url: {}", item.url);
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
                        println!("id,url,event_type,sender,keyword,created_at");
                        for item in store.items {
                            println!(
                                "{},{},{},{},{},{}",
                                csv_escape(&item.id),
                                csv_escape(&item.url),
                                csv_escape(item.event_type.as_deref().unwrap_or_default()),
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
                                "- {} | {} | event={} sender={} keyword={} created_at={}",
                                item.id,
                                item.url,
                                item.event_type.unwrap_or_else(|| "-".to_string()),
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

                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| CliError::Internal(e.to_string()))?;

                let (remaining, retried, delivered, failed) = runtime.block_on(async move {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(8))
                        .build()
                        .map_err(|e| CliError::Network(e.to_string()))?;

                    let mut remaining: Vec<WebhookDeadLetterEntry> = Vec::new();
                    let mut retried = 0usize;
                    let mut delivered = 0usize;
                    let mut failed = 0usize;

                    for mut entry in entries {
                        if retried >= *limit {
                            remaining.push(entry);
                            continue;
                        }
                        retried += 1;

                        let mut ok = false;
                        let mut last_error = String::new();
                        for attempt in 0..3u32 {
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
                                    if attempt < 2 {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            150_u64 * (1_u64 << attempt),
                                        ))
                                        .await;
                                    }
                                }
                                Err(err) => {
                                    last_error = err.to_string();
                                    if attempt < 2 {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            150_u64 * (1_u64 << attempt),
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
    event_type: Option<String>,
    sender: Option<String>,
    keyword: Option<String>,
    created_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WebhookStore {
    items: Vec<WebhookItem>,
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

#[cfg(all(feature = "analysis", feature = "api"))]
fn webhook_item_to_rule(item: &WebhookItem) -> WebhookRule {
    WebhookRule {
        id: item.id.clone(),
        url: item.url.clone(),
        event_type: item.event_type.clone(),
        sender: item.sender.clone(),
        keyword: item.keyword.clone(),
        created_at: Some(item.created_at.clone()),
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
    batch_size: usize,
    max_concurrency: usize,
    queue_capacity: usize,
) -> WebhookDispatchWorker {
    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<WebhookMessageCreatedEvent>(queue_capacity.max(1));
    let join_handle = tokio::spawn(async move {
        let mut total = WebhookDispatchStats::default();
        let mut buffer = Vec::new();

        while let Some(event) = receiver.recv().await {
            buffer.push(event);
            if buffer.len() >= batch_size.max(1) {
                let stats =
                    flush_webhook_queue(&client, items.as_slice(), &mut buffer, max_concurrency)
                        .await;
                merge_webhook_dispatch_stats(&mut total, &stats);
            }
        }

        if !buffer.is_empty() {
            let stats =
                flush_webhook_queue(&client, items.as_slice(), &mut buffer, max_concurrency).await;
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
) -> WebhookDispatchStats {
    let mut stats = WebhookDispatchStats::default();
    for item in items {
        if !webhook_rule_matches_event(item, event) {
            stats.filtered += 1;
            continue;
        }
        stats.attempted += 1;

        let mut delivered = false;
        let mut attempts_used = 0u32;
        let mut last_error = "unknown delivery failure".to_string();
        for attempt in 0..3u32 {
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
                    if attempt < 2 {
                        let wait_ms = 150_u64 * (1_u64 << attempt);
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                    }
                }
                Err(err) => {
                    last_error = err.to_string();
                    if attempt < 2 {
                        let wait_ms = 150_u64 * (1_u64 << attempt);
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
        set.spawn(async move {
            let _permit = semaphore_clone.acquire_owned().await.ok();
            dispatch_webhook_message_created(&client_clone, items_clone.as_slice(), &event).await
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
            return Ok((data.trim().to_string(), image.trim().to_string()))
        }
        (None, None) => {}
        _ => {
            return Err(CliError::Argument(
                "data_key and image_key must be provided together".to_string(),
            ))
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

    let webhook_rules: Vec<WebhookRule> = read_webhook_store()?
        .items
        .iter()
        .map(webhook_item_to_rule)
        .collect();

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
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .map_err(|e| CliError::Network(e.to_string()))?;
            Some(spawn_webhook_dispatch_worker(
                client,
                webhook_rules,
                64,
                8,
                512,
            ))
        };

        let mut summary = MonitorDbWriteSummary {
            meta_id,
            ..Default::default()
        };
        let mut dedup_in_batch: std::collections::HashSet<String> =
            std::collections::HashSet::new();
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
            println!("path,size_bytes,table_count,message_count,member_count,chat_count,migration_versions");
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
        ("chatlog", "chat log"),
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
    let chunks = semantic_chunk_text(
        text,
        SEMANTIC_CHUNK_MAX_CHARS,
        SEMANTIC_CHUNK_OVERLAP_CHARS,
    );
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
fn unix_socket_max_path_bytes() -> usize {
    if cfg!(target_os = "macos") {
        103
    } else {
        107
    }
}

#[cfg(all(feature = "api", unix))]
fn unix_socket_path_within_limit(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len() <= unix_socket_max_path_bytes()
}

#[cfg(all(feature = "api", unix))]
fn select_sandbox_safe_unix_socket_path() -> Result<PathBuf> {
    let mut candidate_dirs = Vec::new();
    if let Ok(explicit_dir) = std::env::var("XENOBOT_API_SOCKET_DIR") {
        let trimmed = explicit_dir.trim();
        if !trimmed.is_empty() {
            candidate_dirs.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let trimmed = tmpdir.trim();
        if !trimmed.is_empty() {
            candidate_dirs.push(PathBuf::from(trimmed));
        }
    }
    candidate_dirs.push(PathBuf::from("/tmp"));

    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();

    let mut chosen: Option<PathBuf> = None;
    for dir in candidate_dirs {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        for name in [
            format!("xb-{}-{}.sock", pid, nanos % 100_000),
            format!("xb-{}.sock", pid),
            "xb.sock".to_string(),
        ] {
            let candidate = dir.join(name);
            if unix_socket_path_within_limit(&candidate) {
                chosen = Some(candidate);
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }

    chosen.ok_or_else(|| {
        CliError::Argument(format!(
            "cannot build unix socket path within {}-byte limit; set XENOBOT_API_SOCKET_DIR to a short writable directory",
            unix_socket_max_path_bytes()
        ))
    })
}

#[cfg(feature = "api")]
fn select_file_gateway_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Ok(env_dir) = std::env::var("XENOBOT_FILE_API_DIR") {
        let trimmed = env_dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let trimmed = tmpdir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("xenobot-file-api"));
        }
    }
    Ok(PathBuf::from("/tmp").join("xenobot-file-api"))
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
fn print_api_server_status() -> Result<()> {
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
            if let Ok(raw) = std::fs::read_to_string(&metrics_path) {
                if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&raw) {
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
            }
        } else {
            println!("api transport: tcp");
            println!("api addr: {}", state.bind_addr);
        }
        println!("pid: {}", state.pid);
        println!("state file: present");
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
        println!("status: {}", status);
        println!("cors enabled: {}", state.cors_enabled);
        println!("websocket enabled: {}", state.websocket_enabled);
        if let Some(path) = state.db_path {
            println!("db path: {}", path);
        }
        return Ok(());
    }

    let target = std::env::var("XENOBOT_API_ADDR").unwrap_or_else(|_| "127.0.0.1:5030".to_string());
    let parsed = target
        .parse::<SocketAddr>()
        .map_err(|e| CliError::Argument(format!("invalid XENOBOT_API_ADDR '{}': {}", target, e)))?;
    let alive = TcpStream::connect_timeout(&parsed, Duration::from_millis(400)).is_ok();
    println!("api addr: {}", parsed);
    println!("state file: missing");
    println!("status: {}", if alive { "running" } else { "stopped" });
    Ok(())
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

    if let Some(path) = db_path.as_ref() {
        std::env::set_var("XENOBOT_DB_PATH", path.as_os_str());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Internal(e.to_string()))?;

    let (status, body) = runtime.block_on(async move {
        xenobot_api::database::init_database()
            .await
            .map_err(|e| CliError::Database(e.to_string()))?;

        let config = xenobot_api::config::ApiConfig::default();
        let app = xenobot_api::router::build_router(&config);
        let request: axum::http::Request<axum::body::Body> = axum::http::Request::builder()
            .method("GET")
            .uri("/health")
            .body(axum::body::Body::empty())
            .map_err(|e| CliError::Internal(format!("failed to build request: {}", e)))?;
        let response: axum::response::Response = app
            .oneshot(request)
            .await
            .unwrap_or_else(|err| match err {});
        let status: axum::http::StatusCode = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .map_err(|e| CliError::Internal(format!("failed to read response body: {}", e)))?;
        let body = String::from_utf8_lossy(&body_bytes).to_string();
        Ok::<(axum::http::StatusCode, String), CliError>((status, body))
    })?;

    println!("api smoke check completed");
    println!("route: GET /health");
    println!("status: {}", status.as_u16());
    println!("body: {}", body);
    if status != axum::http::StatusCode::OK {
        return Err(CliError::Internal(format!(
            "smoke check failed: expected 200, got {}",
            status
        )));
    }
    if !body.trim().eq_ignore_ascii_case("ok") {
        return Err(CliError::Internal(format!(
            "smoke check failed: expected body 'OK', got '{}'",
            body.trim()
        )));
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
        assert!(err
            .to_string()
            .contains("only SELECT statements are allowed"));
    }

    #[test]
    fn validate_select_sql_rejects_multiple_statements() {
        let sql = "SELECT 1; SELECT 2;";
        let err = validate_select_sql(sql).expect_err("multiple statements must be rejected");
        assert!(err
            .to_string()
            .contains("multiple SQL statements are not allowed"));
    }

    #[cfg(all(feature = "analysis", feature = "api"))]
    #[test]
    fn webhook_rule_matches_event_filters_by_event_sender_keyword() {
        let item = WebhookItem {
            id: "wh_1".to_string(),
            url: "http://127.0.0.1:65535/hook".to_string(),
            event_type: Some("message.created".to_string()),
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
        let related = embed_text_for_semantic("incremental import checkpoint for database migration");
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
}

/// Parse command line arguments and run the application.
pub fn run() -> Result<()> {
    let app = App::new()?;
    app.run()
}
