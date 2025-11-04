pub mod failover;
pub mod watchdog;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use tokio::fs as async_fs;
use tokio::process::Command;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    BroadcasterConfig, PlayoutQueueStore, QueueEntry, QueueFilter, QueueItem, QueueMetrics,
    QueueSelectionPolicy, QueueStatus,
};

use self::failover::FailoverError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BroadcasterError {
    #[error("queue error: {0}")]
    Queue(#[from] crate::queue::QueueError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("command failed ({command}): {stderr}")]
    CommandFailure {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("failover error: {0}")]
    Failover(#[from] FailoverError),
}

#[async_trait::async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn run(&self, command: &mut Command) -> std::io::Result<std::process::Output>;
}

#[derive(Debug, Default)]
pub struct SystemCommandExecutor;

#[async_trait::async_trait]
impl CommandExecutor for SystemCommandExecutor {
    async fn run(&self, command: &mut Command) -> std::io::Result<std::process::Output> {
        command.output().await
    }
}

#[derive(Debug, Clone)]
pub struct BroadcasterPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub archive_dir: PathBuf,
    pub temp_dir: PathBuf,
}

pub struct Broadcaster {
    queue: PlayoutQueueStore,
    config: BroadcasterConfig,
    policy: QueueSelectionPolicy,
    paths: BroadcasterPaths,
    executor: Arc<dyn CommandExecutor>,
    last_entry: Mutex<Option<QueueEntry>>,
}

impl fmt::Debug for Broadcaster {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Broadcaster")
            .field("config", &self.config)
            .field("policy", &self.policy)
            .field("paths", &self.paths)
            .finish()
    }
}

impl Broadcaster {
    pub fn new(
        queue: PlayoutQueueStore,
        config: BroadcasterConfig,
        paths: BroadcasterPaths,
        executor: Option<Arc<dyn CommandExecutor>>,
    ) -> Self {
        let executor = executor.unwrap_or_else(|| Arc::new(SystemCommandExecutor));
        if let Some(parent) = paths.temp_dir.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let policy = QueueSelectionPolicy::from_queue_config(&config.queue);
        Self {
            queue,
            config,
            policy,
            paths,
            executor,
            last_entry: Mutex::new(None),
        }
    }

    pub async fn run_once(&self) -> Result<Option<BroadcasterEvent>, BroadcasterError> {
        let metrics = self.queue.metrics()?;
        self.ensure_emergency_buffer(&metrics).await?;

        let previous = { self.last_entry.lock().unwrap().clone() };
        let Some(mut current) = self.queue.begin_playback(&self.policy)? else {
            debug!("playout queue empty");
            return Ok(None);
        };

        let plan = self.compose_plan(previous.as_ref(), &current).await?;
        let started_at = current.play_started_at.unwrap_or_else(Utc::now);

        let mut command = Command::new(&self.paths.ffmpeg);
        for arg in &plan.args {
            command.arg(arg);
        }
        let output = self
            .executor
            .run(&mut command)
            .await
            .map_err(BroadcasterError::Io)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            self.queue.mark_playback_result(
                current.id,
                QueueStatus::Failed,
                None,
                Some(&stderr),
            )?;
            plan.cleanup().await;
            return Err(BroadcasterError::CommandFailure {
                command: format!("{} {}", self.paths.ffmpeg.display(), plan.args.join(" ")),
                status: output.status.code(),
                stderr,
            });
        }

        let finished_at = Utc::now();
        self.queue.mark_playback_result(
            current.id,
            QueueStatus::Played,
            current.duration_s,
            None,
        )?;
        plan.cleanup().await;

        let buffer = self.queue.metrics()?.buffer_duration_hours;
        {
            let mut guard = self.last_entry.lock().unwrap();
            current.status = QueueStatus::Played;
            current.play_finished_at = Some(finished_at);
            *guard = Some(current.clone());
        }

        info!(plan_id = %current.plan_id, buffer_hours = buffer, "playout completed");
        Ok(Some(BroadcasterEvent {
            queue_id: current.id,
            plan_id: current.plan_id,
            status: QueueStatus::Played,
            started_at,
            finished_at,
            buffer_hours_after: buffer,
            destination: self.config.rtmp.origin.clone(),
        }))
    }

    async fn compose_plan(
        &self,
        previous: Option<&QueueEntry>,
        current: &QueueEntry,
    ) -> Result<StreamingPlan, BroadcasterError> {
        let planner = TransitionPlanner { broadcaster: self };
        planner.compose(previous, current).await
    }

    async fn ensure_emergency_buffer(
        &self,
        metrics: &QueueMetrics,
    ) -> Result<(), BroadcasterError> {
        if metrics.buffer_duration_hours >= 1.0 {
            return Ok(());
        }
        let queued = self.queue.list(&QueueFilter {
            status: Some(QueueStatus::Queued),
            limit: Some(50),
        })?;
        if queued
            .iter()
            .any(|item| item.node_origin.as_deref() == Some("emergency-loop"))
        {
            return Ok(());
        }

        let assets = self.collect_emergency_assets()?;
        if assets.is_empty() {
            warn!(
                "no emergency assets found in {}",
                self.paths.archive_dir.display()
            );
            return Ok(());
        }
        let mut injected = 0;
        for asset in assets.into_iter().take(5) {
            let duration = self.probe_duration(&asset).await.ok();
            let plan_id = format!("emergency-{}", Uuid::new_v4());
            let item = QueueItem {
                plan_id,
                asset_path: asset.to_string_lossy().to_string(),
                duration_s: duration,
                curation_score: Some(0.2),
                priority: 1,
                node_origin: Some("emergency-loop".into()),
                content_kind: Some("music".into()),
            };
            if self.queue.enqueue(&item).is_ok() {
                injected += 1;
            }
        }
        if injected > 0 {
            info!(count = injected, "emergency loop injected to queue");
        }
        Ok(())
    }

    fn collect_emergency_assets(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        if self.paths.archive_dir.exists() {
            for entry in fs::read_dir(&self.paths.archive_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|ext| ext == "mp4").unwrap_or(false) {
                    entries.push(path);
                }
            }
        }
        entries.sort();
        entries.reverse();
        Ok(entries)
    }

    async fn probe_duration(&self, asset: &Path) -> Result<i64, BroadcasterError> {
        let args = vec![
            "-v".to_string(),
            "error".to_string(),
            "-show_entries".to_string(),
            "format=duration".to_string(),
            "-of".to_string(),
            "default=noprint_wrappers=1:nokey=1".to_string(),
            asset.to_string_lossy().to_string(),
        ];
        let output = self.run_external(&self.paths.ffprobe, &args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(BroadcasterError::CommandFailure {
                command: format!("{} {}", self.paths.ffprobe.display(), args.join(" ")),
                status: output.status.code(),
                stderr,
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let duration = stdout
            .trim()
            .parse::<f64>()
            .ok()
            .map(|value| value.round() as i64)
            .unwrap_or(0);
        Ok(duration)
    }

    async fn run_external(
        &self,
        program: &Path,
        args: &[String],
    ) -> Result<std::process::Output, BroadcasterError> {
        let mut command = Command::new(program);
        for arg in args {
            command.arg(arg);
        }
        self.executor
            .run(&mut command)
            .await
            .map_err(BroadcasterError::Io)
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            self.config.ffmpeg.log_level.clone(),
        ];
        args.push("-thread_queue_size".to_string());
        args.push(self.config.ffmpeg.thread_queue_size.to_string());
        if !self.config.ffmpeg.stats_period.is_empty() {
            args.push("-stats_period".to_string());
            args.push(self.config.ffmpeg.stats_period.clone());
        }
        args
    }
}

#[derive(Debug, Clone)]
pub struct BroadcasterEvent {
    pub queue_id: i64,
    pub plan_id: String,
    pub status: QueueStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub buffer_hours_after: f64,
    pub destination: String,
}

struct TransitionPlanner<'a> {
    broadcaster: &'a Broadcaster,
}

impl<'a> TransitionPlanner<'a> {
    async fn compose(
        &self,
        previous: Option<&QueueEntry>,
        current: &QueueEntry,
    ) -> Result<StreamingPlan, BroadcasterError> {
        if let Some(prev) = previous {
            if let Some(plan) = self.try_crossfade(prev, current).await? {
                return Ok(plan);
            }
        }
        Ok(self.direct_plan(current))
    }

    fn direct_plan(&self, current: &QueueEntry) -> StreamingPlan {
        let mut args = self.broadcaster.base_args();
        args.push("-re".to_string());
        args.push("-i".to_string());
        args.push(current.asset_path.clone());
        args.push("-c".to_string());
        args.push("copy".to_string());
        args.push("-f".to_string());
        args.push("flv".to_string());
        args.push(self.broadcaster.config.rtmp.origin.clone());
        StreamingPlan::new(args, vec![])
    }

    async fn ensure_temp_dir(&self) -> Result<PathBuf, BroadcasterError> {
        let dir = &self.broadcaster.paths.temp_dir;
        async_fs::create_dir_all(dir).await?;
        Ok(dir.clone())
    }

    async fn try_crossfade(
        &self,
        previous: &QueueEntry,
        current: &QueueEntry,
    ) -> Result<Option<StreamingPlan>, BroadcasterError> {
        let prev_duration = previous.duration_s.unwrap_or(0);
        let current_duration = current.duration_s.unwrap_or(0);
        if prev_duration < 2 || current_duration < 2 {
            return Ok(None);
        }
        let prev_offset = (prev_duration as f64 - 0.4).max(0.0);
        if prev_offset <= 0.0 {
            return Ok(None);
        }
        let temp_dir = self.ensure_temp_dir().await?;
        let transition_path =
            temp_dir.join(format!("{}_to_{}_transition.mp4", previous.id, current.id));
        let trimmed_path = temp_dir.join(format!("{}_trimmed.mp4", current.id));
        let playlist_path = temp_dir.join(format!("{}_playlist.txt", current.id));

        let filter = format!(
            "[0:v]trim=start={prev_offset},setpts=PTS-STARTPTS[v0];\
             [0:a]atrim=start={prev_offset},asetpts=PTS-STARTPTS[a0];\
             [1:v]trim=end=0.4,setpts=PTS-STARTPTS[v1];\
             [1:a]atrim=end=0.4,asetpts=PTS-STARTPTS[a1];\
             [v0][v1]xfade=transition=fade:duration=0.4:offset=0[vout];\
             [a0][a1]acrossfade=d=0.4:c1=sin:c2=sin[aout]"
        );

        let transition_args = vec![
            "-y".to_string(),
            "-i".to_string(),
            previous.asset_path.clone(),
            "-i".to_string(),
            current.asset_path.clone(),
            "-filter_complex".to_string(),
            filter,
            "-map".to_string(),
            "[vout]".to_string(),
            "-map".to_string(),
            "[aout]".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "veryfast".to_string(),
            "-crf".to_string(),
            "20".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-ar".to_string(),
            "48000".to_string(),
            "-ac".to_string(),
            "2".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            transition_path.to_string_lossy().to_string(),
        ];
        self.broadcaster
            .run_external(&self.broadcaster.paths.ffmpeg, &transition_args)
            .await?;

        let trim_args = vec![
            "-y".to_string(),
            "-ss".to_string(),
            "0.4".to_string(),
            "-i".to_string(),
            current.asset_path.clone(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "veryfast".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-ar".to_string(),
            "48000".to_string(),
            "-ac".to_string(),
            "2".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            trimmed_path.to_string_lossy().to_string(),
        ];
        self.broadcaster
            .run_external(&self.broadcaster.paths.ffmpeg, &trim_args)
            .await?;

        let playlist_content = format!(
            "file '{}'\nfile '{}'\n",
            transition_path.display(),
            trimmed_path.display()
        );
        async_fs::write(&playlist_path, playlist_content).await?;

        let mut args = self.broadcaster.base_args();
        args.push("-re".to_string());
        args.push("-f".to_string());
        args.push("concat".to_string());
        args.push("-safe".to_string());
        args.push("0".to_string());
        args.push("-i".to_string());
        args.push(playlist_path.to_string_lossy().to_string());
        args.push("-c".to_string());
        args.push("copy".to_string());
        args.push("-f".to_string());
        args.push("flv".to_string());
        args.push(self.broadcaster.config.rtmp.origin.clone());

        Ok(Some(StreamingPlan::new(
            args,
            vec![transition_path, trimmed_path, playlist_path],
        )))
    }
}

struct StreamingPlan {
    args: Vec<String>,
    cleanup: Vec<PathBuf>,
}

impl StreamingPlan {
    fn new(args: Vec<String>, cleanup: Vec<PathBuf>) -> Self {
        Self { args, cleanup }
    }

    async fn cleanup(&self) {
        for path in &self.cleanup {
            if let Err(error) = async_fs::remove_file(path).await {
                debug!(path = %path.display(), %error, "failed to remove temp artifact");
            }
        }
    }
}
