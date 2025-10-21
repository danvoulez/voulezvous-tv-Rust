use std::fs;
use std::path::Path;

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use rand::{thread_rng, Rng};
use rusqlite::{params, Row};
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::monetization::economy::{EconomyError, EconomyEventType, EconomyStore, NewEconomyEvent};
use crate::queue::{PlayoutQueueStore, QueueItem};

#[derive(Debug, Error)]
pub enum SpotsError {
    #[error("economy error: {0}")]
    Economy(#[from] EconomyError),
    #[error("queue error: {0}")]
    Queue(#[from] crate::queue::QueueError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse contract: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("micro spot {0} not found")]
    NotFound(String),
}

pub type SpotsResult<T> = Result<T, SpotsError>;

#[derive(Debug, Clone, Serialize)]
pub struct MicroSpotContract {
    pub id: String,
    pub sponsor: String,
    pub visual_style: String,
    pub duration_s: i64,
    pub value_eur: f64,
    pub cadence_min_minutes: i64,
    pub cadence_max_minutes: i64,
    pub asset_path: String,
    pub active: bool,
    pub next_available_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_injected_at: Option<DateTime<Utc>>,
    pub total_injections: i64,
}

impl MicroSpotContract {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            sponsor: row.get("sponsor")?,
            visual_style: row.get("visual_style")?,
            duration_s: row.get("duration_s")?,
            value_eur: row.get("value_eur")?,
            cadence_min_minutes: row.get("cadence_min_minutes")?,
            cadence_max_minutes: row.get("cadence_max_minutes")?,
            asset_path: row.get("asset_path")?,
            active: row.get::<_, Option<i64>>("active")?.unwrap_or(1) != 0,
            next_available_at: parse_timestamp(
                row.get::<_, Option<NaiveDateTime>>("next_available_at")?,
            ),
            expires_at: parse_timestamp(row.get::<_, Option<NaiveDateTime>>("expires_at")?),
            last_injected_at: parse_timestamp(
                row.get::<_, Option<NaiveDateTime>>("last_injected_at")?,
            ),
            total_injections: row.get::<_, Option<i64>>("total_injections")?.unwrap_or(0),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MicroSpotContractSpec {
    #[serde(default)]
    id: Option<String>,
    sponsor: String,
    visual_style: String,
    duration_s: i64,
    value_eur: f64,
    #[serde(default = "default_cadence_min")]
    cadence_min_minutes: i64,
    #[serde(default = "default_cadence_max")]
    cadence_max_minutes: i64,
    asset_path: String,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MicroSpotInjection {
    pub contract_id: String,
    pub queue_entry_id: i64,
    pub scheduled_at: DateTime<Utc>,
    pub next_available_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct MicroSpotManager {
    economy: EconomyStore,
}

impl MicroSpotManager {
    pub fn new(economy: EconomyStore) -> Self {
        Self { economy }
    }

    pub fn register_from_file<P: AsRef<Path>>(&self, path: P) -> SpotsResult<MicroSpotContract> {
        let content = fs::read_to_string(&path)?;
        let spec: MicroSpotContractSpec = serde_json::from_str(&content)?;
        self.register(spec)
    }

    pub(crate) fn register(&self, spec: MicroSpotContractSpec) -> SpotsResult<MicroSpotContract> {
        let id = spec
            .id
            .unwrap_or_else(|| format!("spot-{}", Uuid::new_v4().simple()));
        let expires_at = spec
            .expires_at
            .as_deref()
            .and_then(parse_contract_timestamp);
        let conn = self.economy.open()?;
        conn.execute(
            "INSERT INTO micro_spots (
                id, sponsor, visual_style, duration_s, value_eur,
                cadence_min_minutes, cadence_max_minutes, asset_path, active,
                expires_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, 1,
                ?9
            )
            ON CONFLICT(id) DO UPDATE SET
                sponsor = excluded.sponsor,
                visual_style = excluded.visual_style,
                duration_s = excluded.duration_s,
                value_eur = excluded.value_eur,
                cadence_min_minutes = excluded.cadence_min_minutes,
                cadence_max_minutes = excluded.cadence_max_minutes,
                asset_path = excluded.asset_path,
                expires_at = excluded.expires_at",
            params![
                &id,
                &spec.sponsor,
                &spec.visual_style,
                spec.duration_s,
                spec.value_eur,
                spec.cadence_min_minutes,
                spec.cadence_max_minutes,
                &spec.asset_path,
                expires_at.map(|dt| dt.naive_utc()),
            ],
        )?;
        self.fetch(&id)
    }

    pub fn list(&self) -> SpotsResult<Vec<MicroSpotContract>> {
        let conn = self.economy.open()?;
        let mut stmt = conn.prepare("SELECT * FROM micro_spots ORDER BY sponsor ASC")?;
        let contracts = stmt
            .query_map([], MicroSpotContract::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(contracts)
    }

    pub fn set_active(&self, id: &str, active: bool) -> SpotsResult<()> {
        let conn = self.economy.open()?;
        let updated = conn.execute(
            "UPDATE micro_spots SET active = ?2 WHERE id = ?1",
            params![id, if active { 1 } else { 0 }],
        )?;
        if updated == 0 {
            return Err(SpotsError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn inject_due(
        &self,
        queue: &PlayoutQueueStore,
        now: DateTime<Utc>,
    ) -> SpotsResult<Vec<MicroSpotInjection>> {
        let conn = self.economy.open()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM micro_spots
             WHERE active = 1
               AND (next_available_at IS NULL OR next_available_at <= ?1)
               AND (expires_at IS NULL OR expires_at > ?1)",
        )?;
        let mut rows = stmt.query([now.naive_utc()])?;
        let mut injections = Vec::new();
        while let Some(row) = rows.next()? {
            let contract = MicroSpotContract::from_row(row)?;
            let queue_id = queue.enqueue(&QueueItem {
                plan_id: format!("microspot:{}:{}", contract.id, now.timestamp()),
                asset_path: contract.asset_path.clone(),
                duration_s: Some(contract.duration_s),
                curation_score: Some(0.95),
                priority: 50,
                node_origin: Some("microspot".to_string()),
                content_kind: Some("microspot".to_string()),
            })?;
            let next_interval =
                random_cadence(contract.cadence_min_minutes, contract.cadence_max_minutes);
            let next_available = now + Duration::minutes(next_interval as i64);
            conn.execute(
                "UPDATE micro_spots SET
                    last_injected_at = ?2,
                    next_available_at = ?3,
                    total_injections = total_injections + 1
                 WHERE id = ?1",
                params![contract.id, now.naive_utc(), next_available.naive_utc()],
            )?;
            let mut event = NewEconomyEvent::new(
                EconomyEventType::SlotSell,
                contract.value_eur,
                contract.sponsor.clone(),
                format!("microspot:{}", contract.id),
            );
            event.timestamp = Some(now);
            event.notes = Some(contract.visual_style.clone());
            let _ = self.economy.record_event(&event);
            injections.push(MicroSpotInjection {
                contract_id: contract.id.clone(),
                queue_entry_id: queue_id,
                scheduled_at: now,
                next_available_at: Some(next_available),
            });
        }
        Ok(injections)
    }

    pub fn fetch(&self, id: &str) -> SpotsResult<MicroSpotContract> {
        let conn = self.economy.open()?;
        let contract = conn
            .query_row(
                "SELECT * FROM micro_spots WHERE id = ?1",
                [id],
                MicroSpotContract::from_row,
            )
            .map_err(|_| SpotsError::NotFound(id.to_string()))?;
        Ok(contract)
    }
}

fn parse_timestamp(value: Option<NaiveDateTime>) -> Option<DateTime<Utc>> {
    value.map(|dt| Utc.from_utc_datetime(&dt))
}

fn parse_contract_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn random_cadence(min_minutes: i64, max_minutes: i64) -> i64 {
    let min = min_minutes.max(1);
    let max = max_minutes.max(min);
    thread_rng().gen_range(min..=max)
}

fn default_cadence_min() -> i64 {
    25
}

fn default_cadence_max() -> i64 {
    40
}
