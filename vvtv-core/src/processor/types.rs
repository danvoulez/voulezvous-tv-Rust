use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::browser::{BrowserCapture, ContentMetadata, PlaybackValidation};

use super::error::ProcessorError;

#[derive(Debug, Clone)]
pub struct StagingPaths {
    pub root: PathBuf,
    pub source: PathBuf,
    pub remux: PathBuf,
    pub logs: PathBuf,
}

impl StagingPaths {
    pub fn new(root: PathBuf) -> Self {
        let source = root.join("source");
        let remux = root.join("remux");
        let logs = root.join("logs");
        Self {
            root,
            source,
            remux,
            logs,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RevalidationOutcome {
    pub capture: BrowserCapture,
    pub validation: PlaybackValidation,
    pub metadata: ContentMetadata,
    pub hd_missing: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SegmentRecord {
    pub index: usize,
    pub duration: f64,
    pub original_uri: String,
    pub local_path: PathBuf,
}

impl SegmentRecord {
    pub fn extension(&self) -> Option<String> {
        self.local_path
            .extension()
            .map(|ext| ext.to_string_lossy().to_string())
    }
}

#[derive(Debug, Clone)]
pub struct HlsDownload {
    pub playlist_path: PathBuf,
    pub rewritten_playlist: PathBuf,
    pub segments: Vec<SegmentRecord>,
    pub media_sequence: u64,
    pub target_duration: f64,
    pub total_duration: f64,
}

#[derive(Debug, Clone)]
pub struct DashDownload {
    pub manifest_path: PathBuf,
    pub segments: Vec<SegmentRecord>,
    pub total_duration: f64,
}

#[derive(Debug, Clone)]
pub struct ProgressiveDownload {
    pub file_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum DownloadedMedia {
    Hls(HlsDownload),
    Dash(DashDownload),
    Progressive(ProgressiveDownload),
}

impl DownloadedMedia {
    pub fn duration(&self) -> Option<f64> {
        match self {
            DownloadedMedia::Hls(hls) => Some(hls.total_duration),
            DownloadedMedia::Dash(dash) => Some(dash.total_duration),
            DownloadedMedia::Progressive(_) => None,
        }
    }

    pub fn segments(&self) -> &[SegmentRecord] {
        match self {
            DownloadedMedia::Hls(hls) => &hls.segments,
            DownloadedMedia::Dash(dash) => &dash.segments,
            DownloadedMedia::Progressive(_) => &[],
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MasteringStrategy {
    Remux,
    Transcode,
}

#[derive(Debug, Clone)]
pub struct MediaDescriptor {
    pub container: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub width: u32,
    pub height: u32,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct MasteringOutcome {
    pub master_path: PathBuf,
    pub normalized_path: PathBuf,
    pub descriptor: MediaDescriptor,
    pub strategy: MasteringStrategy,
}

#[derive(Debug, Clone)]
pub struct PackagingArtifacts {
    pub ready_dir: PathBuf,
    pub master_path: PathBuf,
    pub normalized_master: PathBuf,
    pub playlists: Vec<PathBuf>,
    pub artifact_paths: Vec<PathBuf>,
    pub chosen_playlist: PathBuf,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct QcArtifacts {
    pub qc_report: PathBuf,
    pub checksums: PathBuf,
    pub manifest: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessorReport {
    pub plan_id: String,
    pub strategy: MasteringStrategy,
    pub hd_missing: bool,
    pub duration_seconds: Option<f64>,
    pub artifacts: Vec<PathBuf>,
    pub completed_at: DateTime<Utc>,
}

impl ProcessorReport {
    pub fn new(
        plan_id: impl Into<String>,
        strategy: MasteringStrategy,
        hd_missing: bool,
        duration_seconds: Option<f64>,
        artifacts: Vec<PathBuf>,
    ) -> Self {
        Self {
            plan_id: plan_id.into(),
            strategy,
            hd_missing,
            duration_seconds,
            artifacts,
            completed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub delay_range: [u32; 2],
}

impl RetryPolicy {
    pub fn compute_delay(&self, attempt: u32) -> Duration {
        if self.attempts <= 1 {
            return Duration::from_secs(self.delay_range[0] as u64);
        }
        let min = self.delay_range[0] as f64;
        let max = self.delay_range[1] as f64;
        let ratio = (attempt as f64) / ((self.attempts - 1) as f64);
        let seconds = min + (max - min) * ratio;
        Duration::from_secs(seconds.round() as u64)
    }
}

impl TryFrom<crate::config::DownloadSection> for RetryPolicy {
    type Error = ProcessorError;

    fn try_from(section: crate::config::DownloadSection) -> Result<Self, Self::Error> {
        if section.max_retries == 0 {
            return Err(ProcessorError::InvalidMedia(
                "max_retries must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            attempts: section.max_retries,
            delay_range: section.retry_delay_seconds,
        })
    }
}
