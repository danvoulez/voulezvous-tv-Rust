use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use thiserror::Error;

use crate::monetization::audience::{AudienceSnapshot, AudienceStore, DesireVector};
use crate::monetization::economy::{EconomyStore, EconomySummary};
use crate::plan::{Plan, PlanAdaptiveUpdate, PlanStatus, SqlitePlanStore};

use super::audience::AudienceError;
use super::economy::EconomyError;

#[derive(Debug, Error)]
pub enum AdaptiveError {
    #[error("plan error: {0}")]
    Plan(#[from] crate::plan::PlanError),
    #[error("economy error: {0}")]
    Economy(#[from] EconomyError),
    #[error("audience error: {0}")]
    Audience(#[from] AudienceError),
}

pub type AdaptiveResult<T> = Result<T, AdaptiveError>;

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveUpdate {
    pub plan_id: String,
    pub previous_score: f64,
    pub new_score: f64,
    pub engagement_score: f64,
    pub desire_vector: DesireVector,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveReport {
    pub updates: Vec<AdaptiveUpdate>,
    pub economy: EconomySummary,
    pub audience: AudienceSnapshot,
}

pub struct AdaptiveProgrammer {
    plan_store: SqlitePlanStore,
    economy_store: EconomyStore,
    audience_store: AudienceStore,
    window: Duration,
}

impl AdaptiveProgrammer {
    pub fn new(
        plan_store: SqlitePlanStore,
        economy_store: EconomyStore,
        audience_store: AudienceStore,
    ) -> Self {
        Self {
            plan_store,
            economy_store,
            audience_store,
            window: Duration::hours(24),
        }
    }

    pub fn with_window(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }

    pub fn run_once(&self, now: DateTime<Utc>) -> AdaptiveResult<AdaptiveReport> {
        let audience = self.audience_store.snapshot(self.window)?;
        let economy = self
            .economy_store
            .summarize(now - self.window, now)?
            .with_range(now - self.window, now);
        let plans = self.plan_store.fetch_candidates_for_scoring(256)?;
        let mut updates = Vec::new();
        let mut plan_updates = Vec::new();
        let economy_factor = economic_pressure(&economy);
        for plan in plans {
            if plan.status != PlanStatus::Planned {
                continue;
            }
            let vector = plan
                .desire_vector
                .as_ref()
                .and_then(|stored| vector_from_plan(stored))
                .unwrap_or_else(|| infer_desire_vector(&plan));
            let alignment = vector.dot(&audience.aggregate_vector) as f64;
            let retention = ((audience.metrics.retention_5min + audience.metrics.retention_30min)
                / 2.0)
                .clamp(0.0, 1.0);
            let engagement = (alignment * retention + plan.trending_score as f64).clamp(0.0, 1.2);
            let mut score = plan.curation_score * 0.65 + engagement * 0.25 + economy_factor * 0.1;
            if plan.hd_missing {
                score -= 0.05;
            }
            score = score.clamp(0.2, 0.98);
            if (score - plan.curation_score).abs() < 0.005 {
                continue;
            }
            updates.push(AdaptiveUpdate {
                plan_id: plan.plan_id.clone(),
                previous_score: plan.curation_score,
                new_score: score,
                engagement_score: engagement,
                desire_vector: vector,
            });
            plan_updates.push(PlanAdaptiveUpdate {
                plan_id: plan.plan_id.clone(),
                curation_score: score,
                desire_vector: Some(vector.values.to_vec()),
                engagement_score: engagement,
            });
        }
        if !plan_updates.is_empty() {
            self.plan_store
                .apply_adaptive_feedback(&plan_updates)
                .map_err(AdaptiveError::from)?;
        }
        Ok(AdaptiveReport {
            updates,
            economy,
            audience,
        })
    }
}

fn infer_desire_vector(plan: &Plan) -> DesireVector {
    let mut values = [0.5f32; 5];
    if let Some(duration) = plan.duration_est_s {
        let minutes = (duration as f32 / 60.0).clamp(0.5, 60.0);
        values[0] = (minutes / 20.0).clamp(0.3, 0.95);
        values[2] = (minutes / 40.0).clamp(0.25, 0.9);
    }
    values[1] = if plan.hd_missing { 0.35 } else { 0.78 };
    values[3] = if plan
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains("night"))
    {
        0.62
    } else {
        0.48
    };
    if plan
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains("music"))
    {
        values[4] = 0.82;
    } else if plan
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains("city"))
    {
        values[4] = 0.68;
    } else {
        values[4] = 0.55;
    }
    if plan
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains("warm"))
    {
        values[0] = (values[0] + 0.1).min(0.95);
        values[3] = (values[3] + 0.05).min(0.9);
    }
    let mut vector = DesireVector { values };
    vector.normalize();
    vector
}

fn economic_pressure(summary: &EconomySummary) -> f64 {
    let revenue = summary.revenue_total();
    let cost = summary.cost_total().abs();
    if revenue + cost <= 0.0 {
        return 0.0;
    }
    (summary.net_revenue / (revenue + cost)).clamp(-0.2, 0.2)
}

fn vector_from_plan(values: &[f32]) -> Option<DesireVector> {
    if values.len() < 5 {
        return None;
    }
    let mut array = [0.0f32; 5];
    for (slot, value) in array.iter_mut().zip(values.iter()) {
        *slot = (*value).clamp(0.0, 1.0);
    }
    let mut vector = DesireVector { values: array };
    vector.normalize();
    Some(vector)
}
