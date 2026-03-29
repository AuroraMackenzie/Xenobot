//! Service layer for legal-safe Viber ingestion workflows.

use std::path::{Path, PathBuf};

use xenobot_analysis::parsers::ParsedChat;
use xenobot_core::platform_sources::SourceCandidate;

use crate::account::{primary_account, Account};
use crate::audio::{
    has_ffmpeg, transcode_audio_bytes_to_mp3, transcode_audio_to_mp3, AudioTranscodeOptions,
};
use crate::config::ViberConfig;
use crate::media::{collect_media_assets, ViberMediaAsset};
use crate::monitor::{FileMonitor, FileMonitorConfig};
use crate::{ViberAdapter, ViberError};

/// Parsed export staged by the Viber service.
#[derive(Debug, Clone)]
pub struct StagedViberExport {
    /// Original source file path.
    pub source_path: PathBuf,
    /// Stable platform identifier.
    pub platform_id: &'static str,
    /// Parsed normalized chat content.
    pub parsed: ParsedChat,
}

/// Aggregated legal-safe workspace assembled from explicit Viber inputs.
#[derive(Debug, Clone)]
pub struct AuthorizedViberWorkspace {
    /// Stable platform identifier.
    pub platform_id: &'static str,
    /// Account views discovered from local sources.
    pub accounts: Vec<Account>,
    /// Preferred primary account view.
    pub primary_account: Option<Account>,
    /// Parsed exports explicitly staged by the user.
    pub staged_exports: Vec<StagedViberExport>,
    /// Classified media assets explicitly provided by the user.
    pub media_inventory: Vec<ViberMediaAsset>,
    /// Optional authorized watch root prepared for incremental monitoring.
    pub watch_dir: Option<PathBuf>,
}

impl AuthorizedViberWorkspace {
    /// Return the number of staged exports.
    pub fn export_count(&self) -> usize {
        self.staged_exports.len()
    }

    /// Return the number of indexed media assets.
    pub fn media_count(&self) -> usize {
        self.media_inventory.len()
    }

    /// Return whether the authorized workspace currently holds no staged content.
    pub fn is_empty(&self) -> bool {
        self.staged_exports.is_empty() && self.media_inventory.is_empty()
    }
}

/// Legal-safe Viber orchestration service.
#[derive(Debug, Clone, Default)]
pub struct ViberService {
    adapter: ViberAdapter,
    config: ViberConfig,
}

impl ViberService {
    /// Create a service from a runtime configuration.
    pub fn new(config: ViberConfig) -> Self {
        Self {
            adapter: ViberAdapter::new(),
            config,
        }
    }

    /// Return the stable platform identifier.
    pub fn platform_id(&self) -> &'static str {
        self.adapter.platform_id()
    }

    /// Return source candidates discovered from the local machine.
    pub fn discover_sources(&self) -> Vec<SourceCandidate> {
        self.adapter.discover_sources()
    }

    /// Discover normalized account views derived from available local sources.
    pub fn discover_accounts(&self) -> Vec<Account> {
        let sources = self.discover_sources();
        crate::account::collect_accounts_from_sources(&sources)
    }

    /// Return the normalized account views exposed by this service.
    pub fn get_accounts(&self) -> Vec<Account> {
        self.discover_accounts()
    }

    /// Return the primary current account context when one is available.
    pub fn primary_account(&self) -> Option<Account> {
        let sources = self.discover_sources();
        primary_account(&sources)
    }

    /// Return the currently configured authorized roots.
    pub fn authorized_roots(&self) -> &[PathBuf] {
        self.config.authorized_roots()
    }

    /// Add an authorized root directory at runtime.
    pub fn add_authorized_root<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        self.config.add_authorized_root(path);
    }

    /// Parse one explicitly authorized export file.
    pub fn parse_authorized_export(&self, path: &Path) -> Result<ParsedChat, ViberError> {
        self.ensure_authorized(path)?;
        self.adapter.parse_authorized_export(path)
    }

    /// Parse and stage multiple explicitly authorized export files.
    pub fn stage_authorized_exports<I, P>(
        &self,
        paths: I,
    ) -> Result<Vec<StagedViberExport>, ViberError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        paths
            .into_iter()
            .map(|path| {
                let source_path = path.as_ref().to_path_buf();
                let parsed = self.parse_authorized_export(&source_path)?;
                Ok(StagedViberExport {
                    source_path,
                    platform_id: self.platform_id(),
                    parsed,
                })
            })
            .collect()
    }

    /// Build a legal-safe media inventory from explicitly authorized asset paths.
    pub fn collect_media_inventory<I, P>(
        &self,
        paths: I,
    ) -> Result<Vec<ViberMediaAsset>, ViberError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let authorized_paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .map(|path| {
                self.ensure_authorized(&path)?;
                Ok(path)
            })
            .collect::<Result<_, ViberError>>()?;

        Ok(collect_media_assets(
            authorized_paths.iter().map(PathBuf::as_path),
        ))
    }

    /// Build an aggregated legal-safe workspace from explicit exports and assets.
    pub fn build_authorized_workspace<I, P, J, Q>(
        &self,
        export_paths: I,
        media_paths: J,
    ) -> Result<AuthorizedViberWorkspace, ViberError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
        J: IntoIterator<Item = Q>,
        Q: AsRef<Path>,
    {
        let accounts = self.discover_accounts();
        let primary_account = self.primary_account();
        let staged_exports = self.stage_authorized_exports(export_paths)?;
        let media_inventory = self.collect_media_inventory(media_paths)?;

        Ok(AuthorizedViberWorkspace {
            platform_id: self.platform_id(),
            accounts,
            primary_account,
            staged_exports,
            media_inventory,
            watch_dir: None,
        })
    }

    /// Prepare a legal-safe workspace and optional export monitor in one step.
    pub fn prepare_authorized_workspace<I, P, J, Q>(
        &self,
        export_paths: I,
        media_paths: J,
        watch_dir: Option<&Path>,
    ) -> Result<(AuthorizedViberWorkspace, Option<FileMonitor>), ViberError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
        J: IntoIterator<Item = Q>,
        Q: AsRef<Path>,
    {
        let mut workspace = self.build_authorized_workspace(export_paths, media_paths)?;
        let monitor = watch_dir
            .map(|path| {
                workspace.watch_dir = Some(path.to_path_buf());
                self.create_export_monitor(path)
            })
            .transpose()?;

        Ok((workspace, monitor))
    }

    /// Return whether `ffmpeg` is available for authorized media transcoding.
    pub fn ffmpeg_available(&self) -> bool {
        has_ffmpeg(None)
    }

    /// Convert one explicitly authorized audio asset into MP3.
    pub fn transcode_audio_asset_to_mp3(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &AudioTranscodeOptions,
    ) -> Result<(), ViberError> {
        self.ensure_authorized(input_path)?;
        if let Some(parent) = output_path.parent() {
            self.ensure_authorized(parent)?;
        }
        transcode_audio_to_mp3(input_path, output_path, options)
    }

    /// Convert an in-memory audio payload into MP3 bytes.
    pub fn transcode_audio_payload_to_mp3(
        &self,
        input_bytes: &[u8],
        input_format: &str,
        options: &AudioTranscodeOptions,
    ) -> Result<Vec<u8>, ViberError> {
        transcode_audio_bytes_to_mp3(input_bytes, input_format, options)
    }

    /// Create a file monitor rooted in an explicitly authorized export directory.
    pub fn create_export_monitor(
        &self,
        watch_dir: impl AsRef<Path>,
    ) -> Result<FileMonitor, ViberError> {
        let watch_dir = watch_dir.as_ref();
        self.ensure_authorized(watch_dir)?;

        FileMonitor::new(FileMonitorConfig {
            watch_dir: watch_dir.to_path_buf(),
            file_patterns: FileMonitor::viber_export_patterns(),
            recursive: true,
        })
    }

    fn ensure_authorized(&self, path: &Path) -> Result<(), ViberError> {
        if self.config.is_authorized_path(path) {
            Ok(())
        } else {
            Err(ViberError::UnauthorizedPath {
                path: path.to_path_buf(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_fixture(path: &Path, content: &str) {
        fs::write(path, content).expect("write fixture");
    }

    #[test]
    fn returns_platform_id_from_service() {
        let service = ViberService::new(ViberConfig::default());
        assert_eq!(service.platform_id(), "viber");
    }

    #[test]
    fn rejects_paths_outside_authorized_roots() {
        let service = ViberService::new(ViberConfig::with_authorized_roots([PathBuf::from(
            "/tmp/allowed",
        )]));

        let err = service
            .parse_authorized_export(Path::new("/tmp/other/export.zip"))
            .expect_err("path outside authorized roots should fail before parsing");

        match err {
            ViberError::UnauthorizedPath { path } => {
                assert_eq!(path, PathBuf::from("/tmp/other/export.zip"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn builds_media_inventory_for_authorized_assets() {
        let dir = tempdir().expect("tempdir");
        let asset = dir.path().join("photo.jpg");
        fs::write(&asset, [1_u8, 2, 3]).expect("write media asset");

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir.path()]));
        let inventory = service
            .collect_media_inventory([asset.as_path()])
            .expect("authorized media inventory");

        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].kind, crate::media::ViberMediaKind::Image);
    }

    #[test]
    fn creates_monitor_for_authorized_directory() {
        let dir = tempdir().expect("tempdir");
        let service = ViberService::new(ViberConfig::with_authorized_roots([dir.path()]));

        let monitor = service.create_export_monitor(dir.path());
        assert!(monitor.is_ok());
    }

    #[test]
    fn discovers_account_views_from_sources() {
        let service = ViberService::new(ViberConfig::default());
        let accounts = service.discover_accounts();

        assert!(!accounts.is_empty());
        assert!(accounts
            .iter()
            .all(|account| !account.name.trim().is_empty()));
    }

    #[test]
    fn get_accounts_matches_discovered_accounts() {
        let service = ViberService::new(ViberConfig::default());
        let discovered = service.discover_accounts();
        let exposed = service.get_accounts();

        assert_eq!(exposed, discovered);
    }

    #[test]
    fn build_authorized_workspace_rejects_unauthorized_media_paths() {
        let export_dir = tempdir().expect("tempdir");
        let media_dir = tempdir().expect("tempdir");
        let export = export_dir.path().join("viber_fixture.json");
        let media = media_dir.path().join("preview.jpg");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );
        std::fs::write(&media, [1_u8, 2, 3]).expect("media fixture");

        let service = ViberService::new(ViberConfig::with_authorized_roots([export_dir
            .path()
            .to_path_buf()]));

        match service.build_authorized_workspace([export.as_path()], [media.as_path()]) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, media),
            _ => panic!("expected unauthorized media path"),
        }
    }

    #[test]
    fn build_authorized_workspace_rejects_unauthorized_export_paths() {
        let export_dir = tempdir().expect("tempdir");
        let unauthorized_dir = tempdir().expect("tempdir");
        let export = unauthorized_dir
            .path()
            .join("viber_unauthorized_fixture.dat");
        std::fs::write(&export, [1_u8, 2, 3]).expect("export fixture");

        let service = ViberService::new(ViberConfig::with_authorized_roots([export_dir
            .path()
            .to_path_buf()]));

        match service.build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>()) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, export),
            _ => panic!("expected unauthorized export path"),
        }
    }

    #[test]
    fn builds_authorized_workspace_from_exports_and_media() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        let asset = dir.path().join("voice.ogg");

        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );
        fs::write(&asset, [1_u8, 2, 3]).expect("write media");

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let workspace = service
            .build_authorized_workspace([export.as_path()], [asset.as_path()])
            .expect("workspace should build");

        assert_eq!(workspace.platform_id, "viber");
        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 1);
        assert!(!workspace.accounts.is_empty());
        assert!(workspace.primary_account.is_some());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn prepares_authorized_workspace_with_monitor() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let (workspace, monitor) = service
            .prepare_authorized_workspace(
                [export.as_path()],
                std::iter::empty::<&Path>(),
                Some(dir.path()),
            )
            .expect("workspace and monitor should build");

        assert_eq!(workspace.watch_dir.as_deref(), Some(dir.path()));
        assert!(monitor.is_some());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn prepare_authorized_workspace_rejects_unauthorized_watch_directory() {
        let input_dir = tempdir().expect("tempdir");
        let watch_dir = tempdir().expect("tempdir");
        let export = input_dir.path().join("viber_fixture.json");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let service = ViberService::new(ViberConfig::with_authorized_roots([input_dir
            .path()
            .to_path_buf()]));
        match service.prepare_authorized_workspace(
            [export.as_path()],
            std::iter::empty::<&Path>(),
            Some(watch_dir.path()),
        ) {
            Err(ViberError::UnauthorizedPath { path }) => {
                assert_eq!(path, watch_dir.path().to_path_buf());
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("unauthorized watch directory should fail"),
        }
    }

    #[test]
    fn authorized_workspace_reports_empty_when_no_exports_or_media_are_staged() {
        let service = ViberService::new(ViberConfig::with_authorized_roots([std::env::temp_dir()]));
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("empty workspace should build");

        assert!(workspace.is_empty());
        assert_eq!(workspace.export_count(), 0);
        assert_eq!(workspace.media_count(), 0);
    }

    #[test]
    fn rejects_audio_asset_transcoding_when_output_directory_is_not_authorized() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.opus");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, [1_u8, 2, 3]).expect("write input");

        let service = ViberService::new(ViberConfig::with_authorized_roots([input_dir
            .path()
            .to_path_buf()]));
        let error = service
            .transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default())
            .expect_err("unauthorized output directory should fail");

        match error {
            ViberError::UnauthorizedPath { path } => {
                assert_eq!(path, output_dir.path().to_path_buf());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_audio_asset_transcoding_when_input_path_is_not_authorized() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.ogg");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, [1_u8, 2, 3]).expect("write input");

        let service = ViberService::new(ViberConfig::with_authorized_roots([output_dir
            .path()
            .to_path_buf()]));
        let error = service
            .transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default())
            .expect_err("unauthorized input path should fail");

        match error {
            ViberError::UnauthorizedPath { path } => {
                assert_eq!(path, input);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn add_authorized_root_allows_runtime_monitor_creation() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));
        assert!(service.create_export_monitor(dir.path()).is_err());

        service.add_authorized_root(dir.path().to_path_buf());
        assert!(service.create_export_monitor(dir.path()).is_ok());
    }

    #[test]
    fn collect_media_inventory_rejects_unauthorized_assets() {
        let media_dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let asset = media_dir.path().join("preview.jpg");
        std::fs::write(&asset, [1_u8, 2, 3]).expect("media fixture");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));
        match service.collect_media_inventory([asset.as_path()]) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, asset),
            other => panic!(
                "expected unauthorized media path before runtime authorization, got {other:?}"
            ),
        }

        service.add_authorized_root(media_dir.path().to_path_buf());
        let inventory = service
            .collect_media_inventory([asset.as_path()])
            .expect("runtime authorization should allow media inventory collection");
        assert_eq!(inventory.len(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_workspace_preparation_with_monitor() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        std::fs::write(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        )
        .expect("fixture");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));

        match service.prepare_authorized_workspace(
            [export.as_path()],
            std::iter::empty::<&Path>(),
            Some(dir.path()),
        ) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, export),
            _ => panic!("expected unauthorized path before runtime authorization"),
        }

        service.add_authorized_root(dir.path().to_path_buf());
        let (workspace, monitor) = service
            .prepare_authorized_workspace(
                [export.as_path()],
                std::iter::empty::<&Path>(),
                Some(dir.path()),
            )
            .expect("runtime authorization should allow workspace preparation with monitor");
        assert!(monitor.is_some());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
        assert_eq!(workspace.watch_dir.as_deref(), Some(dir.path()));
        assert_eq!(workspace.export_count(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_workspace_build() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        let asset = dir.path().join("preview.jpg");
        std::fs::write(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        )
        .expect("fixture");
        fs::write(&asset, [1_u8, 2, 3]).expect("media fixture");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));

        match service.build_authorized_workspace([export.as_path()], [asset.as_path()]) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, export),
            _ => panic!("expected unauthorized path before runtime authorization"),
        }

        service.add_authorized_root(dir.path().to_path_buf());
        let workspace = service
            .build_authorized_workspace([export.as_path()], [asset.as_path()])
            .expect("workspace should build after runtime authorization");
        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 1);
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn add_authorized_root_allows_runtime_audio_input_validation_to_progress() {
        let input_dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.ogg");
        let output = other_dir.path().join("voice.mp3");
        std::fs::write(&input, []).expect("audio input");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));

        match service.transcode_audio_asset_to_mp3(
            &input,
            &output,
            &AudioTranscodeOptions::default(),
        ) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, input),
            _ => panic!("expected unauthorized path before runtime authorization"),
        }

        service.add_authorized_root(input_dir.path().to_path_buf());
        if let Err(ViberError::UnauthorizedPath { path }) =
            service.transcode_audio_asset_to_mp3(&input, &output, &AudioTranscodeOptions::default())
        {
            panic!("authorization should have progressed past unauthorized path for {path:?}");
        }
    }

    #[test]
    fn add_authorized_root_allows_runtime_export_parsing() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));
        assert!(matches!(
            service.parse_authorized_export(&export),
            Err(ViberError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let parsed = service
            .parse_authorized_export(&export)
            .expect("runtime authorization should allow export parsing");
        assert_eq!(parsed.platform, "viber");
        assert_eq!(parsed.messages.len(), 1);
    }

    #[test]
    fn add_authorized_root_allows_runtime_export_staging() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));
        assert!(matches!(
            service.stage_authorized_exports([export.as_path()]),
            Err(ViberError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let staged = service
            .stage_authorized_exports([export.as_path()])
            .expect("runtime authorization should allow export staging");
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].platform_id, "viber");
    }

    #[test]
    fn add_authorized_root_allows_runtime_media_inventory_collection() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let asset = dir.path().join("voice.ogg");
        fs::write(&asset, [1_u8, 2, 3]).expect("write asset");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));
        assert!(matches!(
            service.collect_media_inventory([asset.as_path()]),
            Err(ViberError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(dir.path().to_path_buf());
        let assets = service
            .collect_media_inventory([asset.as_path()])
            .expect("runtime authorization should allow media inventory");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].path, asset);
    }

    #[test]
    fn add_authorized_root_allows_runtime_audio_output_validation_to_progress() {
        let input_dir = tempdir().expect("tempdir");
        let output_dir = tempdir().expect("tempdir");
        let input = input_dir.path().join("voice.ogg");
        let output = output_dir.path().join("voice.mp3");
        fs::write(&input, []).expect("audio input");

        let mut service = ViberService::new(ViberConfig::with_authorized_roots([input_dir
            .path()
            .to_path_buf()]));
        assert!(matches!(
            service.transcode_audio_asset_to_mp3(
                &input,
                &output,
                &AudioTranscodeOptions::default()
            ),
            Err(ViberError::UnauthorizedPath { .. })
        ));

        service.add_authorized_root(output_dir.path().to_path_buf());
        let result = service.transcode_audio_asset_to_mp3(
            &input,
            &output,
            &AudioTranscodeOptions::default(),
        );
        assert!(
            !matches!(result, Err(ViberError::UnauthorizedPath { .. })),
            "runtime authorization should move audio validation beyond output authorization checks"
        );
    }

    #[test]
    fn export_only_workspace_is_not_empty_and_preserves_account_views() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let workspace = service
            .build_authorized_workspace([export.as_path()], std::iter::empty::<&Path>())
            .expect("export-only workspace should build");

        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 0);
        assert!(!workspace.is_empty());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn media_only_workspace_is_not_empty_and_preserves_account_views() {
        let dir = tempdir().expect("tempdir");
        let asset = dir.path().join("photo.jpg");
        fs::write(&asset, [1_u8, 2, 3]).expect("write media");

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), [asset.as_path()])
            .expect("media-only workspace should build");

        assert_eq!(workspace.platform_id, "viber");
        assert_eq!(workspace.export_count(), 0);
        assert_eq!(workspace.media_count(), 1);
        assert!(!workspace.is_empty());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn prepare_authorized_workspace_without_watch_dir_leaves_monitor_absent() {
        let service = ViberService::new(ViberConfig::default());
        let (workspace, monitor) = service
            .prepare_authorized_workspace(
                std::iter::empty::<&Path>(),
                std::iter::empty::<&Path>(),
                None,
            )
            .expect("workspace should build without watch directory");

        assert!(monitor.is_none());
        assert!(workspace.watch_dir.is_none());
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn rejects_empty_audio_payload_transcoding() {
        let service = ViberService::new(ViberConfig::default());
        let error = service
            .transcode_audio_payload_to_mp3(&[], "ogg", &AudioTranscodeOptions::default())
            .expect_err("empty payload should fail");

        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn workspace_account_views_match_service_discovery() {
        let service = ViberService::new(ViberConfig::default());
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("workspace should build");

        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn primary_account_belongs_to_discovered_accounts_when_present() {
        let service = ViberService::new(ViberConfig::default());
        let accounts = service.discover_accounts();

        if let Some(primary) = service.primary_account() {
            assert!(accounts.contains(&primary));
        }
    }

    #[test]
    fn workspace_primary_account_belongs_to_workspace_accounts_when_present() {
        let service = ViberService::new(ViberConfig::default());
        let workspace = service
            .build_authorized_workspace(std::iter::empty::<&Path>(), std::iter::empty::<&Path>())
            .expect("workspace should build");

        if let Some(primary) = workspace.primary_account.clone() {
            assert!(workspace.accounts.contains(&primary));
        }
    }

    #[test]
    fn prepared_workspace_with_monitor_preserves_export_and_media_counts() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.json");
        let asset = dir.path().join("voice.opus");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );
        fs::write(&asset, [1_u8, 2, 3]).expect("write media");

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let (workspace, monitor) = service
            .prepare_authorized_workspace([export.as_path()], [asset.as_path()], Some(dir.path()))
            .expect("workspace and monitor should build");

        assert!(monitor.is_some());
        assert_eq!(workspace.export_count(), 1);
        assert_eq!(workspace.media_count(), 1);
        assert_eq!(workspace.watch_dir.as_deref(), Some(dir.path()));
        assert_eq!(workspace.accounts, service.discover_accounts());
        assert_eq!(workspace.primary_account, service.primary_account());
    }

    #[test]
    fn authorized_roots_include_runtime_root_after_addition() {
        let dir = tempdir().expect("tempdir");
        let other_dir = tempdir().expect("tempdir");
        let mut service = ViberService::new(ViberConfig::with_authorized_roots([other_dir
            .path()
            .to_path_buf()]));

        assert!(!service
            .authorized_roots()
            .iter()
            .any(|path| path == dir.path()));
        service.add_authorized_root(dir.path().to_path_buf());
        assert!(service
            .authorized_roots()
            .iter()
            .any(|path| path == dir.path()));
    }

    #[test]
    fn stage_authorized_exports_rejects_unauthorized_path_directly() {
        let authorized_dir = tempdir().expect("tempdir");
        let unauthorized_dir = tempdir().expect("tempdir");
        let export = unauthorized_dir.path().join("viber_fixture.txt");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let service = ViberService::new(ViberConfig::with_authorized_roots([authorized_dir
            .path()
            .to_path_buf()]));

        match service.stage_authorized_exports([export.as_path()]) {
            Err(ViberError::UnauthorizedPath { path }) => assert_eq!(path, export),
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("unauthorized export path should fail before staging"),
        }
    }

    #[test]
    fn stage_authorized_exports_preserves_source_path_and_platform_id() {
        let dir = tempdir().expect("tempdir");
        let export = dir.path().join("viber_fixture.txt");
        write_fixture(
            &export,
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
        );

        let service = ViberService::new(ViberConfig::with_authorized_roots([dir
            .path()
            .to_path_buf()]));
        let staged = service
            .stage_authorized_exports([export.as_path()])
            .expect("authorized export should stage");

        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].source_path, export);
        assert_eq!(staged[0].platform_id, service.platform_id());
    }

    #[test]
    fn create_export_monitor_rejects_unauthorized_directory() {
        let authorized_dir = tempdir().expect("tempdir");
        let unauthorized_dir = tempdir().expect("tempdir");
        let service = ViberService::new(ViberConfig::with_authorized_roots([authorized_dir
            .path()
            .to_path_buf()]));

        match service.create_export_monitor(unauthorized_dir.path()) {
            Err(ViberError::UnauthorizedPath { path }) => {
                assert_eq!(path, unauthorized_dir.path().to_path_buf());
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("unauthorized watch directory should fail"),
        }
    }
}
