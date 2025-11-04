use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tokio::fs;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tracing::warn;

use super::automation::{BrowserLauncher, LaunchOverrides};
use super::error::{BrowserError, BrowserResult};
use super::error_handler::{telemetry_run, AutomationTelemetry};
use super::metrics::BrowserMetrics;
use super::pbd::{CollectOptions, PbdOutcome, PlayBeforeDownload};
use super::retry::RetryPolicy;
use crate::sqlite::configure_connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum SmokeMode {
    #[default]
    Headless,
    Headed,
}

#[derive(Debug, Clone)]
pub struct SmokeTestOptions {
    pub mode: SmokeMode,
    pub capture_screenshot: bool,
    pub screenshot_dir: Option<PathBuf>,
    pub record_video: bool,
    pub record_duration: Duration,
    pub session_recorder: Option<Arc<SessionRecorder>>,
}

impl Default for SmokeTestOptions {
    fn default() -> Self {
        Self {
            mode: SmokeMode::Headless,
            capture_screenshot: true,
            screenshot_dir: Some(PathBuf::from("artifacts/qa/screenshots")),
            record_video: false,
            record_duration: Duration::from_secs(30),
            session_recorder: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SmokeTestResult {
    pub url: String,
    pub success: bool,
    pub capture: Option<PbdOutcome>,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
    pub metrics: BrowserMetrics,
    pub screenshot_path: Option<PathBuf>,
    pub video_path: Option<PathBuf>,
    pub attempts: usize,
}

#[derive(Debug, Clone)]
pub struct QaScriptResult {
    pub scenario: QaScenario,
    pub result: SmokeTestResult,
}

#[derive(Debug, Clone)]
pub enum QaScenario {
    Smoke {
        url: String,
        options: SmokeTestOptions,
    },
}

pub struct BrowserQaRunner {
    launcher: Arc<BrowserLauncher>,
    playbook: PlayBeforeDownload,
    telemetry: Arc<AutomationTelemetry>,
    retry_policy: RetryPolicy,
    ip_rotator: Option<Arc<tokio::sync::Mutex<super::ip_rotator::IpRotator>>>,
}

impl BrowserQaRunner {
    pub fn new(launcher: BrowserLauncher) -> BrowserResult<Self> {
        let telemetry = launcher.telemetry();
        let retry_policy = launcher.retry_policy();
        let ip_rotator = launcher.ip_rotator();
        let config = Arc::new(launcher.config().clone());
        let playbook = PlayBeforeDownload::new(config);
        Ok(Self {
            launcher: Arc::new(launcher),
            playbook,
            telemetry,
            retry_policy,
            ip_rotator,
        })
    }

    pub async fn run_smoke(
        &self,
        url: &str,
        options: SmokeTestOptions,
    ) -> BrowserResult<SmokeTestResult> {
        let start = Instant::now();
        let overrides = LaunchOverrides {
            headless: Some(matches!(options.mode, SmokeMode::Headless)),
        };
        let automation = self.launcher.launch_with_overrides(overrides).await?;
        let metrics_handle = automation.metrics_handle();
        let proxy = automation.proxy().map(|value| value.to_string());
        let collect_options = CollectOptions {
            capture_screenshot: options.capture_screenshot,
        };
        let telemetry = Arc::clone(&self.telemetry);
        let ip_rotator = self.ip_rotator.clone();
        let playbook = &self.playbook;
        let automation_ref = &automation;
        let mut warnings = Vec::new();
        let retry_policy = self.retry_policy.clone();
        let metrics_handle_clone = metrics_handle.clone();
        let collect_options_shared = collect_options.clone();
        let proxy_for_run = proxy.clone();
        let url_string = url.to_string();
        let url_shared = Arc::new(url_string.clone());

        let operation = move || {
            let telemetry = Arc::clone(&telemetry);
            let ip_rotator = ip_rotator.clone();
            let metrics_handle = metrics_handle_clone.clone();
            let collect_options = collect_options_shared.clone();
            let proxy_value = proxy_for_run.clone();
            let url_for_run = Arc::clone(&url_shared);
            let url_for_attempts = Arc::clone(&url_shared);
            let retry_policy = retry_policy.clone();
            async move {
                retry_policy
                    .run(
                        url_for_run.as_str(),
                        proxy_value.clone(),
                        telemetry,
                        ip_rotator,
                        metrics_handle,
                        move |_| {
                            let collect_options_inner = collect_options.clone();
                            let url_attempt = Arc::clone(&url_for_attempts);
                            async move {
                                playbook
                                    .collect_with_options(
                                        automation_ref,
                                        url_attempt.as_str(),
                                        collect_options_inner,
                                    )
                                    .await
                            }
                        },
                    )
                    .await
            }
        };

        let (result, video_path_opt) = if options.record_video {
            if let Some(recorder) = options.session_recorder.clone() {
                let label = format!("smoke-{}", Utc::now().format("%Y%m%d%H%M%S"));
                let (outcome, path, recorder_warnings) = recorder
                    .record_during(&label, options.record_duration, operation)
                    .await?;
                warnings.extend(recorder_warnings);
                (outcome, Some(path))
            } else {
                (operation().await, None)
            }
        } else {
            (operation().await, None)
        };

        let retry_outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                let metrics_snapshot = automation.metrics();
                let duration_ms = start.elapsed().as_millis() as i64;
                if let Err(log_err) = telemetry_run(
                    &self.telemetry,
                    "smoke",
                    &url_string,
                    false,
                    duration_ms,
                    None,
                    video_path_opt.clone(),
                    metrics_snapshot.proxy_rotations,
                ) {
                    warn!(error = %log_err, "failed to log failed smoke run");
                }
                automation.shutdown().await?;
                return Err(err);
            }
        };

        let mut screenshot_path = None;
        if options.capture_screenshot {
            if let Some(bytes) = retry_outcome.result.screenshot.clone() {
                let dir = options
                    .screenshot_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("artifacts/qa/screenshots"));
                fs::create_dir_all(&dir).await?;
                let file_name = format!(
                    "smoke_{}_{}.png",
                    Utc::now().format("%Y%m%d%H%M%S"),
                    retry_outcome.attempts
                );
                let path = dir.join(file_name);
                fs::write(&path, bytes).await?;
                screenshot_path = Some(path);
            } else {
                warnings.push("screenshot capture returned empty data".to_string());
            }
        }

        let metrics_snapshot = automation.metrics();
        let duration = start.elapsed();
        if let Err(log_err) = telemetry_run(
            &self.telemetry,
            "smoke",
            &url_string,
            true,
            duration.as_millis() as i64,
            screenshot_path.clone(),
            video_path_opt.clone(),
            metrics_snapshot.proxy_rotations,
        ) {
            warn!(error = %log_err, "failed to persist smoke telemetry");
        }
        automation.shutdown().await?;

        Ok(SmokeTestResult {
            url: url_string,
            success: true,
            capture: Some(retry_outcome.result.outcome),
            duration_ms: duration.as_millis() as u64,
            warnings,
            metrics: metrics_snapshot,
            screenshot_path,
            video_path: video_path_opt,
            attempts: retry_outcome.attempts,
        })
    }

    pub async fn run(&self, scenario: QaScenario) -> BrowserResult<QaScriptResult> {
        match scenario {
            QaScenario::Smoke { url, options } => {
                let result = self.run_smoke(&url, options.clone()).await?;
                Ok(QaScriptResult {
                    scenario: QaScenario::Smoke { url, options },
                    result,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionRecorderConfig {
    pub ffmpeg_path: PathBuf,
    pub display: Option<String>,
    pub extra_args: Vec<String>,
    pub output_dir: PathBuf,
    pub allow_placeholder: bool,
}

impl Default for SessionRecorderConfig {
    fn default() -> Self {
        Self {
            ffmpeg_path: PathBuf::from("ffmpeg"),
            display: Some(":0.0".into()),
            extra_args: vec!["-vcodec".into(), "libx264".into()],
            output_dir: PathBuf::from("artifacts/qa/video"),
            allow_placeholder: true,
        }
    }
}

#[derive(Debug)]
pub struct SessionRecorder {
    config: SessionRecorderConfig,
}

#[derive(Debug)]
pub struct SessionRecordingHandle {
    join: Option<JoinHandle<std::io::Result<std::process::ExitStatus>>>,
    output: PathBuf,
    placeholder: bool,
}

impl SessionRecorder {
    pub fn new(config: SessionRecorderConfig) -> Self {
        Self { config }
    }

    pub async fn record_during<F, Fut, T>(
        &self,
        label: &str,
        duration: Duration,
        work: F,
    ) -> BrowserResult<(BrowserResult<T>, PathBuf, Vec<String>)>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = BrowserResult<T>>,
    {
        let handle = self.start(label, duration).await?;
        let result = work().await;
        let (path, warnings) = self.finish(handle).await?;
        Ok((result, path, warnings))
    }

    async fn start(
        &self,
        label: &str,
        duration: Duration,
    ) -> BrowserResult<SessionRecordingHandle> {
        fs::create_dir_all(&self.config.output_dir).await?;
        let timestamp = Utc::now().format("%Y%m%d%H%M%S");
        let filename = format!("{}_{}.mp4", label, timestamp);
        let output = self.config.output_dir.join(filename);
        let ffmpeg_exists = tokio::fs::metadata(&self.config.ffmpeg_path).await.is_ok();
        if !ffmpeg_exists {
            return Ok(SessionRecordingHandle {
                join: None,
                output,
                placeholder: true,
            });
        }
        let mut args = vec!["-y".into()];
        if let Some(display) = &self.config.display {
            args.extend(vec![
                "-f".into(),
                "x11grab".into(),
                "-i".into(),
                display.clone(),
            ]);
        }
        args.extend(vec!["-t".into(), duration.as_secs().to_string()]);
        args.extend(self.config.extra_args.clone());
        args.push(output.to_string_lossy().to_string());
        let mut command = Command::new(&self.config.ffmpeg_path);
        command.args(&args);
        command.kill_on_drop(true);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        let join = tokio::spawn(async move { command.status().await });
        Ok(SessionRecordingHandle {
            join: Some(join),
            output,
            placeholder: false,
        })
    }

    async fn finish(
        &self,
        handle: SessionRecordingHandle,
    ) -> BrowserResult<(PathBuf, Vec<String>)> {
        let mut warnings = Vec::new();
        if let Some(join) = handle.join {
            match join.await {
                Ok(Ok(status)) if status.success() => {}
                Ok(Ok(status)) => {
                    warnings.push(format!(
                        "ffmpeg exited with status {status:?}; using placeholder"
                    ));
                    self.ensure_placeholder(&handle.output).await?;
                }
                Ok(Err(err)) => {
                    warnings.push(format!("ffmpeg execution error: {err}"));
                    self.ensure_placeholder(&handle.output).await?;
                }
                Err(err) => {
                    warnings.push(format!("ffmpeg task join error: {err}"));
                    self.ensure_placeholder(&handle.output).await?;
                }
            }
        } else if handle.placeholder {
            warnings.push("ffmpeg not available; generated placeholder video".to_string());
            self.ensure_placeholder(&handle.output).await?;
        }
        Ok((handle.output, warnings))
    }

    async fn ensure_placeholder(&self, path: &Path) -> BrowserResult<()> {
        if !path.exists() {
            fs::write(path, b"placeholder recording").await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QaStatistics {
    pub total_runs: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub pbd_success_rate: f64,
    pub avg_duration_ms: f64,
    pub proxy_rotations: i64,
    pub bot_detections: i64,
    pub last_run: Option<DateTime<Utc>>,
}

pub struct QaMetricsStore {
    path: PathBuf,
}

impl QaMetricsStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn connection(&self) -> Result<Connection, BrowserError> {
        let conn = Connection::open(&self.path).map_err(|err| BrowserError::Qa(err.to_string()))?;
        configure_connection(&conn).map_err(|err| BrowserError::Qa(err.to_string()))?;
        Ok(conn)
    }

    pub fn summarize(&self) -> BrowserResult<QaStatistics> {
        let conn = self.connection()?;
        let total_runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM curator_runs", [], |row| row.get(0))
            .unwrap_or(0);
        let success_count: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(success), 0) FROM curator_runs",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let avg_duration_ms: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(duration_ms), 0) FROM curator_runs",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        let proxy_rotations: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(proxy_rotations), 0) FROM curator_runs",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let bot_detections: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM curator_failures WHERE category = 'BotDetection'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let last_run: Option<DateTime<Utc>> = conn
            .query_row(
                "SELECT ts FROM curator_runs ORDER BY ts DESC LIMIT 1",
                [],
                |row| {
                    let ts: Option<chrono::NaiveDateTime> = row.get(0).ok();
                    Ok(ts.map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc)))
                },
            )
            .optional()
            .unwrap_or(None)
            .flatten();

        let failure_count = total_runs.saturating_sub(success_count);
        let pbd_success_rate = if total_runs > 0 {
            (success_count as f64 / total_runs as f64) * 100.0
        } else {
            0.0
        };

        Ok(QaStatistics {
            total_runs,
            success_count,
            failure_count,
            pbd_success_rate,
            avg_duration_ms,
            proxy_rotations,
            bot_detections,
            last_run,
        })
    }

    pub fn generate_dashboard(&self, output: impl AsRef<Path>) -> BrowserResult<PathBuf> {
        let stats = self.summarize()?;
        QaDashboard::write(output, &stats)
    }
}

pub struct QaDashboard;

impl QaDashboard {
    pub fn render(stats: &QaStatistics) -> String {
        let last_run = stats
            .last_run
            .map(|ts| ts.to_rfc3339())
            .unwrap_or_else(|| "n/a".into());
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
    <title>VVTV QA Dashboard</title>
    <style>
        body {{ font-family: Arial, sans-serif; background: #111; color: #f5f5f5; }}
        table {{ border-collapse: collapse; width: 60%; margin: 2rem auto; }}
        th, td {{ border: 1px solid #444; padding: 0.75rem; text-align: left; }}
        th {{ background: #222; }}
    </style>
</head>
<body>
    <h1 style="text-align:center;">VVTV QA Smoke Dashboard</h1>
    <table>
        <tr><th>Total runs</th><td>{total_runs}</td></tr>
        <tr><th>Successes</th><td>{success}</td></tr>
        <tr><th>Failures</th><td>{failure}</td></tr>
        <tr><th>PBD success rate</th><td>{pbd:.1}%</td></tr>
        <tr><th>Average duration (ms)</th><td>{avg:.0}</td></tr>
        <tr><th>Proxy rotations</th><td>{rotations}</td></tr>
        <tr><th>Bot detections</th><td>{bots}</td></tr>
        <tr><th>Last run</th><td>{last}</td></tr>
    </table>
</body>
</html>"#,
            total_runs = stats.total_runs,
            success = stats.success_count,
            failure = stats.failure_count,
            pbd = stats.pbd_success_rate,
            avg = stats.avg_duration_ms,
            rotations = stats.proxy_rotations,
            bots = stats.bot_detections,
            last = last_run
        )
    }

    pub fn write(output: impl AsRef<Path>, stats: &QaStatistics) -> BrowserResult<PathBuf> {
        let output = output.as_ref().to_path_buf();
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|err| BrowserError::Qa(err.to_string()))?;
        }
        let html = Self::render(stats);
        std::fs::write(&output, html).map_err(|err| BrowserError::Qa(err.to_string()))?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::error_handler::{
        telemetry_failure, telemetry_run, AutomationTelemetry, RemediationAction,
    };
    use tempfile::tempdir;

    #[tokio::test]
    async fn session_recorder_generates_placeholder_when_ffmpeg_missing() {
        let dir = tempdir().unwrap();
        let config = SessionRecorderConfig {
            ffmpeg_path: dir.path().join("bin").join("ffmpeg"),
            display: None,
            extra_args: vec![],
            output_dir: dir.path().join("video"),
            allow_placeholder: true,
        };
        let recorder = SessionRecorder::new(config);
        let (result, path, warnings) = recorder
            .record_during("smoke", Duration::from_secs(2), || async {
                Ok::<_, BrowserError>(())
            })
            .await
            .unwrap();
        result.unwrap();
        assert!(path.exists());
        assert!(warnings.iter().any(|w| w.contains("placeholder")));
        let data = tokio::fs::read(&path).await.unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn qa_metrics_store_summarizes_and_writes_dashboard() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("failures.log");
        let db_path = dir.path().join("metrics.sqlite");
        let telemetry = AutomationTelemetry::new(&log_path, &db_path).unwrap();

        let error = BrowserError::Network("captcha".into());
        telemetry_failure(
            &telemetry,
            "https://example.com",
            &error,
            1,
            Some("proxy.example".into()),
            RemediationAction::IpRotated {
                exit_node: Some("fra.exit.ts.net".into()),
            },
        )
        .unwrap();
        telemetry_run(
            &telemetry,
            "smoke",
            "https://example.com",
            true,
            1500,
            None,
            None,
            2,
        )
        .unwrap();

        let store = QaMetricsStore::new(&db_path);
        let stats = store.summarize().unwrap();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.proxy_rotations, 2);
        assert_eq!(stats.bot_detections, 1);

        let output = dir.path().join("dashboard.html");
        let path = store.generate_dashboard(&output).unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("VVTV QA Smoke Dashboard"));
        assert!(html.contains("Proxy rotations"));
    }
}
