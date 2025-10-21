use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;
use tokio::time::sleep;

use crate::config::RetrySection;

use super::error::BrowserResult;
use super::error_handler::{
    map_category, telemetry_failure, AutomationTelemetry, BrowserErrorCategory, RemediationAction,
};
use super::ip_rotator::IpRotator;
use super::metrics::BrowserMetrics;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    max_attempts: usize,
    schedule: Vec<Duration>,
    jitter_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct RetryOutcome<T> {
    pub result: T,
    pub attempts: usize,
}

impl RetryPolicy {
    pub fn new(config: RetrySection) -> Self {
        let mut schedule = config
            .schedule_minutes
            .into_iter()
            .map(|minutes| Duration::from_secs(minutes * 60))
            .collect::<Vec<_>>();
        if schedule.is_empty() {
            schedule.push(Duration::from_secs(600));
            schedule.push(Duration::from_secs(2700));
            schedule.push(Duration::from_secs(86400));
        }
        let max_attempts = config.max_attempts.max(1);
        Self {
            max_attempts,
            schedule,
            jitter_seconds: config.jitter_seconds,
        }
    }

    fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            Duration::from_secs(0)
        } else {
            self.schedule
                .get(attempt - 1)
                .cloned()
                .unwrap_or_else(|| *self.schedule.last().unwrap())
        }
    }

    pub async fn run<F, Fut, T>(
        &self,
        url: &str,
        proxy: Option<String>,
        telemetry: Arc<AutomationTelemetry>,
        ip_rotator: Option<Arc<tokio::sync::Mutex<IpRotator>>>,
        metrics: Arc<Mutex<BrowserMetrics>>,
        mut operation: F,
    ) -> BrowserResult<RetryOutcome<T>>
    where
        F: FnMut(usize) -> Fut,
        Fut: Future<Output = BrowserResult<T>>,
    {
        let mut attempt = 0usize;
        loop {
            match operation(attempt).await {
                Ok(result) => {
                    return Ok(RetryOutcome {
                        result,
                        attempts: attempt + 1,
                    });
                }
                Err(error) => {
                    {
                        let mut guard = metrics.lock().unwrap();
                        guard.record_playback_failure();
                    }
                    let category = map_category(&error);
                    let mut rotated_exit: Option<String> = None;
                    if matches!(category, BrowserErrorCategory::BotDetection) {
                        if let Some(rotator) = ip_rotator.as_ref() {
                            let mut guard = rotator.lock().await;
                            match guard.rotate().await {
                                Ok(Some(exit)) => {
                                    let mut metrics_guard = metrics.lock().unwrap();
                                    metrics_guard.record_proxy_rotation();
                                    rotated_exit = Some(exit);
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    telemetry_failure(
                                        &telemetry,
                                        url,
                                        &err,
                                        attempt + 1,
                                        proxy.clone(),
                                        RemediationAction::Abort,
                                    )?;
                                    return Err(error);
                                }
                            }
                        }
                        let mut guard = metrics.lock().unwrap();
                        guard.record_bot_detection();
                    }
                    let next_delay = self.delay_for_attempt(attempt + 1);
                    let mut delay = next_delay;
                    if self.jitter_seconds > 0 {
                        let jitter = rand::thread_rng().gen_range(0..=self.jitter_seconds);
                        delay += Duration::from_secs(jitter);
                    }
                    let mut action = if attempt + 1 >= self.max_attempts {
                        RemediationAction::Abort
                    } else {
                        RemediationAction::RetryScheduled {
                            delay_seconds: delay.as_secs(),
                        }
                    };
                    if matches!(category, BrowserErrorCategory::BotDetection) {
                        action = RemediationAction::IpRotated {
                            exit_node: rotated_exit.clone(),
                        };
                    }
                    telemetry_failure(&telemetry, url, &error, attempt + 1, proxy.clone(), action)?;
                    attempt += 1;
                    if attempt >= self.max_attempts {
                        return Err(error);
                    }
                    if !delay.is_zero() {
                        sleep(delay).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::error::BrowserError;
    use crate::browser::error_handler::AutomationTelemetry;
    use crate::config::{IpRotationSection, RetrySection};
    use rusqlite::Connection;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    use super::super::ip_rotator::{CommandExecutor, IpRotator};

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    #[derive(Clone, Default)]
    struct RecordingExecutor {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    #[async_trait::async_trait]
    impl CommandExecutor for RecordingExecutor {
        async fn run(
            &self,
            _program: &Path,
            args: &[String],
        ) -> std::io::Result<std::process::ExitStatus> {
            {
                let mut guard = self.calls.lock().unwrap();
                guard.push(args.to_vec());
            }
            Ok(std::process::ExitStatus::from_raw(0))
        }
    }

    #[tokio::test]
    async fn retry_rotates_ip_on_bot_detection_and_retries() {
        let dir = tempdir().unwrap();
        let telemetry = Arc::new(
            AutomationTelemetry::new(
                dir.path().join("failures.log"),
                dir.path().join("metrics.sqlite"),
            )
            .unwrap(),
        );
        let retry = RetryPolicy::new(RetrySection {
            max_attempts: 3,
            schedule_minutes: vec![0],
            jitter_seconds: 0,
        });

        let executor = RecordingExecutor::default();
        let calls = executor.calls.clone();
        let rotator = IpRotator::new(
            IpRotationSection {
                enabled: true,
                tailscale_binary: "tailscale".into(),
                exit_nodes: vec!["fra.exit.ts.net".into()],
                cooldown_seconds: 0,
            },
            Arc::clone(&telemetry),
        )
        .unwrap()
        .with_executor(Arc::new(executor));
        let rotator = Arc::new(tokio::sync::Mutex::new(rotator));

        let metrics = Arc::new(Mutex::new(BrowserMetrics::default()));
        let attempt_state = Arc::new(Mutex::new(0usize));

        let telemetry_for_run = Arc::clone(&telemetry);
        let metrics_for_run = Arc::clone(&metrics);
        let attempt_state_for_run = Arc::clone(&attempt_state);
        let rotator_for_run = Arc::clone(&rotator);

        let outcome = retry
            .run(
                "https://example.com",
                None,
                telemetry_for_run,
                Some(rotator_for_run),
                metrics_for_run,
                move |_| {
                    let state = Arc::clone(&attempt_state_for_run);
                    async move {
                        let mut guard = state.lock().unwrap();
                        if *guard == 0 {
                            *guard += 1;
                            Err(BrowserError::Network("captcha triggered".into()))
                        } else {
                            Ok::<_, BrowserError>("ok".to_string())
                        }
                    }
                },
            )
            .await
            .unwrap();
        assert_eq!(outcome.attempts, 2);
        assert_eq!(outcome.result, "ok");

        let metrics_snapshot = metrics.lock().unwrap().clone();
        assert_eq!(metrics_snapshot.playback_failures, 1);
        assert_eq!(metrics_snapshot.bot_detections, 1);
        assert_eq!(metrics_snapshot.proxy_rotations, 1);

        let recorded_calls = calls.lock().unwrap();
        assert_eq!(recorded_calls.len(), 1);
        assert_eq!(
            recorded_calls[0],
            vec![
                String::from("set"),
                String::from("--exit-node"),
                String::from("fra.exit.ts.net"),
            ]
        );

        let conn = Connection::open(telemetry.database_path()).unwrap();
        let rotation_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proxy_rotations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rotation_count, 1);
        let failure_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM curator_failures", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(failure_count, 1);
    }

    #[tokio::test]
    async fn retry_aborts_after_max_attempts() {
        let dir = tempdir().unwrap();
        let telemetry = Arc::new(
            AutomationTelemetry::new(
                dir.path().join("failures.log"),
                dir.path().join("metrics.sqlite"),
            )
            .unwrap(),
        );
        let retry = RetryPolicy::new(RetrySection {
            max_attempts: 2,
            schedule_minutes: vec![0],
            jitter_seconds: 0,
        });
        let metrics = Arc::new(Mutex::new(BrowserMetrics::default()));

        let telemetry_for_run = Arc::clone(&telemetry);
        let metrics_for_run = Arc::clone(&metrics);

        let result = retry
            .run(
                "https://example.org",
                None,
                telemetry_for_run,
                None,
                metrics_for_run,
                |_| async move {
                    Err::<(), BrowserError>(BrowserError::Timeout("video playback".into()))
                },
            )
            .await;
        assert!(result.is_err());

        let metrics_snapshot = metrics.lock().unwrap().clone();
        assert_eq!(metrics_snapshot.playback_failures, 2);

        let conn = Connection::open(telemetry.database_path()).unwrap();
        let failure_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM curator_failures", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(failure_count, 2);
    }
}
