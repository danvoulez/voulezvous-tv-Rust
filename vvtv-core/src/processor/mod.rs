mod error;
mod types;

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use hex::encode as hex_encode;
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tracing::warn;
use url::Url;

use crate::browser::{BrowserAutomation, BrowserCaptureKind, PbdOutcome, PlayBeforeDownload};
use crate::config::{ProcessorConfig, VvtvConfig};
use crate::plan::{Plan, PlanStatus, SqlitePlanStore};
use crate::queue::{PlayoutQueueStore, QueueItem};

pub use error::{ProcessorError, ProcessorResult};
pub use types::{
    DashDownload, DownloadedMedia, HlsDownload, MasteringOutcome, MasteringStrategy,
    MediaDescriptor, PackagingArtifacts, ProcessorReport, ProgressiveDownload, QcArtifacts,
    RetryPolicy, RevalidationOutcome, SegmentRecord, StagingPaths,
};

pub const MASTER_PLAYLIST_NAME: &str = "master.m3u8";

const HLS_SEGMENT_PREFIX_720: &str = "hls_720p";
const HLS_SEGMENT_PREFIX_480: &str = "hls_480p";

#[derive(Clone)]
pub struct Processor {
    plan_store: SqlitePlanStore,
    queue_store: PlayoutQueueStore,
    pbd: Option<Arc<PlayBeforeDownload>>,
    processor_config: Arc<ProcessorConfig>,
    vvtv_config: Arc<VvtvConfig>,
    http_client: Client,
    log_path: PathBuf,
    retry_policy: RetryPolicy,
    retry_sleep_cap: Duration,
}

impl Processor {
    pub fn new(
        plan_store: SqlitePlanStore,
        queue_store: PlayoutQueueStore,
        processor_config: ProcessorConfig,
        vvtv_config: VvtvConfig,
    ) -> ProcessorResult<Self> {
        let processor_config = Arc::new(processor_config);
        let vvtv_config = Arc::new(vvtv_config);
        let http_client = Client::builder()
            .user_agent("VVTV-Processor/1.0")
            .build()
            .map_err(|err| ProcessorError::Network(err.to_string()))?;
        let log_path = Path::new(&vvtv_config.paths.logs_dir).join("processor_failures.log");
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ProcessorError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let retry_policy = RetryPolicy::try_from(processor_config.download.clone())?;
        Ok(Self {
            plan_store,
            queue_store,
            pbd: None,
            processor_config,
            vvtv_config,
            http_client,
            log_path,
            retry_policy,
            retry_sleep_cap: Duration::from_secs(60),
        })
    }

    pub fn with_play_before_download(mut self, pbd: Arc<PlayBeforeDownload>) -> Self {
        self.pbd = Some(pbd);
        self
    }

    pub fn with_retry_sleep_cap(mut self, cap: Duration) -> Self {
        self.retry_sleep_cap = cap;
        self
    }

    pub async fn capture_and_process(
        &self,
        automation: &BrowserAutomation,
        plan: &Plan,
    ) -> ProcessorResult<ProcessorReport> {
        let pbd = self.pbd.as_ref().ok_or_else(|| {
            ProcessorError::InvalidMedia("PlayBeforeDownload not configured".into())
        })?;
        let source_url =
            plan.source_url
                .as_deref()
                .ok_or_else(|| ProcessorError::MissingSourceUrl {
                    plan_id: plan.plan_id.clone(),
                })?;
        let outcome = pbd.collect(automation, source_url).await?;
        self.process_with_capture(plan, outcome).await
    }

    pub async fn process_with_capture(
        &self,
        plan: &Plan,
        capture: PbdOutcome,
    ) -> ProcessorResult<ProcessorReport> {
        let hd_missing = capture.validation.video_height < 720;
        let staging = self.prepare_staging(&plan.plan_id).await?;
        let revalidation = RevalidationOutcome {
            capture: capture.capture.clone(),
            validation: capture.validation.clone(),
            metadata: capture.metadata.clone(),
            hd_missing,
            timestamp: Utc::now(),
        };

        let download_operation =
            || async { self.download_media(plan, &staging, &revalidation).await };
        let downloaded = self.retry_operation("download", download_operation).await?;

        let mastering = self
            .prepare_master(plan, &revalidation, &downloaded)
            .await?;
        let packaging = self
            .package_media(plan, &revalidation, &downloaded, &mastering)
            .await?;
        let _qc = self
            .run_quality_control(plan, &revalidation, &mastering, &packaging)
            .await?;

        self.update_plan_record(plan, &revalidation).await?;
        self.enqueue_plan(plan, &packaging).await?;
        self.cleanup_staging(&staging).await?;

        let report = ProcessorReport::new(
            &plan.plan_id,
            mastering.strategy.clone(),
            revalidation.hd_missing,
            packaging.duration,
            packaging.artifact_paths.clone(),
        );
        Ok(report)
    }

    async fn prepare_staging(&self, plan_id: &str) -> ProcessorResult<StagingPaths> {
        let staging_root = Path::new(&self.vvtv_config.paths.cache_dir)
            .join("tmp_downloads")
            .join(plan_id);
        let staging = StagingPaths::new(staging_root);
        fs::create_dir_all(&staging.source)
            .await
            .map_err(|source| ProcessorError::Io {
                path: staging.source.clone(),
                source,
            })?;
        fs::create_dir_all(&staging.remux)
            .await
            .map_err(|source| ProcessorError::Io {
                path: staging.remux.clone(),
                source,
            })?;
        fs::create_dir_all(&staging.logs)
            .await
            .map_err(|source| ProcessorError::Io {
                path: staging.logs.clone(),
                source,
            })?;
        Ok(staging)
    }

    async fn download_media(
        &self,
        plan: &Plan,
        staging: &StagingPaths,
        revalidation: &RevalidationOutcome,
    ) -> ProcessorResult<DownloadedMedia> {
        match revalidation.capture.kind {
            BrowserCaptureKind::HlsMediaPlaylist | BrowserCaptureKind::HlsMaster => {
                self.download_hls(plan, staging, revalidation).await
            }
            BrowserCaptureKind::DashManifest => {
                self.download_dash(plan, staging, revalidation).await
            }
            BrowserCaptureKind::Progressive => {
                self.download_progressive(plan, staging, revalidation).await
            }
            BrowserCaptureKind::Unknown => Err(ProcessorError::Download(
                "captured media kind is unknown".into(),
            )),
        }
    }

    async fn download_hls(
        &self,
        _plan: &Plan,
        staging: &StagingPaths,
        revalidation: &RevalidationOutcome,
    ) -> ProcessorResult<DownloadedMedia> {
        let playlist_url = &revalidation.capture.url;
        let playlist_contents = self.fetch_text(playlist_url).await?;
        let playlist = HlsPlaylist::parse(&playlist_contents)
            .map_err(|err| ProcessorError::Download(format!("invalid HLS playlist: {err}")))?;
        let original_path = staging.source.join("original.m3u8");
        fs::write(&original_path, playlist_contents)
            .await
            .map_err(|source| ProcessorError::Io {
                path: original_path.clone(),
                source,
            })?;

        let mut local_segments = Vec::new();
        for (index, segment) in playlist.segments.iter().enumerate() {
            let resolved = self.resolve_segment_url(playlist_url, &segment.uri)?;
            let extension = segment
                .uri
                .rsplit_once('.')
                .map(|(_, ext)| format!(".{ext}"))
                .unwrap_or_else(|| ".m4s".to_string());
            let local_name = format!("seg_{:04}{}", index + 1, extension);
            let local_path = staging.source.join(&local_name);
            self.fetch_to_file(&resolved, &local_path).await?;
            local_segments.push(SegmentRecord {
                index,
                duration: segment.duration,
                original_uri: resolved,
                local_path,
            });
        }

        if local_segments.is_empty() {
            return Err(ProcessorError::Download(
                "HLS playlist does not contain segments".into(),
            ));
        }

        let rewritten_path = staging.source.join("index.m3u8");
        let mut rewritten = String::new();
        rewritten.push_str("#EXTM3U\n");
        rewritten.push_str(&format!("#EXT-X-VERSION:{}\n", playlist.version));
        rewritten.push_str(&format!(
            "#EXT-X-TARGETDURATION:{}\n",
            playlist.target_duration
        ));
        rewritten.push_str(&format!(
            "#EXT-X-MEDIA-SEQUENCE:{}\n",
            playlist.media_sequence
        ));
        for segment in &local_segments {
            rewritten.push_str(&format!("#EXTINF:{:.3},\n", segment.duration));
            if let Some(name) = segment.local_path.file_name() {
                rewritten.push_str(&format!("{}\n", name.to_string_lossy()));
            }
        }
        rewritten.push_str("#EXT-X-ENDLIST\n");
        fs::write(&rewritten_path, rewritten)
            .await
            .map_err(|source| ProcessorError::Io {
                path: rewritten_path.clone(),
                source,
            })?;

        let total_duration: f64 = local_segments.iter().map(|s| s.duration).sum();
        Ok(DownloadedMedia::Hls(HlsDownload {
            playlist_path: original_path,
            rewritten_playlist: rewritten_path,
            segments: local_segments,
            media_sequence: playlist.media_sequence,
            target_duration: playlist.target_duration,
            total_duration,
        }))
    }

    async fn download_dash(
        &self,
        _plan: &Plan,
        staging: &StagingPaths,
        revalidation: &RevalidationOutcome,
    ) -> ProcessorResult<DownloadedMedia> {
        let manifest_url = &revalidation.capture.url;
        let manifest_contents = self.fetch_text(manifest_url).await?;
        let manifest_path = staging.source.join("manifest.mpd");
        fs::write(&manifest_path, manifest_contents.as_bytes())
            .await
            .map_err(|source| ProcessorError::Io {
                path: manifest_path.clone(),
                source,
            })?;
        let segments = DashManifest::parse(&manifest_contents)
            .map_err(|err| ProcessorError::Download(format!("invalid DASH manifest: {err}")))?;

        let mut local_segments = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            let resolved = self.resolve_segment_url(manifest_url, &segment.uri)?;
            let extension = segment
                .uri
                .rsplit_once('.')
                .map(|(_, ext)| format!(".{ext}"))
                .unwrap_or_else(|| ".m4s".to_string());
            let local_name = format!("dash_{:04}{}", index + 1, extension);
            let local_path = staging.source.join(&local_name);
            self.fetch_to_file(&resolved, &local_path).await?;
            local_segments.push(SegmentRecord {
                index,
                duration: segment.duration,
                original_uri: resolved,
                local_path,
            });
        }
        if local_segments.is_empty() {
            return Err(ProcessorError::Download(
                "DASH manifest does not contain media segments".into(),
            ));
        }
        let total_duration: f64 = local_segments.iter().map(|s| s.duration).sum();
        Ok(DownloadedMedia::Dash(DashDownload {
            manifest_path,
            segments: local_segments,
            total_duration,
        }))
    }

    async fn download_progressive(
        &self,
        _plan: &Plan,
        staging: &StagingPaths,
        revalidation: &RevalidationOutcome,
    ) -> ProcessorResult<DownloadedMedia> {
        let url = &revalidation.capture.url;
        let local_path = staging.source.join("source.mp4");
        self.fetch_to_file(url, &local_path).await?;
        let metadata = fs::metadata(&local_path)
            .await
            .map_err(|source| ProcessorError::Io {
                path: local_path.clone(),
                source,
            })?;
        Ok(DownloadedMedia::Progressive(ProgressiveDownload {
            file_path: local_path,
            size_bytes: metadata.len(),
        }))
    }

    async fn prepare_master(
        &self,
        plan: &Plan,
        revalidation: &RevalidationOutcome,
        downloaded: &DownloadedMedia,
    ) -> ProcessorResult<MasteringOutcome> {
        let ready_dir = self.ready_directory(&plan.plan_id);
        fs::create_dir_all(&ready_dir)
            .await
            .map_err(|source| ProcessorError::Io {
                path: ready_dir.clone(),
                source,
            })?;

        let mut descriptor = MediaDescriptor {
            container: "mp4".into(),
            video_codec: "avc1".into(),
            audio_codec: "aac".into(),
            width: revalidation.validation.video_width,
            height: revalidation.validation.video_height,
            duration: downloaded.duration(),
        };

        let supports_copy = matches!(
            downloaded,
            DownloadedMedia::Hls(_) | DownloadedMedia::Progressive(_)
        ) && self.processor_config.remux.prefer_copy;
        let strategy = if supports_copy {
            MasteringStrategy::Remux
        } else {
            MasteringStrategy::Transcode
        };

        let master_path = ready_dir.join("master.mp4");
        match downloaded {
            DownloadedMedia::Progressive(progressive)
                if matches!(strategy, MasteringStrategy::Remux) =>
            {
                self.copy_file(&progressive.file_path, &master_path).await?;
            }
            DownloadedMedia::Hls(hls) if matches!(strategy, MasteringStrategy::Remux) => {
                self.write_remux_stub(&master_path, "hls", &hls.segments)
                    .await?;
                descriptor.container = "hls".into();
            }
            DownloadedMedia::Dash(dash) if matches!(strategy, MasteringStrategy::Remux) => {
                self.write_remux_stub(&master_path, "dash", &dash.segments)
                    .await?;
                descriptor.container = "dash".into();
            }
            _ => {
                self.write_transcode_stub(&master_path, downloaded).await?;
            }
        }

        let normalized_path = if self.processor_config.loudnorm.enabled {
            let normalized = ready_dir.join("master_normalized.mp4");
            self.write_loudnorm_stub(&master_path, &normalized).await?;
            normalized
        } else {
            master_path.clone()
        };

        Ok(MasteringOutcome {
            master_path,
            normalized_path,
            descriptor,
            strategy,
        })
    }

    async fn package_media(
        &self,
        plan: &Plan,
        revalidation: &RevalidationOutcome,
        downloaded: &DownloadedMedia,
        mastering: &MasteringOutcome,
    ) -> ProcessorResult<PackagingArtifacts> {
        let ready_dir = self.ready_directory(&plan.plan_id);
        let chosen_playlist = ready_dir.join(format!("{HLS_SEGMENT_PREFIX_720}.m3u8"));
        let playlist_480 = ready_dir.join(format!("{HLS_SEGMENT_PREFIX_480}.m3u8"));

        let segments_720 = self
            .emit_hls_variant(
                &ready_dir,
                &mastering.normalized_path,
                downloaded,
                HLS_SEGMENT_PREFIX_720,
            )
            .await?;
        let segments_480 = self
            .emit_hls_variant(
                &ready_dir,
                &mastering.normalized_path,
                downloaded,
                HLS_SEGMENT_PREFIX_480,
            )
            .await?;

        let duration = downloaded
            .duration()
            .or_else(|| revalidation.validation.duration_seconds.map(|v| v as f64));

        let playlist_720_contents = self.build_variant_playlist(&segments_720);
        fs::write(&chosen_playlist, playlist_720_contents)
            .await
            .map_err(|source| ProcessorError::Io {
                path: chosen_playlist.clone(),
                source,
            })?;

        let playlist_480_contents = self.build_variant_playlist(&segments_480);
        fs::write(&playlist_480, playlist_480_contents)
            .await
            .map_err(|source| ProcessorError::Io {
                path: playlist_480.clone(),
                source,
            })?;

        let mut artifact_paths = Vec::new();
        artifact_paths.push(mastering.master_path.clone());
        if mastering.normalized_path != mastering.master_path {
            artifact_paths.push(mastering.normalized_path.clone());
        }
        artifact_paths.push(chosen_playlist.clone());
        artifact_paths.push(playlist_480.clone());
        artifact_paths.extend(segments_720.iter().map(|(_, path)| path.clone()));
        artifact_paths.extend(segments_480.iter().map(|(_, path)| path.clone()));

        Ok(PackagingArtifacts {
            ready_dir,
            master_path: mastering.master_path.clone(),
            normalized_master: mastering.normalized_path.clone(),
            playlists: vec![chosen_playlist.clone(), playlist_480.clone()],
            artifact_paths,
            chosen_playlist,
            duration,
        })
    }

    async fn run_quality_control(
        &self,
        plan: &Plan,
        revalidation: &RevalidationOutcome,
        mastering: &MasteringOutcome,
        packaging: &PackagingArtifacts,
    ) -> ProcessorResult<QcArtifacts> {
        let ready_dir = &packaging.ready_dir;
        let qc_report_path = ready_dir.join("qc_pre.json");
        let checksums_path = ready_dir.join("checksums.json");
        let manifest_path = ready_dir.join("manifest.json");

        let qc_payload = serde_json::json!({
            "plan_id": plan.plan_id,
            "video_width": revalidation.validation.video_width,
            "video_height": revalidation.validation.video_height,
            "duration_seconds": packaging.duration,
            "strategy": mastering.strategy,
            "hd_missing": revalidation.hd_missing,
        });
        fs::write(&qc_report_path, serde_json::to_vec_pretty(&qc_payload)?)
            .await
            .map_err(|source| ProcessorError::Io {
                path: qc_report_path.clone(),
                source,
            })?;

        let mut checksums = HashMap::new();
        for path in &packaging.artifact_paths {
            if let Ok(relative) = path.strip_prefix(ready_dir) {
                if let Some(rel) = relative.to_str() {
                    let checksum = self.compute_sha256(path).await?;
                    checksums.insert(rel.to_string(), checksum);
                }
            }
        }
        fs::write(&checksums_path, serde_json::to_vec_pretty(&checksums)?)
            .await
            .map_err(|source| ProcessorError::Io {
                path: checksums_path.clone(),
                source,
            })?;

        let manifest = Manifest {
            plan_id: plan.plan_id.clone(),
            source: revalidation.capture.url.clone(),
            capture_kind: format!("{:?}", revalidation.capture.kind),
            resolution: format!(
                "{}x{}",
                revalidation.validation.video_width, revalidation.validation.video_height
            ),
            strategy: mastering.strategy.clone(),
            duration: packaging.duration,
            playlists: packaging
                .playlists
                .iter()
                .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect(),
            created_at: Utc::now(),
        };
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
            .await
            .map_err(|source| ProcessorError::Io {
                path: manifest_path.clone(),
                source,
            })?;

        Ok(QcArtifacts {
            qc_report: qc_report_path,
            checksums: checksums_path,
            manifest: manifest_path,
        })
    }

    async fn update_plan_record(
        &self,
        plan: &Plan,
        revalidation: &RevalidationOutcome,
    ) -> ProcessorResult<()> {
        let resolution_label = format!("{}p", revalidation.validation.video_height);
        self.plan_store.mark_edited(
            &plan.plan_id,
            revalidation.hd_missing,
            Some(&resolution_label),
        )?;
        self.plan_store.record_attempt(
            &plan.plan_id,
            Some(plan.status.clone()),
            Some(PlanStatus::Edited),
            "processor pipeline completed",
        )?;
        Ok(())
    }

    async fn enqueue_plan(
        &self,
        plan: &Plan,
        packaging: &PackagingArtifacts,
    ) -> ProcessorResult<()> {
        let asset_path = packaging.chosen_playlist.to_string_lossy().to_string();
        let item = QueueItem {
            plan_id: plan.plan_id.clone(),
            asset_path,
            duration_s: packaging.duration.map(|d| d.round() as i64),
            curation_score: Some(plan.curation_score),
            priority: 0,
            node_origin: Some(self.vvtv_config.system.node_name.clone()),
            content_kind: Some(plan.kind.clone()),
        };
        self.queue_store.enqueue(&item)?;
        Ok(())
    }

    async fn cleanup_staging(&self, staging: &StagingPaths) -> ProcessorResult<()> {
        if let Err(err) = fs::remove_dir_all(&staging.root).await {
            warn!(path = %staging.root.display(), error = %err, "failed to clean staging directory");
        }
        Ok(())
    }

    async fn retry_operation<F, Fut, T>(&self, label: &str, mut operation: F) -> ProcessorResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ProcessorResult<T>>,
    {
        let attempts = self.retry_policy.attempts.max(1);
        for attempt in 0..attempts {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(err) if attempt + 1 == attempts => {
                    self.log_failure(label, &err);
                    return Err(err);
                }
                Err(err) => {
                    let delay = self.retry_policy.compute_delay(attempt);
                    let capped = delay.min(self.retry_sleep_cap);
                    warn!(attempt = attempt + 1, wait = ?capped, stage = label, error = %err, "retrying operation");
                    if !capped.is_zero() {
                        sleep(capped).await;
                    }
                }
            }
        }
        Err(ProcessorError::Download(format!(
            "operation {label} exhausted retries"
        )))
    }

    fn ready_directory(&self, plan_id: &str) -> PathBuf {
        Path::new(&self.vvtv_config.paths.storage_dir)
            .join("ready")
            .join(plan_id)
    }

    async fn fetch_text(&self, url: &str) -> ProcessorResult<String> {
        if let Ok(parsed) = Url::parse(url) {
            if parsed.scheme() == "file" {
                let path = parsed
                    .to_file_path()
                    .map_err(|_| ProcessorError::Download("invalid file url".into()))?;
                return fs::read_to_string(&path)
                    .await
                    .map_err(|source| ProcessorError::Io { path, source });
            }
        }
        let response = self.http_client.get(url).send().await?.error_for_status()?;
        Ok(response.text().await?)
    }

    async fn fetch_to_file(&self, url: &str, path: &Path) -> ProcessorResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| ProcessorError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }
        if let Ok(parsed) = Url::parse(url) {
            if parsed.scheme() == "file" {
                let source_path = parsed
                    .to_file_path()
                    .map_err(|_| ProcessorError::Download("invalid file url".into()))?;
                self.copy_file(&source_path, path).await?;
                return Ok(());
            }
        }
        let response = self.http_client.get(url).send().await?.error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut file = fs::File::create(path)
            .await
            .map_err(|source| ProcessorError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let data = chunk?;
            file.write_all(&data)
                .await
                .map_err(|source| ProcessorError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
        }
        Ok(())
    }

    async fn copy_file(&self, from: &Path, to: &Path) -> ProcessorResult<()> {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| ProcessorError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }
        fs::copy(from, to)
            .await
            .map(|_| ())
            .map_err(|source| ProcessorError::Io {
                path: to.to_path_buf(),
                source,
            })
    }

    fn resolve_segment_url(&self, base: &str, segment: &str) -> ProcessorResult<String> {
        if let Ok(parsed) = Url::parse(segment) {
            if matches!(parsed.scheme(), "file" | "http" | "https") {
                return Ok(segment.to_string());
            }
        }
        let base = Url::parse(base).map_err(|err| ProcessorError::Download(err.to_string()))?;
        let joined = base
            .join(segment)
            .map_err(|err| ProcessorError::Download(err.to_string()))?;
        Ok(joined.to_string())
    }

    async fn write_remux_stub(
        &self,
        path: &Path,
        label: &str,
        segments: &[SegmentRecord],
    ) -> ProcessorResult<()> {
        let mut file = fs::File::create(path)
            .await
            .map_err(|source| ProcessorError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let mut buffer = format!("REMUX-STUB {label} segments={}\n", segments.len());
        for segment in segments {
            buffer.push_str(&format!(
                "# {} {:.3}s {}\n",
                segment.index, segment.duration, segment.original_uri
            ));
        }
        file.write_all(buffer.as_bytes())
            .await
            .map_err(|source| ProcessorError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(())
    }

    async fn write_transcode_stub(
        &self,
        path: &Path,
        downloaded: &DownloadedMedia,
    ) -> ProcessorResult<()> {
        let mut file = fs::File::create(path)
            .await
            .map_err(|source| ProcessorError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let descriptor = format!("TRANSCODE-STUB {:?}\n", downloaded);
        file.write_all(descriptor.as_bytes())
            .await
            .map_err(|source| ProcessorError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(())
    }

    async fn write_loudnorm_stub(&self, master: &Path, normalized: &Path) -> ProcessorResult<()> {
        self.copy_file(master, normalized).await?;
        Ok(())
    }

    async fn emit_hls_variant(
        &self,
        ready_dir: &Path,
        _source: &Path,
        downloaded: &DownloadedMedia,
        prefix: &str,
    ) -> ProcessorResult<Vec<(f64, PathBuf)>> {
        let mut results = Vec::new();
        let mut index = 0usize;
        match downloaded {
            DownloadedMedia::Hls(hls) => {
                for segment in &hls.segments {
                    index += 1;
                    let extension = segment.extension().unwrap_or_else(|| "m4s".into());
                    let file_name = format!("{prefix}_{:04}.{}", index, extension);
                    let dest = ready_dir.join(&file_name);
                    self.copy_file(&segment.local_path, &dest).await?;
                    results.push((segment.duration, dest));
                }
            }
            DownloadedMedia::Dash(dash) => {
                for segment in &dash.segments {
                    index += 1;
                    let extension = segment.extension().unwrap_or_else(|| "m4s".into());
                    let file_name = format!("{prefix}_{:04}.{}", index, extension);
                    let dest = ready_dir.join(&file_name);
                    self.copy_file(&segment.local_path, &dest).await?;
                    results.push((segment.duration, dest));
                }
            }
            DownloadedMedia::Progressive(_) => {
                let duration = self.estimate_duration_from_prefix(prefix);
                for n in 0..3 {
                    index += 1;
                    let file_name = format!("{prefix}_{:04}.m4s", index);
                    let dest = ready_dir.join(&file_name);
                    let mut file =
                        fs::File::create(&dest)
                            .await
                            .map_err(|source| ProcessorError::Io {
                                path: dest.clone(),
                                source,
                            })?;
                    let contents = format!("VARIANT-STUB prefix={prefix} segment={n}\n");
                    file.write_all(contents.as_bytes())
                        .await
                        .map_err(|source| ProcessorError::Io {
                            path: dest.clone(),
                            source,
                        })?;
                    results.push((duration, dest));
                }
            }
        }
        Ok(results)
    }

    fn build_variant_playlist(&self, segments: &[(f64, PathBuf)]) -> String {
        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:7\n");
        let target = segments
            .iter()
            .map(|(duration, _)| duration.ceil() as u32)
            .max()
            .unwrap_or(4);
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", target));
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        for (duration, path) in segments {
            let file_name = path.file_name().unwrap().to_string_lossy();
            playlist.push_str(&format!("#EXTINF:{:.3},\n", duration));
            playlist.push_str(&format!("{}\n", file_name));
        }
        playlist.push_str("#EXT-X-ENDLIST\n");
        playlist
    }

    async fn compute_sha256(&self, path: &Path) -> ProcessorResult<String> {
        let bytes = fs::read(path).await.map_err(|source| ProcessorError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Ok(hex_encode(hasher.finalize()))
    }

    fn estimate_duration_from_prefix(&self, prefix: &str) -> f64 {
        match prefix {
            HLS_SEGMENT_PREFIX_720 => 4.0,
            HLS_SEGMENT_PREFIX_480 => 6.0,
            _ => 5.0,
        }
    }

    fn log_failure(&self, stage: &str, error: &ProcessorError) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = writeln!(file, "{} [{}] {}", Utc::now().to_rfc3339(), stage, error);
        }
    }
}

#[derive(Debug, Clone)]
struct HlsPlaylist {
    version: u32,
    target_duration: f64,
    media_sequence: u64,
    segments: Vec<HlsSegment>,
}

#[derive(Debug, Clone)]
struct HlsSegment {
    duration: f64,
    uri: String,
}

impl HlsPlaylist {
    fn parse(contents: &str) -> Result<Self, String> {
        if !contents.trim_start().starts_with("#EXTM3U") {
            return Err("missing #EXTM3U header".into());
        }
        let mut version = 3u32;
        let mut target_duration = 4.0f64;
        let mut media_sequence = 0u64;
        let mut segments = Vec::new();
        let mut pending_duration: Option<f64> = None;
        for line in contents.lines().map(|line| line.trim()) {
            if line.starts_with("#EXT-X-VERSION:") {
                version = line[15..].parse().map_err(|_| "invalid EXT-X-VERSION")?;
            } else if line.starts_with("#EXT-X-TARGETDURATION:") {
                target_duration = line[22..]
                    .parse()
                    .map_err(|_| "invalid EXT-X-TARGETDURATION")?;
            } else if line.starts_with("#EXT-X-MEDIA-SEQUENCE:") {
                media_sequence = line[22..]
                    .parse()
                    .map_err(|_| "invalid EXT-X-MEDIA-SEQUENCE")?;
            } else if line.starts_with("#EXTINF:") {
                let value = line[8..]
                    .trim_end_matches(',')
                    .parse()
                    .map_err(|_| "invalid EXTINF duration")?;
                pending_duration = Some(value);
            } else if line.starts_with('#') || line.is_empty() {
                continue;
            } else if let Some(duration) = pending_duration.take() {
                segments.push(HlsSegment {
                    duration,
                    uri: line.to_string(),
                });
            }
        }
        if segments.is_empty() {
            return Err("playlist missing segments".into());
        }
        Ok(Self {
            version,
            target_duration,
            media_sequence,
            segments,
        })
    }
}

#[derive(Debug, Clone)]
struct DashSegment {
    duration: f64,
    uri: String,
}

struct DashManifest;

impl DashManifest {
    fn parse(contents: &str) -> Result<Vec<DashSegment>, String> {
        let mut segments = Vec::new();
        let regex =
            regex::Regex::new("SegmentURL\\s+media=\"([^\\\"]+)\"(?:\\s+duration=\"([^\\\"]+)\")?")
                .map_err(|err| err.to_string())?;
        for capture in regex.captures_iter(contents) {
            let uri = capture
                .get(1)
                .ok_or_else(|| "missing media attribute".to_string())?
                .as_str()
                .to_string();
            let duration = capture
                .get(2)
                .map(|m| m.as_str().parse().unwrap_or(4.0))
                .unwrap_or(4.0);
            segments.push(DashSegment { duration, uri });
        }
        if segments.is_empty() {
            return Err("no SegmentURL entries found".into());
        }
        Ok(segments)
    }
}

#[derive(Debug, Serialize)]
struct Manifest {
    plan_id: String,
    source: String,
    capture_kind: String,
    resolution: String,
    strategy: MasteringStrategy,
    duration: Option<f64>,
    playlists: Vec<String>,
    created_at: chrono::DateTime<Utc>,
}
