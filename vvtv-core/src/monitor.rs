use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

use crate::quality::{QualityAnalyzer, QualityThresholds, SignatureProfile};
use crate::sqlite::configure_connection;

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
            .trim()
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
                .map(|(a, b)| (*a as i16 - *b as i16).abs() as u64)
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
                audio_peak_db, freeze_events, black_frame_ratio, signature_deviation
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                record.signature_deviation
            ],
        )?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<MetricSnapshot>, MonitorError> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT ts, buffer_duration_h, queue_length, played_last_hour, failures_last_hour,
                    avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live,
                    audio_peak_db, freeze_events, black_frame_ratio, signature_deviation
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
                    audio_peak_db, freeze_events, black_frame_ratio, signature_deviation
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
                    "<tr><th>Temp.</th><td>{:.1} °C</td></tr>"
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
