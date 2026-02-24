//! Cache API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `cacheApi` IPC methods.

use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    fs, io,
    path::{Path as FsPath, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::instrument;

use crate::ApiError;

/// Cache API router.
pub fn router() -> Router {
    Router::new()
        .route("/info", get(get_info))
        .route("/clear/:cache_id", post(clear_cache))
        .route("/open-dir/:cache_id", post(open_dir))
        .route("/save-to-downloads", post(save_to_downloads))
        .route("/latest-import-log", get(get_latest_import_log))
        .route("/data-dir", get(get_data_dir))
        .route("/select-data-dir", post(select_data_dir))
        .route("/set-data-dir", post(set_data_dir))
        .route("/show-in-folder", post(show_in_folder))
}

// Request/Response types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub base_dir: String,
    pub directories: Vec<CacheDirInfo>,
    pub total_size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CacheDirInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub icon: String,
    pub can_clear: bool,
    pub size: u64,
    pub file_count: u64,
    pub exists: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveToDownloadsRequest {
    pub filename: String,
    pub data_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirInfo {
    pub path: String,
    pub size_bytes: u64,
    pub session_count: u64,
    pub is_custom: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectDataDirResponse {
    pub success: bool,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataDirRequest {
    pub path: Option<String>,
    pub migrate: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowInFolderRequest {
    pub file_path: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct CacheSettings {
    data_dir: Option<String>,
    is_custom: bool,
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn config_root() -> Result<PathBuf, ApiError> {
    let root = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn settings_path() -> Result<PathBuf, ApiError> {
    Ok(config_root()?.join("cache_settings.json"))
}

fn load_settings() -> Result<CacheSettings, ApiError> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(CacheSettings::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(CacheSettings::default());
    }
    serde_json::from_str(&raw).map_err(ApiError::Json)
}

fn save_settings(settings: &CacheSettings) -> Result<(), ApiError> {
    let path = settings_path()?;
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(path, raw)?;
    Ok(())
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot")
}

fn effective_data_dir() -> Result<(PathBuf, bool), ApiError> {
    let settings = load_settings()?;
    let dir = settings
        .data_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    fs::create_dir_all(&dir)?;
    Ok((dir, settings.is_custom && settings.data_dir.is_some()))
}

fn downloads_dir(base_dir: &FsPath) -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| base_dir.join("downloads"))
}

fn ai_dir(base_dir: &FsPath) -> PathBuf {
    base_dir.join("ai")
}

fn logs_dir(base_dir: &FsPath) -> PathBuf {
    base_dir.join("logs")
}

fn databases_dir(base_dir: &FsPath) -> PathBuf {
    base_dir.join("databases")
}

fn ensure_known_dirs(base_dir: &FsPath) -> Result<(), ApiError> {
    fs::create_dir_all(databases_dir(base_dir))?;
    fs::create_dir_all(ai_dir(base_dir))?;
    fs::create_dir_all(logs_dir(base_dir))?;
    fs::create_dir_all(downloads_dir(base_dir))?;
    Ok(())
}

fn dir_stats(path: &FsPath) -> (u64, u64) {
    fn walk(path: &FsPath, size: &mut u64, files: &mut u64) {
        let entries = match fs::read_dir(path) {
            Ok(v) => v,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let meta = match entry.metadata() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if meta.is_file() {
                *files += 1;
                *size += meta.len();
            } else if meta.is_dir() {
                walk(&entry_path, size, files);
            }
        }
    }

    let mut total_size = 0_u64;
    let mut file_count = 0_u64;
    if path.exists() {
        walk(path, &mut total_size, &mut file_count);
    }
    (total_size, file_count)
}

fn clear_dir_contents(path: &FsPath) -> Result<(), ApiError> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn cache_dir_map(base_dir: &FsPath) -> Vec<CacheDirInfo> {
    let databases = databases_dir(base_dir);
    let ai = ai_dir(base_dir);
    let downloads = downloads_dir(base_dir);
    let logs = logs_dir(base_dir);

    let mut dirs = vec![
        (
            "databases",
            "settings.storage.cache.databases.name",
            "settings.storage.cache.databases.description",
            "i-heroicons-circle-stack",
            true,
            databases,
        ),
        (
            "ai",
            "settings.storage.cache.ai.name",
            "settings.storage.cache.ai.description",
            "i-heroicons-cpu-chip",
            true,
            ai,
        ),
        (
            "downloads",
            "settings.storage.cache.downloads.name",
            "settings.storage.cache.downloads.description",
            "i-heroicons-arrow-down-tray",
            true,
            downloads,
        ),
        (
            "logs",
            "settings.storage.cache.logs.name",
            "settings.storage.cache.logs.description",
            "i-heroicons-document-text",
            true,
            logs,
        ),
        (
            "base",
            "settings.storage.title",
            "settings.storage.description",
            "i-heroicons-folder-open",
            false,
            base_dir.to_path_buf(),
        ),
    ];

    dirs.sort_by(|a, b| a.0.cmp(b.0));

    dirs.into_iter()
        .map(|(id, name, desc, icon, can_clear, path)| {
            let (size, file_count) = dir_stats(&path);
            CacheDirInfo {
                id: id.to_string(),
                name: name.to_string(),
                description: desc.to_string(),
                path: path.to_string_lossy().to_string(),
                icon: icon.to_string(),
                can_clear,
                size,
                file_count,
                exists: path.exists(),
            }
        })
        .collect()
}

fn resolve_cache_path(base_dir: &FsPath, cache_id: &str) -> Option<(PathBuf, bool)> {
    match cache_id {
        "databases" => Some((databases_dir(base_dir), true)),
        "ai" => Some((ai_dir(base_dir), true)),
        "downloads" => Some((downloads_dir(base_dir), true)),
        "logs" => Some((logs_dir(base_dir), true)),
        "base" => Some((base_dir.to_path_buf(), false)),
        _ => None,
    }
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if matches!(
            ch,
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>'
        ) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out.trim().trim_matches('.').to_string()
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ApiError> {
    fn val(ch: u8) -> Option<u8> {
        match ch {
            b'A'..=b'Z' => Some(ch - b'A'),
            b'a'..=b'z' => Some(ch - b'a' + 26),
            b'0'..=b'9' => Some(ch - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0_u8;

    for ch in input.bytes() {
        if matches!(ch, b' ' | b'\n' | b'\r' | b'\t') {
            continue;
        }
        if ch == b'=' {
            break;
        }
        let Some(v) = val(ch) else {
            return Err(ApiError::InvalidRequest(
                "invalid base64 payload".to_string(),
            ));
        };
        buffer = (buffer << 6) | v as u32;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(out)
}

fn hex_value(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(input: &str) -> Result<String, ApiError> {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_value(bytes[i + 1]).ok_or_else(|| {
                    ApiError::InvalidRequest("invalid percent encoding".to_string())
                })?;
                let lo = hex_value(bytes[i + 2]).ok_or_else(|| {
                    ApiError::InvalidRequest("invalid percent encoding".to_string())
                })?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    String::from_utf8(out)
        .map_err(|e| ApiError::InvalidRequest(format!("invalid utf-8 payload: {e}")))
}

fn parse_data_url(data_url: &str) -> Result<Vec<u8>, ApiError> {
    if !data_url.starts_with("data:") {
        return Ok(data_url.as_bytes().to_vec());
    }
    let Some(comma_index) = data_url.find(',') else {
        return Err(ApiError::InvalidRequest("invalid data URL".to_string()));
    };
    let meta = &data_url[..comma_index];
    let payload = &data_url[comma_index + 1..];
    if meta.contains(";base64") {
        return decode_base64(payload);
    }
    Ok(percent_decode(payload)?.into_bytes())
}

fn copy_dir_recursive(src: &FsPath, dst: &FsPath) -> io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// Handler functions

#[axum::debug_handler]
#[instrument]
pub async fn get_info() -> Result<Json<CacheInfo>, ApiError> {
    let (base_dir, _) = effective_data_dir()?;
    ensure_known_dirs(&base_dir)?;
    let directories = cache_dir_map(&base_dir);
    let total_size = directories.iter().map(|d| d.size).sum();
    Ok(Json(CacheInfo {
        base_dir: base_dir.to_string_lossy().to_string(),
        directories,
        total_size,
    }))
}

#[axum::debug_handler]
#[instrument]
pub async fn clear_cache(
    Path(cache_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (base_dir, _) = effective_data_dir()?;
    let Some((path, can_clear)) = resolve_cache_path(&base_dir, cache_id.as_str()) else {
        return Err(ApiError::InvalidRequest("unknown cache_id".to_string()));
    };
    if !can_clear {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "cache area cannot be cleared",
        })));
    }
    clear_dir_contents(&path)?;
    Ok(Json(serde_json::json!({
        "success": true,
        "cacheId": cache_id,
        "path": path.to_string_lossy(),
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn open_dir(Path(cache_id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let (base_dir, _) = effective_data_dir()?;
    let Some((path, _)) = resolve_cache_path(&base_dir, cache_id.as_str()) else {
        return Err(ApiError::InvalidRequest("unknown cache_id".to_string()));
    };
    Ok(Json(serde_json::json!({
        "success": true,
        "opened": false,
        "path": path.to_string_lossy(),
        "message": "open_dir_not_supported_in_http_mode",
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn save_to_downloads(
    Json(req): Json<SaveToDownloadsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (base_dir, _) = effective_data_dir()?;
    ensure_known_dirs(&base_dir)?;
    let filename = sanitize_filename(req.filename.trim());
    if filename.is_empty() {
        return Err(ApiError::InvalidRequest("invalid filename".to_string()));
    }
    let data = parse_data_url(&req.data_url)?;
    let download_dir = downloads_dir(&base_dir);
    fs::create_dir_all(&download_dir)?;
    let file_path = download_dir.join(filename);
    fs::write(&file_path, data)?;
    Ok(Json(serde_json::json!({
        "success": true,
        "filePath": file_path.to_string_lossy().to_string(),
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_latest_import_log() -> Result<Json<serde_json::Value>, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
        .ok();
    if let Some(pool) = pool {
        let row = sqlx::query(
            r#"
            SELECT file_path
            FROM import_progress
            WHERE status IN ('done', 'completed', 'success')
              AND file_path IS NOT NULL
              AND TRIM(file_path) != ''
            ORDER BY COALESCE(completed_at, started_at, id) DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

        if let Some(row) = row {
            let path: String = row.try_get("file_path").unwrap_or_default();
            if !path.trim().is_empty() {
                return Ok(Json(serde_json::json!({
                    "success": true,
                    "path": path,
                })));
            }
        }
    }

    let (base_dir, _) = effective_data_dir()?;
    let log_dir = logs_dir(&base_dir);
    let latest_log = fs::read_dir(&log_dir)
        .ok()
        .into_iter()
        .flat_map(|it| it.filter_map(Result::ok))
        .filter_map(|e| {
            let path = e.path();
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified = meta.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, p)| p);

    if let Some(path) = latest_log {
        return Ok(Json(serde_json::json!({
            "success": true,
            "path": path.to_string_lossy().to_string(),
        })));
    }

    Ok(Json(serde_json::json!({
        "success": false,
        "error": "no_import_log_found",
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_data_dir() -> Result<Json<DataDirInfo>, ApiError> {
    let (path, is_custom) = effective_data_dir()?;
    let (size_bytes, _) = dir_stats(&path);

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
        .ok();
    let session_count = match pool {
        Some(pool) => sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meta")
            .fetch_one(pool.as_ref())
            .await
            .unwrap_or(0)
            .max(0) as u64,
        None => 0,
    };

    Ok(Json(DataDirInfo {
        path: path.to_string_lossy().to_string(),
        size_bytes,
        session_count,
        is_custom,
    }))
}

#[axum::debug_handler]
#[instrument]
pub async fn select_data_dir() -> Result<Json<SelectDataDirResponse>, ApiError> {
    // HTTP backend does not have native dialog access. Return current path so UI can continue.
    let (path, _) = effective_data_dir()?;
    Ok(Json(SelectDataDirResponse {
        success: true,
        path: Some(path.to_string_lossy().to_string()),
        error: None,
    }))
}

#[axum::debug_handler]
#[instrument]
pub async fn set_data_dir(
    Json(req): Json<SetDataDirRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (current_dir, _) = effective_data_dir()?;
    let target_dir = req
        .path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);

    fs::create_dir_all(&target_dir)?;

    if req.migrate && current_dir != target_dir {
        let copy_targets = vec![
            (databases_dir(&current_dir), databases_dir(&target_dir)),
            (ai_dir(&current_dir), ai_dir(&target_dir)),
            (logs_dir(&current_dir), logs_dir(&target_dir)),
            (downloads_dir(&current_dir), downloads_dir(&target_dir)),
        ];
        for (src, dst) in copy_targets {
            copy_dir_recursive(&src, &dst).map_err(|e| {
                ApiError::Io(io::Error::new(e.kind(), format!("migrate failed: {e}")))
            })?;
        }

        let old_db = current_dir.join("xenobot.db");
        let new_db = target_dir.join("xenobot.db");
        if old_db.exists() && !new_db.exists() {
            let _ = fs::copy(old_db, new_db);
        }
    }

    let settings = CacheSettings {
        data_dir: req.path.clone(),
        is_custom: req.path.is_some(),
    };
    save_settings(&settings)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "path": target_dir.to_string_lossy().to_string(),
        "migrated": req.migrate,
        "updatedAt": now_ts(),
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn show_in_folder(
    Json(req): Json<ShowInFolderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let path = PathBuf::from(req.file_path);
    Ok(Json(serde_json::json!({
        "success": path.exists(),
        "opened": false,
        "path": path.to_string_lossy().to_string(),
        "message": "show_in_folder_not_supported_in_http_mode",
    })))
}
