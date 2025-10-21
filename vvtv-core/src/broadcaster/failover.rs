use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tracing::info;

use super::{CommandExecutor, SystemCommandExecutor};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FailoverError {
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct FailoverManager {
    executor: Arc<dyn CommandExecutor>,
    rsync_path: PathBuf,
    archive_root: PathBuf,
}

impl fmt::Debug for FailoverManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FailoverManager")
            .field("rsync_path", &self.rsync_path)
            .field("archive_root", &self.archive_root)
            .finish()
    }
}

impl FailoverManager {
    pub fn new(
        rsync_path: PathBuf,
        archive_root: PathBuf,
        executor: Option<Arc<dyn CommandExecutor>>,
    ) -> Self {
        let executor = executor.unwrap_or_else(|| Arc::new(SystemCommandExecutor::default()));
        Self {
            executor,
            rsync_path,
            archive_root,
        }
    }

    pub async fn sync_ready(&self, source: &Path, destination: &str) -> Result<(), FailoverError> {
        let mut command = Command::new(&self.rsync_path);
        command
            .arg("-az")
            .arg("--delete")
            .arg(source)
            .arg(destination);
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(FailoverError::Io)?;
        if !output.status.success() {
            return Err(FailoverError::CommandFailure {
                command: format!(
                    "{} -az --delete {} {}",
                    self.rsync_path.display(),
                    source.display(),
                    destination
                ),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        info!(target = destination, "failover sync completed");
        Ok(())
    }

    pub async fn record_archive(
        &self,
        ffmpeg_path: &Path,
        input: &str,
        duration: Duration,
        output: &Path,
    ) -> Result<(), FailoverError> {
        let mut command = Command::new(ffmpeg_path);
        command
            .arg("-hide_banner")
            .arg("-y")
            .arg("-i")
            .arg(input)
            .arg("-t")
            .arg(format!("{}", duration.as_secs()))
            .arg("-c:v")
            .arg("copy")
            .arg("-c:a")
            .arg("copy")
            .arg(output);
        let output_data = self
            .executor
            .run(&mut command)
            .await
            .map_err(FailoverError::Io)?;
        if !output_data.status.success() {
            return Err(FailoverError::CommandFailure {
                command: format!(
                    "{} -i {} -t {} -c:v copy -c:a copy {}",
                    ffmpeg_path.display(),
                    input,
                    duration.as_secs(),
                    output.display()
                ),
                status: output_data.status.code(),
                stderr: String::from_utf8_lossy(&output_data.stderr).to_string(),
            });
        }
        info!(target = %output.display(), "failover archive recorded");
        Ok(())
    }

    pub async fn promote_failover(&self, script: &Path) -> Result<(), FailoverError> {
        let mut command = Command::new(script);
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(FailoverError::Io)?;
        if !output.status.success() {
            return Err(FailoverError::CommandFailure {
                command: script.display().to_string(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        info!(script = %script.display(), "failover promotion executed");
        Ok(())
    }

    pub fn archive_root(&self) -> &Path {
        &self.archive_root
    }
}
