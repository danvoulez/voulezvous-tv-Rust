use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use thiserror::Error;

use crate::{
    distribution::{
        cdn::{BackupSyncReport, CdnMetrics},
        edge::EdgeLatencyRecord,
        replicator::ReplicationReport,
        security::SegmentToken,
    },
    sqlite::configure_connection,
};

const METRICS_SCHEMA: &str = include_str!("../../sql/metrics.sql");

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("failed to open metrics database {path}: {source}")]
    Open {
        source: rusqlite::Error,
        path: PathBuf,
    },
    #[error("metrics execution error: {0}")]
    Execute(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct MetricsStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl MetricsStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MonitorError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            flags: OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        })
    }

    fn open(&self) -> Result<Connection, MonitorError> {
        let conn = Connection::open_with_flags(&self.path, self.flags).map_err(|source| {
            MonitorError::Open {
                source,
                path: self.path.clone(),
            }
        })?;
        configure_connection(&conn).map_err(|source| MonitorError::Open {
            source,
            path: self.path.clone(),
        })?;
        Ok(conn)
    }

    pub fn initialize(&self) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute_batch(METRICS_SCHEMA)?;
        Ok(())
    }

    pub fn record(&self, record: &MetricRecord) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO metrics (
                buffer_duration_h, queue_length, played_last_hour, failures_last_hour,
                avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.buffer_duration_h,
                record.queue_length,
                record.played_last_hour,
                record.failures_last_hour,
                record.avg_cpu_load,
                record.avg_temp_c,
                record.latency_s,
                record.stream_bitrate_mbps,
                record.vmaf_live
            ],
        )?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<MetricSnapshot>, MonitorError> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT ts, buffer_duration_h, queue_length, played_last_hour, failures_last_hour,
                    avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live
             FROM metrics ORDER BY ts DESC LIMIT 1",
        )?;
        let snapshot = stmt
            .query_row([], |row| MetricSnapshot::from_row(row))
            .optional()?;
        Ok(snapshot)
    }

    pub fn history(&self, limit: usize) -> Result<Vec<MetricSnapshot>, MonitorError> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT ts, buffer_duration_h, queue_length, played_last_hour, failures_last_hour,
                    avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live
             FROM metrics ORDER BY ts DESC LIMIT ?1",
        )?;
        let mut rows = stmt.query([limit as i64])?;
        let mut snapshots = Vec::new();
        while let Some(row) = rows.next()? {
            snapshots.push(MetricSnapshot::from_row(row)?);
        }
        snapshots.reverse();
        Ok(snapshots)
    }

    pub fn record_replication_report(
        &self,
        report: &ReplicationReport,
    ) -> Result<(), MonitorError> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        for sync in &report.syncs {
            tx.execute(
                "INSERT INTO replication_syncs (path, bytes_transferred, duration_ms)
                 VALUES (?1, ?2, ?3)",
                params![
                    sync.path.to_string_lossy(),
                    sync.bytes_transferred as i64,
                    sync.duration_ms as i64,
                ],
            )?;
        }
        let check = &report.check;
        tx.execute(
            "INSERT INTO replication_events (path, differences, total_files, drift_percent, failover_triggered)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                check.path.to_string_lossy(),
                check.differences as i64,
                check.total_files as i64,
                check.drift_percent,
                if check.triggered_failover { 1 } else { 0 },
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_cdn_metrics(&self, metrics: &CdnMetrics) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO cdn_metrics (provider, cdn_hits, latency_avg_ms, cache_hit_rate, origin_errors)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                metrics.provider.clone(),
                metrics.cdn_hits as i64,
                metrics.latency_avg_ms,
                metrics.cache_hit_rate,
                metrics.origin_errors as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_backup_sync(&self, report: &BackupSyncReport) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO backup_syncs (provider, files_uploaded, bytes_uploaded, removed_segments, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                report.provider.clone(),
                report.files_uploaded as i64,
                report.bytes_uploaded as i64,
                serde_json::to_string(&report.removed_segments)?,
                report.duration_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_edge_latency(&self, record: &EdgeLatencyRecord) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO edge_latency (region, target, latency_ms)
             VALUES (?1, ?2, ?3)",
            params![
                record.region.clone(),
                record.target.clone(),
                record.latency_ms
            ],
        )?;
        Ok(())
    }

    pub fn record_cdn_token(&self, token: &SegmentToken) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO cdn_tokens (path, token, expires_at)
             VALUES (?1, ?2, ?3)",
            params![
                token.path.clone(),
                token.token.clone(),
                token.expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricRecord {
    pub buffer_duration_h: f64,
    pub queue_length: i64,
    pub played_last_hour: i64,
    pub failures_last_hour: i64,
    pub avg_cpu_load: f64,
    pub avg_temp_c: f64,
    pub latency_s: f64,
    pub stream_bitrate_mbps: f64,
    pub vmaf_live: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSnapshot {
    pub timestamp: DateTime<Utc>,
    pub buffer_duration_h: f64,
    pub queue_length: i64,
    pub played_last_hour: i64,
    pub failures_last_hour: i64,
    pub avg_cpu_load: f64,
    pub avg_temp_c: f64,
    pub latency_s: f64,
    pub stream_bitrate_mbps: f64,
    pub vmaf_live: f64,
}

impl MetricSnapshot {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let ts: NaiveDateTime = row.get("ts")?;
        Ok(Self {
            timestamp: DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc),
            buffer_duration_h: row.get("buffer_duration_h")?,
            queue_length: row.get("queue_length")?,
            played_last_hour: row.get("played_last_hour")?,
            failures_last_hour: row.get("failures_last_hour")?,
            avg_cpu_load: row.get("avg_cpu_load")?,
            avg_temp_c: row.get("avg_temp_c")?,
            latency_s: row.get("latency_s")?,
            stream_bitrate_mbps: row.get("stream_bitrate_mbps")?,
            vmaf_live: row.get("vmaf_live")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DashboardGenerator {
    store: MetricsStore,
    output_path: PathBuf,
}

impl DashboardGenerator {
    pub fn new(store: MetricsStore, output_path: impl AsRef<Path>) -> Self {
        Self {
            store,
            output_path: output_path.as_ref().to_path_buf(),
        }
    }

    pub fn generate(&self, limit: usize) -> Result<(), MonitorError> {
        let history = self.store.history(limit)?;
        let latest = history.last().cloned();
        let json = serde_json::to_string(&history)?;
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let alerts = self.collect_alerts(&history);
        let html = self.render_html(latest, &json, &alerts);
        fs::write(&self.output_path, html)?;
        Ok(())
    }

    fn collect_alerts(&self, history: &[MetricSnapshot]) -> Vec<String> {
        let mut alerts = Vec::new();
        if let Some(last) = history.last() {
            if last.buffer_duration_h < 1.0 {
                alerts.push(format!(
                    "Buffer abaixo de 1h ({:.2}h)",
                    last.buffer_duration_h
                ));
            }
            if last.stream_bitrate_mbps < 1.0 {
                alerts.push(format!(
                    "Bitrate reduzido: {:.2} Mbps",
                    last.stream_bitrate_mbps
                ));
            }
            if last.failures_last_hour > 0 {
                alerts.push(format!(
                    "Falhas na última hora: {}",
                    last.failures_last_hour
                ));
            }
        }
        alerts
    }

    fn render_html(
        &self,
        latest: Option<MetricSnapshot>,
        history_json: &str,
        alerts: &[String],
    ) -> String {
        let summary_rows = if let Some(snapshot) = latest {
            format!(
                "<tr><th>Timestamp</th><td>{}</td></tr>
                 <tr><th>Buffer</th><td>{:.2} h</td></tr>
                 <tr><th>Queue Length</th><td>{}</td></tr>
                 <tr><th>Failures (1h)</th><td>{}</td></tr>
                 <tr><th>Latency</th><td>{:.2} s</td></tr>
                 <tr><th>Bitrate</th><td>{:.2} Mbps</td></tr>
                 <tr><th>VMAF</th><td>{:.1}</td></tr>
                 <tr><th>CPU</th><td>{:.1}%</td></tr>
                 <tr><th>Temp.</th><td>{:.1} °C</td></tr>",
                snapshot.timestamp.to_rfc3339(),
                snapshot.buffer_duration_h,
                snapshot.queue_length,
                snapshot.failures_last_hour,
                snapshot.latency_s,
                snapshot.stream_bitrate_mbps,
                snapshot.vmaf_live,
                snapshot.avg_cpu_load,
                snapshot.avg_temp_c
            )
        } else {
            "<tr><td colspan=2>Sem dados</td></tr>".to_string()
        };
        let alerts_html = if alerts.is_empty() {
            "<li>Nenhum alerta ativo</li>".to_string()
        } else {
            alerts
                .iter()
                .map(|alert| format!("<li>{}</li>", alert))
                .collect::<Vec<_>>()
                .join("")
        };
        format!(
            "<!DOCTYPE html>
<html lang=\"pt-BR\">
<head>
<meta charset=\"utf-8\">
<title>VVTV Monitor</title>
<style>
body {{ font-family: 'Inter', sans-serif; background: #0b0c10; color: #f8f8f2; margin: 0; padding: 2rem; }}
section {{ margin-bottom: 2rem; }}
h1 {{ font-size: 1.8rem; margin-bottom: 0.5rem; }}
table {{ border-collapse: collapse; width: 100%; max-width: 420px; }}
th, td {{ border-bottom: 1px solid #444; padding: 0.4rem 0.6rem; text-align: left; }}
#chart {{ width: 100%; height: 240px; background: #16181d; border-radius: 8px; position: relative; }}
#chart canvas {{ width: 100%; height: 100%; }}
.alerts li {{ margin-bottom: 0.3rem; }}
</style>
</head>
<body>
<h1>VoulezVous.TV — Dashboard Local</h1>
<section>
  <h2>Status Atual</h2>
  <table>{summary_rows}</table>
</section>
<section>
  <h2>Alertas</h2>
  <ul class=\"alerts\">{alerts_html}</ul>
</section>
<section>
  <h2>Buffer (últimas medições)</h2>
  <div id=\"chart\"><canvas id=\"bufferChart\"></canvas></div>
</section>
<script>
const history = {history_json};
const canvas = document.getElementById('bufferChart');
const ctx = canvas.getContext('2d');
const width = canvas.width = canvas.offsetWidth;
const height = canvas.height = canvas.offsetHeight;
ctx.clearRect(0, 0, width, height);
ctx.fillStyle = '#1f2128';
ctx.fillRect(0, 0, width, height);
ctx.strokeStyle = '#444';
ctx.lineWidth = 1;
ctx.beginPath();
ctx.moveTo(0, height - 1);
ctx.lineTo(width, height - 1);
ctx.stroke();
if (history.length > 1) {{
  const buffers = history.map(item => item.buffer_duration_h);
  const maxBuffer = Math.max(...buffers, 1);
  ctx.strokeStyle = '#e94560';
  ctx.lineWidth = 2;
  ctx.beginPath();
  history.forEach((item, index) => {{
    const x = (index / (history.length - 1)) * (width - 10) + 5;
    const y = height - (item.buffer_duration_h / maxBuffer) * (height - 10) - 5;
    if (index === 0) {{ ctx.moveTo(x, y); }} else {{ ctx.lineTo(x, y); }}
  }});
  ctx.stroke();
}}
</script>
</body>
</html>",
            summary_rows = summary_rows,
            alerts_html = alerts_html,
            history_json = history_json
        )
    }
}
