//! Audio transcoding helpers for WeChat media.
//!
//! The current implementation uses an external `ffmpeg` binary as the runtime
//! backend to convert voice payloads into MP3.

use crate::error::{WeChatError, WeChatResult};
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Convert an input audio file (including SILK) into MP3.
pub fn transcode_audio_to_mp3(
    input_path: &Path,
    output_path: &Path,
    options: &AudioTranscodeOptions,
) -> WeChatResult<()> {
    if !input_path.exists() {
        return Err(WeChatError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "input audio file not found: {}",
                input_path.to_string_lossy()
            ),
        )));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(WeChatError::Io)?;
    }

    let binary = options
        .ffmpeg_binary
        .as_ref()
        .map(|p| p.as_os_str())
        .unwrap_or_else(|| std::ffi::OsStr::new("ffmpeg"));

    let args = build_ffmpeg_args(input_path, output_path, options);
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| WeChatError::Internal(anyhow::anyhow!("failed to start ffmpeg: {}", e)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(WeChatError::Internal(anyhow::anyhow!(
        "ffmpeg transcoding failed (status: {}): {}",
        output
            .status
            .code()
            .map(|c| c.to_string())
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
    let mut args = Vec::new();
    args.push("-hide_banner".to_string());
    args.push("-loglevel".to_string());
    args.push("error".to_string());
    args.push(if options.overwrite {
        "-y".to_string()
    } else {
        "-n".to_string()
    });

    if input_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("silk"))
        .unwrap_or(false)
    {
        args.push("-f".to_string());
        args.push("silk".to_string());
    }

    args.push("-i".to_string());
    args.push(input_path.to_string_lossy().to_string());
    args.push("-ac".to_string());
    args.push(options.channels.to_string());
    args.push("-ar".to_string());
    args.push(options.sample_rate_hz.to_string());
    args.push("-b:a".to_string());
    args.push(format!("{}k", options.bitrate_kbps));
    args.push("-codec:a".to_string());
    args.push("libmp3lame".to_string());
    args.push(output_path.to_string_lossy().to_string());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ffmpeg_args_for_silk_input() {
        let options = AudioTranscodeOptions::default();
        let args = build_ffmpeg_args(Path::new("/tmp/a.silk"), Path::new("/tmp/a.mp3"), &options);

        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"silk".to_string()));
        assert!(args.contains(&"libmp3lame".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_args_for_wav_input() {
        let options = AudioTranscodeOptions {
            overwrite: false,
            ..AudioTranscodeOptions::default()
        };
        let args = build_ffmpeg_args(Path::new("/tmp/a.wav"), Path::new("/tmp/a.mp3"), &options);

        assert!(args.contains(&"-n".to_string()));
        assert!(!args.windows(2).any(|pair| pair == ["-f", "silk"]));
    }
}
