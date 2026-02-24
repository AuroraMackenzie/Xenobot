use once_cell::sync::Lazy;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex;
use tracing::info;
use xenobot_core::config::DatabaseConfig;

static DB_POOL: Lazy<Mutex<Option<SqlitePool>>> = Lazy::new(|| Mutex::new(None));
static DB_PATH: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
// Migrator is created dynamically in run_migrations()

pub fn get_db_path() -> PathBuf {
    if let Some(path) = DB_PATH
        .read()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
    {
        return path;
    }

    if let Ok(path) = std::env::var("XENOBOT_DB_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    default_db_path()
}

fn default_db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("xenobot.db")
}

fn resolve_db_path(config: &DatabaseConfig) -> PathBuf {
    if let Ok(path) = std::env::var("XENOBOT_DB_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if config.sqlite_path == PathBuf::from("xenobot.db") {
        return default_db_path();
    }

    config.sqlite_path.clone()
}

pub async fn init_database() -> Result<(), Box<dyn std::error::Error>> {
    let config = DatabaseConfig::default();
    init_database_with_config(&config).await
}

pub async fn init_database_with_config(
    config: &DatabaseConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = resolve_db_path(config);
    info!(
        "Initializing database at: {:?} with config: {:?}",
        db_path, config
    );

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let connect_options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections as u32)
        .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout))
        .connect_with(connect_options)
        .await?;

    run_migrations(&pool).await?;

    let mut pool_guard = DB_POOL.lock().await;
    *pool_guard = Some(pool);

    if let Ok(mut path_guard) = DB_PATH.write() {
        *path_guard = Some(db_path);
    }

    info!("Database initialized successfully with SQLx connection pooling");
    Ok(())
}

pub async fn get_pool() -> Result<Arc<SqlitePool>, Box<dyn std::error::Error>> {
    let pool_guard = DB_POOL.lock().await;
    if let Some(pool) = pool_guard.as_ref() {
        Ok(Arc::new(pool.clone()))
    } else {
        Err("Database not initialized".into())
    }
}

pub async fn with_pool<F, T, E>(f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&SqlitePool) -> Result<T, E>,
    E: Into<Box<dyn std::error::Error>>,
{
    let pool = get_pool().await?;
    f(&pool).map_err(|e| e.into())
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running SQLx database migrations...");
    let migrator = Migrator::new(Path::new("crates/api/migrations")).await?;
    migrator.run(pool).await?;
    info!("Database migrations completed");
    Ok(())
}

pub fn ensure_migrations_dir() -> Result<(), std::io::Error> {
    let migrations_dir = Path::new("crates/api/migrations");
    if !migrations_dir.exists() {
        info!(
            "Migrations directory does not exist, creating: {:?}",
            migrations_dir
        );
        fs::create_dir_all(migrations_dir)?;
    }
    Ok(())
}

pub mod repository;
pub use repository::*;
