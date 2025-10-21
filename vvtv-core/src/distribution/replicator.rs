use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use regex::Regex;
use serde::Serialize;
use thiserror::Error;
use tokio::process::Command;
use walkdir::WalkDir;

use chrono::Utc;
use serde_json::json;

#[async_trait]
pub trait ReplicationExecutor: Send + Sync {
    async fn run(&self, command: &mut Command) -> std::io::Result<std::process::Output>;
}

#[derive(Debug, Default, Clone)]
pub struct SystemReplicationExecutor;

#[async_trait]
impl ReplicationExecutor for SystemReplicationExecutor {
    async fn run(&self, command: &mut Command) -> std::io::Result<std::process::Output> {
        command.output().await
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplicationSyncReport {
    pub path: PathBuf,
    pub duration_ms: u128,
    pub bytes_transferred: u64,
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplicationCheckReport {
    pub path: PathBuf,
    pub differences: u64,
    pub total_files: u64,
    pub drift_percent: f64,
    pub command: String,
    pub triggered_failover: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplicationReport {
    pub syncs: Vec<ReplicationSyncReport>,
    pub check: ReplicationCheckReport,
}

#[derive(Debug, Error)]
pub enum ReplicationError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct ReplicationManager {
    rclone_path: PathBuf,
    sync_paths: Vec<PathBuf>,
    remote: String,
    bandwidth_limit_mbps: Option<u64>,
    check_threshold_percent: f64,
    log_path: PathBuf,
    failover_script: Option<PathBuf>,
    executor: Arc<dyn ReplicationExecutor>,
}

impl fmt::Debug for ReplicationManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplicationManager")
            .field("rclone_path", &self.rclone_path)
            .field("sync_paths", &self.sync_paths)
            .field("remote", &self.remote)
            .field("bandwidth_limit_mbps", &self.bandwidth_limit_mbps)
            .field("check_threshold_percent", &self.check_threshold_percent)
            .field("log_path", &self.log_path)
            .field("failover_script", &self.failover_script)
            .finish()
    }
}

impl ReplicationManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rclone_path: PathBuf,
        sync_paths: Vec<PathBuf>,
        remote: String,
        bandwidth_limit_mbps: Option<u64>,
        check_threshold_percent: f64,
        log_path: PathBuf,
        failover_script: Option<PathBuf>,
        executor: Option<Arc<dyn ReplicationExecutor>>,
    ) -> Self {
        let executor = executor.unwrap_or_else(|| Arc::new(SystemReplicationExecutor));
        Self {
            rclone_path,
            sync_paths,
            remote,
            bandwidth_limit_mbps,
            check_threshold_percent,
            log_path,
            failover_script,
            executor,
        }
    }

    pub async fn run_cycle(&self) -> Result<ReplicationReport, ReplicationError> {
        let mut syncs = Vec::with_capacity(self.sync_paths.len());
        for path in &self.sync_paths {
            syncs.push(self.sync_path(path).await?);
        }
        let mut check = self.verify_paths().await?;
        if check.drift_percent > self.check_threshold_percent {
            if let Some(script) = &self.failover_script {
                self.trigger_failover(script).await?;
                check.triggered_failover = true;
            }
        }
        self.append_log(&syncs, &check)?;
        Ok(ReplicationReport { syncs, check })
    }

    async fn sync_path(&self, path: &Path) -> Result<ReplicationSyncReport, ReplicationError> {
        let mut command = Command::new(&self.rclone_path);
        let mut parts = vec![self.rclone_path.to_string_lossy().into_owned()];
        command.arg("sync");
        parts.push("sync".to_string());
        command.arg(path);
        parts.push(path.to_string_lossy().into_owned());
        let remote_path = build_remote_path(&self.remote, path);
        command.arg(remote_path.as_str());
        parts.push(remote_path.clone());
        command.arg("--fast-list");
        parts.push("--fast-list".to_string());
        command.arg("--transfers");
        parts.push("--transfers".to_string());
        command.arg("4");
        parts.push("4".to_string());
        if let Some(limit) = self.bandwidth_limit_mbps {
            command.arg("--bwlimit");
            parts.push("--bwlimit".to_string());
            let limit_arg = format!("{}M", limit);
            command.arg(limit_arg.as_str());
            parts.push(limit_arg);
        }
        let cmd_string = parts.join(" ");
        let start = Instant::now();
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(ReplicationError::Io)?;
        if !output.status.success() {
            return Err(ReplicationError::CommandFailure {
                command: cmd_string,
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        let duration = start.elapsed();
        let bytes = extract_transferred_bytes(&output.stdout);
        Ok(ReplicationSyncReport {
            path: path.to_path_buf(),
            duration_ms: duration.as_millis(),
            bytes_transferred: bytes,
            command: cmd_string,
        })
    }

    async fn verify_paths(&self) -> Result<ReplicationCheckReport, ReplicationError> {
        let mut total_differences = 0u64;
        let mut total_files = 0u64;
        let mut last_command = String::new();
        for path in &self.sync_paths {
            let mut command = Command::new(&self.rclone_path);
            let mut parts = vec![self.rclone_path.to_string_lossy().into_owned()];
            command.arg("check");
            parts.push("check".to_string());
            command.arg(path);
            parts.push(path.to_string_lossy().into_owned());
            let remote_path = build_remote_path(&self.remote, path);
            command.arg(remote_path.as_str());
            parts.push(remote_path);
            command.arg("--size-only");
            parts.push("--size-only".to_string());
            command.arg("--one-way");
            parts.push("--one-way".to_string());
            let cmd_string = parts.join(" ");
            let output = self
                .executor
                .run(&mut command)
                .await
                .map_err(ReplicationError::Io)?;
            if !output.status.success() {
                return Err(ReplicationError::CommandFailure {
                    command: cmd_string,
                    status: output.status.code(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
            total_differences += parse_differences(&output.stdout);
            total_files += count_local_files(path);
            last_command = cmd_string;
        }
        let drift_percent = if total_files == 0 {
            0.0
        } else {
            (total_differences as f64 / total_files as f64) * 100.0
        };
        Ok(ReplicationCheckReport {
            path: self.sync_paths.first().cloned().unwrap_or_default(),
            differences: total_differences,
            total_files,
            drift_percent,
            command: last_command,
            triggered_failover: false,
        })
    }

    async fn trigger_failover(&self, script: &Path) -> Result<(), ReplicationError> {
        let mut command = Command::new(script);
        let cmd_string = script.to_string_lossy().into_owned();
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(ReplicationError::Io)?;
        if !output.status.success() {
            return Err(ReplicationError::CommandFailure {
                command: cmd_string,
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    fn append_log(
        &self,
        syncs: &[ReplicationSyncReport],
        check: &ReplicationCheckReport,
    ) -> Result<(), ReplicationError> {
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let record = serde_json::to_string(&json!({
            "timestamp": Utc::now().to_rfc3339(),
            "syncs": syncs,
            "check": check,
        }))?;
        writeln!(file, "{}", record)?;
        Ok(())
    }
}

pub(crate) fn build_remote_path(remote: &str, path: &Path) -> String {
    let mut remote = remote.trim_end_matches('/').to_string();
    if !remote.ends_with('/') {
        remote.push('/');
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    remote.push_str(name);
    remote
}

fn extract_transferred_bytes(stdout: &[u8]) -> u64 {
    let output = String::from_utf8_lossy(stdout);
    let regex = Regex::new(r"Transferred:\s+([0-9]+)").unwrap();
    if let Some(captures) = regex.captures(&output) {
        captures
            .get(1)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0)
    } else {
        0
    }
}

fn parse_differences(stdout: &[u8]) -> u64 {
    let output = String::from_utf8_lossy(stdout);
    let regex = Regex::new(r"(?i)([0-9]+) differences?").unwrap();
    if let Some(captures) = regex.captures(&output) {
        captures
            .get(1)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0)
    } else {
        0
    }
}

fn count_local_files(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

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

    #[tokio::test]
    async fn parse_rclone_cycle() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("hls");
        std::fs::create_dir_all(&source_path).unwrap();
        std::fs::write(source_path.join("segment1.ts"), b"data").unwrap();

        let sync_output = std::process::Output {
            status: success_status(),
            stdout: b"Transferred:    1024 / 1024 Bytes".to_vec(),
            stderr: Vec::new(),
        };
        let check_output = std::process::Output {
            status: success_status(),
            stdout: b"2024/06/01 12:00:00 NOTICE: 0 differences found".to_vec(),
            stderr: Vec::new(),
        };
        let outputs = vec![check_output, sync_output];
        let executor = Arc::new(MockExecutor {
            outputs: Mutex::new(outputs),
        });

        let manager = ReplicationManager::new(
            PathBuf::from("rclone"),
            vec![source_path.clone()],
            "remote".into(),
            Some(64),
            5.0,
            temp.path().join("log.jsonl"),
            None,
            Some(executor),
        );

        let report = manager.run_cycle().await.unwrap();
        assert_eq!(report.syncs.len(), 1);
        assert_eq!(report.syncs[0].bytes_transferred, 1024);
        assert_eq!(report.check.differences, 0);
        assert!(report.check.drift_percent < 0.01);
        assert!(!report.check.triggered_failover);
    }

    #[tokio::test]
    async fn triggers_failover_on_drift() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("storage");
        std::fs::create_dir_all(&source_path).unwrap();
        std::fs::write(source_path.join("a.ts"), b"data").unwrap();
        std::fs::write(source_path.join("b.ts"), b"data").unwrap();

        let sync_output = std::process::Output {
            status: success_status(),
            stdout: b"Transferred:    2048 / 2048 Bytes".to_vec(),
            stderr: Vec::new(),
        };
        let check_output = std::process::Output {
            status: success_status(),
            stdout: b"2024/06/01 12:00:00 NOTICE: 1 differences found".to_vec(),
            stderr: Vec::new(),
        };
        let failover_output = std::process::Output {
            status: success_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let outputs = vec![failover_output, check_output, sync_output];
        let executor = Arc::new(MockExecutor {
            outputs: Mutex::new(outputs),
        });

        let manager = ReplicationManager::new(
            PathBuf::from("rclone"),
            vec![source_path.clone()],
            "remote".into(),
            None,
            20.0,
            temp.path().join("log.jsonl"),
            Some(temp.path().join("failover.sh")),
            Some(executor),
        );

        let report = manager.run_cycle().await.unwrap();
        assert!(report.check.triggered_failover);
    }
}
