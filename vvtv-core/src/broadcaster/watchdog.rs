use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use tokio::process::Command;
use tracing::warn;

use crate::config::WatchdogSection;
use crate::{PlayoutQueueStore, QueueMetrics};

use super::{BroadcasterPaths, CommandExecutor, SystemCommandExecutor};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatchdogError {
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("queue error: {0}")]
    Queue(#[from] crate::queue::QueueError),
}

#[derive(Debug, Clone)]
pub struct WatchdogReport {
    pub metrics: QueueMetrics,
    pub actions: Vec<WatchdogAction>,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchdogAction {
    RestartEncoder,
    RestartNginx,
    InjectEmergencyLoop,
    PauseDownloads,
    Escalate(String),
}

pub struct Watchdog {
    queue: PlayoutQueueStore,
    config: WatchdogSection,
    paths: BroadcasterPaths,
    executor: Arc<dyn CommandExecutor>,
    restart_history: Mutex<VecDeque<DateTime<Utc>>>,
    scripts_dir: Option<PathBuf>,
    rtmp_url: String,
}

impl fmt::Debug for Watchdog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Watchdog")
            .field("config", &self.config)
            .field("paths", &self.paths)
            .field("rtmp_url", &self.rtmp_url)
            .finish()
    }
}

impl Watchdog {
    pub fn new(
        queue: PlayoutQueueStore,
        config: WatchdogSection,
        paths: BroadcasterPaths,
        scripts_dir: Option<PathBuf>,
        executor: Option<Arc<dyn CommandExecutor>>,
        rtmp_url: String,
    ) -> Self {
        let executor = executor.unwrap_or_else(|| Arc::new(SystemCommandExecutor));
        Self {
            queue,
            config,
            paths,
            executor,
            restart_history: Mutex::new(VecDeque::new()),
            scripts_dir,
            rtmp_url,
        }
    }

    pub async fn evaluate(&self) -> Result<WatchdogReport, WatchdogError> {
        let metrics = self.queue.metrics()?;
        let mut actions = Vec::new();
        let mut observations = Vec::new();

        if metrics.buffer_duration_hours < 1.0 {
            observations.push(format!(
                "buffer abaixo de 1h ({:.2}h)",
                metrics.buffer_duration_hours
            ));
            actions.push(WatchdogAction::InjectEmergencyLoop);
            actions.push(WatchdogAction::PauseDownloads);
        } else if metrics.buffer_duration_hours < 2.0 {
            observations.push(format!(
                "buffer crítico ({:.2}h)",
                metrics.buffer_duration_hours
            ));
            actions.push(WatchdogAction::InjectEmergencyLoop);
        }

        if !self.check_stream_health().await? {
            observations.push("ffprobe detectou stream inativo".into());
            actions.push(WatchdogAction::RestartEncoder);
            if self.config.restart_on_freeze {
                actions.push(WatchdogAction::RestartNginx);
            }
            let mut history = self.restart_history.lock().unwrap();
            let now = Utc::now();
            history.push_back(now);
            let window = Duration::minutes(5);
            while history
                .front()
                .map(|timestamp| *timestamp < now - window)
                .unwrap_or(false)
            {
                history.pop_front();
            }
            if history.len() as u32 > self.config.restart_max_attempts {
                actions.push(WatchdogAction::Escalate(
                    "limite de restart excedido em 5 minutos".into(),
                ));
            }
        }

        Ok(WatchdogReport {
            metrics,
            actions,
            observations,
        })
    }

    async fn check_stream_health(&self) -> Result<bool, WatchdogError> {
        let mut command = Command::new(&self.paths.ffprobe);
        command
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=codec_name")
            .arg("-of")
            .arg("csv=p=0")
            .arg(&self.rtmp_url);
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(WatchdogError::Io)?;
        if !output.status.success() {
            warn!(
                status = output.status.code(),
                "ffprobe returned error while probing stream"
            );
            return Ok(false);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    pub async fn apply(&self, action: &WatchdogAction) -> Result<(), WatchdogError> {
        match action {
            WatchdogAction::RestartEncoder => {
                if let Some(script) = self.script_path("restart_encoder.sh") {
                    self.run_script(&script).await?
                }
            }
            WatchdogAction::RestartNginx => {
                if let Some(script) = self.script_path("restart_nginx.sh") {
                    self.run_script(&script).await?
                }
            }
            WatchdogAction::PauseDownloads => {
                if let Some(script) = self.script_path("pause_downloads.sh") {
                    self.run_script(&script).await?
                }
            }
            WatchdogAction::InjectEmergencyLoop | WatchdogAction::Escalate(_) => {
                // handled externally
            }
        }
        Ok(())
    }

    fn script_path(&self, name: &str) -> Option<PathBuf> {
        self.scripts_dir
            .as_ref()
            .map(|dir| dir.join(name))
            .filter(|path| path.exists())
    }

    async fn run_script(&self, script: &Path) -> Result<(), WatchdogError> {
        let mut command = Command::new(script);
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(WatchdogError::Io)?;
        if !output.status.success() {
            return Err(WatchdogError::CommandFailure {
                command: script.display().to_string(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }
}
