//! Media helpers for legal-safe Google Chat export workflows.

use std::fs;
use std::path::{Path, PathBuf};

/// Media kind inferred from a Google Chat export asset path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleChatMediaKind {
    /// Image asset.
    Image,
    /// Video asset.
    Video,
    /// Audio asset.
    Audio,
    /// Document or office file.
    Document,
    /// Archive file.
    Archive,
    /// Sticker or vector asset.
    Sticker,
    /// Unknown or unsupported media type.
    Unknown,
}

/// Classified media asset from an authorized export tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatMediaAsset {
    /// Original asset path.
    pub path: PathBuf,
    /// Inferred media kind.
    pub kind: GoogleChatMediaKind,
    /// Lowercase file extension when available.
    pub extension: Option<String>,
    /// File size in bytes when metadata is available.
    pub size_bytes: u64,
}

/// Classify one media path by extension.
pub fn classify_media_path(path: &Path) -> GoogleChatMediaKind {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match ext.as_deref() {
        Some("jpg" | "jpeg" | "png" | "gif" | "bmp" | "heic" | "webp") => {
            GoogleChatMediaKind::Image
        }
        Some("mp4" | "mov" | "avi" | "mkv" | "webm") => GoogleChatMediaKind::Video,
        Some("ogg" | "opus" | "mp3" | "m4a" | "wav" | "aac") => GoogleChatMediaKind::Audio,
        Some("pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "csv") => {
            GoogleChatMediaKind::Document
        }
        Some("zip" | "7z" | "rar" | "tar" | "gz") => GoogleChatMediaKind::Archive,
        Some("svg") => GoogleChatMediaKind::Sticker,
        _ => GoogleChatMediaKind::Unknown,
    }
}

/// Build a media inventory from explicitly provided asset paths.
pub fn collect_media_assets<I, P>(paths: I) -> Vec<GoogleChatMediaAsset>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    paths
        .into_iter()
        .filter_map(|path| {
            let path = path.as_ref();
            let metadata = fs::metadata(path).ok()?;
            if !metadata.is_file() {
                return None;
            }

            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());

            Some(GoogleChatMediaAsset {
                path: path.to_path_buf(),
                kind: classify_media_path(path),
                extension,
                size_bytes: metadata.len(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn classifies_common_media_extensions() {
        assert_eq!(
            classify_media_path(Path::new("photo.jpg")),
            GoogleChatMediaKind::Image
        );
        assert_eq!(
            classify_media_path(Path::new("video.mp4")),
            GoogleChatMediaKind::Video
        );
        assert_eq!(
            classify_media_path(Path::new("audio.ogg")),
            GoogleChatMediaKind::Audio
        );
        assert_eq!(
            classify_media_path(Path::new("report.pdf")),
            GoogleChatMediaKind::Document
        );
    }

    #[test]
    fn collects_existing_media_assets() {
        let dir = tempdir().expect("tempdir");
        let image = dir.path().join("image.jpg");
        fs::write(&image, [1_u8, 2, 3]).expect("write media asset");

        let assets = collect_media_assets([image.as_path()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, GoogleChatMediaKind::Image);
        assert_eq!(assets[0].size_bytes, 3);
    }

    #[test]
    fn classifies_unknown_extensions_as_unknown() {
        assert_eq!(
            classify_media_path(Path::new("mystery.bin")),
            GoogleChatMediaKind::Unknown
        );
    }

    #[test]
    fn ignores_directories_when_collecting_assets() {
        let dir = tempdir().expect("tempdir");
        let nested_dir = dir.path().join("media-dir");
        fs::create_dir(&nested_dir).expect("create nested dir");

        let assets = collect_media_assets([nested_dir.as_path()]);
        assert!(assets.is_empty());
    }
}
