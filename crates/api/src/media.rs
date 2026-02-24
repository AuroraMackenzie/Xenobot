//! Media API module for Xenobot HTTP API.
//!
//! Provides safe media file routing for image/video/audio/document payloads.

use axum::{
    extract::{Path, Query},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;
use std::path::{Path as FsPath, PathBuf};

use crate::ApiError;

/// Media API router.
pub fn router() -> Router {
    Router::new()
        .route("/resolve", get(resolve_media_path))
        .route("/file", get(stream_media_file))
        .route("/messages/:message_id", get(stream_message_media))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaPathRequest {
    path: String,
    download: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageMediaRequest {
    download: Option<bool>,
}

async fn resolve_media_path(
    Query(req): Query<MediaPathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let resolved = resolve_allowed_path(&req.path)?;
    let metadata = tokio::fs::metadata(&resolved).await?;
    if !metadata.is_file() {
        return Err(ApiError::InvalidRequest("path is not a file".to_string()));
    }

    let file_name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let content_type = guess_content_type(&resolved);

    Ok(Json(serde_json::json!({
        "ok": true,
        "path": resolved.to_string_lossy().to_string(),
        "fileName": file_name,
        "size": metadata.len(),
        "contentType": content_type,
    })))
}

async fn stream_media_file(Query(req): Query<MediaPathRequest>) -> Result<Response, ApiError> {
    let path = resolve_allowed_path(&req.path)?;
    media_response_from_path(path, req.download.unwrap_or(false)).await
}

async fn stream_message_media(
    Path(message_id): Path<i64>,
    Query(req): Query<MessageMediaRequest>,
) -> Result<Response, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let row = sqlx::query("SELECT content FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let Some(row) = row else {
        return Err(ApiError::NotFound(format!(
            "message {} not found",
            message_id
        )));
    };

    let content: Option<String> = row
        .try_get("content")
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let Some(content) = content else {
        return Err(ApiError::NotFound(format!(
            "message {} has no media content",
            message_id
        )));
    };

    let Some(path) = extract_media_path_from_content(&content) else {
        return Err(ApiError::InvalidRequest(format!(
            "message {} does not contain a resolvable local media path",
            message_id
        )));
    };

    let safe_path = resolve_allowed_path(path.to_string_lossy().as_ref())?;
    media_response_from_path(safe_path, req.download.unwrap_or(false)).await
}

async fn media_response_from_path(path: PathBuf, as_download: bool) -> Result<Response, ApiError> {
    let metadata = tokio::fs::metadata(&path).await?;
    if !metadata.is_file() {
        return Err(ApiError::InvalidRequest("path is not a file".to_string()));
    }
    let bytes = tokio::fs::read(&path).await?;
    let content_type = guess_content_type(&path);
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("media.bin");

    let mut response = Response::new(bytes.into_response().into_body());
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    if as_download {
        let disposition = format!("attachment; filename=\"{}\"", sanitize_filename(file_name));
        if let Ok(value) = HeaderValue::from_str(&disposition) {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok(response)
}

fn resolve_allowed_path(raw: &str) -> Result<PathBuf, ApiError> {
    let candidate = normalize_input_path(raw)
        .ok_or_else(|| ApiError::InvalidRequest("invalid media path".to_string()))?;
    if !candidate.is_absolute() {
        return Err(ApiError::InvalidRequest(
            "media path must be an absolute path".to_string(),
        ));
    }

    let canonical = std::fs::canonicalize(&candidate).map_err(|_| {
        ApiError::NotFound(format!(
            "media file not found: {}",
            candidate.to_string_lossy()
        ))
    })?;
    ensure_path_allowed(&canonical)?;
    Ok(canonical)
}

fn normalize_input_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("file://") {
        return Some(PathBuf::from(rest));
    }

    Some(PathBuf::from(trimmed))
}

fn ensure_path_allowed(path: &PathBuf) -> Result<(), ApiError> {
    let mut allowed_roots: Vec<PathBuf> = Vec::new();

    if let Some(download_dir) = dirs::download_dir() {
        allowed_roots.push(download_dir);
    }
    if let Some(data_dir) = dirs::data_dir() {
        allowed_roots.push(data_dir.join("xenobot"));
    }
    if let Some(home) = dirs::home_dir() {
        allowed_roots.push(
            home.join("Library")
                .join("Containers")
                .join("com.tencent.xinWeChat")
                .join("Data")
                .join("Library")
                .join("Application Support")
                .join("com.tencent.xinWeChat"),
        );
    }

    if let Ok(extra) = std::env::var("XENOBOT_MEDIA_ROOTS") {
        for item in extra.split(';') {
            let item = item.trim();
            if !item.is_empty() {
                allowed_roots.push(PathBuf::from(item));
            }
        }
    }

    for root in allowed_roots {
        if let Ok(canonical_root) = std::fs::canonicalize(&root) {
            if path.starts_with(&canonical_root) {
                return Ok(());
            }
        }
    }

    Err(ApiError::Auth(format!(
        "media path is outside allowed roots: {}",
        path.to_string_lossy()
    )))
}

fn extract_media_path_from_content(content: &str) -> Option<PathBuf> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(path) = normalize_input_path(trimmed) {
        if path.is_absolute() {
            return Some(path);
        }
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        for key in [
            "file_path",
            "path",
            "local_path",
            "media_path",
            "image_path",
            "video_path",
            "audio_path",
            "voice_path",
            "thumbnail_path",
        ] {
            if let Some(path) = value
                .get(key)
                .and_then(|v| v.as_str())
                .and_then(normalize_input_path)
            {
                if path.is_absolute() {
                    return Some(path);
                }
            }
        }
    }

    for token in trimmed.split_whitespace() {
        if token.starts_with('/')
            && (token.contains(".jpg")
                || token.contains(".jpeg")
                || token.contains(".png")
                || token.contains(".gif")
                || token.contains(".webp")
                || token.contains(".mp4")
                || token.contains(".mov")
                || token.contains(".m4a")
                || token.contains(".mp3")
                || token.contains(".wav")
                || token.contains(".silk")
                || token.contains(".pdf")
                || token.contains(".zip"))
        {
            return Some(PathBuf::from(token.trim_matches('"')));
        }
    }

    None
}

fn guess_content_type(path: &FsPath) -> &'static str {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "m4v" => "video/x-m4v",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "silk" => "audio/silk",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || ['.', '-', '_'].contains(c))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_media_path_from_json_content() {
        let content = r#"{"file_path":"/tmp/demo.jpg"}"#;
        let parsed = extract_media_path_from_content(content);
        assert_eq!(parsed, Some(PathBuf::from("/tmp/demo.jpg")));
    }

    #[test]
    fn test_guess_content_type_image() {
        assert_eq!(guess_content_type(FsPath::new("/tmp/x.png")), "image/png");
    }
}
