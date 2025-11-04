use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::llm::LlmAction;
use crate::monitor::{BusinessMetric, BusinessMetricType, MetricsStore};
use crate::plan::Plan;
use crate::plan::PlanSelectionDecision;

#[derive(Debug, Clone)]
pub struct CuratorVigilanteConfig {
    pub log_dir: PathBuf,
    pub confidence_threshold: f64,
    pub max_reorder_distance: usize,
    pub never_remove_items: bool,
    pub token_bucket_capacity: f64,
    pub token_bucket_refill_per_hour: f64,
    pub locked: bool,
}

impl CuratorVigilanteConfig {
    pub fn with_log_dir<P: AsRef<Path>>(log_dir: P) -> Self {
        Self {
            log_dir: log_dir.as_ref().to_path_buf(),
            confidence_threshold: 0.62,
            max_reorder_distance: 4,
            never_remove_items: true,
            token_bucket_capacity: 6.0,
            token_bucket_refill_per_hour: 6.0,
            locked: false,
        }
    }
}

#[derive(Debug)]
pub struct CuratorVigilante {
    config: CuratorVigilanteConfig,
    bucket: Mutex<TokenBucket>,
    metrics_store: Option<std::sync::Arc<MetricsStore>>,
}

impl CuratorVigilante {
    pub fn new(config: CuratorVigilanteConfig) -> std::io::Result<Self> {
        if let Some(parent) = config.log_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        let bucket = TokenBucket::new(
            config.token_bucket_capacity,
            config.token_bucket_refill_per_hour,
        );
        Ok(Self {
            config,
            bucket: Mutex::new(bucket),
            metrics_store: None,
        })
    }

    pub fn with_metrics_store(mut self, metrics_store: std::sync::Arc<MetricsStore>) -> Self {
        self.metrics_store = Some(metrics_store);
        self
    }

    pub fn review(
        &self,
        now: DateTime<Utc>,
        candidates: &[CuratorCandidate],
        llm_action: Option<&LlmAction>,
    ) -> CuratorReview {
        let signals = self.evaluate_signals(now, candidates);
        let triggered = signals.iter().filter(|signal| signal.triggered).count();
        let confidence = if signals.is_empty() {
            0.0
        } else {
            triggered as f64 / signals.len() as f64
        };

        let mut notes = Vec::new();
        let mut decision = CuratorDecision::Advice;
        let mut order: Vec<String> = candidates.iter().map(|c| c.plan.plan_id.clone()).collect();

        if confidence >= self.config.confidence_threshold
            && !self.config.locked
            && !candidates.is_empty()
        {
            if let Some(mut bucket) = self.bucket.lock().ok() {
                if bucket.take(1.0, Instant::now()) {
                    decision = CuratorDecision::Apply;
                    order = self.reorder(now, candidates);
                    notes.push("token_bucket:apply".to_string());
                } else {
                    notes.push("token_bucket:exhausted".to_string());
                }
            }
        }

        if self.config.locked {
            notes.push("curator_locked".to_string());
        }

        let evaluation = CuratorEvaluation {
            decision,
            confidence,
            signals,
            llm_action: llm_action.cloned(),
            notes,
        };

        if let Err(err) = self.log_evaluation(now, &evaluation, &order, candidates) {
            eprintln!("failed to log curator evaluation: {err}");
        }

        // Record curator budget usage and novelty metrics for P6 observability
        if let Some(metrics_store) = &self.metrics_store {
            if let Some(bucket) = self.bucket.lock().ok() {
                let budget_used_pct = 1.0 - (bucket.tokens / bucket.capacity);
                let context = serde_json::json!({
                    "decision": format!("{:?}", evaluation.decision),
                    "confidence": evaluation.confidence,
                    "tokens_remaining": bucket.tokens,
                    "capacity": bucket.capacity
                });
                let metric = BusinessMetric::new(BusinessMetricType::CuratorApplyBudgetUsedPct, budget_used_pct)
                    .with_context(context);
                
                if let Err(e) = metrics_store.record_business_metric(&metric) {
                    eprintln!("failed to record curator budget metric: {e}");
                }
            }

            // Record novelty temporal KLD from signals
            if let Some(novelty_signal) = evaluation.signals.iter().find(|s| s.name == "novelty_temporal_kld") {
                let context = serde_json::json!({
                    "candidates_count": candidates.len(),
                    "triggered": novelty_signal.triggered,
                    "threshold": novelty_signal.threshold
                });
                let metric = BusinessMetric::new(BusinessMetricType::NoveltyTemporalKld, novelty_signal.value)
                    .with_context(context);
                
                if let Err(e) = metrics_store.record_business_metric(&metric) {
                    eprintln!("failed to record novelty temporal KLD metric: {e}");
                }
            }
        }

        CuratorReview { evaluation, order }
    }

    fn evaluate_signals(
        &self,
        now: DateTime<Utc>,
        candidates: &[CuratorCandidate],
    ) -> Vec<CuratorSignal> {
        let mut signals = Vec::new();
        if candidates.len() >= 2 {
            let palette = palette_similarity(candidates);
            signals.push(CuratorSignal {
                name: "palette_similarity".into(),
                value: palette,
                threshold: 0.92,
                triggered: palette >= 0.92,
            });

            let tag_dup = tag_duplication(candidates);
            signals.push(CuratorSignal {
                name: "tag_duplication".into(),
                value: tag_dup,
                threshold: 0.7,
                triggered: tag_dup >= 0.7,
            });
        }

        let streak = duration_streak(candidates);
        signals.push(CuratorSignal {
            name: "duration_streak".into(),
            value: streak as f64,
            threshold: 4.0,
            triggered: streak >= 4,
        });

        let imbalance = bucket_imbalance(candidates);
        signals.push(CuratorSignal {
            name: "bucket_imbalance".into(),
            value: imbalance,
            threshold: 0.55,
            triggered: imbalance >= 0.55,
        });

        let novelty = novelty_temporal_kld(now, candidates);
        signals.push(CuratorSignal {
            name: "novelty_temporal_kld".into(),
            value: novelty,
            threshold: 0.25,
            triggered: novelty >= 0.25,
        });

        let cadence = cadence_variation(candidates);
        signals.push(CuratorSignal {
            name: "cadence_variation".into(),
            value: cadence,
            threshold: 0.05,
            triggered: cadence <= 0.05,
        });

        signals
    }

    fn reorder(&self, now: DateTime<Utc>, candidates: &[CuratorCandidate]) -> Vec<String> {
        if self.config.never_remove_items {
            let mut best_idx = None;
            let mut best_score = f64::MIN;
            for (idx, candidate) in candidates.iter().enumerate() {
                let novelty = novelty_score(now, candidate, candidates);
                if novelty > best_score {
                    best_score = novelty;
                    best_idx = Some(idx);
                }
            }

            if let Some(current_idx) = best_idx {
                let target = current_idx.saturating_sub(self.config.max_reorder_distance);
                let mut order: Vec<String> = candidates
                    .iter()
                    .map(|candidate| candidate.plan.plan_id.clone())
                    .collect();
                if current_idx != target {
                    let plan_id = order.remove(current_idx);
                    order.insert(target, plan_id);
                    return order;
                }
            }
        }

        candidates
            .iter()
            .map(|candidate| candidate.plan.plan_id.clone())
            .collect()
    }

    fn log_evaluation(
        &self,
        now: DateTime<Utc>,
        evaluation: &CuratorEvaluation,
        order: &[String],
        candidates: &[CuratorCandidate],
    ) -> std::io::Result<()> {
        let file_path = self
            .config
            .log_dir
            .join(format!("curator_vigilante_{}.jsonl", now.date_naive()));
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        let entry = CuratorLogEntry {
            timestamp: now,
            evaluation,
            order,
            plans: candidates
                .iter()
                .map(|candidate| candidate.plan.plan_id.clone())
                .collect(),
        };
        let line = serde_json::to_string(&entry)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CuratorCandidate {
    pub plan: Plan,
    pub score: f64,
    pub rationale: String,
}

impl CuratorCandidate {
    pub fn into_decision(self, rationale_suffix: &str) -> PlanSelectionDecision {
        PlanSelectionDecision {
            plan_id: self.plan.plan_id,
            score: self.score,
            rationale: format!("{} | {}", self.rationale, rationale_suffix),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CuratorSignal {
    pub name: String,
    pub value: f64,
    pub threshold: f64,
    pub triggered: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CuratorDecision {
    Advice,
    Apply,
}

#[derive(Debug, Clone, Serialize)]
pub struct CuratorEvaluation {
    pub decision: CuratorDecision,
    pub confidence: f64,
    pub signals: Vec<CuratorSignal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_action: Option<LlmAction>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CuratorReview {
    pub evaluation: CuratorEvaluation,
    pub order: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CuratorLogEntry<'a> {
    pub timestamp: DateTime<Utc>,
    pub evaluation: &'a CuratorEvaluation,
    pub order: &'a [String],
    pub plans: Vec<String>,
}

#[derive(Debug)]
pub struct TokenBucket {
    pub capacity: f64,
    pub tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_per_hour: f64) -> Self {
        let refill_per_sec = if refill_per_hour <= 0.0 {
            0.0
        } else {
            refill_per_hour / 3600.0
        };
        Self {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last_refill: Instant::now(),
        }
    }

    pub fn take(&mut self, amount: f64, now: Instant) -> bool {
        self.refill(now);
        if self.tokens + 1e-6 >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        let seconds = elapsed.as_secs_f64();
        self.tokens = (self.tokens + seconds * self.refill_per_sec).min(self.capacity);
        self.last_refill = now;
    }
}

fn palette_similarity(candidates: &[CuratorCandidate]) -> f64 {
    let mut similarities = Vec::new();
    for pair in candidates.windows(2) {
        if let [a, b] = pair {
            if let (Some(vec_a), Some(vec_b)) = (&a.plan.desire_vector, &b.plan.desire_vector) {
                if let Some(sim) = cosine_similarity(vec_a, vec_b) {
                    similarities.push(sim);
                }
            }
        }
    }
    if similarities.is_empty() {
        0.0
    } else {
        similarities.iter().copied().sum::<f64>() / similarities.len() as f64
    }
}

fn tag_duplication(candidates: &[CuratorCandidate]) -> f64 {
    let mut scores = Vec::new();
    for pair in candidates.windows(2) {
        if let [a, b] = pair {
            let jaccard = jaccard_index(&a.plan.tags, &b.plan.tags);
            scores.push(jaccard);
        }
    }
    if scores.is_empty() {
        0.0
    } else {
        scores.iter().copied().sum::<f64>() / scores.len() as f64
    }
}

fn duration_streak(candidates: &[CuratorCandidate]) -> usize {
    let mut longest = 0;
    let mut current = 0;
    let mut last_duration = None;
    for candidate in candidates {
        if let Some(duration) = candidate.plan.duration_est_s {
            match last_duration {
                Some(prev) if duration.abs_diff(prev) <= 20 => {
                    current += 1;
                }
                _ => {
                    current = 1;
                }
            }
            longest = longest.max(current);
            last_duration = Some(duration);
        }
    }
    longest
}

fn bucket_imbalance(candidates: &[CuratorCandidate]) -> f64 {
    if candidates.is_empty() {
        return 0.0;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for candidate in candidates {
        *counts.entry(&candidate.plan.kind).or_default() += 1;
    }
    let max = counts.values().copied().max().unwrap_or(0) as f64;
    let min = counts.values().copied().min().unwrap_or(0) as f64;
    if (max + min).abs() < f64::EPSILON {
        0.0
    } else {
        (max - min) / (max + min)
    }
}

fn novelty_temporal_kld(now: DateTime<Utc>, candidates: &[CuratorCandidate]) -> f64 {
    if candidates.is_empty() {
        return 0.0;
    }
    let mut buckets = [0usize; 3];
    for candidate in candidates {
        let age_hours = candidate
            .plan
            .created_at
            .map(|ts| (now - ts).num_minutes() as f64 / 60.0)
            .unwrap_or(24.0);
        if age_hours <= 12.0 {
            buckets[0] += 1;
        } else if age_hours <= 24.0 {
            buckets[1] += 1;
        } else {
            buckets[2] += 1;
        }
    }

    let total = buckets.iter().sum::<usize>() as f64;
    if total <= 0.0 {
        return 0.0;
    }

    let target = [0.45, 0.35, 0.20];
    buckets
        .iter()
        .zip(target.iter())
        .map(|(count, reference)| {
            let p = *count as f64 / total;
            if p <= 0.0 {
                0.0
            } else {
                let denom = if *reference < 1e-6_f64 {
                    1e-6_f64
                } else {
                    *reference
                };
                p * (p / denom).ln()
            }
        })
        .sum()
}

fn cadence_variation(candidates: &[CuratorCandidate]) -> f64 {
    if candidates.len() < 2 {
        return 0.0;
    }
    let values: Vec<f64> = candidates
        .iter()
        .map(|candidate| candidate.plan.engagement_score)
        .collect();
    let mean = values.iter().copied().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return None;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += (*x as f64) * (*y as f64);
        norm_a += (*x as f64).powi(2);
        norm_b += (*y as f64).powi(2);
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return None;
    }
    Some(dot / (norm_a.sqrt() * norm_b.sqrt()))
}

fn jaccard_index(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let set_a: HashSet<&str> = a.iter().map(|value| value.as_str()).collect();
    let set_b: HashSet<&str> = b.iter().map(|value| value.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn novelty_score(
    now: DateTime<Utc>,
    candidate: &CuratorCandidate,
    candidates: &[CuratorCandidate],
) -> f64 {
    let age_hours = candidate
        .plan
        .created_at
        .map(|ts| (now - ts).num_minutes() as f64 / 60.0)
        .unwrap_or(24.0)
        .min(72.0);
    let recency = (age_hours / 72.0).clamp(0.0, 1.0);
    let uniqueness = candidates
        .iter()
        .filter(|other| other.plan.plan_id != candidate.plan.plan_id)
        .map(|other| 1.0 - jaccard_index(&candidate.plan.tags, &other.plan.tags))
        .sum::<f64>()
        / (candidates.len().saturating_sub(1).max(1) as f64);
    recency * 0.5 + uniqueness * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn token_bucket_refills() {
        let mut bucket = TokenBucket::new(2.0, 6.0);
        assert!(bucket.take(1.0, Instant::now()));
        assert!(bucket.take(1.0, Instant::now()));
        assert!(!bucket.take(1.0, Instant::now()));
        let later = Instant::now() + Duration::from_secs(3600);
        assert!(bucket.take(1.0, later));
    }

    #[test]
    fn curator_detects_tag_duplication() {
        let plan = |id: &str, tags: &[&str]| {
            let mut plan = Plan::new(id, "music");
            plan.tags = tags.iter().map(|tag| tag.to_string()).collect();
            CuratorCandidate {
                plan,
                score: 1.0,
                rationale: "r".into(),
            }
        };

        let candidates = vec![plan("a", &["x", "y"]), plan("b", &["x", "y"])];
        let config = CuratorVigilanteConfig::with_log_dir("/tmp/curator-tests");
        let vigilante = CuratorVigilante::new(config).unwrap();
        let review = vigilante.review(Utc::now(), &candidates, None);
        assert!(!review.evaluation.signals.is_empty());
        assert!(review
            .evaluation
            .signals
            .iter()
            .any(|signal| signal.name == "tag_duplication"));
    }
}
