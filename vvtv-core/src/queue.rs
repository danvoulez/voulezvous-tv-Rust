use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use flate2::{write::GzEncoder, Compression};
use rusqlite::backup::Backup;
use rusqlite::types::Value;
use rusqlite::{params, Connection, OpenFlags, Row};
use thiserror::Error;

use crate::config::QueueSection;
use crate::sqlite::configure_connection;

const QUEUE_SCHEMA: &str = include_str!("../../sql/queue.sql");

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("failed to open queue database {path}: {source}")]
    Open {
        source: rusqlite::Error,
        path: PathBuf,
    },
    #[error("failed to execute statement on queue database: {0}")]
    Execute(#[from] rusqlite::Error),
    #[error("queue path not configured")]
    MissingStore,
    #[error("invalid queue status: {0}")]
    InvalidStatus(String),
    #[error("queue record not found: {0}")]
    NotFound(i64),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub type QueueResult<T> = Result<T, QueueError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueStatus {
    Queued,
    Playing,
    Played,
    Failed,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueStatus::Queued => "queued",
            QueueStatus::Playing => "playing",
            QueueStatus::Played => "played",
            QueueStatus::Failed => "failed",
        }
    }
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for QueueStatus {
    type Err = QueueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "playing" => Ok(Self::Playing),
            "played" => Ok(Self::Played),
            "failed" => Ok(Self::Failed),
            other => Err(QueueError::InvalidStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueueItem {
    pub plan_id: String,
    pub asset_path: String,
    pub duration_s: Option<i64>,
    pub curation_score: Option<f64>,
    pub priority: i64,
    pub node_origin: Option<String>,
    pub content_kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub id: i64,
    pub plan_id: String,
    pub asset_path: String,
    pub duration_s: Option<i64>,
    pub status: QueueStatus,
    pub curation_score: Option<f64>,
    pub priority: i64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub node_origin: Option<String>,
    pub play_started_at: Option<DateTime<Utc>>,
    pub play_finished_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
    pub content_kind: Option<String>,
}

impl QueueEntry {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            plan_id: row.get("plan_id")?,
            asset_path: row.get("asset_path")?,
            duration_s: row.get("duration_s")?,
            status: row
                .get::<_, String>("status")?
                .parse()
                .unwrap_or(QueueStatus::Queued),
            curation_score: row.get("curation_score")?,
            priority: row.get::<_, Option<i64>>("priority")?.unwrap_or(0),
            created_at: parse_timestamp(row.get("created_at")?)?,
            updated_at: parse_timestamp(row.get("updated_at")?)?,
            node_origin: row.get("node_origin")?,
            play_started_at: parse_timestamp(row.get("play_started_at")?)?,
            play_finished_at: parse_timestamp(row.get("play_finished_at")?)?,
            failure_reason: row.get("failure_reason")?,
            content_kind: row.get("content_kind")?,
        })
    }

    pub fn is_music(&self) -> bool {
        self.content_kind
            .as_deref()
            .map(|kind| kind.eq_ignore_ascii_case("music") || kind.to_lowercase().contains("music"))
            .unwrap_or(false)
    }

    pub fn waiting_seconds(&self, now: DateTime<Utc>) -> f64 {
        self.created_at
            .map(|created| (now - created).num_seconds().max(0) as f64)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueueSummary {
    pub counts: HashMap<QueueStatus, i64>,
    pub buffer_duration_hours: f64,
}

#[derive(Debug, Clone, Default)]
pub struct QueueMetrics {
    pub buffer_duration_hours: f64,
    pub queue_length: i64,
    pub played_last_hour: i64,
    pub failures_last_hour: i64,
}

#[derive(Debug, Clone)]
pub struct QueueSelectionPolicy {
    pub music_every: Option<usize>,
    pub curation_bump_threshold: f64,
    pub curation_bump_min_age: Duration,
}

impl QueueSelectionPolicy {
    pub fn new(
        music_every: Option<usize>,
        curation_bump_threshold: f64,
        curation_bump_min_age: Duration,
    ) -> Self {
        Self {
            music_every,
            curation_bump_threshold,
            curation_bump_min_age,
        }
    }

    pub fn from_queue_config(config: &QueueSection) -> Self {
        let music_every = if config.music_ratio > 0.0 {
            let every = (1.0 / config.music_ratio).round() as usize;
            Some(every.max(1))
        } else {
            None
        };
        Self {
            music_every,
            curation_bump_threshold: config.curation_bump_threshold as f64,
            curation_bump_min_age: Duration::hours(24),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueueFilter {
    pub status: Option<QueueStatus>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct PlayoutQueueStoreBuilder {
    path: Option<PathBuf>,
    read_only: bool,
    create_if_missing: bool,
}

impl Default for PlayoutQueueStoreBuilder {
    fn default() -> Self {
        Self {
            path: None,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl PlayoutQueueStoreBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }

    pub fn create_if_missing(mut self, value: bool) -> Self {
        self.create_if_missing = value;
        self
    }

    pub fn build(self) -> QueueResult<PlayoutQueueStore> {
        let path = self.path.ok_or(QueueError::MissingStore)?;
        let mut flags = if self.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if !self.read_only && self.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        Ok(PlayoutQueueStore { path, flags })
    }
}

#[derive(Debug, Clone)]
pub struct PlayoutQueueStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl PlayoutQueueStore {
    pub fn builder() -> PlayoutQueueStoreBuilder {
        PlayoutQueueStoreBuilder::new()
    }

    pub fn new(path: impl AsRef<Path>) -> QueueResult<Self> {
        PlayoutQueueStoreBuilder::new().path(path).build()
    }

    fn open(&self) -> QueueResult<Connection> {
        let conn = Connection::open_with_flags(&self.path, self.flags).map_err(|source| {
            QueueError::Open {
                source,
                path: self.path.clone(),
            }
        })?;
        configure_connection(&conn).map_err(|source| QueueError::Open {
            source,
            path: self.path.clone(),
        })?;
        Ok(conn)
    }

    pub fn initialize(&self) -> QueueResult<()> {
        let conn = self.open()?;
        conn.execute_batch(QUEUE_SCHEMA)?;
        Ok(())
    }

    pub fn enqueue(&self, item: &QueueItem) -> QueueResult<i64> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO playout_queue (
                plan_id, asset_path, duration_s, status, curation_score, priority,
                node_origin, content_kind
            ) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?7)",
            params![
                &item.plan_id,
                &item.asset_path,
                &item.duration_s,
                &item.curation_score,
                item.priority,
                &item.node_origin,
                &item.content_kind
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list(&self, filter: &QueueFilter) -> QueueResult<Vec<QueueEntry>> {
        let conn = self.open()?;
        let mut query = String::from("SELECT * FROM playout_queue");
        let mut params: Vec<Value> = Vec::new();
        if let Some(status) = filter.status {
            query.push_str(" WHERE status = ?");
            params.push(Value::Text(status.as_str().to_string()));
        }
        query.push_str(" ORDER BY priority DESC, created_at ASC");
        if let Some(limit) = filter.limit {
            query.push_str(" LIMIT ?");
            params.push(Value::Integer(limit as i64));
        }
        let mut stmt = conn.prepare(&query)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(
            params.iter().map(|value| value as &dyn rusqlite::ToSql),
        ))?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(QueueEntry::from_row(row)?);
        }
        Ok(entries)
    }

    pub fn summary(&self) -> QueueResult<QueueSummary> {
        let conn = self.open()?;
        let mut counts = HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*), COALESCE(SUM(duration_s), 0) FROM playout_queue GROUP BY status",
        )?;
        let mut total_buffer_s = 0.0;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let status: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let duration: f64 = row.get::<_, Option<f64>>(2)?.unwrap_or(0.0);
            let status = status.parse().unwrap_or(QueueStatus::Queued);
            if status == QueueStatus::Queued {
                total_buffer_s += duration;
            }
            counts.insert(status, count);
        }
        Ok(QueueSummary {
            counts,
            buffer_duration_hours: total_buffer_s / 3600.0,
        })
    }

    pub fn metrics(&self) -> QueueResult<QueueMetrics> {
        let conn = self.open()?;
        let mut metrics = QueueMetrics::default();
        metrics.buffer_duration_hours = conn.query_row(
            "SELECT COALESCE(SUM(duration_s), 0) FROM playout_queue WHERE status='queued'",
            [],
            |row| row.get::<_, f64>(0),
        )? / 3600.0;
        metrics.queue_length = conn.query_row(
            "SELECT COUNT(*) FROM playout_queue WHERE status='queued'",
            [],
            |row| row.get(0),
        )?;
        let cutoff = (Utc::now() - Duration::hours(1)).naive_utc();
        metrics.played_last_hour = conn
            .query_row(
                "SELECT COUNT(*) FROM playout_queue WHERE status='played' AND play_finished_at >= ?1",
                [cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0);
        metrics.failures_last_hour = conn
            .query_row(
                "SELECT COUNT(*) FROM playout_queue WHERE status='failed' AND play_finished_at >= ?1",
                [cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(metrics)
    }

    pub fn begin_playback(&self, policy: &QueueSelectionPolicy) -> QueueResult<Option<QueueEntry>> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        let now = Utc::now();
        let mut entries = self.fetch_candidates(&tx)?;
        if entries.is_empty() {
            tx.commit()?;
            return Ok(None);
        }

        let prefer_music = self.should_prefer_music(&tx, policy)?;
        let bump_cutoff = now - policy.curation_bump_min_age;

        let mut weighted: Vec<WeightedEntry> = entries
            .drain(..)
            .map(|entry| {
                let bump = entry
                    .curation_score
                    .map(|score| score >= policy.curation_bump_threshold)
                    .unwrap_or(false)
                    && entry
                        .created_at
                        .map(|created| created <= bump_cutoff)
                        .unwrap_or(false);
                let waiting_score =
                    entry.waiting_seconds(now) * (1.0 + entry.curation_score.unwrap_or(0.0));
                WeightedEntry {
                    entry,
                    bump,
                    waiting_score,
                }
            })
            .collect();

        weighted.sort_by(|a, b| compare_weighted(a, b));

        let mut selected: Option<WeightedEntry> = None;
        if prefer_music {
            if let Some(pos) = weighted
                .iter()
                .position(|candidate| candidate.entry.is_music())
            {
                selected = Some(weighted.remove(pos));
            }
        }
        if selected.is_none() {
            selected = weighted.into_iter().next();
        }

        if let Some(mut chosen) = selected {
            tx.execute(
                "UPDATE playout_queue SET status='playing', play_started_at=CURRENT_TIMESTAMP, failure_reason=NULL WHERE id=?1",
                [chosen.entry.id],
            )?;
            tx.commit()?;
            chosen.entry.status = QueueStatus::Playing;
            chosen.entry.play_started_at = Some(now);
            return Ok(Some(chosen.entry));
        }

        tx.commit()?;
        Ok(None)
    }

    pub fn mark_priority(&self, id: i64, priority: i64) -> QueueResult<()> {
        let conn = self.open()?;
        let affected = conn.execute(
            "UPDATE playout_queue SET priority=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![priority, id],
        )?;
        if affected == 0 {
            return Err(QueueError::NotFound(id));
        }
        Ok(())
    }

    pub fn remove(&self, id: i64) -> QueueResult<()> {
        let conn = self.open()?;
        let affected = conn.execute("DELETE FROM playout_queue WHERE id=?1", [id])?;
        if affected == 0 {
            return Err(QueueError::NotFound(id));
        }
        Ok(())
    }

    pub fn mark_playback_result(
        &self,
        id: i64,
        status: QueueStatus,
        actual_duration: Option<i64>,
        failure_reason: Option<&str>,
    ) -> QueueResult<()> {
        let conn = self.open()?;
        let (finish_ts, failure) = match status {
            QueueStatus::Played => (Some(Utc::now().naive_utc()), None),
            QueueStatus::Failed => (
                Some(Utc::now().naive_utc()),
                failure_reason.map(str::to_string),
            ),
            QueueStatus::Playing | QueueStatus::Queued => {
                (None, failure_reason.map(str::to_string))
            }
        };
        let affected = conn.execute(
            "UPDATE playout_queue SET status=?1, play_finished_at=?2, failure_reason=?3, duration_s=COALESCE(?4, duration_s) WHERE id=?5",
            params![status.as_str(), finish_ts, failure, actual_duration, id],
        )?;
        if affected == 0 {
            return Err(QueueError::NotFound(id));
        }
        Ok(())
    }

    pub fn cleanup_played(&self, older_than: Duration) -> QueueResult<usize> {
        let conn = self.open()?;
        let cutoff = (Utc::now() - older_than).naive_utc();
        let affected = conn.execute(
            "DELETE FROM playout_queue WHERE status='played' AND play_finished_at IS NOT NULL AND play_finished_at < ?1",
            [cutoff],
        )?;
        Ok(affected as usize)
    }

    pub fn export_backup(&self, output: impl AsRef<Path>) -> QueueResult<()> {
        let output = output.as_ref();
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.open()?;
        let mut dump = String::new();
        dump.push_str(QUEUE_SCHEMA);
        dump.push('\n');
        dump.push_str("BEGIN;\n");

        let mut stmt = conn.prepare(
            "SELECT id, plan_id, asset_path, duration_s, status, curation_score, priority,
                    created_at, updated_at, node_origin, play_started_at, play_finished_at,
                    failure_reason, content_kind
             FROM playout_queue ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<f64>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
            ))
        })?;

        for row in rows {
            let (
                id,
                plan_id,
                asset_path,
                duration_s,
                status,
                curation_score,
                priority,
                created_at,
                updated_at,
                node_origin,
                play_started_at,
                play_finished_at,
                failure_reason,
                content_kind,
            ) = row?;
            dump.push_str(&format!(
                "INSERT INTO playout_queue (id, plan_id, asset_path, duration_s, status, curation_score, priority, created_at, updated_at, node_origin, play_started_at, play_finished_at, failure_reason, content_kind) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
                id,
                sql_quote(&plan_id),
                sql_quote(&asset_path),
                format_optional_integer(duration_s),
                sql_quote(&status),
                format_optional_float(curation_score),
                priority,
                format_optional_text(created_at),
                format_optional_text(updated_at),
                format_optional_text(node_origin),
                format_optional_text(play_started_at),
                format_optional_text(play_finished_at),
                format_optional_text(failure_reason),
                format_optional_text(content_kind),
            ));
        }

        dump.push_str("COMMIT;\n");

        let file = File::create(output)?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(dump.as_bytes())?;
        encoder.finish()?;
        Ok(())
    }

    pub fn backup_to(&self, destination: impl AsRef<Path>) -> QueueResult<()> {
        let destination_path = destination.as_ref();
        let source = self.open()?;
        let mut dest = Connection::open(destination_path)?;
        configure_connection(&dest).map_err(|source| QueueError::Open {
            source,
            path: destination_path.to_path_buf(),
        })?;
        let backup = Backup::new(&source, &mut dest)?;
        backup.run_to_completion(10, StdDuration::from_millis(50), None)?;
        Ok(())
    }

    fn fetch_candidates(&self, conn: &Connection) -> QueueResult<Vec<QueueEntry>> {
        let mut stmt = conn.prepare("SELECT * FROM playout_queue WHERE status='queued'")?;
        let mut rows = stmt.query([])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(QueueEntry::from_row(row)?);
        }
        Ok(entries)
    }

    fn should_prefer_music(
        &self,
        conn: &Connection,
        policy: &QueueSelectionPolicy,
    ) -> QueueResult<bool> {
        let window = match policy.music_every {
            Some(value) if value > 0 => value,
            _ => return Ok(false),
        };
        if window <= 1 {
            return Ok(true);
        }
        let mut stmt = conn.prepare(
            "SELECT content_kind FROM playout_queue WHERE status='played' ORDER BY play_finished_at DESC LIMIT ?1",
        )?;
        let mut rows = stmt.query([window as i64 - 1])?;
        let mut without_music = true;
        let mut considered = 0;
        while let Some(row) = rows.next()? {
            considered += 1;
            let kind: Option<String> = row.get(0)?;
            if kind
                .as_deref()
                .map(|value| {
                    value.eq_ignore_ascii_case("music") || value.to_lowercase().contains("music")
                })
                .unwrap_or(false)
            {
                without_music = false;
                break;
            }
        }
        if considered < window.saturating_sub(1) {
            return Ok(false);
        }
        Ok(without_music)
    }
}

struct WeightedEntry {
    entry: QueueEntry,
    bump: bool,
    waiting_score: f64,
}

fn compare_weighted(a: &WeightedEntry, b: &WeightedEntry) -> Ordering {
    a.entry
        .priority
        .cmp(&b.entry.priority)
        .reverse()
        .then_with(|| a.bump.cmp(&b.bump).reverse())
        .then_with(|| {
            a.waiting_score
                .partial_cmp(&b.waiting_score)
                .unwrap_or(Ordering::Equal)
                .reverse()
        })
        .then_with(|| match (a.entry.created_at, b.entry.created_at) {
            (Some(a_ts), Some(b_ts)) => a_ts.cmp(&b_ts),
            _ => Ordering::Equal,
        })
}

fn sql_quote(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{}'", escaped)
}

fn format_optional_integer(value: Option<i64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn format_optional_float(value: Option<f64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn format_optional_text(value: Option<String>) -> String {
    value
        .map(|v| sql_quote(&v))
        .unwrap_or_else(|| "NULL".to_string())
}

fn parse_timestamp(value: Option<NaiveDateTime>) -> Result<Option<DateTime<Utc>>, rusqlite::Error> {
    Ok(value.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)))
}
