use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use thiserror::Error;
use tokio::process::Command;

use super::replicator::{ReplicationExecutor, SystemReplicationExecutor};

#[derive(Debug, Error)]
pub enum EdgeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeLatencyRecord {
    pub region: String,
    pub target: String,
    pub latency_ms: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeBufferStatus {
    pub total_buffer_seconds: u64,
    pub newest_segment_age: u64,
    pub needs_reload: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeSummary {
    pub region: String,
    pub targets: Vec<String>,
    pub buffer_seconds: u64,
    pub reload_threshold_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SegmentSnapshot {
    pub path: PathBuf,
    pub age_seconds: u64,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct EdgeConfig {
    pub init_command: PathBuf,
    pub init_args: Vec<String>,
    pub region: String,
    pub latency_targets: Vec<String>,
    pub latency_log_path: PathBuf,
    pub heatmap_output_path: PathBuf,
    pub buffer_seconds: u64,
    pub reload_threshold_seconds: u64,
}

#[derive(Clone)]
pub struct EdgeOrchestrator {
    client: Client,
    config: EdgeConfig,
    executor: Arc<dyn ReplicationExecutor>,
}

impl EdgeOrchestrator {
    pub fn new(client: Client, config: EdgeConfig) -> Self {
        Self {
            client,
            config,
            executor: Arc::new(SystemReplicationExecutor),
        }
    }

    pub fn with_executor(
        client: Client,
        config: EdgeConfig,
        executor: Arc<dyn ReplicationExecutor>,
    ) -> Self {
        Self {
            client,
            config,
            executor,
        }
    }

    pub async fn initialize_node(&self) -> Result<(), EdgeError> {
        let mut command = Command::new(&self.config.init_command);
        let mut parts = vec![self.config.init_command.to_string_lossy().into_owned()];
        for arg in &self.config.init_args {
            command.arg(arg);
            parts.push(arg.clone());
        }
        let cmd_string = parts.join(" ");
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(EdgeError::Io)?;
        if !output.status.success() {
            return Err(EdgeError::CommandFailure {
                command: cmd_string,
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    pub async fn probe_latency(&self) -> Result<Vec<EdgeLatencyRecord>, EdgeError> {
        let mut records = Vec::new();
        for target in &self.config.latency_targets {
            let start = Instant::now();
            self.client.get(target).send().await?.error_for_status()?;
            let duration = start.elapsed();
            let record = EdgeLatencyRecord {
                region: self.config.region.clone(),
                target: target.clone(),
                latency_ms: duration.as_secs_f64() * 1000.0,
                timestamp: Utc::now(),
            };
            records.push(record);
        }
        self.append_latency_log(&records)?;
        self.update_heatmap(&records)?;
        Ok(records)
    }

    pub fn evaluate_buffer(&self, segments: &[SegmentSnapshot]) -> EdgeBufferStatus {
        let total_buffer_seconds: u64 = segments
            .iter()
            .map(|segment| segment.duration_seconds)
            .sum();
        let newest_segment_age = segments
            .iter()
            .map(|segment| segment.age_seconds)
            .min()
            .unwrap_or(u64::MAX);
        let needs_reload = total_buffer_seconds < self.config.buffer_seconds
            && newest_segment_age > self.config.reload_threshold_seconds;
        EdgeBufferStatus {
            total_buffer_seconds,
            newest_segment_age,
            needs_reload,
        }
    }

    pub fn summary(&self) -> EdgeSummary {
        EdgeSummary {
            region: self.config.region.clone(),
            targets: self.config.latency_targets.clone(),
            buffer_seconds: self.config.buffer_seconds,
            reload_threshold_seconds: self.config.reload_threshold_seconds,
        }
    }

    fn append_latency_log(&self, records: &[EdgeLatencyRecord]) -> Result<(), EdgeError> {
        if let Some(parent) = self.config.latency_log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.latency_log_path)?;
        for record in records {
            let json = serde_json::to_string(record)?;
            writeln!(file, "{}", json)?;
        }
        Ok(())
    }

    fn update_heatmap(&self, records: &[EdgeLatencyRecord]) -> Result<(), EdgeError> {
        if records.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.config.heatmap_output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut map: HashMap<String, Vec<f64>> = if self.config.heatmap_output_path.exists() {
            let data = fs::read_to_string(&self.config.heatmap_output_path)?;
            serde_json::from_str(&data)?
        } else {
            HashMap::new()
        };
        let entry = map.entry(self.config.region.clone()).or_default();
        for record in records {
            entry.push(record.latency_ms);
        }
        let serialized = serde_json::to_string_pretty(&map)?;
        fs::write(&self.config.heatmap_output_path, serialized)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_buffer_status() {
        let client = Client::builder().build().unwrap();
        let config = EdgeConfig {
            init_command: PathBuf::from("logline"),
            init_args: vec!["--init-node".into(), "--role=edge".into()],
            region: "eu-west".into(),
            latency_targets: vec!["https://voulezvous.tv/live.m3u8".into()],
            latency_log_path: PathBuf::from("/tmp/latency.log"),
            heatmap_output_path: PathBuf::from("/tmp/heatmap.json"),
            buffer_seconds: 15,
            reload_threshold_seconds: 20,
        };
        let orchestrator = EdgeOrchestrator::new(client, config);
        let segments = vec![
            SegmentSnapshot {
                path: PathBuf::from("segment1.ts"),
                age_seconds: 5,
                duration_seconds: 4,
            },
            SegmentSnapshot {
                path: PathBuf::from("segment2.ts"),
                age_seconds: 9,
                duration_seconds: 4,
            },
            SegmentSnapshot {
                path: PathBuf::from("segment3.ts"),
                age_seconds: 13,
                duration_seconds: 4,
            },
        ];
        let status = orchestrator.evaluate_buffer(&segments);
        assert!(!status.needs_reload);
        assert_eq!(status.total_buffer_seconds, 12);
        assert_eq!(status.newest_segment_age, 5);

        let degraded_segments = vec![SegmentSnapshot {
            path: PathBuf::from("segment4.ts"),
            age_seconds: 25,
            duration_seconds: 4,
        }];
        let status = orchestrator.evaluate_buffer(&degraded_segments);
        assert!(status.needs_reload);
    }
}
