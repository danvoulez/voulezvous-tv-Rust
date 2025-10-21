use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use thiserror::Error;

use crate::monetization::audience::{AudienceReport, AudienceStore};
use crate::monetization::economy::{EconomyError, EconomyStore, EconomySummary};
use crate::plan::SqlitePlanStore;

#[derive(Debug, Error)]
pub enum DashboardError {
    #[error("economy error: {0}")]
    Economy(#[from] EconomyError),
    #[error("audience error: {0}")]
    Audience(#[from] super::audience::AudienceError),
    #[error("plan error: {0}")]
    Plan(#[from] crate::plan::PlanError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub type DashboardResult<T> = Result<T, DashboardError>;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardArtifacts {
    pub html_path: PathBuf,
    pub finance_path: PathBuf,
    pub trends_path: PathBuf,
    pub heatmap_path: PathBuf,
}

pub struct MonetizationDashboard<'a> {
    economy: &'a EconomyStore,
    audience: &'a AudienceStore,
    plans: &'a SqlitePlanStore,
}

impl<'a> MonetizationDashboard<'a> {
    pub fn new(
        economy: &'a EconomyStore,
        audience: &'a AudienceStore,
        plans: &'a SqlitePlanStore,
    ) -> Self {
        Self {
            economy,
            audience,
            plans,
        }
    }

    pub fn generate<P: AsRef<Path>>(
        &self,
        output_dir: P,
        now: DateTime<Utc>,
    ) -> DashboardResult<DashboardArtifacts> {
        fs::create_dir_all(output_dir.as_ref())?;
        let finance_path = output_dir.as_ref().join("finance_daily.json");
        let trends_path = output_dir.as_ref().join("trends_weekly.json");
        let heatmap_path = output_dir.as_ref().join("audience_heatmap.png");
        let html_path = output_dir.as_ref().join("monetization_dashboard.html");

        let economy_summary = self
            .economy
            .summarize(now - Duration::hours(24), now)?
            .with_range(now - Duration::hours(24), now);
        let events = self.economy.list_events(now - Duration::hours(24), now)?;
        let audience_report = self.audience.metrics(now - Duration::hours(24), now)?;
        let _ = self
            .audience
            .generate_heatmap(&audience_report, &heatmap_path)?;
        let trends = self.plans.top_tags(Duration::days(7), 16)?;
        let finance = FinanceSnapshot::from(&economy_summary, &events);
        let trends_payload = TrendsSnapshot::new(&trends, &audience_report);

        write_json(&finance_path, &finance)?;
        write_json(&trends_path, &trends_payload)?;
        write_dashboard_html(&html_path, &finance, &trends_payload, &audience_report)?;

        Ok(DashboardArtifacts {
            html_path,
            finance_path,
            trends_path,
            heatmap_path,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct FinanceSnapshot {
    net_revenue: f64,
    revenue_total: f64,
    cost_total: f64,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    hourly: Vec<HourlyBreakdown>,
}

impl FinanceSnapshot {
    fn from(
        summary: &EconomySummary,
        events: &[crate::monetization::economy::EconomyEvent],
    ) -> Self {
        let mut hourly = HashMap::new();
        for event in events {
            let hour_key = event.timestamp.format("%Y-%m-%d %H:00").to_string();
            let entry = hourly.entry(hour_key).or_insert(HourlyBreakdown::new());
            if event.event_type.is_revenue() {
                entry.revenue += event.value_eur;
            } else {
                entry.costs += event.value_eur;
            }
        }
        let mut breakdown = hourly
            .into_iter()
            .map(|(hour, entry)| HourlyBreakdown { hour, ..entry })
            .collect::<Vec<_>>();
        breakdown.sort_by(|a, b| a.hour.cmp(&b.hour));
        Self {
            net_revenue: summary.net_revenue,
            revenue_total: summary.revenue_total(),
            cost_total: summary.cost_total(),
            period_start: summary.start,
            period_end: summary.end,
            hourly: breakdown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct HourlyBreakdown {
    hour: String,
    revenue: f64,
    costs: f64,
}

impl HourlyBreakdown {
    fn new() -> Self {
        Self {
            hour: String::new(),
            revenue: 0.0,
            costs: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TrendsSnapshot {
    top_tags: Vec<TagTrend>,
    audience_metrics: super::audience::AudienceMetrics,
}

impl TrendsSnapshot {
    fn new(trends: &[(String, usize)], audience: &AudienceReport) -> Self {
        let top_tags = trends
            .iter()
            .map(|(tag, count)| TagTrend {
                tag: tag.clone(),
                count: *count as u64,
            })
            .collect();
        Self {
            top_tags,
            audience_metrics: audience.metrics.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TagTrend {
    tag: String,
    count: u64,
}

fn write_json<P: AsRef<Path>, T: Serialize>(path: P, value: &T) -> DashboardResult<()> {
    let mut file = File::create(path.as_ref())?;
    let data = serde_json::to_string_pretty(value)?;
    file.write_all(data.as_bytes())?;
    Ok(())
}

fn write_dashboard_html(
    path: &Path,
    finance: &FinanceSnapshot,
    trends: &TrendsSnapshot,
    audience: &AudienceReport,
) -> DashboardResult<()> {
    let mut file = File::create(path)?;
    let template = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>VVTV Monetization Dashboard</title>
    <style>
        body {{ font-family: sans-serif; background: #0f0f16; color: #f5f5f7; margin: 2rem; }}
        h1 {{ color: #ff9f43; }}
        .section {{ margin-bottom: 2rem; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #222; padding: 0.6rem; }}
        th {{ background: #1e1e2b; }}
        .positive {{ color: #9ef17d; }}
        .negative {{ color: #ff6b6b; }}
    </style>
</head>
<body>
    <h1>VVTV Monetization Overview</h1>
    <div class="section">
        <h2>Finance (Últimas 24h)</h2>
        <p>Receita: <span class="positive">€{:.2}</span> | Custos: <span class="negative">€{:.2}</span> | Resultado: <strong>{:+.2}</strong></p>
        <table>
            <thead>
                <tr><th>Hora</th><th>Receita (€)</th><th>Custos (€)</th></tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
    </div>
    <div class="section">
        <h2>Tendências</h2>
        <ol>
            {}
        </ol>
    </div>
    <div class="section">
        <h2>Métricas de Audiência</h2>
        <p>Total de sessões: {} | Retenção 5min: {:.0}% | Retenção 30min: {:.0}% | Duração média: {:.1} min</p>
    </div>
</body>
</html>"#,
        finance.revenue_total,
        finance.cost_total,
        finance.net_revenue,
        finance
            .hourly
            .iter()
            .map(|row| format!(
                "<tr><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                row.hour, row.revenue, row.costs
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        trends
            .top_tags
            .iter()
            .map(|trend| format!("<li>{} — {} plays</li>", trend.tag, trend.count))
            .collect::<Vec<_>>()
            .join("\n"),
        audience.metrics.total_sessions,
        audience.metrics.retention_5min * 100.0,
        audience.metrics.retention_30min * 100.0,
        audience.metrics.avg_duration_minutes,
    );
    file.write_all(template.as_bytes())?;
    Ok(())
}
