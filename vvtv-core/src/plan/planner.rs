use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tracing::{info, warn};

use super::models::{Plan, PlanSelectionDecision};
use super::store::SqlitePlanStore;
use super::{PlanError, PlanResult, PlanStatus};

#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub selection_batch_size: usize,
    pub diversity_quota: f64,
    pub target_duration_seconds: i64,
    pub selection_limit: usize,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            selection_batch_size: 12,
            diversity_quota: 0.2,
            target_duration_seconds: 600,
            selection_limit: 64,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlannerEvent {
    Idle,
    Selected(Vec<PlanSelectionDecision>),
}

#[derive(Debug)]
pub struct Planner {
    store: SqlitePlanStore,
    config: PlannerConfig,
}

impl Planner {
    pub fn new(store: SqlitePlanStore, config: PlannerConfig) -> Self {
        Self { store, config }
    }

    pub fn run_once(&self, now: DateTime<Utc>) -> PlanResult<PlannerEvent> {
        let candidates = self
            .store
            .fetch_candidates_for_scoring(self.config.selection_limit)?;
        if candidates.is_empty() {
            return Ok(PlannerEvent::Idle);
        }

        let kind_frequency = self.kind_frequency(&candidates);
        let scored = self.score_candidates(&candidates, &kind_frequency, now);
        let decisions = self.apply_selection_rules(scored, &kind_frequency);
        if decisions.is_empty() {
            warn!(target: "planner", "nenhum plano elegível após heurísticas");
            return Ok(PlannerEvent::Idle);
        }

        info!(
            target: "planner",
            selected = decisions.len(),
            "promovendo planos para selected"
        );
        self.store.store_decisions(&decisions)?;
        Ok(PlannerEvent::Selected(decisions))
    }

    fn kind_frequency(&self, plans: &[Plan]) -> HashMap<String, usize> {
        let mut freq = HashMap::new();
        for plan in plans {
            *freq.entry(plan.kind.clone()).or_insert(0) += 1;
        }
        freq
    }

    fn score_candidates(
        &self,
        plans: &[Plan],
        freq: &HashMap<String, usize>,
        now: DateTime<Utc>,
    ) -> Vec<(Plan, f64, String)> {
        plans
            .iter()
            .map(|plan| {
                let base = plan.curation_score;
                let trending = plan.trending_score;
                let diversity_bonus = 1.0 / (1.0 + *freq.get(&plan.kind).unwrap_or(&1) as f64);
                let age_hours = plan
                    .created_at
                    .map(|ts| (now - ts).num_minutes() as f64 / 60.0)
                    .unwrap_or(0.0);
                let duration_score = plan
                    .duration_est_s
                    .map(|duration| {
                        let diff = (duration - self.config.target_duration_seconds).abs() as f64;
                        let normalized = (1.0 - (diff / self.config.target_duration_seconds as f64)).clamp(0.0, 1.0);
                        normalized
                    })
                    .unwrap_or(0.5);
                let hd_penalty = if plan.hd_missing { 0.25 } else { 0.0 };
                let recency_bonus = (age_hours / 24.0).clamp(0.0, 1.0);

                let score = base * 0.4 + trending * 0.3 + diversity_bonus * 0.2 + duration_score * 0.2
                    + recency_bonus * 0.1 - hd_penalty;
                let rationale = format!(
                    "base={:.2} trending={:.2} diversity={:.2} duration={:.2} recency={:.2} hd_penalty={:.2}",
                    base, trending, diversity_bonus, duration_score, recency_bonus, hd_penalty
                );
                (plan.clone(), score, rationale)
            })
            .collect()
    }

    fn apply_selection_rules(
        &self,
        mut scored: Vec<(Plan, f64, String)>,
        freq: &HashMap<String, usize>,
    ) -> Vec<PlanSelectionDecision> {
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let target = scored.len().min(self.config.selection_batch_size).max(0);
        if target == 0 {
            return Vec::new();
        }

        let min_diverse = ((target as f64 * self.config.diversity_quota).ceil() as usize).max(1);
        let mut selected = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        let mut unique_candidates: Vec<_> = scored
            .iter()
            .filter(|(plan, _, _)| freq.get(&plan.kind).copied().unwrap_or(0) <= 1)
            .take(min_diverse)
            .cloned()
            .collect();
        if unique_candidates.len() < min_diverse {
            let deficit = min_diverse - unique_candidates.len();
            let mut fallback = scored
                .iter()
                .filter(|(plan, _, _)| freq.get(&plan.kind).copied().unwrap_or(0) == 2)
                .take(deficit)
                .cloned()
                .collect::<Vec<_>>();
            unique_candidates.append(&mut fallback);
        }

        for (plan, score, rationale) in unique_candidates {
            if selected.len() >= target {
                break;
            }
            seen_ids.insert(plan.plan_id.clone());
            selected.push(PlanSelectionDecision {
                plan_id: plan.plan_id,
                score,
                rationale,
            });
        }

        for (plan, score, rationale) in scored {
            if selected.len() >= target {
                break;
            }
            if seen_ids.contains(&plan.plan_id) {
                continue;
            }
            seen_ids.insert(plan.plan_id.clone());
            selected.push(PlanSelectionDecision {
                plan_id: plan.plan_id,
                score,
                rationale,
            });
        }

        selected
    }

    pub fn rollback_to_planned(&self, plan_id: &str) -> PlanResult<()> {
        let plan = self
            .store
            .fetch_by_id(plan_id)?
            .ok_or_else(|| PlanError::NotFound {
                plan_id: plan_id.to_string(),
            })?;
        if plan.status != PlanStatus::Selected {
            return Err(PlanError::InvalidStatus {
                plan_id: plan.plan_id,
                status: plan.status.to_string(),
            });
        }
        self.store.update_status(plan_id, PlanStatus::Planned)
    }
}
