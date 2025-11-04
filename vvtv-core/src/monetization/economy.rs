use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use hex::encode as hex_encode;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::sqlite::configure_connection;

const ECONOMY_SCHEMA: &str = include_str!("../../../sql/economy.sql");

#[derive(Debug, Error)]
pub enum EconomyError {
    #[error("failed to open economy database {path}: {source}")]
    Open {
        source: rusqlite::Error,
        path: PathBuf,
    },
    #[error("failed to execute statement on economy database: {0}")]
    Execute(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("invalid event type: {0}")]
    InvalidEventType(String),
    #[error("economy store path not configured")]
    MissingStore,
}

pub type EconomyResult<T> = Result<T, EconomyError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EconomyEventType {
    View,
    Click,
    SlotSell,
    Affiliate,
    Cost,
    Payout,
}

impl EconomyEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EconomyEventType::View => "view",
            EconomyEventType::Click => "click",
            EconomyEventType::SlotSell => "slot_sell",
            EconomyEventType::Affiliate => "affiliate",
            EconomyEventType::Cost => "cost",
            EconomyEventType::Payout => "payout",
        }
    }

    pub fn is_revenue(&self) -> bool {
        matches!(
            self,
            EconomyEventType::View
                | EconomyEventType::Click
                | EconomyEventType::SlotSell
                | EconomyEventType::Affiliate
        )
    }
}

impl std::fmt::Display for EconomyEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EconomyEventType {
    type Err = EconomyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "view" => Ok(EconomyEventType::View),
            "click" => Ok(EconomyEventType::Click),
            "slot_sell" => Ok(EconomyEventType::SlotSell),
            "affiliate" => Ok(EconomyEventType::Affiliate),
            "cost" => Ok(EconomyEventType::Cost),
            "payout" => Ok(EconomyEventType::Payout),
            other => Err(EconomyError::InvalidEventType(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EconomyEvent {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: EconomyEventType,
    pub value_eur: f64,
    pub source: String,
    pub context: String,
    pub proof: String,
    pub notes: Option<String>,
}

impl EconomyEvent {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let timestamp: NaiveDateTime = row.get("timestamp")?;
        let event_type: String = row.get("event_type")?;
        Ok(Self {
            id: row.get("id")?,
            timestamp: Utc.from_utc_datetime(&timestamp),
            event_type: event_type.parse().unwrap_or(EconomyEventType::View),
            value_eur: row.get("value_eur")?,
            source: row.get("source")?,
            context: row.get("context")?,
            proof: row.get("proof")?,
            notes: row.get("notes")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewEconomyEvent {
    pub timestamp: Option<DateTime<Utc>>,
    pub event_type: EconomyEventType,
    pub value_eur: f64,
    pub source: String,
    pub context: String,
    pub notes: Option<String>,
}

impl NewEconomyEvent {
    pub fn new(
        event_type: EconomyEventType,
        value_eur: f64,
        source: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: None,
            event_type,
            value_eur,
            source: source.into(),
            context: context.into(),
            notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EconomySummary {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub totals: HashMap<EconomyEventType, f64>,
    pub net_revenue: f64,
    pub total_events: usize,
}

impl EconomySummary {
    pub fn revenue_total(&self) -> f64 {
        self.totals
            .iter()
            .filter(|(event, _)| event.is_revenue())
            .map(|(_, value)| value)
            .sum()
    }

    pub fn cost_total(&self) -> f64 {
        self.totals
            .iter()
            .filter(|(event, _)| !event.is_revenue())
            .map(|(_, value)| value)
            .sum()
    }

    pub fn with_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start = start;
        self.end = end;
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerExport {
    pub csv_path: PathBuf,
    pub manifest_path: PathBuf,
    pub checksum: String,
    pub summary: EconomySummary,
}

#[derive(Debug, Clone, Serialize)]
struct LedgerManifest {
    version: String,
    generated_at: DateTime<Utc>,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    checksum: String,
    totals: HashMap<String, f64>,
    net_revenue: f64,
    total_events: usize,
}

#[derive(Debug, Clone)]
pub struct EconomyStoreBuilder {
    path: Option<PathBuf>,
    read_only: bool,
    create_if_missing: bool,
}

impl Default for EconomyStoreBuilder {
    fn default() -> Self {
        Self {
            path: None,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl EconomyStoreBuilder {
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

    pub fn build(self) -> EconomyResult<EconomyStore> {
        let path = self.path.ok_or(EconomyError::MissingStore)?;
        let mut flags = if self.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if !self.read_only && self.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        Ok(EconomyStore { path, flags })
    }
}

#[derive(Debug, Clone)]
pub struct EconomyStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl EconomyStore {
    pub fn builder() -> EconomyStoreBuilder {
        EconomyStoreBuilder::new()
    }

    pub fn new(path: impl AsRef<Path>) -> EconomyResult<Self> {
        EconomyStoreBuilder::new().path(path).build()
    }

    pub(crate) fn open(&self) -> EconomyResult<Connection> {
        let conn = Connection::open_with_flags(&self.path, self.flags).map_err(|source| {
            EconomyError::Open {
                source,
                path: self.path.clone(),
            }
        })?;
        configure_connection(&conn).map_err(|source| EconomyError::Open {
            source,
            path: self.path.clone(),
        })?;
        Ok(conn)
    }

    pub fn initialize(&self) -> EconomyResult<()> {
        let conn = self.open()?;
        conn.execute_batch(ECONOMY_SCHEMA)?;
        Ok(())
    }

    pub fn record_event(&self, event: &NewEconomyEvent) -> EconomyResult<EconomyEvent> {
        let conn = self.open()?;
        let timestamp = event.timestamp.unwrap_or_else(Utc::now);
        let proof = compute_proof(timestamp, event.event_type, &event.context, event.value_eur);
        conn.execute(
            "INSERT INTO economy_events (
                timestamp, event_type, value_eur, source, context, proof, notes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                timestamp.naive_utc(),
                event.event_type.as_str(),
                event.value_eur,
                &event.source,
                &event.context,
                proof,
                &event.notes
            ],
        )?;
        let id = conn.last_insert_rowid();
        let row = conn.query_row(
            "SELECT * FROM economy_events WHERE id = ?1",
            [id],
            EconomyEvent::from_row,
        )?;
        Ok(row)
    }

    pub fn list_events(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> EconomyResult<Vec<EconomyEvent>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM economy_events
             WHERE timestamp >= ?1 AND timestamp < ?2
             ORDER BY timestamp ASC",
        )?;
        let events = stmt
            .query_map(
                params![start.naive_utc(), end.naive_utc()],
                EconomyEvent::from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn summarize(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> EconomyResult<EconomySummary> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT event_type, SUM(value_eur) as total, COUNT(*) as count
             FROM economy_events
             WHERE timestamp >= ?1 AND timestamp < ?2
             GROUP BY event_type",
        )?;
        let mut totals = HashMap::new();
        let mut total_events = 0usize;
        let mut rows = stmt.query(params![start.naive_utc(), end.naive_utc()])?;
        while let Some(row) = rows.next()? {
            let event_type: String = row.get(0)?;
            let total: f64 = row.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
            let count: i64 = row.get::<_, Option<i64>>(2)?.unwrap_or(0);
            let kind = event_type.parse().unwrap_or(EconomyEventType::View);
            totals.insert(kind, total);
            total_events += count as usize;
        }
        let revenue: f64 = totals
            .iter()
            .filter(|(kind, _)| kind.is_revenue())
            .map(|(_, value)| *value)
            .sum();
        let costs: f64 = totals
            .iter()
            .filter(|(kind, _)| !kind.is_revenue())
            .map(|(_, value)| *value)
            .sum();
        Ok(EconomySummary {
            start,
            end,
            totals,
            net_revenue: revenue - costs,
            total_events,
        })
    }

    pub fn export_ledger<P: AsRef<Path>>(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        output_dir: P,
    ) -> EconomyResult<LedgerExport> {
        let events = self.list_events(start, end)?;
        fs::create_dir_all(output_dir.as_ref())?;
        let csv_name = format!("ledger_week_{}.csv", start.format("%Y-%m-%d"));
        let manifest_name = format!("ledger_week_{}.logline", start.format("%Y-%m-%d"));
        let csv_path = output_dir.as_ref().join(csv_name);
        let manifest_path = output_dir.as_ref().join(manifest_name);
        let mut file = File::create(&csv_path)?;
        writeln!(
            file,
            "timestamp,event_type,value_eur,source,context,proof,notes"
        )?;
        for event in &events {
            let line = format!(
                "{}\t{}\t{:.4}\t{}\t{}\t{}\t{}",
                event.timestamp.to_rfc3339(),
                event.event_type.as_str(),
                event.value_eur,
                escape_field(&event.source),
                escape_field(&event.context),
                event.proof,
                escape_field(event.notes.as_deref().unwrap_or(""))
            )
            .replace('\t', ",");
            writeln!(file, "{line}")?;
        }
        drop(file);
        let checksum = file_checksum(&csv_path)?;
        let summary = self.summarize(start, end)?;
        let manifest = LedgerManifest {
            version: "v1".to_string(),
            generated_at: Utc::now(),
            period_start: start,
            period_end: end,
            checksum: checksum.clone(),
            totals: summary
                .totals
                .iter()
                .map(|(kind, value)| (kind.as_str().to_string(), *value))
                .collect(),
            net_revenue: summary.net_revenue,
            total_events: summary.total_events,
        };
        let serialized = serde_json::to_string_pretty(&manifest)?;
        let mut manifest_file = File::create(&manifest_path)?;
        manifest_file.write_all(serialized.as_bytes())?;
        Ok(LedgerExport {
            csv_path,
            manifest_path,
            checksum,
            summary,
        })
    }

    pub fn most_recent_event(&self) -> EconomyResult<Option<EconomyEvent>> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT * FROM economy_events ORDER BY timestamp DESC LIMIT 1",
            [],
            EconomyEvent::from_row,
        )
        .optional()
        .map_err(EconomyError::from)
    }

    pub fn ensure_schema(&self) -> EconomyResult<()> {
        self.initialize()
    }

    pub fn purge_before(&self, cutoff: DateTime<Utc>) -> EconomyResult<usize> {
        let conn = self.open()?;
        let affected = conn.execute(
            "DELETE FROM economy_events WHERE timestamp < ?1",
            [cutoff.naive_utc()],
        )?;
        Ok(affected as usize)
    }

    pub fn time_window(&self, days: i64) -> EconomyResult<(DateTime<Utc>, DateTime<Utc>)> {
        let end = Utc::now();
        let start = end - Duration::days(days);
        Ok((start, end))
    }
}

fn compute_proof(
    timestamp: DateTime<Utc>,
    kind: EconomyEventType,
    context: &str,
    value_eur: f64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.to_rfc3339().as_bytes());
    hasher.update(kind.as_str().as_bytes());
    hasher.update(context.as_bytes());
    hasher.update(format!("{:.6}", value_eur).as_bytes());
    hex_encode(hasher.finalize())
}

fn file_checksum(path: &Path) -> EconomyResult<String> {
    let content = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(hex_encode(hasher.finalize()))
}

fn escape_field(value: &str) -> String {
    if value.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "'"))
    } else {
        value.to_string()
    }
}
