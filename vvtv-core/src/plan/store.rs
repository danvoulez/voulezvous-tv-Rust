use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use crate::browser::{Candidate, PbdOutcome};
use crate::sqlite::configure_connection;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use super::models::{
    Plan, PlanAuditFinding, PlanAuditKind, PlanBlacklistEntry, PlanImportRecord, PlanMetrics,
    PlanSelectionDecision, PlanStatus,
};
use super::{PlanError, PlanResult};

const PLAN_SCHEMA: &str = include_str!("../../../sql/plans.sql");

#[derive(Debug, Clone)]
pub struct SqlitePlanStoreBuilder {
    path: Option<PathBuf>,
    read_only: bool,
    create_if_missing: bool,
}

impl Default for SqlitePlanStoreBuilder {
    fn default() -> Self {
        Self {
            path: None,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl SqlitePlanStoreBuilder {
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

    pub fn build(self) -> PlanResult<SqlitePlanStore> {
        let path = self.path.ok_or(PlanError::MissingStore)?;
        let mut flags = if self.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };

        if !self.read_only && self.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }

        Ok(SqlitePlanStore { path, flags })
    }
}

#[derive(Debug, Clone)]
pub struct SqlitePlanStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl SqlitePlanStore {
    pub fn builder() -> SqlitePlanStoreBuilder {
        SqlitePlanStoreBuilder::new()
    }

    pub fn new(path: impl AsRef<Path>) -> PlanResult<Self> {
        SqlitePlanStoreBuilder::new().path(path).build()
    }

    fn open(&self) -> PlanResult<Connection> {
        let conn = Connection::open_with_flags(&self.path, self.flags).map_err(|source| {
            PlanError::OpenDatabase {
                path: self.path.clone(),
                source,
            }
        })?;
        configure_connection(&conn).map_err(|source| PlanError::OpenDatabase {
            path: self.path.clone(),
            source,
        })?;
        Ok(conn)
    }

    pub fn initialize(&self) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute_batch(PLAN_SCHEMA)?;
        Ok(())
    }

    pub fn create_plan_from_discovery(
        &self,
        candidate: &Candidate,
        outcome: &PbdOutcome,
    ) -> PlanResult<Plan> {
        let now = Utc::now();
        let mut plan = Plan::new(format!("dl-{}", Uuid::new_v4().simple()), "video");
        plan.title = outcome
            .metadata
            .title
            .clone()
            .or_else(|| candidate.title.clone());
        plan.source_url = Some(candidate.url.clone());
        plan.duration_est_s = outcome
            .metadata
            .duration_seconds
            .map(|value| value as i64)
            .or_else(|| {
                outcome
                    .validation
                    .duration_seconds
                    .map(|value| value.round() as i64)
            });
        plan.resolution_observed = outcome
            .metadata
            .resolution_label
            .clone()
            .or_else(|| Some(format!("{}p", outcome.validation.video_height)));
        plan.hd_missing = outcome.validation.video_height < 720;
        plan.curation_score =
            estimate_curation_score(candidate.rank, plan.duration_est_s, plan.hd_missing);
        plan.license_proof = outcome.metadata.license_hint.clone();
        plan.node_origin = Some("discovery-loop".to_string());
        plan.updated_at = Some(now);
        plan.created_at = Some(now);
        plan.tags = outcome
            .metadata
            .tags
            .iter()
            .map(|tag| tag.normalized.clone())
            .collect();
        plan.trending_score = 0.0;
        plan.failure_count = 0;

        self.upsert_plan(&plan)?;
        Ok(plan)
    }

    pub fn upsert_plan(&self, plan: &Plan) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO plans (
                plan_id, kind, title, source_url, duration_est_s, resolution_observed,
                curation_score, status, license_proof, hd_missing, node_origin, updated_at,
                failure_count, tags, trending_score
            ) VALUES (
                :plan_id, :kind, :title, :source_url, :duration_est_s, :resolution_observed,
                :curation_score, :status, :license_proof, :hd_missing, :node_origin, :updated_at,
                :failure_count, :tags, :trending_score
            )
            ON CONFLICT(plan_id) DO UPDATE SET
                kind = excluded.kind,
                title = excluded.title,
                source_url = excluded.source_url,
                duration_est_s = excluded.duration_est_s,
                resolution_observed = excluded.resolution_observed,
                curation_score = excluded.curation_score,
                status = excluded.status,
                license_proof = excluded.license_proof,
                hd_missing = excluded.hd_missing,
                node_origin = excluded.node_origin,
                updated_at = excluded.updated_at,
                failure_count = excluded.failure_count,
                tags = excluded.tags,
                trending_score = excluded.trending_score",
            params![
                &plan.plan_id,
                &plan.kind,
                &plan.title,
                &plan.source_url,
                &plan.duration_est_s,
                &plan.resolution_observed,
                &plan.curation_score,
                plan.status.as_str(),
                &plan.license_proof,
                if plan.hd_missing { 1 } else { 0 },
                &plan.node_origin,
                plan.updated_at.map(|dt| dt.naive_utc()),
                plan.failure_count,
                Plan::serialize_tags(&plan.tags),
                plan.trending_score,
            ],
        )?;
        Ok(())
    }

    pub fn fetch_by_id(&self, plan_id: &str) -> PlanResult<Option<Plan>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare("SELECT * FROM plans WHERE plan_id = ?1")?;
        let plan = stmt
            .query_row([plan_id], |row| Plan::from_row(row))
            .optional()?;
        Ok(plan)
    }

    pub fn delete(&self, plan_id: &str) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute("DELETE FROM plans WHERE plan_id = ?1", [plan_id])?;
        Ok(())
    }

    pub fn list_by_status(
        &self,
        status: Option<PlanStatus>,
        limit: usize,
    ) -> PlanResult<Vec<Plan>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM plans
             WHERE (?1 IS NULL OR status = ?1)
             ORDER BY (updated_at IS NULL) ASC, updated_at DESC, created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(
                (status.as_ref().map(PlanStatus::as_str), limit as i64),
                |row| Plan::from_row(row),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn count_by_status(&self) -> PlanResult<HashMap<String, usize>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM plans GROUP BY status")?;
        let mut map = HashMap::new();
        for row in stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (status, count) = row?;
            map.insert(status, count as usize);
        }
        Ok(map)
    }

    pub fn compute_metrics(&self) -> PlanResult<PlanMetrics> {
        let counts = self.count_by_status()?;
        let total = counts.values().copied().sum();

        let conn = self.open()?;
        let hd_missing: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM plans WHERE hd_missing = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let average_score: f64 = conn
            .query_row("SELECT AVG(curation_score) FROM plans", [], |row| {
                row.get::<_, Option<f64>>(0)
            })
            .unwrap_or(Some(0.0))
            .unwrap_or(0.0);

        Ok(PlanMetrics {
            total,
            by_status: counts,
            hd_missing: hd_missing as usize,
            average_score,
        })
    }

    pub fn update_status(&self, plan_id: &str, status: PlanStatus) -> PlanResult<()> {
        let conn = self.open()?;
        let affected = conn.execute(
            "UPDATE plans SET status = ?2, updated_at = CURRENT_TIMESTAMP WHERE plan_id = ?1",
            params![plan_id, status.as_str()],
        )?;
        if affected == 0 {
            return Err(PlanError::NotFound {
                plan_id: plan_id.to_string(),
            });
        }
        Ok(())
    }

    pub fn update_score(&self, plan_id: &str, score: f64) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE plans SET curation_score = ?2, updated_at = CURRENT_TIMESTAMP WHERE plan_id = ?1",
            params![plan_id, score],
        )?;
        Ok(())
    }

    pub fn record_attempt(
        &self,
        plan_id: &str,
        from: Option<PlanStatus>,
        to: Option<PlanStatus>,
        note: impl AsRef<str>,
    ) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO plan_attempts(plan_id, status_from, status_to, note) VALUES (?1, ?2, ?3, ?4)",
            params![
                plan_id,
                from.map(|status| status.as_str().to_string()),
                to.map(|status| status.as_str().to_string()),
                note.as_ref(),
            ],
        )?;
        Ok(())
    }

    pub fn increment_failure(&self, plan_id: &str) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE plans
             SET failure_count = failure_count + 1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE plan_id = ?1",
            [plan_id],
        )?;
        Ok(())
    }

    pub fn reset_failures(&self, plan_id: &str) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE plans SET failure_count = 0 WHERE plan_id = ?1",
            [plan_id],
        )?;
        Ok(())
    }

    pub fn reserve_next(&self) -> PlanResult<Option<Plan>> {
        let conn = self.open()?;
        conn.execute("BEGIN IMMEDIATE TRANSACTION", [])?;
        let mut stmt = conn.prepare(
            "SELECT * FROM plans
             WHERE status = 'selected'
             ORDER BY failure_count ASC, (updated_at IS NULL) DESC, updated_at ASC
             LIMIT 1",
        )?;
        let plan_opt = stmt.query_row([], |row| Plan::from_row(row)).optional()?;
        if let Some(plan) = plan_opt {
            conn.execute(
                "UPDATE plans SET status = 'in_progress', updated_at = CURRENT_TIMESTAMP WHERE plan_id = ?1",
                [plan.plan_id.as_str()],
            )?;
            conn.execute("COMMIT", [])?;
            return Ok(self.fetch_by_id(&plan.plan_id)?);
        }
        conn.execute("ROLLBACK", [])?;
        Ok(None)
    }

    pub fn complete_download(&self, plan_id: &str) -> PlanResult<()> {
        let conn = self.open()?;
        let affected = conn.execute(
            "UPDATE plans
             SET status = 'downloaded', updated_at = CURRENT_TIMESTAMP, failure_count = 0
             WHERE plan_id = ?1",
            [plan_id],
        )?;
        if affected == 0 {
            return Err(PlanError::NotFound {
                plan_id: plan_id.to_string(),
            });
        }
        Ok(())
    }

    pub fn mark_edited(
        &self,
        plan_id: &str,
        hd_missing: bool,
        resolution: Option<&str>,
    ) -> PlanResult<()> {
        let conn = self.open()?;
        let affected = conn.execute(
            "UPDATE plans
             SET status = 'edited',
                 hd_missing = CASE WHEN ?2 THEN 1 ELSE 0 END,
                 resolution_observed = COALESCE(?3, resolution_observed),
                 updated_at = CURRENT_TIMESTAMP,
                 failure_count = 0
             WHERE plan_id = ?1",
            params![plan_id, hd_missing, resolution],
        )?;
        if affected == 0 {
            return Err(PlanError::NotFound {
                plan_id: plan_id.to_string(),
            });
        }
        Ok(())
    }

    pub fn finalize_ready(&self, plan_id: &str) -> PlanResult<()> {
        self.update_status(plan_id, PlanStatus::Ready)
    }

    pub fn fail_plan(&self, plan_id: &str, note: impl AsRef<str>) -> PlanResult<()> {
        self.update_status(plan_id, PlanStatus::Failed)?;
        self.record_attempt(plan_id, None, Some(PlanStatus::Failed), note)
    }

    pub fn audit(&self, now: chrono::DateTime<Utc>) -> PlanResult<Vec<PlanAuditFinding>> {
        let conn = self.open()?;
        let mut findings = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT plan_id, status, created_at, updated_at, license_proof, hd_missing
             FROM plans",
        )?;
        let rows = stmt.query_map([], |row| {
            let plan_id: String = row.get(0)?;
            let status: String = row.get(1)?;
            let created_at: Option<chrono::NaiveDateTime> = row.get(2)?;
            let updated_at: Option<chrono::NaiveDateTime> = row.get(3)?;
            let license_proof: Option<String> = row.get(4)?;
            let hd_missing: i64 = row.get::<_, Option<i64>>(5)?.unwrap_or(0);
            Ok((
                plan_id,
                status,
                created_at,
                updated_at,
                license_proof,
                hd_missing,
            ))
        })?;

        for row in rows {
            let (plan_id, status_raw, created_at, updated_at, license_proof, hd_missing) = row?;
            let status = status_raw
                .parse::<PlanStatus>()
                .unwrap_or(PlanStatus::Planned);
            let reference = updated_at.or(created_at);
            let age_hours = reference
                .map(|dt| {
                    now.signed_duration_since(Utc.from_utc_datetime(&dt))
                        .num_minutes() as f64
                        / 60.0
                })
                .unwrap_or_default();

            if status == PlanStatus::Planned && age_hours > 24.0 {
                findings.push(PlanAuditFinding {
                    plan_id: plan_id.clone(),
                    kind: PlanAuditKind::Expired,
                    status: status.clone(),
                    age_hours,
                    note: Some("plano aguardando seleção há mais de 24h".to_string()),
                });
            }
            if license_proof
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                findings.push(PlanAuditFinding {
                    plan_id: plan_id.clone(),
                    kind: PlanAuditKind::MissingLicense,
                    status: status.clone(),
                    age_hours,
                    note: Some("license_proof ausente".to_string()),
                });
            }
            if hd_missing != 0 {
                findings.push(PlanAuditFinding {
                    plan_id: plan_id.clone(),
                    kind: PlanAuditKind::HdMissing,
                    status: status.clone(),
                    age_hours,
                    note: None,
                });
            }
            if matches!(status, PlanStatus::InProgress | PlanStatus::Selected) && age_hours > 6.0 {
                findings.push(PlanAuditFinding {
                    plan_id: plan_id.clone(),
                    kind: PlanAuditKind::Stuck,
                    status,
                    age_hours,
                    note: Some("plano bloqueado há mais de 6h".to_string()),
                });
            }
        }

        Ok(findings)
    }

    pub fn blacklist_add(
        &self,
        domain: &str,
        reason: Option<&str>,
    ) -> PlanResult<PlanBlacklistEntry> {
        let conn = self.open()?;
        conn.execute(
            "INSERT OR REPLACE INTO plan_blacklist(domain, reason, created_at)
             VALUES (?1, ?2, COALESCE((SELECT created_at FROM plan_blacklist WHERE domain = ?1), CURRENT_TIMESTAMP))",
            params![domain, reason],
        )?;
        Ok(PlanBlacklistEntry {
            domain: domain.to_string(),
            reason: reason.map(|s| s.to_string()),
            created_at: Some(Utc::now()),
        })
    }

    pub fn blacklist_remove(&self, domain: &str) -> PlanResult<()> {
        let conn = self.open()?;
        let affected = conn.execute("DELETE FROM plan_blacklist WHERE domain = ?1", [domain])?;
        if affected == 0 {
            return Err(PlanError::BlacklistNotFound {
                domain: domain.to_string(),
            });
        }
        Ok(())
    }

    pub fn blacklist_list(&self) -> PlanResult<Vec<PlanBlacklistEntry>> {
        let conn = self.open()?;
        let mut stmt =
            conn.prepare("SELECT domain, reason, created_at FROM plan_blacklist ORDER BY domain")?;
        let rows = stmt
            .query_map([], |row| {
                let created_at: Option<chrono::NaiveDateTime> = row.get(2)?;
                Ok(PlanBlacklistEntry {
                    domain: row.get(0)?,
                    reason: row.get(1)?,
                    created_at: created_at.map(|dt| Utc.from_utc_datetime(&dt)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn import(&self, records: &[PlanImportRecord]) -> PlanResult<usize> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        let mut inserted = 0;
        for record in records {
            let existing = tx
                .query_row(
                    "SELECT status FROM plans WHERE plan_id = ?1",
                    [record.plan.plan_id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if existing.is_some() && !record.overwrite {
                continue;
            }
            tx.execute(
                "INSERT INTO plans (
                    plan_id, kind, title, source_url, duration_est_s, resolution_observed,
                    curation_score, status, license_proof, hd_missing, node_origin, updated_at,
                    failure_count, tags, trending_score
                ) VALUES (
                    :plan_id, :kind, :title, :source_url, :duration_est_s, :resolution_observed,
                    :curation_score, :status, :license_proof, :hd_missing, :node_origin, :updated_at,
                    :failure_count, :tags, :trending_score
                )
                ON CONFLICT(plan_id) DO UPDATE SET
                    kind = excluded.kind,
                    title = excluded.title,
                    source_url = excluded.source_url,
                    duration_est_s = excluded.duration_est_s,
                    resolution_observed = excluded.resolution_observed,
                    curation_score = excluded.curation_score,
                    status = excluded.status,
                    license_proof = excluded.license_proof,
                    hd_missing = excluded.hd_missing,
                    node_origin = excluded.node_origin,
                    updated_at = excluded.updated_at,
                    failure_count = excluded.failure_count,
                    tags = excluded.tags,
                    trending_score = excluded.trending_score",
                params![
                    &record.plan.plan_id,
                    &record.plan.kind,
                    &record.plan.title,
                    &record.plan.source_url,
                    &record.plan.duration_est_s,
                    &record.plan.resolution_observed,
                    &record.plan.curation_score,
                    record.plan.status.as_str(),
                    &record.plan.license_proof,
                    if record.plan.hd_missing { 1 } else { 0 },
                    &record.plan.node_origin,
                    record.plan.updated_at.map(|dt| dt.naive_utc()),
                    record.plan.failure_count,
                    Plan::serialize_tags(&record.plan.tags),
                    record.plan.trending_score,
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn store_decisions(&self, decisions: &[PlanSelectionDecision]) -> PlanResult<()> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        for decision in decisions {
            tx.execute(
                "UPDATE plans
                 SET curation_score = ?2,
                     status = 'selected',
                     updated_at = CURRENT_TIMESTAMP
                 WHERE plan_id = ?1",
                params![decision.plan_id, decision.score],
            )?;
            tx.execute(
                "INSERT INTO plan_attempts(plan_id, status_from, status_to, note)
                 VALUES (?1, 'planned', 'selected', ?2)",
                params![decision.plan_id, decision.rationale.clone()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn fetch_candidates_for_scoring(&self, limit: usize) -> PlanResult<Vec<Plan>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM plans
             WHERE status = 'planned'
             ORDER BY created_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |row| Plan::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_trending_score(&self, plan_id: &str, score: f64) -> PlanResult<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE plans SET trending_score = ?2 WHERE plan_id = ?1",
            params![plan_id, score],
        )?;
        Ok(())
    }
}

fn estimate_curation_score(rank: usize, duration: Option<i64>, hd_missing: bool) -> f64 {
    let rank_component = (1.0 / (rank as f64 + 0.5)).min(1.0);
    let duration_bonus = duration
        .map(|seconds| {
            if seconds >= 900 {
                0.18
            } else if seconds >= 420 {
                0.12
            } else if seconds >= 180 {
                0.06
            } else {
                0.02
            }
        })
        .unwrap_or(0.05);
    let hd_adjustment = if hd_missing { -0.18 } else { 0.12 };
    let base = 0.45;
    (base + 0.35 * rank_component + duration_bonus + hd_adjustment).clamp(0.2, 0.98)
}
