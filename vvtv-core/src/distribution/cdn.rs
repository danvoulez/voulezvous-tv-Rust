use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::Command;
use walkdir::WalkDir;

use super::replicator::{build_remote_path, ReplicationExecutor, SystemReplicationExecutor};

#[derive(Debug, Error)]
pub enum CdnError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("missing api token at {0}")]
    MissingToken(PathBuf),
    #[error("missing totals field in CDN analytics response")]
    MissingTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct CdnCoordinator {
    pub provider: String,
    pub primary_hostname: String,
    pub backup_hostname: String,
    pub manifest_ttl_seconds: u64,
    pub segment_ttl_seconds: u64,
    pub health_check_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CdnMetrics {
    pub provider: String,
    pub timestamp: DateTime<Utc>,
    pub cdn_hits: u64,
    pub latency_avg_ms: f64,
    pub cache_hit_rate: f64,
    pub origin_errors: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupSyncReport {
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub files_uploaded: u64,
    pub bytes_uploaded: u64,
    pub removed_segments: Vec<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct PrimaryCdnConfig {
    pub provider: String,
    pub api_base: String,
    pub zone_id: String,
    pub account_id: String,
    pub api_token_path: PathBuf,
    pub dns_record_id: String,
    pub dns_record_name: String,
    pub primary_hostname: String,
    pub backup_hostname: String,
    pub health_check_url: String,
    pub manifest_ttl_seconds: u64,
    pub segment_ttl_seconds: u64,
    pub metrics_log_path: PathBuf,
    pub worker_script_path: PathBuf,
    pub worker_service: String,
    pub worker_environment: String,
}

#[derive(Debug, Clone)]
pub struct BackupCdnConfig {
    pub provider: String,
    pub rclone_path: PathBuf,
    pub source_paths: Vec<PathBuf>,
    pub remote: String,
    pub manifest_path: PathBuf,
    pub ttl_days: u64,
    pub switch_script: PathBuf,
    pub log_path: PathBuf,
}

pub struct PrimaryCdnManager {
    client: Client,
    config: PrimaryCdnConfig,
    coordinator: CdnCoordinator,
}

impl PrimaryCdnManager {
    pub fn new(client: Client, config: PrimaryCdnConfig) -> Result<Self, CdnError> {
        let coordinator = CdnCoordinator {
            provider: config.provider.clone(),
            primary_hostname: config.primary_hostname.clone(),
            backup_hostname: config.backup_hostname.clone(),
            manifest_ttl_seconds: config.manifest_ttl_seconds,
            segment_ttl_seconds: config.segment_ttl_seconds,
            health_check_url: config.health_check_url.clone(),
        };
        Ok(Self {
            client,
            config,
            coordinator,
        })
    }

    pub fn coordinator(&self) -> CdnCoordinator {
        self.coordinator.clone()
    }

    pub async fn fetch_metrics(&self) -> Result<CdnMetrics, CdnError> {
        let token = self.load_token()?;
        let url = format!(
            "{}/zones/{}/analytics/dashboard",
            self.config.api_base, self.config.zone_id
        );
        let response: AnalyticsResponse = self
            .client
            .get(url)
            .bearer_auth(token)
            .query(&[("since", "-15 minutes"), ("until", "now")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let totals = response.result.totals.ok_or(CdnError::MissingTotals)?;
        let cdn_hits = totals.requests.all;
        let latency_avg_ms = totals
            .ttfb
            .as_ref()
            .map(|ttfb| ttfb.p50 * 1000.0)
            .unwrap_or_default();
        let cache_hit_rate = totals
            .requests
            .cached
            .map(|cached| cached as f64 / cdn_hits.max(1) as f64)
            .unwrap_or_default();
        let metrics = CdnMetrics {
            provider: self.coordinator.provider.clone(),
            timestamp: Utc::now(),
            cdn_hits,
            latency_avg_ms,
            cache_hit_rate,
            origin_errors: totals.origin_errors.unwrap_or_default(),
        };
        self.append_metrics_log(&metrics)?;
        Ok(metrics)
    }

    pub async fn update_origin(&self, hostname: &str) -> Result<(), CdnError> {
        let token = self.load_token()?;
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.config.api_base, self.config.zone_id, self.config.dns_record_id
        );
        let payload = serde_json::json!({
            "type": "CNAME",
            "name": self.config.dns_record_name,
            "content": hostname,
            "ttl": 120,
            "proxied": true
        });
        self.client
            .patch(url)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn deploy_worker(&self) -> Result<(), CdnError> {
        let token = self.load_token()?;
        let script = fs::read_to_string(&self.config.worker_script_path)?;
        let url = format!(
            "{}/accounts/{}/workers/services/{}/environments/{}/script",
            self.config.api_base,
            self.config.account_id,
            self.config.worker_service,
            self.config.worker_environment
        );
        self.client
            .put(url)
            .bearer_auth(token)
            .header("Content-Type", "application/javascript")
            .body(script)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    fn append_metrics_log(&self, metrics: &CdnMetrics) -> Result<(), CdnError> {
        if let Some(parent) = self.config.metrics_log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.metrics_log_path)?;
        let record = serde_json::to_string(metrics)?;
        writeln!(file, "{}", record)?;
        Ok(())
    }

    fn load_token(&self) -> Result<String, CdnError> {
        let token = fs::read_to_string(&self.config.api_token_path)
            .map_err(|_| CdnError::MissingToken(self.config.api_token_path.clone()))?;
        Ok(token.trim().to_string())
    }
}

#[derive(Debug, Deserialize)]
struct AnalyticsResponse {
    result: AnalyticsResult,
}

#[derive(Debug, Deserialize)]
struct AnalyticsResult {
    totals: Option<AnalyticsTotals>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsTotals {
    requests: AnalyticsRequests,
    ttfb: Option<AnalyticsTtfb>,
    #[serde(default)]
    origin_errors: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsRequests {
    all: u64,
    #[serde(default)]
    cached: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsTtfb {
    #[serde(default)]
    p50: f64,
}

pub struct BackupCdnManager {
    config: BackupCdnConfig,
    coordinator: CdnCoordinator,
    executor: Arc<dyn ReplicationExecutor>,
}

impl BackupCdnManager {
    pub fn new(config: BackupCdnConfig, coordinator: CdnCoordinator) -> Self {
        Self {
            config,
            coordinator,
            executor: Arc::new(SystemReplicationExecutor),
        }
    }

    pub fn with_executor(
        config: BackupCdnConfig,
        coordinator: CdnCoordinator,
        executor: Arc<dyn ReplicationExecutor>,
    ) -> Self {
        Self {
            config,
            coordinator,
            executor,
        }
    }

    pub fn coordinator(&self) -> CdnCoordinator {
        self.coordinator.clone()
    }

    pub async fn sync_backup(&self) -> Result<BackupSyncReport, CdnError> {
        let start = Instant::now();
        let mut uploaded_files = 0u64;
        let mut uploaded_bytes = 0u64;
        for path in &self.config.source_paths {
            let mut command = Command::new(&self.config.rclone_path);
            let mut parts = vec![self.config.rclone_path.to_string_lossy().into_owned()];
            command.arg("copy");
            parts.push("copy".to_string());
            command.arg(path);
            parts.push(path.to_string_lossy().into_owned());
            let remote_path = build_remote_path(&self.config.remote, path);
            command.arg(remote_path.as_str());
            parts.push(remote_path.clone());
            command.arg("--fast-list");
            parts.push("--fast-list".to_string());
            command.arg("--transfers");
            parts.push("--transfers".to_string());
            command.arg("4");
            parts.push("4".to_string());
            command.arg("--immutable");
            parts.push("--immutable".to_string());
            let cmd_string = parts.join(" ");
            let output = self
                .executor
                .run(&mut command)
                .await
                .map_err(CdnError::Io)?;
            if !output.status.success() {
                return Err(CdnError::CommandFailure {
                    command: cmd_string,
                    status: output.status.code(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
            let (files, bytes) = parse_rclone_transfer(&output.stdout);
            uploaded_files += files;
            uploaded_bytes += bytes;
        }
        let removed = self.cleanup_stale_segments()?;
        let duration = start.elapsed();
        self.append_log(uploaded_files, uploaded_bytes, &removed, duration)?;
        Ok(BackupSyncReport {
            timestamp: Utc::now(),
            provider: self.config.provider.clone(),
            files_uploaded: uploaded_files,
            bytes_uploaded: uploaded_bytes,
            removed_segments: removed,
            duration_ms: duration.as_millis(),
        })
    }

    fn cleanup_stale_segments(&self) -> Result<Vec<String>, CdnError> {
        let manifest = fs::read_to_string(&self.config.manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&manifest)?;
        let valid: HashSet<String> = manifest.segments.into_iter().collect();
        let ttl = Duration::from_secs(self.config.ttl_days * 24 * 3600);
        let mut removed = Vec::new();
        for path in &self.config.source_paths {
            for entry in WalkDir::new(path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.path().is_file())
            {
                let file_path = entry.path();
                let metadata = fs::metadata(file_path)?;
                let modified = metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if let Ok(age) = modified.elapsed() {
                    let name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    if age > ttl && !valid.contains(&name) {
                        fs::remove_file(file_path)?;
                        removed.push(name);
                    }
                }
            }
        }
        Ok(removed)
    }

    fn append_log(
        &self,
        files_uploaded: u64,
        bytes_uploaded: u64,
        removed: &[String],
        duration: Duration,
    ) -> Result<(), CdnError> {
        if let Some(parent) = self.config.log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_path)?;
        let record = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "provider": self.config.provider.clone(),
            "files_uploaded": files_uploaded,
            "bytes_uploaded": bytes_uploaded,
            "removed_segments": removed,
            "duration_ms": duration.as_millis(),
        });
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Manifest {
    segments: Vec<String>,
}

fn parse_rclone_transfer(stdout: &[u8]) -> (u64, u64) {
    let output = String::from_utf8_lossy(stdout);
    let files_regex = Regex::new(r"Transferred:\s+([0-9]+)/([0-9]+)").unwrap();
    let bytes_regex = Regex::new(r"Bytes,\s+([0-9]+)").unwrap();
    let files = files_regex
        .captures(&output)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
        .unwrap_or(0);
    let bytes = bytes_regex
        .captures(&output)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
        .unwrap_or(0);
    (files, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use async_trait::async_trait;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    struct MockExecutor {
        outputs: Mutex<Vec<std::process::Output>>,
    }

    #[async_trait]
    impl ReplicationExecutor for MockExecutor {
        async fn run(&self, _command: &mut Command) -> std::io::Result<std::process::Output> {
            self.outputs
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| std::io::Error::other("no output"))
        }
    }

    fn success_status() -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            std::process::ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            std::process::ExitStatus::from_raw(0)
        }
    }

    #[tokio::test]
    async fn parses_backup_sync_report() {
        let temp = tempfile::tempdir().unwrap();
        let hls = temp.path().join("hls");
        fs::create_dir_all(&hls).unwrap();
        fs::write(hls.join("segment1.ts"), b"data").unwrap();
        let manifest_path = temp.path().join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::json!({ "segments": ["segment1.ts"] }).to_string(),
        )
        .unwrap();

        let output = std::process::Output {
            status: success_status(),
            stdout: b"Transferred:        1/1, 1024 Bytes".to_vec(),
            stderr: Vec::new(),
        };
        let executor = Arc::new(MockExecutor {
            outputs: Mutex::new(vec![output]),
        });

        let config = BackupCdnConfig {
            provider: "backblaze".into(),
            rclone_path: PathBuf::from("rclone"),
            source_paths: vec![hls.clone()],
            remote: "b2:vv_hls".into(),
            manifest_path: manifest_path.clone(),
            ttl_days: 7,
            switch_script: temp.path().join("switch.sh"),
            log_path: temp.path().join("backup.log"),
        };
        let coordinator = CdnCoordinator {
            provider: "backblaze".into(),
            primary_hostname: "cdn.voulezvous.tv".into(),
            backup_hostname: "backup.voulezvous.tv".into(),
            manifest_ttl_seconds: 60,
            segment_ttl_seconds: 3600,
            health_check_url: "https://voulezvous.tv/live.m3u8".into(),
        };
        let manager = BackupCdnManager::with_executor(config, coordinator, executor);
        let report = manager.sync_backup().await.unwrap();
        assert_eq!(report.files_uploaded, 1);
        assert_eq!(report.provider, "backblaze");
    }
}
