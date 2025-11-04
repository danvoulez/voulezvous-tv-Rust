use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use tokio::runtime::Handle;
use tracing::{info, warn};

use crate::business_logic::{BusinessLogic, SelectionMethod};
use crate::curation::{CuratorCandidate, CuratorDecision, CuratorReview, CuratorVigilante};
use crate::llm::{LlmAction, LlmInvocation, LlmInvocationResult, LlmOrchestrator, LlmResultMode};
use crate::monitor::{BusinessMetric, BusinessMetricType, MetricsStore};

use super::models::{Plan, PlanSelectionDecision};
use super::selection::{generate_slot_seed_robust, gumbel_topk_indices, normalize_scores};
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

pub struct Planner {
    store: SqlitePlanStore,
    config: PlannerConfig,
    business_logic: Arc<BusinessLogic>,
    llm: Option<Arc<LlmOrchestrator>>,
    curator: Option<Arc<CuratorVigilante>>,
    metrics_store: Option<Arc<MetricsStore>>,
}

impl Planner {
    pub fn new(
        store: SqlitePlanStore,
        config: PlannerConfig,
        business_logic: Arc<BusinessLogic>,
    ) -> Self {
        Self {
            store,
            config,
            business_logic,
            llm: None,
            curator: None,
            metrics_store: None,
        }
    }

    pub fn with_llm(mut self, orchestrator: Arc<LlmOrchestrator>) -> Self {
        self.llm = Some(orchestrator);
        self
    }

    pub fn with_curator(mut self, curator: Arc<CuratorVigilante>) -> Self {
        self.curator = Some(curator);
        self
    }

    pub fn with_metrics_store(mut self, metrics_store: Arc<MetricsStore>) -> Self {
        self.metrics_store = Some(metrics_store);
        self
    }

    pub fn run_once(&self, now: DateTime<Utc>) -> PlanResult<PlannerEvent> {
        if let Ok(handle) = Handle::try_current() {
            handle.block_on(self.run_once_async(now))
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(self.run_once_async(now))
        }
    }

    pub async fn run_once_async(&self, now: DateTime<Utc>) -> PlanResult<PlannerEvent> {
        let candidates = self
            .store
            .fetch_candidates_for_scoring(self.config.selection_limit)?;
        if candidates.is_empty() {
            return Ok(PlannerEvent::Idle);
        }

        let kind_frequency = self.kind_frequency(&candidates);
        let mut scored = self.score_candidates(&candidates, &kind_frequency, now);
        self.apply_business_bias(&mut scored);

        let (ordered, llm_result) = self.apply_llm(scored).await;
        let mut selected = self.apply_selection_strategy(&ordered, &kind_frequency, now);

        if let Some(result) = &llm_result {
            for candidate in selected.iter_mut() {
                candidate.set_llm_action(Some(result.action.clone()), result.mode.clone());
            }
            info!(
                target: "planner.llm",
                mode = ?result.mode,
                source = %result.action.source,
                reason = %result.action.reason,
                "llm rerank processado"
            );
        }

        if selected.is_empty() {
            warn!(target: "planner", "nenhum plano elegível após heurísticas");
            return Ok(PlannerEvent::Idle);
        }

        let curator_review = self.apply_curator(now, &mut selected, &llm_result);
        if let Some(review) = &curator_review {
            info!(
                target: "planner.curator",
                decision = ?review.evaluation.decision,
                confidence = review.evaluation.confidence,
                notes = ?review.evaluation.notes,
                "curator vigilante avaliou seleção"
            );
            if review.evaluation.decision == CuratorDecision::Apply {
                self.annotate(&mut selected, "curator=apply");
            } else {
                self.annotate(&mut selected, "curator=advice");
            }
        }

        let decisions: Vec<PlanSelectionDecision> = selected
            .into_iter()
            .map(SelectedCandidate::into_decision)
            .collect();

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
                        (1.0 - (diff / self.config.target_duration_seconds as f64)).clamp(0.0, 1.0)
                    })
                    .unwrap_or(0.5);
                let hd_penalty = if plan.hd_missing { 0.25 } else { 0.0 };
                let recency_bonus = (age_hours / 24.0).clamp(0.0, 1.0);

                let score = base * 0.4
                    + trending * 0.3
                    + diversity_bonus * 0.2
                    + duration_score * 0.2
                    + recency_bonus * 0.1
                    - hd_penalty;
                let rationale = format!(
                    "base={:.2} trending={:.2} diversity={:.2} duration={:.2} recency={:.2} hd_penalty={:.2}",
                    base, trending, diversity_bonus, duration_score, recency_bonus, hd_penalty
                );
                (plan.clone(), score, rationale)
            })
            .collect()
    }

    fn apply_business_bias(&self, scored: &mut [(Plan, f64, String)]) {
        let bias = self.business_logic.plan_selection_bias();
        for (_, score, _) in scored.iter_mut() {
            *score += bias;
        }
    }

    async fn apply_llm(
        &self,
        scored: Vec<(Plan, f64, String)>,
    ) -> (Vec<(Plan, f64, String)>, Option<LlmInvocationResult>) {
        let orchestrator = match &self.llm {
            Some(orchestrator) => orchestrator.clone(),
            None => return (scored, None),
        };

        let invocations: Vec<LlmInvocation> = scored
            .iter()
            .map(|(plan, score, rationale)| {
                LlmInvocation::from_plan(plan, *score, rationale.clone())
            })
            .collect();
        match orchestrator.rerank_candidates(invocations).await {
            Ok(result) => {
                if result.mode == LlmResultMode::Apply && !result.order.is_empty() {
                    let mut by_id: HashMap<String, (Plan, f64, String)> = scored
                        .into_iter()
                        .map(|entry| (entry.0.plan_id.clone(), entry))
                        .collect();
                    let mut ordered = Vec::new();
                    for plan_id in &result.order {
                        if let Some(entry) = by_id.remove(plan_id) {
                            ordered.push(entry);
                        }
                    }
                    ordered.extend(by_id.into_values());
                    (ordered, Some(result))
                } else {
                    (scored, Some(result))
                }
            }
            Err(err) => {
                warn!(target: "planner.llm", "rerank_candidates falhou: {err}");
                (scored, None)
            }
        }
    }

    fn apply_selection_strategy(
        &self,
        ordered: &[(Plan, f64, String)],
        freq: &HashMap<String, usize>,
        now: DateTime<Utc>,
    ) -> Vec<SelectedCandidate> {
        if ordered.is_empty() {
            return Vec::new();
        }

        let method = self.business_logic.selection_method();
        let top_k = self
            .business_logic
            .selection_top_k(self.config.selection_batch_size)
            .min(ordered.len());

        let ordered = match method {
            SelectionMethod::GumbelTopK => {
                let temperature = self.business_logic.selection_temperature().max(1e-3);
                let scaled_scores: Vec<f64> = ordered
                    .iter()
                    .map(|(_, score, _)| *score / temperature)
                    .collect();
                let seed = generate_slot_seed_robust(
                    now,
                    self.business_logic.slot_duration(),
                    self.business_logic.window_id(),
                    self.business_logic.global_seed(),
                );
                let mut rng = ChaCha20Rng::seed_from_u64(seed);
                let indices = gumbel_topk_indices(&scaled_scores, top_k, &mut rng);
                let normalized = normalize_scores(&scaled_scores);
                info!(
                    target: "planner.selection",
                    method = ?method,
                    seed,
                    temperature,
                    top_k,
                    indices = ?indices,
                    scores_norm = ?normalized,
                    "executed gumbel-top-k"
                );
                indices
                    .into_iter()
                    .map(|index| ordered[index].clone())
                    .collect::<Vec<_>>()
            }
            _ => {
                let mut copy = ordered.to_vec();
                copy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                copy.truncate(top_k);
                copy
            }
        };

        let selected = self.enforce_diversity(&ordered, freq, top_k);
        
        // Record selection entropy metric for P6 observability
        if let Some(metrics_store) = &self.metrics_store {
            let entropy = self.calculate_selection_entropy(&selected);
            let context = serde_json::json!({
                "method": format!("{:?}", method),
                "temperature": self.business_logic.selection_temperature(),
                "top_k": top_k,
                "selected_count": selected.len()
            });
            let metric = BusinessMetric::new(BusinessMetricType::SelectionEntropy, entropy)
                .with_context(context);
            
            if let Err(e) = metrics_store.record_business_metric(&metric) {
                warn!(target: "planner.metrics", "failed to record selection entropy: {e}");
            }
        }
        
        selected
    }

    fn enforce_diversity(
        &self,
        ordered: &[(Plan, f64, String)],
        freq: &HashMap<String, usize>,
        top_k: usize,
    ) -> Vec<SelectedCandidate> {
        let target = ordered.len().min(top_k);
        if target == 0 {
            return Vec::new();
        }

        let min_diverse = ((target as f64 * self.config.diversity_quota).ceil() as usize).max(1);
        let mut selected = Vec::new();
        let mut selected_ids = HashSet::new();
        let mut selected_kinds = HashSet::new();

        for (plan, score, rationale) in ordered {
            if selected.len() >= min_diverse {
                break;
            }
            if selected_kinds.insert(plan.kind.clone()) {
                selected_ids.insert(plan.plan_id.clone());
                selected.push(SelectedCandidate::new(
                    plan.clone(),
                    *score,
                    rationale.clone(),
                ));
            }
        }

        for (plan, score, rationale) in ordered {
            if selected.len() >= target {
                break;
            }
            if selected_ids.contains(&plan.plan_id) {
                continue;
            }
            if selected_kinds.len() < min_diverse
                && freq.get(&plan.kind).copied().unwrap_or(0) <= 1
                && selected_kinds.insert(plan.kind.clone())
            {
                selected_ids.insert(plan.plan_id.clone());
                selected.push(SelectedCandidate::new(
                    plan.clone(),
                    *score,
                    rationale.clone(),
                ));
            } else if selected.len() < target {
                selected_ids.insert(plan.plan_id.clone());
                selected.push(SelectedCandidate::new(
                    plan.clone(),
                    *score,
                    rationale.clone(),
                ));
            }
        }

        selected
    }

    fn apply_curator(
        &self,
        now: DateTime<Utc>,
        selected: &mut Vec<SelectedCandidate>,
        llm_result: &Option<LlmInvocationResult>,
    ) -> Option<CuratorReview> {
        let curator = self.curator.as_ref()?;
        let inputs: Vec<CuratorCandidate> = selected
            .iter()
            .map(|candidate| candidate.to_curator_candidate())
            .collect();
        let llm_action = llm_result.as_ref().map(|result| &result.action);
        let review = curator.review(now, &inputs, llm_action);

        if review.evaluation.decision == CuratorDecision::Apply {
            let positions: HashMap<&str, usize> = review
                .order
                .iter()
                .enumerate()
                .map(|(idx, id)| (id.as_str(), idx))
                .collect();
            selected.sort_by_key(|candidate| {
                positions
                    .get(candidate.plan.plan_id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }

        Some(review)
    }

    fn annotate(&self, selected: &mut [SelectedCandidate], annotation: &str) {
        for candidate in selected.iter_mut() {
            candidate.annotations.push(annotation.to_string());
        }
    }

    /// Calculate Shannon entropy of the selection to measure diversity
    fn calculate_selection_entropy(&self, selected: &[SelectedCandidate]) -> f64 {
        if selected.is_empty() {
            return 0.0;
        }

        // Count frequency of each content type/kind
        let mut kind_counts: HashMap<String, usize> = HashMap::new();
        for candidate in selected {
            *kind_counts.entry(candidate.plan.kind.clone()).or_insert(0) += 1;
        }

        let total = selected.len() as f64;
        let mut entropy = 0.0;

        for count in kind_counts.values() {
            let probability = *count as f64 / total;
            if probability > 0.0 {
                entropy -= probability * probability.log2();
            }
        }

        entropy
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

#[derive(Debug, Clone)]
struct SelectedCandidate {
    plan: Plan,
    score: f64,
    rationale: String,
    annotations: Vec<String>,
    llm_action: Option<LlmAction>,
}

impl SelectedCandidate {
    fn new(plan: Plan, score: f64, rationale: String) -> Self {
        Self {
            plan,
            score,
            rationale,
            annotations: Vec::new(),
            llm_action: None,
        }
    }

    fn set_llm_action(&mut self, action: Option<LlmAction>, mode: LlmResultMode) {
        if let Some(action) = action {
            let annotation = format!(
                "llm_action={{source:{} model:{} mode={:?} reason={}}}",
                action.source, action.model, mode, action.reason
            );
            self.annotations.push(annotation);
            self.llm_action = Some(action);
        }
    }

    fn to_curator_candidate(&self) -> CuratorCandidate {
        CuratorCandidate {
            plan: self.plan.clone(),
            score: self.score,
            rationale: self.rationale.clone(),
        }
    }

    fn into_decision(mut self) -> PlanSelectionDecision {
        if let Some(action) = &self.llm_action {
            self.annotations
                .push(format!("llm_confidence={:.2}", action.confidence));
        }
        let rationale = if self.annotations.is_empty() {
            self.rationale
        } else {
            format!("{} | {}", self.rationale, self.annotations.join(" | "))
        };
        PlanSelectionDecision {
            plan_id: self.plan.plan_id,
            score: self.score,
            rationale,
        }
    }
}
