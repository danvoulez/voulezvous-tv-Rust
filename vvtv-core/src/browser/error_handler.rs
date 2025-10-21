use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use super::error::{BrowserError, BrowserResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BrowserErrorCategory {
    PlayerNotFound,
    HdUnavailable,
    BotDetection,
    ManifestInvalid,
    NetworkTimeout,
    NetworkOther,
    Unexpected,
}

pub struct ErrorCategorizer;

impl ErrorCategorizer {
    pub fn categorize(error: &BrowserError) -> BrowserErrorCategory {
        match error {
            BrowserError::Timeout(message) => {
                if message.contains("video playback") || message.contains("validation") {
                    BrowserErrorCategory::HdUnavailable
                } else {
                    BrowserErrorCategory::NetworkTimeout
                }
            }
            BrowserError::Network(message) => {
                if message.to_lowercase().contains("captcha")
                    || message.to_lowercase().contains("bot")
                    || message.to_lowercase().contains("forbidden")
                {
                    BrowserErrorCategory::BotDetection
                } else if message.to_lowercase().contains("manifest") {
                    BrowserErrorCategory::ManifestInvalid
                } else if message.to_lowercase().contains("video element not found") {
                    BrowserErrorCategory::PlayerNotFound
                } else {
                    BrowserErrorCategory::NetworkOther
                }
            }
            BrowserError::Metadata(message) if message.to_lowercase().contains("manifest") => {
                BrowserErrorCategory::ManifestInvalid
            }
            BrowserError::Metadata(_) => BrowserErrorCategory::Unexpected,
            BrowserError::Profile(message) if message.to_lowercase().contains("play") => {
                BrowserErrorCategory::PlayerNotFound
            }
            BrowserError::Profile(_) => BrowserErrorCategory::Unexpected,
            BrowserError::Unexpected(message) => {
                if message.to_lowercase().contains("captcha") {
                    BrowserErrorCategory::BotDetection
                } else {
                    BrowserErrorCategory::Unexpected
                }
            }
            BrowserError::Launch(message) => {
                if message.to_lowercase().contains("permission") {
                    BrowserErrorCategory::Unexpected
                } else {
                    BrowserErrorCategory::NetworkOther
                }
            }
            BrowserError::Cdp(err) => {
                let text = err.to_string().to_lowercase();
                if text.contains("timeout") {
                    BrowserErrorCategory::NetworkTimeout
                } else if text.contains("captcha") {
                    BrowserErrorCategory::BotDetection
                } else {
                    BrowserErrorCategory::Unexpected
                }
            }
            BrowserError::Io(_) => BrowserErrorCategory::NetworkOther,
            BrowserError::Qa(_) => BrowserErrorCategory::Unexpected,
            BrowserError::Telemetry(_) => BrowserErrorCategory::Unexpected,
            BrowserError::IpRotation(_) => BrowserErrorCategory::NetworkOther,
            BrowserError::Screenshot(_) => BrowserErrorCategory::Unexpected,
            BrowserError::SessionRecording(_) => BrowserErrorCategory::Unexpected,
            BrowserError::Configuration(_) => BrowserErrorCategory::Unexpected,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum RemediationAction {
    RetryScheduled { delay_seconds: u64 },
    IpRotated { exit_node: Option<String> },
    Abort,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureContext {
    pub timestamp: DateTime<Utc>,
    pub url: String,
    pub category: BrowserErrorCategory,
    pub error_message: String,
    pub attempt: usize,
    pub proxy: Option<String>,
    pub action: RemediationAction,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunContext {
    pub timestamp: DateTime<Utc>,
    pub scenario: String,
    pub url: String,
    pub success: bool,
    pub duration_ms: i64,
    pub screenshot_path: Option<PathBuf>,
    pub video_path: Option<PathBuf>,
    pub proxy_rotations: u64,
}

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl From<TelemetryError> for BrowserError {
    fn from(error: TelemetryError) -> Self {
        BrowserError::Telemetry(error.to_string())
    }
}

#[derive(Debug)]
pub struct AutomationTelemetry {
    log: Mutex<File>,
    db_path: PathBuf,
    flags: OpenFlags,
}

impl AutomationTelemetry {
    pub fn new(
        log_path: impl AsRef<Path>,
        db_path: impl AsRef<Path>,
    ) -> Result<Self, TelemetryError> {
        let log_path = log_path.as_ref().to_path_buf();
        if let Some(parent) = log_path.parent() {
            create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            create_dir_all(parent)?;
        }
        let telemetry = Self {
            log: Mutex::new(file),
            db_path,
            flags: OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        };
        telemetry.initialize_db()?;
        Ok(telemetry)
    }

    fn initialize_db(&self) -> Result<(), TelemetryError> {
        let conn = self.open_db()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS curator_failures (
                ts DATETIME DEFAULT CURRENT_TIMESTAMP,
                url TEXT,
                category TEXT,
                error_message TEXT,
                attempt INTEGER,
                proxy TEXT,
                remediation TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_curator_failures_ts ON curator_failures(ts DESC);
            CREATE TABLE IF NOT EXISTS curator_runs (
                ts DATETIME DEFAULT CURRENT_TIMESTAMP,
                scenario TEXT,
                url TEXT,
                success INTEGER,
                duration_ms INTEGER,
                screenshot_path TEXT,
                video_path TEXT,
                proxy_rotations INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_curator_runs_ts ON curator_runs(ts DESC);
            CREATE TABLE IF NOT EXISTS proxy_rotations (
                ts DATETIME DEFAULT CURRENT_TIMESTAMP,
                exit_node TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_proxy_rotations_ts ON proxy_rotations(ts DESC);",
        )?;
        Ok(())
    }

    fn open_db(&self) -> Result<Connection, TelemetryError> {
        Ok(Connection::open_with_flags(&self.db_path, self.flags)?)
    }

    pub fn record_failure(&self, failure: &FailureContext) -> Result<(), TelemetryError> {
        let json = serde_json::to_string(failure)?;
        if let Ok(mut guard) = self.log.lock() {
            writeln!(guard, "{json}")?;
            guard.flush()?;
        }
        let conn = self.open_db()?;
        conn.execute(
            "INSERT INTO curator_failures (url, category, error_message, attempt, proxy, remediation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                failure.url,
                format!("{:?}", failure.category),
                failure.error_message,
                failure.attempt as i64,
                failure.proxy.clone().unwrap_or_default(),
                format!("{:?}", failure.action),
            ],
        )?;
        Ok(())
    }

    pub fn record_run(&self, run: &RunContext) -> Result<(), TelemetryError> {
        let conn = self.open_db()?;
        conn.execute(
            "INSERT INTO curator_runs (
                scenario, url, success, duration_ms, screenshot_path, video_path, proxy_rotations
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run.scenario,
                run.url,
                if run.success { 1 } else { 0 },
                run.duration_ms,
                run.screenshot_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                run.video_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                run.proxy_rotations as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_proxy_rotation(&self, exit_node: &str) -> Result<(), TelemetryError> {
        if !exit_node.is_empty() {
            let conn = self.open_db()?;
            conn.execute(
                "INSERT INTO proxy_rotations (exit_node) VALUES (?1)",
                params![exit_node],
            )?;
        }
        Ok(())
    }

    pub fn database_path(&self) -> &Path {
        &self.db_path
    }
}

pub fn map_category(error: &BrowserError) -> BrowserErrorCategory {
    ErrorCategorizer::categorize(error)
}

pub fn telemetry_failure(
    telemetry: &AutomationTelemetry,
    url: &str,
    error: &BrowserError,
    attempt: usize,
    proxy: Option<String>,
    action: RemediationAction,
) -> BrowserResult<()> {
    let entry = FailureContext {
        timestamp: Utc::now(),
        url: url.to_string(),
        category: map_category(error),
        error_message: error.to_string(),
        attempt,
        proxy,
        action,
    };
    telemetry.record_failure(&entry)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn telemetry_run(
    telemetry: &AutomationTelemetry,
    scenario: &str,
    url: &str,
    success: bool,
    duration_ms: i64,
    screenshot_path: Option<PathBuf>,
    video_path: Option<PathBuf>,
    proxy_rotations: u64,
) -> BrowserResult<()> {
    let run = RunContext {
        timestamp: Utc::now(),
        scenario: scenario.to_string(),
        url: url.to_string(),
        success,
        duration_ms,
        screenshot_path,
        video_path,
        proxy_rotations,
    };
    telemetry.record_run(&run)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn categorize_bot_detection() {
        let err = BrowserError::Network("captcha challenge".into());
        assert!(matches!(
            ErrorCategorizer::categorize(&err),
            BrowserErrorCategory::BotDetection
        ));
    }

    #[test]
    fn telemetry_persists_entries() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("failures.log");
        let db_path = dir.path().join("metrics.sqlite");
        let telemetry = AutomationTelemetry::new(&log_path, &db_path).unwrap();

        let error = BrowserError::Timeout("video playback".into());
        telemetry_failure(
            &telemetry,
            "https://example.com",
            &error,
            1,
            None,
            RemediationAction::Abort,
        )
        .unwrap();
        telemetry_run(
            &telemetry,
            "smoke",
            "https://example.com",
            true,
            1200,
            None,
            None,
            0,
        )
        .unwrap();

        let log_contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_contents.contains("video playback"));

        let conn = Connection::open(&db_path).unwrap();
        let failure_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM curator_failures", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(failure_count, 1);
        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM curator_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(run_count, 1);
    }
}
