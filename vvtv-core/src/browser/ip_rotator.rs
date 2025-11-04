use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;
use tokio::time::sleep;
use tracing::warn;

use crate::config::IpRotationSection;

use super::error::{BrowserError, BrowserResult};
use super::error_handler::AutomationTelemetry;

#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn run(&self, program: &Path, args: &[String]) -> std::io::Result<ExitStatus>;
}

pub struct SystemCommandExecutor;

#[async_trait]
impl CommandExecutor for SystemCommandExecutor {
    async fn run(&self, program: &Path, args: &[String]) -> std::io::Result<ExitStatus> {
        Command::new(program).args(args).status().await
    }
}

pub struct IpRotator {
    binary: PathBuf,
    exit_nodes: Vec<String>,
    current_index: usize,
    cooldown: Duration,
    telemetry: Arc<AutomationTelemetry>,
    executor: Arc<dyn CommandExecutor>,
}

impl IpRotator {
    pub fn new(
        config: IpRotationSection,
        telemetry: Arc<AutomationTelemetry>,
    ) -> BrowserResult<Self> {
        let binary = PathBuf::from(config.tailscale_binary);
        let cooldown = Duration::from_secs(config.cooldown_seconds);
        let mut exit_nodes = config.exit_nodes;
        exit_nodes.retain(|node| !node.trim().is_empty());
        Ok(Self {
            binary,
            exit_nodes,
            current_index: 0,
            cooldown,
            telemetry,
            executor: Arc::new(SystemCommandExecutor),
        })
    }

    pub fn with_executor(mut self, executor: Arc<dyn CommandExecutor>) -> Self {
        self.executor = executor;
        self
    }

    pub fn has_exit_nodes(&self) -> bool {
        !self.exit_nodes.is_empty()
    }

    fn next_index(&mut self) -> Option<usize> {
        if self.exit_nodes.is_empty() {
            None
        } else {
            let index = self.current_index;
            self.current_index = (self.current_index + 1) % self.exit_nodes.len();
            Some(index)
        }
    }

    pub async fn rotate(&mut self) -> BrowserResult<Option<String>> {
        let Some(index) = self.next_index() else {
            return Ok(None);
        };
        let exit_node = self.exit_nodes[index].clone();
        let args = vec![
            "set".to_string(),
            "--exit-node".to_string(),
            exit_node.clone(),
        ];
        let status = self
            .executor
            .run(&self.binary, &args)
            .await
            .map_err(|err| BrowserError::IpRotation(err.to_string()))?;
        if !status.success() {
            return Err(BrowserError::IpRotation(format!(
                "tailscale returned status {status:?} for exit node {exit_node}"
            )));
        }
        if let Err(err) = self.telemetry.record_proxy_rotation(&exit_node) {
            warn!(error = %err, "failed to record proxy rotation telemetry");
        }
        sleep(self.cooldown).await;
        Ok(Some(exit_node))
    }
}

impl fmt::Debug for IpRotator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IpRotator")
            .field("binary", &self.binary)
            .field("exit_nodes", &self.exit_nodes)
            .field("current_index", &self.current_index)
            .field("cooldown", &self.cooldown)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::error_handler::AutomationTelemetry;
    use crate::config::IpRotationSection;
    use async_trait::async_trait;
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    type RecordedCalls = Arc<Mutex<Vec<(PathBuf, Vec<String>)>>>;

    struct RecordingExecutor {
        calls: RecordedCalls,
    }

    impl RecordingExecutor {
        fn build() -> (Arc<dyn CommandExecutor>, RecordedCalls) {
            let calls: RecordedCalls = Arc::new(Mutex::new(Vec::new()));
            let executor: Arc<dyn CommandExecutor> = Arc::new(Self {
                calls: Arc::clone(&calls),
            });
            (executor, calls)
        }
    }

    #[async_trait]
    impl CommandExecutor for RecordingExecutor {
        async fn run(&self, program: &Path, args: &[String]) -> std::io::Result<ExitStatus> {
            let entry = (program.to_path_buf(), args.to_vec());
            let mut guard = self.calls.lock().unwrap();
            guard.push(entry);
            Ok(ExitStatus::from_raw(0))
        }
    }

    #[tokio::test]
    async fn rotate_records_exit_and_telemetry() {
        let dir = tempdir().unwrap();
        let telemetry = Arc::new(
            AutomationTelemetry::new(
                dir.path().join("failures.log"),
                dir.path().join("metrics.sqlite"),
            )
            .unwrap(),
        );
        let (executor, calls) = RecordingExecutor::build();
        let mut rotator = IpRotator::new(
            IpRotationSection {
                enabled: true,
                tailscale_binary: "tailscale".into(),
                exit_nodes: vec!["fra.exit.ts.net".into(), "nyc.exit.ts.net".into()],
                cooldown_seconds: 0,
            },
            Arc::clone(&telemetry),
        )
        .unwrap()
        .with_executor(executor);

        let telemetry_clone = Arc::clone(&telemetry);
        let result = rotator.rotate().await.unwrap();
        assert_eq!(result.as_deref(), Some("fra.exit.ts.net"));

        let entries = calls.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].1,
            vec![
                String::from("set"),
                String::from("--exit-node"),
                String::from("fra.exit.ts.net"),
            ]
        );

        let conn = Connection::open(telemetry_clone.database_path()).unwrap();
        let rotation_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proxy_rotations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rotation_count, 1);
    }

    #[tokio::test]
    async fn rotate_returns_none_without_exit_nodes() {
        let dir = tempdir().unwrap();
        let telemetry = Arc::new(
            AutomationTelemetry::new(
                dir.path().join("failures.log"),
                dir.path().join("metrics.sqlite"),
            )
            .unwrap(),
        );
        let mut rotator = IpRotator::new(
            IpRotationSection {
                enabled: true,
                tailscale_binary: "tailscale".into(),
                exit_nodes: vec![],
                cooldown_seconds: 5,
            },
            telemetry,
        )
        .unwrap();
        assert!(rotator.rotate().await.unwrap().is_none());
    }
}
