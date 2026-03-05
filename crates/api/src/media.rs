//! Media API module for Xenobot HTTP API.
//!
//! Provides safe media file routing for image/video/audio/document payloads.

use axum::{
    extract::{Path, Query},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
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
        .route("/decrypt/dat", post(decrypt_dat_image))
        .route("/transcode/audio/mp3", post(transcode_audio_mp3))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinaryMediaRequest {
    path: Option<String>,
    payload_base64: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatDecryptRequest {
    #[serde(flatten)]
    source: BinaryMediaRequest,
    xor_key_hex: Option<String>,
    aes_key_hex: Option<String>,
    aes_iv_hex: Option<String>,
    auto_detect_xor: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioTranscodeRequest {
    #[serde(flatten)]
    source: BinaryMediaRequest,
    input_format: Option<String>,
    bitrate_kbps: Option<u32>,
    sample_rate_hz: Option<u32>,
    channels: Option<u8>,
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

async fn decrypt_dat_image(
    Json(req): Json<DatDecryptRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    #[cfg(feature = "wechat")]
    {
        let payload = load_binary_media_payload(&req.source).await?;
        let params = xenobot_wechat::media::DatImageDecryptParams {
            xor_key: decode_hex_optional(req.xor_key_hex.as_deref(), "xorKeyHex")?,
            aes_key: decode_hex_optional(req.aes_key_hex.as_deref(), "aesKeyHex")?,
            aes_iv: decode_hex_optional(req.aes_iv_hex.as_deref(), "aesIvHex")?,
            auto_detect_xor: req.auto_detect_xor.unwrap_or(true),
        };

        let (decrypted, (format, xor_used)) =
            xenobot_wechat::media::decrypt_dat_image_bytes(&payload, &params)?;
        let bytes_base64 = base64::engine::general_purpose::STANDARD.encode(&decrypted);
        let xor_key_used_hex = xor_used.map(hex::encode);

        return Ok(Json(serde_json::json!({
            "ok": true,
            "format": format.extension(),
            "contentType": wechat_image_content_type(format),
            "bytes": bytes_base64,
            "byteLength": decrypted.len(),
            "xorKeyUsedHex": xor_key_used_hex,
        })));
    }

    #[cfg(not(feature = "wechat"))]
    {
        let _ = req;
        Err(ApiError::NotImplemented(
            "media decrypt endpoint requires api feature 'wechat'".to_string(),
        ))
    }
}

async fn transcode_audio_mp3(
    Json(req): Json<AudioTranscodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    #[cfg(feature = "wechat")]
    {
        if !xenobot_wechat::audio::has_ffmpeg(None) {
            return Err(ApiError::NotImplemented(
                "ffmpeg is required for audio transcoding but is not available in PATH".to_string(),
            ));
        }

        let input = load_binary_media_payload(&req.source).await?;
        let input_format = req
            .input_format
            .as_deref()
            .map(normalize_audio_format)
            .or_else(|| {
                req.source
                    .path
                    .as_deref()
                    .and_then(infer_audio_format_from_path)
                    .map(normalize_audio_format)
            })
            .unwrap_or("silk");

        let options = xenobot_wechat::audio::AudioTranscodeOptions {
            bitrate_kbps: req.bitrate_kbps.unwrap_or(128),
            sample_rate_hz: req.sample_rate_hz.unwrap_or(24_000),
            channels: req.channels.unwrap_or(1),
            overwrite: true,
            ffmpeg_binary: None,
        };

        let mp3 =
            xenobot_wechat::audio::transcode_audio_bytes_to_mp3(&input, input_format, &options)?;
        let bytes_base64 = base64::engine::general_purpose::STANDARD.encode(&mp3);

        return Ok(Json(serde_json::json!({
            "ok": true,
            "inputFormat": input_format,
            "contentType": "audio/mpeg",
            "bytes": bytes_base64,
            "byteLength": mp3.len(),
            "options": {
                "bitrateKbps": options.bitrate_kbps,
                "sampleRateHz": options.sample_rate_hz,
                "channels": options.channels
            }
        })));
    }

    #[cfg(not(feature = "wechat"))]
    {
        let _ = req;
        Err(ApiError::NotImplemented(
            "audio transcode endpoint requires api feature 'wechat'".to_string(),
        ))
    }
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

async fn load_binary_media_payload(source: &BinaryMediaRequest) -> Result<Vec<u8>, ApiError> {
    match (source.path.as_deref(), source.payload_base64.as_deref()) {
        (Some(_), Some(_)) => Err(ApiError::InvalidRequest(
            "provide either path or payloadBase64, not both".to_string(),
        )),
        (None, None) => Err(ApiError::InvalidRequest(
            "either path or payloadBase64 must be provided".to_string(),
        )),
        (Some(path), None) => {
            let safe_path = resolve_allowed_path(path)?;
            let bytes = tokio::fs::read(safe_path).await?;
            if bytes.is_empty() {
                return Err(ApiError::InvalidRequest(
                    "media payload is empty".to_string(),
                ));
            }
            Ok(bytes)
        }
        (None, Some(payload_base64)) => decode_base64_payload(payload_base64),
    }
}

fn decode_base64_payload(raw: &str) -> Result<Vec<u8>, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::InvalidRequest(
            "payloadBase64 cannot be empty".to_string(),
        ));
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .map_err(|e| ApiError::InvalidRequest(format!("invalid payloadBase64: {}", e)))?;
    if decoded.is_empty() {
        return Err(ApiError::InvalidRequest(
            "decoded payload is empty".to_string(),
        ));
    }
    Ok(decoded)
}

fn decode_hex_optional(value: Option<&str>, field: &str) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::InvalidRequest(format!("{field} cannot be empty")));
    }

    let decoded = hex::decode(trimmed)
        .map_err(|e| ApiError::InvalidRequest(format!("invalid {field} hex string: {e}")))?;
    if decoded.is_empty() {
        return Err(ApiError::InvalidRequest(format!(
            "{field} cannot decode to empty bytes"
        )));
    }
    Ok(Some(decoded))
}

fn infer_audio_format_from_path(path: &str) -> Option<&str> {
    let ext = FsPath::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    match ext.as_str() {
        "silk" => Some("silk"),
        "wav" => Some("wav"),
        "ogg" => Some("ogg"),
        "mp3" => Some("mp3"),
        "m4a" | "mp4" => Some("m4a"),
        "aac" => Some("aac"),
        _ => None,
    }
}

fn normalize_audio_format(input: &str) -> &str {
    match input.trim().to_ascii_lowercase().as_str() {
        "silk" => "silk",
        "wav" => "wav",
        "ogg" => "ogg",
        "mp3" => "mp3",
        "m4a" | "mp4" => "m4a",
        "aac" => "aac",
        _ => "silk",
    }
}

#[cfg(feature = "wechat")]
fn wechat_image_content_type(format: xenobot_wechat::media::ImageFormat) -> &'static str {
    match format {
        xenobot_wechat::media::ImageFormat::Jpeg => "image/jpeg",
        xenobot_wechat::media::ImageFormat::Png => "image/png",
        xenobot_wechat::media::ImageFormat::Gif => "image/gif",
        xenobot_wechat::media::ImageFormat::Webp => "image/webp",
        xenobot_wechat::media::ImageFormat::Bmp => "image/bmp",
    }
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

    #[test]
    fn test_decode_hex_optional_rejects_empty() {
        let err = decode_hex_optional(Some(""), "xorKeyHex").expect_err("empty should fail");
        assert!(matches!(err, ApiError::InvalidRequest(_)));
    }

    #[test]
    fn test_infer_audio_format_from_path_silk() {
        assert_eq!(infer_audio_format_from_path("/tmp/a.silk"), Some("silk"));
        assert_eq!(infer_audio_format_from_path("/tmp/a.unknown"), None);
    }
}
