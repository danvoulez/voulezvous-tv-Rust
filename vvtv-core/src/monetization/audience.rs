use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use image::{ImageBuffer, ImageError, Rgb};
use rusqlite::{params, Connection, OpenFlags, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sqlite::configure_connection;

const VIEWERS_SCHEMA: &str = include_str!("../../../sql/viewers.sql");

#[derive(Debug, Error)]
pub enum AudienceError {
    #[error("failed to open viewers database {path}: {source}")]
    Open {
        source: rusqlite::Error,
        path: PathBuf,
    },
    #[error("failed to execute statement on viewers database: {0}")]
    Execute(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("image error: {0}")]
    Image(#[from] ImageError),
    #[error("audience store path not configured")]
    MissingStore,
}

pub type AudienceResult<T> = Result<T, AudienceError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSession {
    pub id: i64,
    pub session_id: String,
    pub viewer_id: Option<String>,
    pub join_time: DateTime<Utc>,
    pub leave_time: DateTime<Utc>,
    pub duration_seconds: i64,
    pub region: String,
    pub device: String,
    pub bandwidth_mbps: Option<f64>,
    pub engagement_score: Option<f64>,
    pub notes: Option<String>,
}

impl ViewerSession {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let join: NaiveDateTime = row.get("join_time")?;
        let leave: NaiveDateTime = row.get("leave_time")?;
        Ok(Self {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            viewer_id: row.get("viewer_id")?,
            join_time: Utc.from_utc_datetime(&join),
            leave_time: Utc.from_utc_datetime(&leave),
            duration_seconds: row.get("duration_seconds")?,
            region: row.get("region")?,
            device: row.get("device")?,
            bandwidth_mbps: row.get("bandwidth_mbps")?,
            engagement_score: row.get("engagement_score")?,
            notes: row.get("notes")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewViewerSession {
    pub session_id: String,
    pub viewer_id: Option<String>,
    pub join_time: DateTime<Utc>,
    pub leave_time: Option<DateTime<Utc>>,
    pub region: String,
    pub device: String,
    pub bandwidth_mbps: Option<f64>,
    pub engagement_score: Option<f64>,
    pub notes: Option<String>,
}

impl NewViewerSession {
    pub fn new(
        session_id: impl Into<String>,
        region: impl Into<String>,
        device: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            viewer_id: None,
            join_time: Utc::now(),
            leave_time: None,
            region: region.into(),
            device: device.into(),
            bandwidth_mbps: None,
            engagement_score: None,
            notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AudienceMetrics {
    pub total_sessions: usize,
    pub avg_duration_minutes: f64,
    pub retention_5min: f64,
    pub retention_30min: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegionEngagement {
    pub region: String,
    pub sessions: usize,
    pub avg_duration_minutes: f64,
    pub avg_engagement: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceReport {
    pub metrics: AudienceMetrics,
    pub regions: Vec<RegionEngagement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceSnapshot {
    pub metrics: AudienceMetrics,
    pub regions: Vec<RegionEngagement>,
    pub aggregate_vector: DesireVector,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DesireVector {
    pub values: [f32; 5],
}

impl DesireVector {
    pub fn neutral() -> Self {
        Self {
            values: [0.5, 0.5, 0.5, 0.5, 0.5],
        }
    }

    pub fn dot(&self, other: &Self) -> f32 {
        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    pub fn scale(&self, factor: f32) -> Self {
        let mut values = self.values;
        for value in &mut values {
            *value *= factor;
        }
        Self { values }
    }

    pub fn normalize(&mut self) {
        let max = self.values.iter().cloned().fold(0.0_f32, f32::max);
        if max > 0.0 {
            for value in &mut self.values {
                *value /= max;
            }
        }
    }

    pub fn blend(&self, other: &Self, weight: f32) -> Self {
        let mut values = self.values;
        for (value, other_value) in values.iter_mut().zip(other.values.iter()) {
            *value = (*value * (1.0 - weight)) + other_value * weight;
        }
        Self { values }
    }
}

#[derive(Debug, Clone)]
pub struct AudienceStoreBuilder {
    path: Option<PathBuf>,
    read_only: bool,
    create_if_missing: bool,
}

impl Default for AudienceStoreBuilder {
    fn default() -> Self {
        Self {
            path: None,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl AudienceStoreBuilder {
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

    pub fn build(self) -> AudienceResult<AudienceStore> {
        let path = self.path.ok_or(AudienceError::MissingStore)?;
        let mut flags = if self.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if !self.read_only && self.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        Ok(AudienceStore { path, flags })
    }
}

#[derive(Debug, Clone)]
pub struct AudienceStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl AudienceStore {
    pub fn builder() -> AudienceStoreBuilder {
        AudienceStoreBuilder::new()
    }

    pub fn new(path: impl AsRef<Path>) -> AudienceResult<Self> {
        AudienceStoreBuilder::new().path(path).build()
    }

    fn open(&self) -> AudienceResult<Connection> {
        let conn = Connection::open_with_flags(&self.path, self.flags).map_err(|source| {
            AudienceError::Open {
                source,
                path: self.path.clone(),
            }
        })?;
        configure_connection(&conn).map_err(|source| AudienceError::Open {
            source,
            path: self.path.clone(),
        })?;
        Ok(conn)
    }

    pub fn initialize(&self) -> AudienceResult<()> {
        let conn = self.open()?;
        conn.execute_batch(VIEWERS_SCHEMA)?;
        Ok(())
    }

    pub fn record_session(&self, session: &NewViewerSession) -> AudienceResult<ViewerSession> {
        let conn = self.open()?;
        let join = session.join_time;
        let leave = session
            .leave_time
            .unwrap_or_else(|| session.join_time + Duration::minutes(1));
        let duration_seconds = (leave - join).num_seconds().max(0);
        conn.execute(
            "INSERT INTO viewer_sessions (
                session_id, viewer_id, join_time, leave_time, duration_seconds,
                region, device, bandwidth_mbps, engagement_score, notes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &session.session_id,
                &session.viewer_id,
                join.naive_utc(),
                leave.naive_utc(),
                duration_seconds,
                &session.region,
                &session.device,
                &session.bandwidth_mbps,
                &session.engagement_score,
                &session.notes
            ],
        )?;
        let id = conn.last_insert_rowid();
        let row = conn.query_row(
            "SELECT * FROM viewer_sessions WHERE id = ?1",
            [id],
            ViewerSession::from_row,
        )?;
        Ok(row)
    }

    pub fn metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> AudienceResult<AudienceReport> {
        let conn = self.open()?;
        let mut metrics = AudienceMetrics::default();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), COALESCE(AVG(duration_seconds), 0),
                    SUM(CASE WHEN duration_seconds >= 300 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN duration_seconds >= 1800 THEN 1 ELSE 0 END)
             FROM viewer_sessions
             WHERE join_time >= ?1 AND join_time < ?2",
        )?;
        let row = stmt.query_row(params![start.naive_utc(), end.naive_utc()], |row| {
            let total: i64 = row.get(0)?;
            let avg_duration: f64 = row.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
            let retention_5: f64 = row.get::<_, Option<f64>>(2)?.unwrap_or(0.0);
            let retention_30: f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
            Ok((total, avg_duration, retention_5, retention_30))
        })?;
        metrics.total_sessions = row.0 as usize;
        metrics.avg_duration_minutes = if metrics.total_sessions > 0 {
            row.1 / 60.0
        } else {
            0.0
        };
        if metrics.total_sessions > 0 {
            metrics.retention_5min = row.2 / metrics.total_sessions as f64;
            metrics.retention_30min = row.3 / metrics.total_sessions as f64;
        }

        let mut region_stmt = conn.prepare(
            "SELECT region, COUNT(*), COALESCE(AVG(duration_seconds), 0), COALESCE(AVG(engagement_score), 0)
             FROM viewer_sessions
             WHERE join_time >= ?1 AND join_time < ?2
             GROUP BY region
             ORDER BY COUNT(*) DESC",
        )?;
        let mut region_rows = region_stmt.query(params![start.naive_utc(), end.naive_utc()])?;
        let mut regions = Vec::new();
        while let Some(row) = region_rows.next()? {
            let region: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let avg_duration: f64 = row.get::<_, Option<f64>>(2)?.unwrap_or(0.0);
            let avg_engagement: f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
            regions.push(RegionEngagement {
                region,
                sessions: count as usize,
                avg_duration_minutes: avg_duration / 60.0,
                avg_engagement,
            });
        }

        Ok(AudienceReport { metrics, regions })
    }

    pub fn generate_heatmap<P: AsRef<Path>>(
        &self,
        report: &AudienceReport,
        output: P,
    ) -> AudienceResult<PathBuf> {
        let width = 640u32;
        let height = 360u32;
        let mut image = ImageBuffer::from_pixel(width, height, Rgb([12, 12, 24]));
        if report.metrics.total_sessions == 0 {
            image.save(output.as_ref())?;
            return Ok(output.as_ref().to_path_buf());
        }
        let cells_per_row = 8u32;
        let cell_width = width / cells_per_row.max(1);
        let rows = ((report.regions.len() as u32 + cells_per_row - 1) / cells_per_row).max(1);
        let cell_height = (height / rows).max(40);
        let mut idx = 0u32;
        let max_sessions = report
            .regions
            .iter()
            .map(|region| region.sessions as u32)
            .max()
            .unwrap_or(1);
        for region in &report.regions {
            let x = (idx % cells_per_row) * cell_width;
            let y = (idx / cells_per_row) * cell_height;
            let intensity = (region.sessions as f32 / max_sessions as f32).clamp(0.0, 1.0);
            let color = Rgb([
                (20.0 + intensity * 180.0) as u8,
                (32.0 + intensity * 160.0) as u8,
                (80.0 + intensity * 100.0) as u8,
            ]);
            for px in x..(x + cell_width).min(width) {
                for py in y..(y + cell_height).min(height) {
                    image.put_pixel(px, py, color);
                }
            }
            idx += 1;
        }
        image.save(output.as_ref())?;
        Ok(output.as_ref().to_path_buf())
    }

    pub fn export_report<P: AsRef<Path>>(
        &self,
        report: &AudienceReport,
        output: P,
    ) -> AudienceResult<PathBuf> {
        let serialized = serde_json::to_string_pretty(report)?;
        let mut file = File::create(output.as_ref())?;
        file.write_all(serialized.as_bytes())?;
        Ok(output.as_ref().to_path_buf())
    }

    pub fn snapshot(&self, window: Duration) -> AudienceResult<AudienceSnapshot> {
        let end = Utc::now();
        let start = end - window;
        let report = self.metrics(start, end)?;
        let aggregate_vector = aggregate_desire(&report);
        Ok(AudienceSnapshot {
            metrics: report.metrics.clone(),
            regions: report.regions,
            aggregate_vector,
        })
    }
}

fn aggregate_desire(report: &AudienceReport) -> DesireVector {
    if report.metrics.total_sessions == 0 {
        return DesireVector::neutral();
    }
    let mut values = [0.0f32; 5];
    let mut total_weight = 0.0f32;
    for region in &report.regions {
        let profile = region_profile(&region.region);
        let weight = region.sessions as f32 * (region.avg_engagement as f32 + 1.0);
        for (slot, component) in values.iter_mut().zip(profile.values.iter()) {
            *slot += component * weight;
        }
        total_weight += weight;
    }
    if total_weight <= 0.0 {
        return DesireVector::neutral();
    }
    for value in &mut values {
        *value /= total_weight;
    }
    let mut vector = DesireVector { values };
    vector.normalize();
    vector
}

fn region_profile(region: &str) -> DesireVector {
    let region = region.to_lowercase();
    if region.contains("eu") {
        DesireVector {
            values: [0.7, 0.65, 0.6, 0.55, 0.75],
        }
    } else if region.contains("na") {
        DesireVector {
            values: [0.6, 0.55, 0.7, 0.5, 0.58],
        }
    } else if region.contains("sa") || region.contains("latam") {
        DesireVector {
            values: [0.75, 0.6, 0.65, 0.62, 0.7],
        }
    } else if region.contains("apac") || region.contains("asia") {
        DesireVector {
            values: [0.58, 0.72, 0.6, 0.55, 0.68],
        }
    } else if region.contains("af") {
        DesireVector {
            values: [0.62, 0.57, 0.64, 0.6, 0.66],
        }
    } else {
        DesireVector::neutral()
    }
}
