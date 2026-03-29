//! Audio transcoding helpers for legal-safe media workflows.
//!
//! The runtime backend is an external `ffmpeg` binary. This module only
//! converts user-provided files or byte payloads and does not intercept
//! protected application streams.

use crate::{GoogleChatError, GoogleChatResult};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Audio transcoding options for MP3 output.
#[derive(Debug, Clone)]
pub struct AudioTranscodeOptions {
    /// Target bitrate in kbps.
    pub bitrate_kbps: u32,
    /// Target sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Target channel count.
    pub channels: u8,
    /// Whether to overwrite existing output.
    pub overwrite: bool,
    /// Optional explicit ffmpeg binary path.
    pub ffmpeg_binary: Option<PathBuf>,
}

impl Default for AudioTranscodeOptions {
    fn default() -> Self {
        Self {
            bitrate_kbps: 128,
            sample_rate_hz: 24_000,
            channels: 1,
            overwrite: true,
            ffmpeg_binary: None,
        }
    }
}

/// Convert an input audio file into MP3.
pub fn transcode_audio_to_mp3(
    input_path: &Path,
    output_path: &Path,
    options: &AudioTranscodeOptions,
) -> GoogleChatResult<()> {
    if !input_path.exists() {
        return Err(GoogleChatError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "input audio file not found: {}",
                input_path.to_string_lossy()
            ),
        )));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(GoogleChatError::Io)?;
    }

    let binary = options
        .ffmpeg_binary
        .as_ref()
        .map(|path| path.as_os_str())
        .unwrap_or_else(|| std::ffi::OsStr::new("ffmpeg"));

    let output = Command::new(binary)
        .args(build_ffmpeg_args(input_path, output_path, options))
        .output()
        .map_err(|error| {
            GoogleChatError::Internal(anyhow::anyhow!("failed to start ffmpeg: {}", error))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(GoogleChatError::Internal(anyhow::anyhow!(
        "ffmpeg transcoding failed (status: {}): {}",
        output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string()),
        stderr
    )))
}

/// Convert in-memory audio bytes into MP3 bytes.
pub fn transcode_audio_bytes_to_mp3(
    input_bytes: &[u8],
    input_format: &str,
    options: &AudioTranscodeOptions,
) -> GoogleChatResult<Vec<u8>> {
    if input_bytes.is_empty() {
        return Err(GoogleChatError::Internal(anyhow::anyhow!(
            "input audio payload is empty"
        )));
    }

    let binary = options
        .ffmpeg_binary
        .as_ref()
        .map(|path| path.as_os_str())
        .unwrap_or_else(|| std::ffi::OsStr::new("ffmpeg"));

    let mut child = Command::new(binary)
        .args(build_ffmpeg_pipe_args(
            normalize_input_format(input_format),
            options,
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            GoogleChatError::Internal(anyhow::anyhow!("failed to start ffmpeg: {}", error))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_bytes).map_err(|error| {
            GoogleChatError::Internal(anyhow::anyhow!("failed to write ffmpeg stdin: {}", error))
        })?;
    }

    let output = child.wait_with_output().map_err(|error| {
        GoogleChatError::Internal(anyhow::anyhow!("failed to wait ffmpeg: {}", error))
    })?;

    if output.status.success() {
        if output.stdout.is_empty() {
            return Err(GoogleChatError::Internal(anyhow::anyhow!(
                "ffmpeg succeeded but produced empty mp3 output"
            )));
        }
        return Ok(output.stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(GoogleChatError::Internal(anyhow::anyhow!(
        "ffmpeg in-memory transcoding failed (status: {}): {}",
        output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string()),
        stderr
    )))
}

/// Check whether ffmpeg is available in PATH.
pub fn has_ffmpeg(ffmpeg_binary: Option<&Path>) -> bool {
    let binary = ffmpeg_binary.unwrap_or_else(|| Path::new("ffmpeg"));
    Command::new(binary)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn build_ffmpeg_args(
    input_path: &Path,
    output_path: &Path,
    options: &AudioTranscodeOptions,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        if options.overwrite {
            "-y".to_string()
        } else {
            "-n".to_string()
        },
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-ac".to_string(),
        options.channels.to_string(),
        "-ar".to_string(),
        options.sample_rate_hz.to_string(),
        "-b:a".to_string(),
        format!("{}k", options.bitrate_kbps),
        "-codec:a".to_string(),
        "libmp3lame".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

fn build_ffmpeg_pipe_args(input_format: &str, options: &AudioTranscodeOptions) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-f".to_string(),
        input_format.to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
        "-ac".to_string(),
        options.channels.to_string(),
        "-ar".to_string(),
        options.sample_rate_hz.to_string(),
        "-b:a".to_string(),
        format!("{}k", options.bitrate_kbps),
        "-codec:a".to_string(),
        "libmp3lame".to_string(),
        "-f".to_string(),
        "mp3".to_string(),
        "pipe:1".to_string(),
    ]
}

fn normalize_input_format(format: &str) -> &str {
    match format.trim().to_ascii_lowercase().as_str() {
        "wav" => "wav",
        "ogg" => "ogg",
        "opus" => "ogg",
        "mp3" => "mp3",
        "m4a" => "mp4",
        "aac" => "aac",
        _ => "ogg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ffmpeg_args_for_file_output() {
        let options = AudioTranscodeOptions::default();
        let args = build_ffmpeg_args(Path::new("/tmp/a.ogg"), Path::new("/tmp/a.mp3"), &options);

        assert!(args.contains(&"libmp3lame".to_string()));
        assert!(args.contains(&"/tmp/a.ogg".to_string()));
        assert!(args.contains(&"/tmp/a.mp3".to_string()));
    }

    #[test]
    fn builds_ffmpeg_args_for_memory_output() {
        let options = AudioTranscodeOptions::default();
        let args = build_ffmpeg_pipe_args("ogg", &options);

        assert!(args.contains(&"pipe:0".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn normalizes_known_input_formats() {
        assert_eq!(normalize_input_format("opus"), "ogg");
        assert_eq!(normalize_input_format("m4a"), "mp4");
        assert_eq!(normalize_input_format("unknown"), "ogg");
    }

    #[test]
    fn missing_input_file_returns_not_found() {
        let options = AudioTranscodeOptions::default();
        let input_path = Path::new("/tmp/googlechat-missing-input.ogg");
        let output_path = Path::new("/tmp/googlechat-missing-output.mp3");
        let error = transcode_audio_to_mp3(input_path, output_path, &options)
            .expect_err("missing input should fail");

        match error {
            GoogleChatError::Io(inner) => assert_eq!(inner.kind(), std::io::ErrorKind::NotFound),
            other => panic!("expected Io(NotFound), got {other:?}"),
        }
    }

    #[test]
    fn reports_missing_ffmpeg_binary() {
        let missing = Path::new("/definitely/not/a/real/ffmpeg-binary");
        assert!(!has_ffmpeg(Some(missing)));
    }

    #[test]
    fn builds_ffmpeg_args_without_overwrite_when_disabled() {
        let options = AudioTranscodeOptions {
            overwrite: false,
            ..AudioTranscodeOptions::default()
        };
        let args = build_ffmpeg_args(Path::new("/tmp/a.ogg"), Path::new("/tmp/a.mp3"), &options);

        assert!(args.contains(&"-n".to_string()));
        assert!(!args.contains(&"-y".to_string()));
    }

    #[test]
    fn builds_ffmpeg_args_with_custom_audio_settings() {
        let options = AudioTranscodeOptions {
            bitrate_kbps: 192,
            sample_rate_hz: 48_000,
            channels: 2,
            ..AudioTranscodeOptions::default()
        };
        let args = build_ffmpeg_args(Path::new("/tmp/a.ogg"), Path::new("/tmp/a.mp3"), &options);

        assert!(args.contains(&"192k".to_string()));
        assert!(args.contains(&"48000".to_string()));
        assert!(args.contains(&"2".to_string()));
    }

    #[test]
    fn builds_ffmpeg_pipe_args_for_m4a_payload() {
        let options = AudioTranscodeOptions::default();
        let args = build_ffmpeg_pipe_args(normalize_input_format("m4a"), &options);

        assert!(args.contains(&"mp4".to_string()));
        assert!(args.contains(&"pipe:0".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn transcode_audio_bytes_rejects_empty_payload() {
        let error = transcode_audio_bytes_to_mp3(&[], "ogg", &AudioTranscodeOptions::default())
            .expect_err("empty payload should fail");

        match error {
            GoogleChatError::Internal(_) => {}
            other => panic!("expected Internal error, got {other:?}"),
        }
    }
}
