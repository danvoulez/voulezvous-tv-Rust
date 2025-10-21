use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::{ConfigError, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VvtvConfig {
    pub system: SystemSection,
    pub paths: PathsSection,
    pub limits: LimitsSection,
    pub network: NetworkSection,
    pub quality: QualitySection,
    pub security: SecuritySection,
    pub monitoring: MonitoringSection,
    pub economy: EconomySection,
}

impl VvtvConfig {
    pub fn resolve_path<P: AsRef<Path>>(&self, candidate: P) -> PathBuf {
        let path = candidate.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            Path::new(&self.paths.base_dir).join(path)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemSection {
    pub node_name: String,
    pub node_role: String,
    pub node_id: String,
    pub environment: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsSection {
    pub base_dir: String,
    pub data_dir: String,
    pub cache_dir: String,
    pub storage_dir: String,
    pub broadcast_dir: String,
    pub logs_dir: String,
    pub vault_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LimitsSection {
    pub buffer_target_hours: f64,
    pub buffer_warning_hours: f64,
    pub buffer_critical_hours: f64,
    pub max_concurrent_downloads: u32,
    pub max_concurrent_transcodes: u32,
    pub max_browser_instances: u32,
    pub cpu_limit_percent: u32,
    pub memory_limit_gb: u32,
    pub disk_warning_percent: u32,
    pub disk_critical_percent: u32,
    pub plans_retention_days: u32,
    pub played_retention_hours: u32,
    pub logs_retention_days: u32,
    pub cache_retention_hours: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkSection {
    pub tailscale_domain: String,
    pub rtmp_port: u16,
    pub hls_port: u16,
    pub control_port: u16,
    pub cdn_primary: String,
    pub cdn_backup: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualitySection {
    pub target_lufs: f64,
    pub lufs_tolerance: f64,
    pub audio_codec: String,
    pub audio_bitrate: String,
    pub vmaf_threshold: u32,
    pub ssim_threshold: f64,
    pub target_resolution: String,
    pub fallback_resolution: String,
    pub hls_segment_duration: u32,
    pub hls_playlist_length_minutes: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecuritySection {
    pub sandbox_enabled: bool,
    pub fingerprint_randomization: bool,
    pub proxy_rotation_enabled: bool,
    pub csam_check_enabled: bool,
    pub drm_detection_abort: bool,
    pub allow_rtmp_from: Vec<String>,
    pub allow_hls_from: Vec<String>,
    pub allow_ssh_from: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringSection {
    pub health_check_interval_seconds: u64,
    pub metrics_collection_interval_seconds: u64,
    pub capture_interval_minutes: u64,
    pub alert_telegram_enabled: bool,
    pub alert_email_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EconomySection {
    pub monetization_enabled: bool,
    pub base_rate_per_minute: f64,
    pub ledger_export_interval_hours: u32,
    pub reconciliation_interval_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrowserConfig {
    pub chromium: ChromiumSection,
    pub flags: FlagsSection,
    pub user_agents: UserAgentSection,
    pub viewport: ViewportSection,
    pub human_simulation: HumanSimulationSection,
    pub pbd: PbdSection,
    pub selectors: SelectorSection,
    pub proxy: ProxySection,
    pub sources: SourcesSection,
    pub fingerprint: FingerprintSection,
    pub retry: RetrySection,
    pub ip_rotation: IpRotationSection,
    pub observability: ObservabilitySection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChromiumSection {
    pub executable_path: String,
    pub headless: bool,
    pub sandbox: bool,
    pub disable_gpu: bool,
    pub max_memory_mb: Option<u64>,
    pub max_tabs_per_instance: Option<u32>,
    pub tab_timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlagsSection {
    pub no_first_run: bool,
    pub disable_automation_controlled: bool,
    pub disable_blink_features: Vec<String>,
    pub mute_audio: bool,
    pub autoplay_policy: String,
    pub lang: Option<String>,
    pub accept_language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserAgentSection {
    pub pool: Vec<String>,
    pub rotation_frequency: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ViewportSection {
    pub resolutions: Vec<[u32; 2]>,
    pub jitter_pixels: u32,
    pub device_scale_factor: [f32; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct HumanSimulationSection {
    pub mouse_speed_min_px_s: u32,
    pub mouse_speed_max_px_s: u32,
    pub mouse_jitter_px: u32,
    pub click_hesitation_ms: [u32; 2],
    pub click_duration_ms: [u32; 2],
    pub scroll_burst_px: [u32; 2],
    pub scroll_pause_ms: [u32; 2],
    pub scroll_near_player_slow: bool,
    pub typing_cadence_cpm: [u32; 2],
    pub typing_jitter_ms: [u32; 2],
    pub error_frequency_chars: [u32; 2],
    pub idle_duration_ms: [u32; 2],
    pub ociosidade_frequency: [u32; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct PbdSection {
    pub enabled: bool,
    pub force_hd: bool,
    pub hd_priority: Vec<String>,
    pub playback_wait_seconds: [u32; 2],
    pub validation_buffer_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectorSection {
    pub video_element: String,
    pub play_buttons: Vec<String>,
    pub quality_menu: Vec<String>,
    pub consent_buttons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxySection {
    pub enabled: bool,
    pub r#type: String,
    pub rotation_pages: u32,
    pub timeout_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourcesSection {
    pub whitelist: Vec<String>,
    pub blacklist: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FingerprintSection {
    pub enable_canvas_noise: bool,
    pub enable_webgl_mask: bool,
    pub enable_audio_mask: bool,
    pub canvas_noise_range: [i32; 2],
    pub audio_noise: f64,
    pub webgl_vendor: Option<String>,
    pub webgl_renderer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetrySection {
    pub max_attempts: usize,
    pub schedule_minutes: Vec<u64>,
    pub jitter_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IpRotationSection {
    pub enabled: bool,
    pub tailscale_binary: String,
    pub exit_nodes: Vec<String>,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilitySection {
    pub failure_log: String,
    pub metrics_db: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessorConfig {
    pub download: DownloadSection,
    pub hls: HlsSection,
    pub dash: DashSection,
    pub progressive: ProgressiveSection,
    pub remux: RemuxSection,
    pub transcode: TranscodeSection,
    pub loudnorm: LoudnormSection,
    pub profiles: ProfilesSection,
    pub qc: QcSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadSection {
    pub tool: String,
    pub max_retries: u32,
    pub retry_delay_seconds: [u32; 2],
    pub bandwidth_limit_mbps: u32,
    pub resume_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HlsSection {
    pub vod_only: bool,
    pub verify_sequence: bool,
    pub rewrite_playlist: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashSection {
    pub prefer_h264: bool,
    pub remux_to_hls: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProgressiveSection {
    pub head_check: bool,
    pub min_size_mb: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemuxSection {
    pub prefer_copy: bool,
    pub faststart: bool,
    pub fallback_transcode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscodeSection {
    pub codec: String,
    pub preset: String,
    pub crf: u8,
    pub profile: String,
    pub level: String,
    pub pix_fmt: String,
    pub keyint: u32,
    pub min_keyint: u32,
    pub scenecut: u32,
    pub vbv_maxrate: String,
    pub vbv_bufsize: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoudnormSection {
    pub enabled: bool,
    pub integrated: f64,
    pub true_peak: f64,
    pub lra: f64,
    pub linear: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfilesSection {
    pub hls_720p: ProfileEntry,
    pub hls_480p: ProfileEntry,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileEntry {
    pub scale: String,
    pub video_bitrate: String,
    pub maxrate: String,
    pub bufsize: String,
    pub audio_bitrate: String,
    pub preset: String,
    pub profile: String,
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QcSection {
    pub ffprobe_validation: bool,
    pub checksums_sha256: bool,
    pub duration_tolerance_percent: u32,
    pub min_duration_video_s: u32,
    pub min_duration_music_s: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BroadcasterConfig {
    pub queue: QueueSection,
    pub rtmp: RtmpSection,
    pub hls: BroadcasterHlsSection,
    pub failover: FailoverSection,
    pub watchdog: WatchdogSection,
    pub ffmpeg: FfmpegSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueSection {
    pub policy: String,
    pub music_ratio: f32,
    pub curation_bump_threshold: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RtmpSection {
    pub origin: String,
    pub chunk_size: u32,
    pub reconnect_attempts: u32,
    pub reconnect_delay_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BroadcasterHlsSection {
    pub output_path: String,
    pub segment_duration: u32,
    pub playlist_length: String,
    pub segment_type: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailoverSection {
    pub enabled: bool,
    pub standby_encoder: bool,
    pub detection_timeout_seconds: u32,
    pub emergency_loop_hours: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WatchdogSection {
    pub interval_seconds: u32,
    pub restart_on_freeze: bool,
    pub restart_max_attempts: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FfmpegSection {
    pub log_level: String,
    pub stats_period: String,
    pub thread_queue_size: u32,
}

#[derive(Debug, Clone)]
pub struct ConfigBundle {
    pub vvtv: VvtvConfig,
    pub browser: BrowserConfig,
    pub processor: ProcessorConfig,
    pub broadcaster: BroadcasterConfig,
}

impl ConfigBundle {
    pub fn from_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        let vvtv = load_vvtv_config(dir.join("vvtv.toml"))?;
        let browser = load_browser_config(dir.join("browser.toml"))?;
        let processor = load_processor_config(dir.join("processor.toml"))?;
        let broadcaster = load_broadcaster_config(dir.join("broadcaster.toml"))?;
        Ok(Self {
            vvtv,
            browser,
            processor,
            broadcaster,
        })
    }
}

pub fn load_vvtv_config<P: AsRef<Path>>(path: P) -> Result<VvtvConfig> {
    load_toml(path)
}

pub fn load_browser_config<P: AsRef<Path>>(path: P) -> Result<BrowserConfig> {
    load_toml(path)
}

pub fn load_processor_config<P: AsRef<Path>>(path: P) -> Result<ProcessorConfig> {
    load_toml(path)
}

pub fn load_broadcaster_config<P: AsRef<Path>>(path: P) -> Result<BroadcasterConfig> {
    load_toml(path)
}

fn load_toml<T, P>(path: P) -> Result<T>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        source,
        path: path.to_path_buf(),
    })?;
    toml::from_str(&content).map_err(|source| ConfigError::Parse {
        source,
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_fixture_configs() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs");
        let bundle = ConfigBundle::from_directory(dir).expect("configs should parse");
        assert_eq!(bundle.vvtv.system.node_name, "vvtv-primary");
        assert!(bundle.browser.user_agents.pool.len() >= 2);
        assert_eq!(bundle.processor.download.tool, "aria2");
        assert_eq!(bundle.broadcaster.queue.policy, "fifo_with_bump");
    }
}
