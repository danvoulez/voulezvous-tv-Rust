use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

use crate::{
    distribution::{
        cdn::{BackupSyncReport, CdnMetrics},
        edge::EdgeLatencyRecord,
        replicator::ReplicationReport,
        security::SegmentToken,
    },
    quality::{QualityAnalyzer, QualityThresholds, SignatureProfile},
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
    #[error("quality analysis error: {0}")]
    Quality(#[from] crate::quality::QualityError),
}

/// Business metrics for P6 observability requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetric {
    pub timestamp: DateTime<Utc>,
    pub metric_type: BusinessMetricType,
    pub value: f64,
    pub context: serde_json::Value,
}

impl BusinessMetric {
    pub fn new(metric_type: BusinessMetricType, value: f64) -> Self {
        Self {
            timestamp: Utc::now(),
            metric_type,
            value,
            context: serde_json::Value::Null,
        }
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }
}

/// Business metric types for P6 observability
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusinessMetricType {
    SelectionEntropy,
    CuratorApplyBudgetUsedPct,
    NoveltyTemporalKld,
    HdDetectionSlowRate,
    AutopilotPredVsRealError,
}

#[derive(Debug, Clone)]
pub struct LiveQcSample {
    pub capture_path: PathBuf,
    pub stream_bitrate_mbps: f64,
    pub vmaf_estimate: f64,
    pub audio_peak_db: f64,
    pub freeze_events: u32,
    pub black_ratio: f64,
    pub signature_deviation: f64,
}

impl LiveQcSample {
    fn new(
        capture_path: PathBuf,
        metrics: LiveImageMetrics,
        signature_deviation: f64,
        stream_bitrate_mbps: f64,
    ) -> Self {
        Self {
            capture_path,
            stream_bitrate_mbps,
            vmaf_estimate: metrics.vmaf_estimate,
            audio_peak_db: metrics.audio_peak_db,
            freeze_events: metrics.freeze_events,
            black_ratio: metrics.black_ratio,
            signature_deviation,
        }
    }
}

#[derive(Clone)]
pub struct LiveQualityCollector {
    analyzer: QualityAnalyzer,
    stream_url: String,
    capture_dir: PathBuf,
    timeout: Duration,
}

impl LiveQualityCollector {
    pub fn new(
        thresholds: QualityThresholds,
        profile: SignatureProfile,
        stream_url: impl Into<String>,
        capture_dir: impl AsRef<Path>,
    ) -> Self {
        let analyzer = QualityAnalyzer::new(thresholds, Arc::new(profile));
        Self {
            analyzer,
            stream_url: stream_url.into(),
            capture_dir: capture_dir.as_ref().to_path_buf(),
            timeout: Duration::from_secs(25),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn collect(&self) -> Result<LiveQcSample, MonitorError> {
        fs::create_dir_all(&self.capture_dir)?;
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
        let capture_path = self.capture_dir.join(format!("capture_{timestamp}.png"));
        let mut command = Command::new("ffmpeg");
        command
            .kill_on_drop(true)
            .arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(&self.stream_url)
            .arg("-frames:v")
            .arg("1")
            .arg("-vf")
            .arg("scale=640:-1")
            .arg(&capture_path);
        let executed = timeout(self.timeout, command.status()).await;
        let mut placeholder = true;
        if let Ok(Ok(status)) = executed {
            if status.success() {
                placeholder = false;
            }
        }
        if placeholder {
            self.generate_placeholder(&capture_path)?;
        }

        let current = image::open(&capture_path).map_err(|err| {
            MonitorError::Quality(crate::quality::QualityError::Image(err.to_string()))
        })?;
        let previous = self
            .find_previous_capture(&capture_path)?
            .and_then(|path| image::open(path).ok());
        let metrics = self.compute_image_metrics(&current, previous.as_ref());
        let signature = self
            .analyzer
            .analyze_signature(&capture_path, placeholder)
            .map_err(MonitorError::from)?;
        let bitrate = self
            .probe_bitrate()
            .await
            .unwrap_or(metrics.estimated_bitrate_mbps);
        Ok(LiveQcSample::new(
            capture_path,
            metrics,
            signature.signature_deviation,
            bitrate,
        ))
    }

    async fn probe_bitrate(&self) -> Option<f64> {
        let mut command = Command::new("ffprobe");
        command
            .kill_on_drop(true)
            .arg("-v")
            .arg("quiet")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=bit_rate")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(&self.stream_url);
        let Ok(result) = timeout(self.timeout, command.output()).await else {
            return None;
        };
        let Ok(output) = result else {
            return None;
        };
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .and_then(|raw| raw.parse::<f64>().ok())?;
        Some(value / 1_000_000.0)
    }

    fn find_previous_capture(&self, current: &Path) -> Result<Option<PathBuf>, MonitorError> {
        let mut entries = fs::read_dir(&self.capture_dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path == current || !path.is_file() {
                    return None;
                }
                Some(path)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok());
        Ok(entries.pop())
    }

    fn generate_placeholder(&self, path: &Path) -> Result<(), MonitorError> {
        let mut buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(320, 180);
        for (x, y, pixel) in buffer.enumerate_pixels_mut() {
            let fx = x as f32 / 320.0;
            let fy = y as f32 / 180.0;
            *pixel = Rgb([
                (32.0 + 120.0 * fx) as u8,
                (24.0 + 80.0 * (1.0 - fx)) as u8,
                (48.0 + 70.0 * fy) as u8,
            ]);
        }
        buffer.save(path).map_err(|err| {
            MonitorError::Quality(crate::quality::QualityError::Image(err.to_string()))
        })
    }

    fn compute_image_metrics(
        &self,
        current: &DynamicImage,
        previous: Option<&DynamicImage>,
    ) -> LiveImageMetrics {
        let mut black_pixels = 0u64;
        let mut total_pixels = 0u64;
        let mut luma_total = 0f64;
        let mut luma_sq_total = 0f64;
        for pixel in current.pixels() {
            let rgb = pixel.2 .0;
            let luma =
                0.2126 * (rgb[0] as f64) + 0.7152 * (rgb[1] as f64) + 0.0722 * (rgb[2] as f64);
            if luma < 10.0 {
                black_pixels += 1;
            }
            luma_total += luma;
            luma_sq_total += luma * luma;
            total_pixels += 1;
        }
        let mean = if total_pixels == 0 {
            0.0
        } else {
            luma_total / total_pixels as f64
        };
        let variance = if total_pixels == 0 {
            0.0
        } else {
            (luma_sq_total / total_pixels as f64) - (mean * mean)
        };
        let freeze_events = if let Some(prev) = previous {
            let diff = self.frame_difference(current, prev);
            if diff < 0.02 {
                1
            } else {
                0
            }
        } else {
            0
        };
        let black_ratio = if total_pixels == 0 {
            0.0
        } else {
            black_pixels as f64 / total_pixels as f64
        };
        let vmaf_estimate =
            (95.0 - black_ratio * 50.0 - freeze_events as f64 * 5.0).clamp(60.0, 99.0);
        LiveImageMetrics {
            black_ratio,
            freeze_events,
            audio_peak_db: -1.0 + variance.sqrt() * 0.01,
            vmaf_estimate,
            estimated_bitrate_mbps: 3.0 + (1.0 - black_ratio) * 1.5,
        }
    }

    fn frame_difference(&self, current: &DynamicImage, previous: &DynamicImage) -> f64 {
        let resized_a = current.resize(160, 90, image::imageops::FilterType::Triangle);
        let resized_b = previous.resize(160, 90, image::imageops::FilterType::Triangle);
        let mut diff = 0u64;
        let mut total = 0u64;
        for (pixel_a, pixel_b) in resized_a.pixels().zip(resized_b.pixels()) {
            let a = pixel_a.2 .0;
            let b = pixel_b.2 .0;
            let delta = a
                .iter()
                .zip(b.iter())
                .map(|(a, b)| (*a as i16 - *b as i16).unsigned_abs() as u64)
                .sum::<u64>();
            if delta > 30 {
                diff += 1;
            }
            total += 1;
        }
        if total == 0 {
            0.0
        } else {
            diff as f64 / total as f64
        }
    }
}

struct LiveImageMetrics {
    black_ratio: f64,
    freeze_events: u32,
    audio_peak_db: f64,
    vmaf_estimate: f64,
    estimated_bitrate_mbps: f64,
}

pub struct QcReportGenerator {
    store: MetricsStore,
    reports_dir: PathBuf,
}

impl QcReportGenerator {
    pub fn new(store: MetricsStore, reports_dir: impl AsRef<Path>) -> Self {
        Self {
            store,
            reports_dir: reports_dir.as_ref().to_path_buf(),
        }
    }

    pub fn generate_daily(&self) -> Result<PathBuf, MonitorError> {
        self.generate_for_date(None)
    }

    pub fn generate_for_date(
        &self,
        date: Option<chrono::NaiveDate>,
    ) -> Result<PathBuf, MonitorError> {
        let target_date = date.unwrap_or_else(|| Utc::now().date_naive());
        let history = self.store.history(288)?;
        let total = history.len() as f64;
        let avg = |f: Box<dyn Iterator<Item = f64>>| {
            if total > 0.0 {
                f.sum::<f64>() / total
            } else {
                0.0
            }
        };
        let avg_bitrate = avg(Box::new(history.iter().map(|s| s.stream_bitrate_mbps)));
        let avg_vmaf = avg(Box::new(history.iter().map(|s| s.vmaf_live)));
        let avg_signature = avg(Box::new(history.iter().map(|s| s.signature_deviation)));
        let avg_black = avg(Box::new(history.iter().map(|s| s.black_frame_ratio)));
        let critical_events = history
            .iter()
            .filter(|s| s.vmaf_live < 80.0 || s.black_frame_ratio > 0.2 || s.freeze_events > 0)
            .count();
        fs::create_dir_all(&self.reports_dir)?;
        let report_path = self
            .reports_dir
            .join(format!("qc_report_{}.json", target_date));
        let payload = serde_json::json!({
            "date": target_date.to_string(),
            "total_samples": history.len(),
            "averages": {
                "bitrate_mbps": avg_bitrate,
                "vmaf": avg_vmaf,
                "signature_deviation": avg_signature,
                "black_ratio": avg_black,
            },
            "critical_events": critical_events,
            "samples": history.iter().map(|snapshot| serde_json::json!({
                "timestamp": snapshot.timestamp.to_rfc3339(),
                "bitrate_mbps": snapshot.stream_bitrate_mbps,
                "vmaf": snapshot.vmaf_live,
                "black_ratio": snapshot.black_frame_ratio,
                "freeze_events": snapshot.freeze_events,
                "signature_deviation": snapshot.signature_deviation,
            })).collect::<Vec<_>>()
        });
        fs::write(&report_path, serde_json::to_vec_pretty(&payload)?)?;
        Ok(report_path)
    }
}

pub struct VisualReviewPanel {
    capture_dir: PathBuf,
    output_path: PathBuf,
}

impl VisualReviewPanel {
    pub fn new(capture_dir: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Self {
        Self {
            capture_dir: capture_dir.as_ref().to_path_buf(),
            output_path: output_path.as_ref().to_path_buf(),
        }
    }

    pub fn render(&self) -> Result<(), MonitorError> {
        let captures = self.recent_captures(4)?;
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let questions = vec![
            "O enquadramento mantém o padrão VoulezVous?",
            "A paleta respeita a assinatura cromática?",
            "Há ruídos visuais ou artefatos?",
            "O áudio aparente está balanceado?",
            "Existe alguma indicação de congelamento?",
            "O conteúdo mantém consistência narrativa?",
        ];
        let question_items = questions
            .into_iter()
            .map(|q| format!("<li>{}</li>", q))
            .collect::<Vec<_>>()
            .join("");
        let capture_items = captures
            .iter()
            .map(|path| {
                format!(
                    "<div class=\"frame\"><img src=\"file://{}\" alt=\"QC frame\"></div>",
                    path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("");
        let html = format!(
            "<!DOCTYPE html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n<title>Revisão Visual QC</title>\n<style>\nbody {{ font-family: 'Inter', sans-serif; background: #0f1014; color: #f6f6f6; margin: 0; padding: 2rem; }}\n.gallery {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; }}\n.frame {{ background: #16181f; padding: 0.5rem; border-radius: 6px; }}\n.frame img {{ width: 100%; border-radius: 4px; }}\nol {{ line-height: 1.6; }}\n</style>\n</head>\n<body>\n<h1>Painel de Revisão QC</h1>\n<section>\n  <h2>Capturas recentes</h2>\n  <div class=\"gallery\">{captures}</div>\n</section>\n<section>\n  <h2>Checklist</h2>\n  <ol>{questions}</ol>\n</section>\n</body>\n</html>",
            captures = capture_items,
            questions = question_items
        );
        fs::write(&self.output_path, html)?;
        Ok(())
    }

    fn recent_captures(&self, limit: usize) -> Result<Vec<PathBuf>, MonitorError> {
        let mut captures = fs::read_dir(&self.capture_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        captures.sort_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok());
        captures.reverse();
        captures.truncate(limit);
        Ok(captures)
    }
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
                avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live,
                audio_peak_db, freeze_events, black_frame_ratio, signature_deviation,
                power_watts, ups_runtime_minutes, ups_charge_percent, ups_status,
                ssd_wear_percent, gpu_temp_c, ssd_temp_c, fan_rpm
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                record.buffer_duration_h,
                record.queue_length,
                record.played_last_hour,
                record.failures_last_hour,
                record.avg_cpu_load,
                record.avg_temp_c,
                record.latency_s,
                record.stream_bitrate_mbps,
                record.vmaf_live,
                record.audio_peak_db,
                record.freeze_events,
                record.black_frame_ratio,
                record.signature_deviation,
                record.power_watts,
                record.ups_runtime_minutes,
                record.ups_charge_percent,
                record.ups_status.as_deref(),
                record.ssd_wear_percent,
                record.gpu_temp_c,
                record.ssd_temp_c,
                record.fan_rpm
            ],
        )?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<MetricSnapshot>, MonitorError> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT ts, buffer_duration_h, queue_length, played_last_hour, failures_last_hour,
                    avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live,
                    audio_peak_db, freeze_events, black_frame_ratio, signature_deviation,
                    power_watts, ups_runtime_minutes, ups_charge_percent, ups_status,
                    ssd_wear_percent, gpu_temp_c, ssd_temp_c, fan_rpm
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
                    avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live,
                    audio_peak_db, freeze_events, black_frame_ratio, signature_deviation,
                    power_watts, ups_runtime_minutes, ups_charge_percent, ups_status,
                    ssd_wear_percent, gpu_temp_c, ssd_temp_c, fan_rpm
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

    /// Record a business metric for P6 observability
    pub fn record_business_metric(&self, metric: &BusinessMetric) -> Result<(), MonitorError> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO business_metrics (timestamp, metric_type, value, context)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                metric.timestamp.to_rfc3339(),
                format!("{:?}", metric.metric_type).to_lowercase(),
                metric.value,
                serde_json::to_string(&metric.context)?,
            ],
        )?;
        Ok(())
    }

    /// Record business metric with error handling that doesn't fail main operations
    pub async fn record_metric_safe(&self, metric: BusinessMetric) -> Result<(), MonitorError> {
        match self.record_business_metric(&metric) {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::warn!(target: "metrics", "failed to record metric: {e}");
                // In production, could queue for retry here
                Ok(())
            }
        }
    }

    /// Query business metrics by type and time range
    pub fn query_business_metrics(
        &self,
        metric_type: BusinessMetricType,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<BusinessMetric>, MonitorError> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT timestamp, metric_type, value, context 
             FROM business_metrics 
             WHERE metric_type = ?1 AND timestamp BETWEEN ?2 AND ?3
             ORDER BY timestamp ASC",
        )?;
        
        let metric_type_str = format!("{:?}", metric_type).to_lowercase();
        let mut rows = stmt.query(params![
            metric_type_str,
            start.to_rfc3339(),
            end.to_rfc3339(),
        ])?;
        
        let mut metrics = Vec::new();
        while let Some(row) = rows.next()? {
            let timestamp_str: String = row.get("timestamp")?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "timestamp".to_string(), rusqlite::types::Type::Text))?
                .with_timezone(&Utc);
            let context_str: String = row.get("context")?;
            let context: serde_json::Value = serde_json::from_str(&context_str)?;
            
            metrics.push(BusinessMetric {
                timestamp,
                metric_type,
                value: row.get("value")?,
                context,
            });
        }
        Ok(metrics)
    }

    /// Cleanup expired business metrics based on retention policy
    pub fn cleanup_expired_business_metrics(&self, retention_days: u32) -> Result<usize, MonitorError> {
        let conn = self.open()?;
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let deleted = conn.execute(
            "DELETE FROM business_metrics WHERE timestamp < ?1",
            params![cutoff.to_rfc3339()],
        )?;
        Ok(deleted)
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
    pub audio_peak_db: f64,
    pub freeze_events: i64,
    pub black_frame_ratio: f64,
    pub signature_deviation: f64,
    pub power_watts: Option<f64>,
    pub ups_runtime_minutes: Option<f64>,
    pub ups_charge_percent: Option<f64>,
    pub ups_status: Option<String>,
    pub ssd_wear_percent: Option<f64>,
    pub gpu_temp_c: Option<f64>,
    pub ssd_temp_c: Option<f64>,
    pub fan_rpm: Option<f64>,
}

impl MetricRecord {
    pub fn with_live_sample(mut self, sample: &LiveQcSample) -> Self {
        self.apply_live_sample(sample);
        self
    }

    pub fn apply_live_sample(&mut self, sample: &LiveQcSample) {
        self.stream_bitrate_mbps = sample.stream_bitrate_mbps;
        self.vmaf_live = sample.vmaf_estimate;
        self.audio_peak_db = sample.audio_peak_db;
        self.freeze_events = sample.freeze_events as i64;
        self.black_frame_ratio = sample.black_ratio;
        self.signature_deviation = sample.signature_deviation;
    }
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
    pub audio_peak_db: f64,
    pub freeze_events: i64,
    pub black_frame_ratio: f64,
    pub signature_deviation: f64,
    pub power_watts: Option<f64>,
    pub ups_runtime_minutes: Option<f64>,
    pub ups_charge_percent: Option<f64>,
    pub ups_status: Option<String>,
    pub ssd_wear_percent: Option<f64>,
    pub gpu_temp_c: Option<f64>,
    pub ssd_temp_c: Option<f64>,
    pub fan_rpm: Option<f64>,
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
            audio_peak_db: row.get("audio_peak_db")?,
            freeze_events: row.get("freeze_events")?,
            black_frame_ratio: row.get("black_frame_ratio")?,
            signature_deviation: row.get("signature_deviation")?,
            power_watts: row.get("power_watts")?,
            ups_runtime_minutes: row.get("ups_runtime_minutes")?,
            ups_charge_percent: row.get("ups_charge_percent")?,
            ups_status: row.get("ups_status")?,
            ssd_wear_percent: row.get("ssd_wear_percent")?,
            gpu_temp_c: row.get("gpu_temp_c")?,
            ssd_temp_c: row.get("ssd_temp_c")?,
            fan_rpm: row.get("fan_rpm")?,
        })
    }
}

/// Enhanced dashboard generator for P6 observability
#[derive(Debug, Clone)]
pub struct EnhancedDashboardGenerator {
    store: MetricsStore,
    template_engine: TemplateEngine,
}

#[derive(Debug, Clone)]
pub struct DashboardConfig {
    pub title: String,
    pub time_range: TimeRange,
    pub metrics: Vec<BusinessMetricType>,
    pub format: DashboardFormat,
}

#[derive(Debug, Clone)]
pub enum DashboardFormat {
    Html,
    GrafanaJson,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn last_hours(hours: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(hours);
        Self { start, end }
    }

    pub fn last_days(days: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(days);
        Self { start, end }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Dashboard {
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub time_range: TimeRange,
    pub panels: Vec<DashboardPanel>,
    pub alerts: Vec<DashboardAlert>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardPanel {
    pub title: String,
    pub panel_type: PanelType,
    pub metrics: Vec<MetricSeries>,
    pub thresholds: Vec<Threshold>,
}

#[derive(Debug, Clone, Serialize)]
pub enum PanelType {
    TimeSeries,
    Gauge,
    Stat,
    Table,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSeries {
    pub name: String,
    pub metric_type: BusinessMetricType,
    pub data_points: Vec<DataPoint>,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Threshold {
    pub value: f64,
    pub color: String,
    pub condition: ThresholdCondition,
}

#[derive(Debug, Clone, Serialize)]
pub enum ThresholdCondition {
    Above,
    Below,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardAlert {
    pub title: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct TemplateEngine {
    // Simple template engine for HTML generation
}

impl TemplateEngine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render_html(&self, dashboard: &Dashboard) -> String {
        self.generate_html_template(dashboard)
    }

    pub fn render_grafana_json(&self, dashboard: &Dashboard) -> Result<String, serde_json::Error> {
        let grafana_dashboard = self.convert_to_grafana_format(dashboard);
        serde_json::to_string_pretty(&grafana_dashboard)
    }

    fn generate_html_template(&self, dashboard: &Dashboard) -> String {
        let panels_html = dashboard.panels.iter()
            .map(|panel| self.render_panel_html(panel))
            .collect::<Vec<_>>()
            .join("\n");

        let alerts_html = if dashboard.alerts.is_empty() {
            "<li class=\"alert-info\">No active alerts</li>".to_string()
        } else {
            dashboard.alerts.iter()
                .map(|alert| format!(
                    "<li class=\"alert-{severity}\">{title}: {message}</li>",
                    severity = format!("{:?}", alert.severity).to_lowercase(),
                    title = alert.title,
                    message = alert.message
                ))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>{title}</title>
    <style>
        body {{ font-family: 'Inter', sans-serif; background: #0f1419; color: #f8f8f2; margin: 0; padding: 2rem; }}
        .header {{ margin-bottom: 2rem; }}
        .header h1 {{ font-size: 2rem; margin: 0; }}
        .header .subtitle {{ color: #8b949e; margin-top: 0.5rem; }}
        .dashboard-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(400px, 1fr)); gap: 2rem; }}
        .panel {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1.5rem; }}
        .panel h3 {{ margin: 0 0 1rem 0; font-size: 1.2rem; }}
        .metric-chart {{ width: 100%; height: 200px; background: #0d1117; border-radius: 4px; position: relative; }}
        .alerts {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1.5rem; margin-top: 2rem; }}
        .alerts h2 {{ margin: 0 0 1rem 0; }}
        .alerts ul {{ list-style: none; padding: 0; margin: 0; }}
        .alerts li {{ padding: 0.5rem; margin: 0.5rem 0; border-radius: 4px; }}
        .alert-info {{ background: #0969da; }}
        .alert-warning {{ background: #fb8500; }}
        .alert-critical {{ background: #da3633; }}
        .metric-value {{ font-size: 2rem; font-weight: bold; }}
        .metric-unit {{ font-size: 0.9rem; color: #8b949e; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>{title}</h1>
        <div class="subtitle">Generated at {generated_at} | Range: {time_range}</div>
    </div>
    
    <div class="dashboard-grid">
        {panels}
    </div>
    
    <div class="alerts">
        <h2>Alerts</h2>
        <ul>
            {alerts}
        </ul>
    </div>
</body>
</html>"#,
            title = dashboard.title,
            generated_at = dashboard.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            time_range = format!("{} to {}", 
                dashboard.time_range.start.format("%Y-%m-%d %H:%M"),
                dashboard.time_range.end.format("%Y-%m-%d %H:%M")
            ),
            panels = panels_html,
            alerts = alerts_html
        )
    }

    fn render_panel_html(&self, panel: &DashboardPanel) -> String {
        let metrics_html = panel.metrics.iter()
            .map(|metric| {
                let latest_value = metric.data_points.last()
                    .map(|dp| dp.value)
                    .unwrap_or(0.0);
                format!(
                    r#"<div class="metric">
                        <div class="metric-name">{name}</div>
                        <div class="metric-value">{value:.3}</div>
                        <div class="metric-unit">{unit}</div>
                    </div>"#,
                    name = metric.name,
                    value = latest_value,
                    unit = metric.unit
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<div class="panel">
                <h3>{title}</h3>
                <div class="metric-chart">
                    {metrics}
                </div>
            </div>"#,
            title = panel.title,
            metrics = metrics_html
        )
    }

    fn convert_to_grafana_format(&self, dashboard: &Dashboard) -> serde_json::Value {
        serde_json::json!({
            "dashboard": {
                "id": null,
                "title": dashboard.title,
                "tags": ["vvtv", "business-metrics"],
                "timezone": "UTC",
                "panels": dashboard.panels.iter().enumerate().map(|(i, panel)| {
                    serde_json::json!({
                        "id": i + 1,
                        "title": panel.title,
                        "type": match panel.panel_type {
                            PanelType::TimeSeries => "timeseries",
                            PanelType::Gauge => "gauge",
                            PanelType::Stat => "stat",
                            PanelType::Table => "table",
                        },
                        "targets": panel.metrics.iter().map(|metric| {
                            serde_json::json!({
                                "expr": format!("vvtv_{}", format!("{:?}", metric.metric_type).to_lowercase()),
                                "legendFormat": metric.name,
                            })
                        }).collect::<Vec<_>>(),
                        "gridPos": {
                            "h": 8,
                            "w": 12,
                            "x": (i % 2) * 12,
                            "y": (i / 2) * 8
                        }
                    })
                }).collect::<Vec<_>>(),
                "time": {
                    "from": dashboard.time_range.start.to_rfc3339(),
                    "to": dashboard.time_range.end.to_rfc3339()
                },
                "refresh": "5s"
            }
        })
    }
}

/// Alert Engine for P6 observability requirements
#[derive(Debug)]
pub struct AlertEngine {
    metrics_store: Arc<MetricsStore>,
    alert_rules: Vec<AlertRule>,
    channels: Vec<Box<dyn AlertChannel>>,
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub metric_type: BusinessMetricType,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub cooldown: Duration,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    ThresholdBelow { value: f64, duration: Duration },
    ThresholdAbove { value: f64, duration: Duration },
    RateOfChange { threshold: f64, window: Duration },
}

#[derive(Debug, Clone)]
pub struct AlertState {
    pub rule_name: String,
    pub last_triggered: Option<DateTime<Utc>>,
    pub current_state: AlertStateType,
    pub trigger_count: u32,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertStateType {
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct AlertNotification {
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub recommended_actions: Vec<String>,
}

pub trait AlertChannel: Send + Sync + std::fmt::Debug {
    fn send_alert(&self, notification: &AlertNotification) -> Result<(), Box<dyn std::error::Error>>;
    fn name(&self) -> &str;
}

#[derive(Debug)]
pub struct LogAlertChannel {
    name: String,
}

impl LogAlertChannel {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl AlertChannel for LogAlertChannel {
    fn send_alert(&self, notification: &AlertNotification) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!(
            target: "alerts",
            rule = %notification.rule_name,
            severity = ?notification.severity,
            message = %notification.message,
            actions = ?notification.recommended_actions,
            "alert triggered"
        );
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl AlertEngine {
    pub fn new(metrics_store: Arc<MetricsStore>) -> Self {
        Self {
            metrics_store,
            alert_rules: Vec::new(),
            channels: Vec::new(),
        }
    }

    pub fn add_rule(mut self, rule: AlertRule) -> Self {
        self.alert_rules.push(rule);
        self
    }

    pub fn add_channel(mut self, channel: Box<dyn AlertChannel>) -> Self {
        self.channels.push(channel);
        self
    }

    /// Create default alert rules for P6 requirements
    pub fn with_default_rules(mut self) -> Self {
        // Diversity loss alert - selection_entropy below 0.3 for 60 minutes
        self.alert_rules.push(AlertRule {
            name: "diversity_loss".to_string(),
            metric_type: BusinessMetricType::SelectionEntropy,
            condition: AlertCondition::ThresholdBelow {
                value: 0.3,
                duration: Duration::from_secs(3600), // 60 minutes
            },
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(1800), // 30 minutes
        });

        // Curator budget exhausted - budget usage above 90% within 1 hour
        self.alert_rules.push(AlertRule {
            name: "curator_budget_exhausted".to_string(),
            metric_type: BusinessMetricType::CuratorApplyBudgetUsedPct,
            condition: AlertCondition::ThresholdAbove {
                value: 0.9,
                duration: Duration::from_secs(3600), // 1 hour
            },
            severity: AlertSeverity::Critical,
            cooldown: Duration::from_secs(900), // 15 minutes
        });

        // Quality degradation - HD detection slow rate above 25%
        self.alert_rules.push(AlertRule {
            name: "quality_degradation".to_string(),
            metric_type: BusinessMetricType::HdDetectionSlowRate,
            condition: AlertCondition::ThresholdAbove {
                value: 0.25,
                duration: Duration::from_secs(1800), // 30 minutes
            },
            severity: AlertSeverity::Critical,
            cooldown: Duration::from_secs(1800), // 30 minutes
        });

        self
    }

    /// Evaluate all alert rules and send notifications
    pub async fn evaluate_alerts(&self) -> Result<Vec<AlertNotification>, MonitorError> {
        let mut notifications = Vec::new();
        let now = Utc::now();

        for rule in &self.alert_rules {
            if let Some(notification) = self.evaluate_rule(rule, now).await? {
                // Check cooldown period
                if self.is_in_cooldown(&rule.name, now).await? {
                    continue;
                }

                // Send notification via all channels
                for channel in &self.channels {
                    if let Err(e) = channel.send_alert(&notification) {
                        tracing::error!(
                            target: "alerts",
                            channel = channel.name(),
                            error = %e,
                            "failed to send alert"
                        );
                    }
                }

                // Update alert state
                self.update_alert_state(&rule.name, &notification.severity, now).await?;
                notifications.push(notification);
            }
        }

        Ok(notifications)
    }

    async fn evaluate_rule(&self, rule: &AlertRule, now: DateTime<Utc>) -> Result<Option<AlertNotification>, MonitorError> {
        match &rule.condition {
            AlertCondition::ThresholdBelow { value, duration } => {
                let start = now - chrono::Duration::from_std(*duration).unwrap_or_default();
                let metrics = self.metrics_store.query_business_metrics(rule.metric_type, start, now)?;
                
                if metrics.iter().all(|m| m.value < *value) && !metrics.is_empty() {
                    let message = format!(
                        "{:?} has been below {:.3} for the past {:?}",
                        rule.metric_type, value, duration
                    );
                    let actions = self.get_recommended_actions(&rule.name);
                    
                    return Ok(Some(AlertNotification {
                        rule_name: rule.name.clone(),
                        severity: rule.severity.clone(),
                        message,
                        triggered_at: now,
                        recommended_actions: actions,
                    }));
                }
            }
            AlertCondition::ThresholdAbove { value, duration } => {
                let start = now - chrono::Duration::from_std(*duration).unwrap_or_default();
                let metrics = self.metrics_store.query_business_metrics(rule.metric_type, start, now)?;
                
                if metrics.iter().any(|m| m.value > *value) && !metrics.is_empty() {
                    let message = format!(
                        "{:?} exceeded {:.3} within the past {:?}",
                        rule.metric_type, value, duration
                    );
                    let actions = self.get_recommended_actions(&rule.name);
                    
                    return Ok(Some(AlertNotification {
                        rule_name: rule.name.clone(),
                        severity: rule.severity.clone(),
                        message,
                        triggered_at: now,
                        recommended_actions: actions,
                    }));
                }
            }
            AlertCondition::RateOfChange { threshold, window } => {
                let start = now - chrono::Duration::from_std(*window).unwrap_or_default();
                let metrics = self.metrics_store.query_business_metrics(rule.metric_type, start, now)?;
                
                if metrics.len() >= 2 {
                    let first = metrics.first().unwrap().value;
                    let last = metrics.last().unwrap().value;
                    let rate_of_change = (last - first).abs() / first;
                    
                    if rate_of_change > *threshold {
                        let message = format!(
                            "{:?} changed by {:.1}% within {:?} (threshold: {:.1}%)",
                            rule.metric_type, rate_of_change * 100.0, window, threshold * 100.0
                        );
                        let actions = self.get_recommended_actions(&rule.name);
                        
                        return Ok(Some(AlertNotification {
                            rule_name: rule.name.clone(),
                            severity: rule.severity.clone(),
                            message,
                            triggered_at: now,
                            recommended_actions: actions,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn is_in_cooldown(&self, rule_name: &str, now: DateTime<Utc>) -> Result<bool, MonitorError> {
        let conn = self.metrics_store.open()?;
        let mut stmt = conn.prepare(
            "SELECT last_triggered FROM alert_state WHERE rule_name = ?1"
        )?;
        
        if let Some(last_triggered_str) = stmt.query_row([rule_name], |row| {
            let last_triggered: Option<String> = row.get(0)?;
            Ok(last_triggered)
        }).optional()? {
            if let Some(last_triggered_str) = last_triggered_str {
                if let Ok(last_triggered) = DateTime::parse_from_rfc3339(&last_triggered_str) {
                    let last_triggered = last_triggered.with_timezone(&Utc);
                    if let Some(rule) = self.alert_rules.iter().find(|r| r.name == rule_name) {
                        let cooldown_end = last_triggered + chrono::Duration::from_std(rule.cooldown).unwrap_or_default();
                        return Ok(now < cooldown_end);
                    }
                }
            }
        }
        
        Ok(false)
    }

    async fn update_alert_state(&self, rule_name: &str, severity: &AlertSeverity, now: DateTime<Utc>) -> Result<(), MonitorError> {
        let conn = self.metrics_store.open()?;
        conn.execute(
            "INSERT OR REPLACE INTO alert_state (rule_name, last_triggered, current_state, trigger_count, last_updated)
             VALUES (?1, ?2, ?3, COALESCE((SELECT trigger_count FROM alert_state WHERE rule_name = ?1), 0) + 1, ?4)",
            params![
                rule_name,
                now.to_rfc3339(),
                format!("{:?}", severity).to_lowercase(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_recommended_actions(&self, rule_name: &str) -> Vec<String> {
        match rule_name {
            "diversity_loss" => vec![
                "Check business logic temperature setting".to_string(),
                "Review content selection algorithm".to_string(),
                "Verify candidate pool diversity".to_string(),
            ],
            "curator_budget_exhausted" => vec![
                "Review curator confidence threshold".to_string(),
                "Check token bucket refill rate".to_string(),
                "Investigate high curator activity".to_string(),
            ],
            "quality_degradation" => vec![
                "Check HD detection configuration".to_string(),
                "Review source domain reliability".to_string(),
                "Verify browser automation settings".to_string(),
            ],
            _ => vec!["Check system logs for more details".to_string()],
        }
    }

    /// Get current alert states
    pub async fn get_alert_states(&self) -> Result<Vec<AlertState>, MonitorError> {
        let conn = self.metrics_store.open()?;
        let mut stmt = conn.prepare(
            "SELECT rule_name, last_triggered, current_state, trigger_count, last_updated 
             FROM alert_state ORDER BY last_updated DESC"
        )?;
        
        let mut rows = stmt.query([])?;
        let mut states = Vec::new();
        
        while let Some(row) = rows.next()? {
            let rule_name: String = row.get("rule_name")?;
            let last_triggered: Option<String> = row.get("last_triggered")?;
            let current_state_str: String = row.get("current_state")?;
            let trigger_count: u32 = row.get("trigger_count")?;
            let last_updated_str: String = row.get("last_updated")?;
            
            let last_triggered = last_triggered
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            
            let current_state = match current_state_str.as_str() {
                "warning" => AlertStateType::Warning,
                "critical" => AlertStateType::Critical,
                _ => AlertStateType::Ok,
            };
            
            let last_updated = DateTime::parse_from_rfc3339(&last_updated_str)
                .map_err(|_| MonitorError::Serialize(serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid timestamp"))))?
                .with_timezone(&Utc);
            
            states.push(AlertState {
                rule_name,
                last_triggered,
                current_state,
                trigger_count,
                last_updated,
            });
        }
        
        Ok(states)
    }
}

// Keep the original DashboardGenerator for backward compatibility
#[derive(Debug, Clone)]
pub struct DashboardGenerator {
    store: MetricsStore,
    output_path: PathBuf,
}

impl EnhancedDashboardGenerator {
    pub fn new(store: MetricsStore) -> Self {
        Self {
            store,
            template_engine: TemplateEngine::new(),
        }
    }

    /// Generate Business Logic Overview dashboard
    pub fn generate_business_logic_overview(&self) -> Result<Dashboard, MonitorError> {
        let time_range = TimeRange::last_hours(24);
        let config = DashboardConfig {
            title: "Business Logic Overview".to_string(),
            time_range: time_range.clone(),
            metrics: vec![
                BusinessMetricType::SelectionEntropy,
                BusinessMetricType::CuratorApplyBudgetUsedPct,
            ],
            format: DashboardFormat::Html,
        };
        self.generate_dashboard(config)
    }

    /// Generate Autopilot Health dashboard
    pub fn generate_autopilot_health(&self) -> Result<Dashboard, MonitorError> {
        let time_range = TimeRange::last_hours(24);
        let config = DashboardConfig {
            title: "Autopilot Health".to_string(),
            time_range: time_range.clone(),
            metrics: vec![
                BusinessMetricType::AutopilotPredVsRealError,
                BusinessMetricType::NoveltyTemporalKld,
                BusinessMetricType::HdDetectionSlowRate,
            ],
            format: DashboardFormat::Html,
        };
        self.generate_dashboard(config)
    }

    /// Generate dashboard from configuration
    pub fn generate_dashboard(&self, config: DashboardConfig) -> Result<Dashboard, MonitorError> {
        let mut panels = Vec::new();
        let mut all_alerts = Vec::new();

        for metric_type in &config.metrics {
            let metrics_data = self.store.query_business_metrics(
                *metric_type,
                config.time_range.start,
                config.time_range.end,
            )?;

            let data_points: Vec<DataPoint> = metrics_data
                .iter()
                .map(|m| DataPoint {
                    timestamp: m.timestamp,
                    value: m.value,
                })
                .collect();

            let (panel_title, unit, thresholds) = self.get_metric_display_info(*metric_type);
            
            let metric_series = MetricSeries {
                name: format!("{:?}", metric_type),
                metric_type: *metric_type,
                data_points: data_points.clone(),
                unit,
            };

            let panel = DashboardPanel {
                title: panel_title,
                panel_type: PanelType::TimeSeries,
                metrics: vec![metric_series],
                thresholds,
            };

            panels.push(panel);

            // Check for alerts based on latest values
            if let Some(latest) = data_points.last() {
                if let Some(alert) = self.check_metric_alert(*metric_type, latest.value, latest.timestamp) {
                    all_alerts.push(alert);
                }
            }
        }

        Ok(Dashboard {
            title: config.title,
            generated_at: Utc::now(),
            time_range: config.time_range,
            panels,
            alerts: all_alerts,
        })
    }

    /// Export dashboard in specified format
    pub fn export_dashboard(&self, dashboard: Dashboard, format: DashboardFormat) -> Result<String, MonitorError> {
        match format {
            DashboardFormat::Html => Ok(self.template_engine.render_html(&dashboard)),
            DashboardFormat::GrafanaJson => {
                self.template_engine.render_grafana_json(&dashboard)
                    .map_err(|e| MonitorError::Serialize(e))
            }
        }
    }

    fn get_metric_display_info(&self, metric_type: BusinessMetricType) -> (String, String, Vec<Threshold>) {
        match metric_type {
            BusinessMetricType::SelectionEntropy => (
                "Selection Entropy".to_string(),
                "bits".to_string(),
                vec![
                    Threshold {
                        value: 0.3,
                        color: "#da3633".to_string(),
                        condition: ThresholdCondition::Below,
                    },
                    Threshold {
                        value: 1.0,
                        color: "#28a745".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                ],
            ),
            BusinessMetricType::CuratorApplyBudgetUsedPct => (
                "Curator Budget Usage".to_string(),
                "%".to_string(),
                vec![
                    Threshold {
                        value: 0.9,
                        color: "#da3633".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                    Threshold {
                        value: 0.7,
                        color: "#fb8500".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                ],
            ),
            BusinessMetricType::NoveltyTemporalKld => (
                "Content Novelty (KLD)".to_string(),
                "divergence".to_string(),
                vec![
                    Threshold {
                        value: 0.25,
                        color: "#fb8500".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                ],
            ),
            BusinessMetricType::HdDetectionSlowRate => (
                "HD Detection Failure Rate".to_string(),
                "%".to_string(),
                vec![
                    Threshold {
                        value: 0.25,
                        color: "#da3633".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                    Threshold {
                        value: 0.1,
                        color: "#fb8500".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                ],
            ),
            BusinessMetricType::AutopilotPredVsRealError => (
                "Autopilot Prediction Error".to_string(),
                "%".to_string(),
                vec![
                    Threshold {
                        value: 0.2,
                        color: "#fb8500".to_string(),
                        condition: ThresholdCondition::Above,
                    },
                ],
            ),
        }
    }

    fn check_metric_alert(&self, metric_type: BusinessMetricType, value: f64, timestamp: DateTime<Utc>) -> Option<DashboardAlert> {
        match metric_type {
            BusinessMetricType::SelectionEntropy if value < 0.3 => Some(DashboardAlert {
                title: "Low Selection Diversity".to_string(),
                severity: AlertSeverity::Warning,
                message: format!("Selection entropy dropped to {:.3}, indicating low content diversity", value),
                triggered_at: timestamp,
            }),
            BusinessMetricType::CuratorApplyBudgetUsedPct if value > 0.9 => Some(DashboardAlert {
                title: "Curator Budget Exhausted".to_string(),
                severity: AlertSeverity::Critical,
                message: format!("Curator budget usage at {:.1}%, apply rate will be limited", value * 100.0),
                triggered_at: timestamp,
            }),
            BusinessMetricType::HdDetectionSlowRate if value > 0.25 => Some(DashboardAlert {
                title: "HD Detection Issues".to_string(),
                severity: AlertSeverity::Critical,
                message: format!("HD detection failure rate at {:.1}%, quality may be degraded", value * 100.0),
                triggered_at: timestamp,
            }),
            BusinessMetricType::NoveltyTemporalKld if value > 0.25 => Some(DashboardAlert {
                title: "Content Novelty Alert".to_string(),
                severity: AlertSeverity::Warning,
                message: format!("Novelty KLD at {:.3}, content distribution may be skewed", value),
                triggered_at: timestamp,
            }),
            _ => None,
        }
    }
}

// Keep original DashboardGenerator implementation for backward compatibility
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
            if last.vmaf_live < 80.0 {
                alerts.push(format!("VMAF ao vivo em queda ({:.1})", last.vmaf_live));
            }
            if last.freeze_events > 0 {
                alerts.push(format!(
                    "Possível congelamento detectado ({} eventos)",
                    last.freeze_events
                ));
            }
            if last.black_frame_ratio > 0.15 {
                alerts.push(format!(
                    "Quadros escuros excessivos ({:.0}%)",
                    last.black_frame_ratio * 100.0
                ));
            }
            if last.signature_deviation > 0.35 {
                alerts.push(format!(
                    "Desvio estético elevado ({:.2})",
                    last.signature_deviation
                ));
            }
            if let Some(power) = last.power_watts {
                if power > 120.0 {
                    alerts.push(format!("Consumo elevado ({power:.1} W)"));
                }
            }
            if let Some(charge) = last.ups_charge_percent {
                if charge < 20.0 {
                    alerts.push(format!("UPS com carga baixa ({charge:.0}%)"));
                }
            }
            if let Some(runtime) = last.ups_runtime_minutes {
                if runtime < 5.0 {
                    alerts.push(format!("Autonomia UPS crítica ({runtime:.1} min)"));
                }
            }
            if let Some(wear) = last.ssd_wear_percent {
                if wear > 80.0 {
                    alerts.push(format!("SSD desgaste elevado ({wear:.0}% usado)"));
                }
            }
            if let Some(gpu_temp) = last.gpu_temp_c {
                if gpu_temp > 85.0 {
                    alerts.push(format!("Temperatura GPU alta ({gpu_temp:.1} °C)"));
                }
            }
            if let Some(ssd_temp) = last.ssd_temp_c {
                if ssd_temp > 70.0 {
                    alerts.push(format!("SSD superaquecido ({ssd_temp:.1} °C)"));
                }
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
            let power_display = snapshot
                .power_watts
                .map(|value| format!("{value:.1} W"))
                .unwrap_or_else(|| "—".into());
            let ups_display = {
                let mut parts = Vec::new();
                if let Some(charge) = snapshot.ups_charge_percent {
                    parts.push(format!("{charge:.0}%"));
                }
                if let Some(runtime) = snapshot.ups_runtime_minutes {
                    parts.push(format!("{runtime:.1} min"));
                }
                if let Some(status) = &snapshot.ups_status {
                    parts.push(status.clone());
                }
                if parts.is_empty() {
                    "—".into()
                } else {
                    parts.join(" / ")
                }
            };
            let ssd_wear_display = snapshot
                .ssd_wear_percent
                .map(|value| format!("{value:.0}% usado"))
                .unwrap_or_else(|| "—".into());
            let fan_display = snapshot
                .fan_rpm
                .map(|rpm| format!("{rpm:.0} RPM"))
                .unwrap_or_else(|| "—".into());
            let gpu_temp_display = snapshot
                .gpu_temp_c
                .map(|value| format!("{value:.1} °C"))
                .unwrap_or_else(|| "—".into());
            let ssd_temp_display = snapshot
                .ssd_temp_c
                .map(|value| format!("{value:.1} °C"))
                .unwrap_or_else(|| "—".into());
            format!(
                concat!(
                    "<tr><th>Timestamp</th><td>{}</td></tr>",
                    "<tr><th>Buffer</th><td>{:.2} h</td></tr>",
                    "<tr><th>Queue Length</th><td>{}</td></tr>",
                    "<tr><th>Failures (1h)</th><td>{}</td></tr>",
                    "<tr><th>Latency</th><td>{:.2} s</td></tr>",
                    "<tr><th>Bitrate</th><td>{:.2} Mbps</td></tr>",
                    "<tr><th>VMAF</th><td>{:.1}</td></tr>",
                    "<tr><th>Audio Peak</th><td>{:.1} dBFS</td></tr>",
                    "<tr><th>Freeze</th><td>{}</td></tr>",
                    "<tr><th>Signature Δ</th><td>{:.2}</td></tr>",
                    "<tr><th>CPU</th><td>{:.1}%</td></tr>",
                    "<tr><th>Temp.</th><td>{:.1} °C</td></tr>",
                    "<tr><th>Power</th><td>{}</td></tr>",
                    "<tr><th>UPS</th><td>{}</td></tr>",
                    "<tr><th>SSD Wear</th><td>{}</td></tr>",
                    "<tr><th>Fan RPM</th><td>{}</td></tr>",
                    "<tr><th>GPU Temp</th><td>{}</td></tr>",
                    "<tr><th>SSD Temp</th><td>{}</td></tr>"
                ),
                snapshot.timestamp.to_rfc3339(),
                snapshot.buffer_duration_h,
                snapshot.queue_length,
                snapshot.failures_last_hour,
                snapshot.latency_s,
                snapshot.stream_bitrate_mbps,
                snapshot.vmaf_live,
                snapshot.audio_peak_db,
                snapshot.freeze_events,
                snapshot.signature_deviation,
                snapshot.avg_cpu_load,
                snapshot.avg_temp_c,
                power_display,
                ups_display,
                ssd_wear_display,
                fan_display,
                gpu_temp_display,
                ssd_temp_display
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn test_business_metric_creation() {
        let metric = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.85);
        assert!(matches!(metric.metric_type, BusinessMetricType::SelectionEntropy));
        assert_eq!(metric.value, 0.85);
        assert_eq!(metric.context, serde_json::Value::Null);
    }

    #[test]
    fn test_business_metric_with_context() {
        let context = serde_json::json!({
            "temperature": 0.7,
            "top_k": 12,
            "seed": 42
        });
        let metric = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, 0.65)
            .with_context(context.clone());
        
        assert!(matches!(metric.metric_type, BusinessMetricType::CuratorApplyBudgetUsedPct));
        assert_eq!(metric.value, 0.65);
        assert_eq!(metric.context, context);
    }

    #[test]
    fn test_business_metric_serialization() {
        let metric = BusinessMetric::new(BusinessMetricType::NoveltyTemporalKld, 1.23)
            .with_context(serde_json::json!({"window": "15min"}));
        
        let serialized = serde_json::to_string(&metric).unwrap();
        let deserialized: BusinessMetric = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized.metric_type, BusinessMetricType::NoveltyTemporalKld));
        assert_eq!(deserialized.value, 1.23);
    }

    #[tokio::test]
    async fn test_metrics_store_business_metrics() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_metrics.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Record a business metric
        let metric = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.75)
            .with_context(serde_json::json!({"algorithm": "gumbel_topk"}));
        
        store.record_business_metric(&metric).unwrap();
        
        // Query it back
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let results = store.query_business_metrics(
            BusinessMetricType::SelectionEntropy,
            start,
            end,
        ).unwrap();
        
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].metric_type, BusinessMetricType::SelectionEntropy));
        assert_eq!(results[0].value, 0.75);
    }

    #[tokio::test]
    async fn test_metrics_store_safe_recording() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_safe_metrics.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let metric = BusinessMetric::new(BusinessMetricType::HdDetectionSlowRate, 0.15);
        
        // This should not panic even if there are issues
        let result = store.record_metric_safe(metric).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_expired_metrics() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_cleanup.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Record some metrics
        let metric1 = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.8);
        let metric2 = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, 0.6);
        
        store.record_business_metric(&metric1).unwrap();
        store.record_business_metric(&metric2).unwrap();
        
        // Cleanup with 0 days retention (should delete everything)
        let deleted = store.cleanup_expired_business_metrics(0).unwrap();
        assert_eq!(deleted, 2);
    }

    #[test]
    fn test_enhanced_dashboard_generator_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_dashboard.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let _generator = EnhancedDashboardGenerator::new(store);
        
        // Test that we can create dashboard configs
        let time_range = TimeRange::last_hours(24);
        assert!(time_range.start < time_range.end);
        
        let config = DashboardConfig {
            title: "Test Dashboard".to_string(),
            time_range,
            metrics: vec![BusinessMetricType::SelectionEntropy],
            format: DashboardFormat::Html,
        };
        
        assert_eq!(config.title, "Test Dashboard");
        assert_eq!(config.metrics.len(), 1);
    }

    #[test]
    fn test_dashboard_generation_with_data() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_dashboard_data.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Add some test metrics
        let metric1 = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.75);
        let metric2 = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, 0.45);
        
        store.record_business_metric(&metric1).unwrap();
        store.record_business_metric(&metric2).unwrap();
        
        let generator = EnhancedDashboardGenerator::new(store);
        let dashboard = generator.generate_business_logic_overview().unwrap();
        
        assert_eq!(dashboard.title, "Business Logic Overview");
        assert_eq!(dashboard.panels.len(), 2); // SelectionEntropy and CuratorApplyBudgetUsedPct
        assert!(dashboard.generated_at <= Utc::now());
    }

    #[test]
    fn test_dashboard_html_export() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_html_export.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let generator = EnhancedDashboardGenerator::new(store);
        let dashboard = generator.generate_business_logic_overview().unwrap();
        
        let html = generator.export_dashboard(dashboard, DashboardFormat::Html).unwrap();
        
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Business Logic Overview"));
        assert!(html.contains("Selection Entropy"));
        assert!(html.contains("Curator Budget Usage"));
    }

    #[test]
    fn test_dashboard_grafana_export() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_grafana_export.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let generator = EnhancedDashboardGenerator::new(store);
        let dashboard = generator.generate_autopilot_health().unwrap();
        
        let json = generator.export_dashboard(dashboard, DashboardFormat::GrafanaJson).unwrap();
        
        assert!(json.contains("\"dashboard\""));
        assert!(json.contains("\"title\": \"Autopilot Health\""));
        assert!(json.contains("\"type\": \"timeseries\""));
    }

    #[test]
    fn test_metric_alert_generation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_alerts.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Add metrics that should trigger alerts
        let low_entropy = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.2); // Below 0.3 threshold
        let high_budget = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, 0.95); // Above 0.9 threshold
        
        store.record_business_metric(&low_entropy).unwrap();
        store.record_business_metric(&high_budget).unwrap();
        
        let generator = EnhancedDashboardGenerator::new(store);
        let dashboard = generator.generate_business_logic_overview().unwrap();
        
        // Should have alerts for both low entropy and high budget usage
        assert!(dashboard.alerts.len() >= 1);
        assert!(dashboard.alerts.iter().any(|alert| alert.title.contains("Selection Diversity") || alert.title.contains("Curator Budget")));
    }

    #[tokio::test]
    async fn test_alert_engine_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_alert_engine.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let engine = AlertEngine::new(Arc::new(store))
            .with_default_rules()
            .add_channel(Box::new(LogAlertChannel::new("test".to_string())));
        
        assert_eq!(engine.alert_rules.len(), 3); // diversity_loss, curator_budget_exhausted, quality_degradation
        assert_eq!(engine.channels.len(), 1);
    }

    #[tokio::test]
    async fn test_alert_rule_evaluation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_alert_eval.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Add metric that should trigger diversity_loss alert
        let low_entropy = BusinessMetric::new(BusinessMetricType::SelectionEntropy, 0.2);
        store.record_business_metric(&low_entropy).unwrap();
        
        let engine = AlertEngine::new(Arc::new(store))
            .with_default_rules()
            .add_channel(Box::new(LogAlertChannel::new("test".to_string())));
        
        let notifications = engine.evaluate_alerts().await.unwrap();
        
        // Should trigger diversity_loss alert
        assert!(notifications.iter().any(|n| n.rule_name == "diversity_loss"));
    }

    #[tokio::test]
    async fn test_alert_cooldown() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_alert_cooldown.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        // Add metric that triggers alert
        let high_budget = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, 0.95);
        store.record_business_metric(&high_budget).unwrap();
        
        let engine = AlertEngine::new(Arc::new(store))
            .add_rule(AlertRule {
                name: "test_rule".to_string(),
                metric_type: BusinessMetricType::CuratorApplyBudgetUsedPct,
                condition: AlertCondition::ThresholdAbove {
                    value: 0.9,
                    duration: Duration::from_secs(60),
                },
                severity: AlertSeverity::Critical,
                cooldown: Duration::from_secs(300), // 5 minutes
            })
            .add_channel(Box::new(LogAlertChannel::new("test".to_string())));
        
        // First evaluation should trigger alert
        let notifications1 = engine.evaluate_alerts().await.unwrap();
        assert_eq!(notifications1.len(), 1);
        
        // Second evaluation should be in cooldown
        let notifications2 = engine.evaluate_alerts().await.unwrap();
        assert_eq!(notifications2.len(), 0); // Should be in cooldown
    }

    #[tokio::test]
    async fn test_alert_state_tracking() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_alert_state.db");
        
        let store = MetricsStore::new(&db_path).unwrap();
        store.initialize().unwrap();
        
        let engine = AlertEngine::new(Arc::new(store))
            .with_default_rules()
            .add_channel(Box::new(LogAlertChannel::new("test".to_string())));
        
        // Initially no alert states
        let states = engine.get_alert_states().await.unwrap();
        assert_eq!(states.len(), 0);
        
        // Update alert state manually
        let now = Utc::now();
        engine.update_alert_state("test_rule", &AlertSeverity::Warning, now).await.unwrap();
        
        // Should have one alert state
        let states = engine.get_alert_states().await.unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].rule_name, "test_rule");
        assert!(matches!(states[0].current_state, AlertStateType::Warning));
    }
}
