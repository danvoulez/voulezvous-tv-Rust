use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use rusqlite::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Planned,
    Selected,
    InProgress,
    Downloaded,
    Edited,
    Ready,
    Failed,
    Archived,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanStatus::Planned => "planned",
            PlanStatus::Selected => "selected",
            PlanStatus::InProgress => "in_progress",
            PlanStatus::Downloaded => "downloaded",
            PlanStatus::Edited => "edited",
            PlanStatus::Ready => "ready",
            PlanStatus::Failed => "failed",
            PlanStatus::Archived => "archived",
        }
    }

    pub fn terminal(&self) -> bool {
        matches!(
            self,
            PlanStatus::Ready | PlanStatus::Failed | PlanStatus::Archived
        )
    }
}

impl fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PlanStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "planned" => Ok(PlanStatus::Planned),
            "selected" => Ok(PlanStatus::Selected),
            "in_progress" => Ok(PlanStatus::InProgress),
            "downloaded" => Ok(PlanStatus::Downloaded),
            "edited" => Ok(PlanStatus::Edited),
            "ready" => Ok(PlanStatus::Ready),
            "failed" => Ok(PlanStatus::Failed),
            "archived" => Ok(PlanStatus::Archived),
            other => Err(format!("unknown plan status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub plan_id: String,
    pub created_at: Option<DateTime<Utc>>,
    pub kind: String,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub duration_est_s: Option<i64>,
    pub resolution_observed: Option<String>,
    pub curation_score: f64,
    pub status: PlanStatus,
    pub license_proof: Option<String>,
    pub hd_missing: bool,
    pub node_origin: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub failure_count: i64,
    pub tags: Vec<String>,
    pub trending_score: f64,
}

impl Plan {
    pub fn new(plan_id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            created_at: None,
            kind: kind.into(),
            title: None,
            source_url: None,
            duration_est_s: None,
            resolution_observed: None,
            curation_score: 0.5,
            status: PlanStatus::Planned,
            license_proof: None,
            hd_missing: false,
            node_origin: None,
            updated_at: None,
            failure_count: 0,
            tags: Vec::new(),
            trending_score: 0.0,
        }
    }

    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let created_at: Option<NaiveDateTime> = row.get("created_at")?;
        let updated_at: Option<NaiveDateTime> = row.get("updated_at")?;
        let tags: Option<String> = row.get("tags")?;
        Ok(Self {
            plan_id: row.get("plan_id")?,
            created_at: created_at.map(|dt| Utc.from_utc_datetime(&dt)),
            kind: row.get("kind")?,
            title: row.get("title")?,
            source_url: row.get("source_url")?,
            duration_est_s: row.get("duration_est_s")?,
            resolution_observed: row.get("resolution_observed")?,
            curation_score: row.get("curation_score")?,
            status: row
                .get::<_, String>("status")?
                .parse()
                .unwrap_or(PlanStatus::Planned),
            license_proof: row.get("license_proof")?,
            hd_missing: match row.get::<_, Option<i64>>("hd_missing")? {
                Some(value) => value != 0,
                None => false,
            },
            node_origin: row.get("node_origin")?,
            updated_at: updated_at.map(|dt| Utc.from_utc_datetime(&dt)),
            failure_count: row.get::<_, Option<i64>>("failure_count")?.unwrap_or(0),
            tags: tags
                .map(|value| {
                    value
                        .split(',')
                        .filter(|item| !item.trim().is_empty())
                        .map(|item| item.trim().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            trending_score: row.get::<_, Option<f64>>("trending_score")?.unwrap_or(0.0),
        })
    }

    pub fn serialize_tags(tags: &[String]) -> Option<String> {
        if tags.is_empty() {
            None
        } else {
            Some(tags.join(","))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlanMetrics {
    pub total: usize,
    pub by_status: HashMap<String, usize>,
    pub hd_missing: usize,
    pub average_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanAuditKind {
    Expired,
    MissingLicense,
    HdMissing,
    Stuck,
}

impl fmt::Display for PlanAuditKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            PlanAuditKind::Expired => "expired",
            PlanAuditKind::MissingLicense => "missing_license",
            PlanAuditKind::HdMissing => "hd_missing",
            PlanAuditKind::Stuck => "stuck",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanAuditFinding {
    pub plan_id: String,
    pub kind: PlanAuditKind,
    pub status: PlanStatus,
    pub age_hours: f64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanBlacklistEntry {
    pub domain: String,
    pub reason: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanImportRecord {
    pub plan: Plan,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanSelectionDecision {
    pub plan_id: String,
    pub score: f64,
    pub rationale: String,
}
