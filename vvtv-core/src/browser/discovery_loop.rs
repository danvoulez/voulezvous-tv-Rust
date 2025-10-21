use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rand::Rng;
use serde::Serialize;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::browser::{
    BrowserAutomation, BrowserCaptureKind, BrowserError, BrowserResult, ContentSearcher,
    PbdOutcome, PlayBeforeDownload,
};
use crate::plan::{Plan, PlanError, PlanResult, SqlitePlanStore};

use super::searcher::Candidate;

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub max_plans_per_run: usize,
    pub candidate_delay_range_ms: (u64, u64),
    pub stop_on_first_error: bool,
    pub dry_run: bool,
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DiscoveryStats {
    pub query: String,
    pub search_engine: String,
    pub candidates_found: usize,
    pub candidates_processed: usize,
    pub plans_created: usize,
    pub dry_run: bool,
    pub total_wait_ms: u64,
    pub duration_secs: u64,
    pub errors: Vec<String>,
}

impl DiscoveryStats {
    fn new(query: &str, engine: &str, dry_run: bool) -> Self {
        Self {
            query: query.to_string(),
            search_engine: engine.to_string(),
            dry_run,
            ..Default::default()
        }
    }
}

pub struct DiscoveryLoop {
    searcher: ContentSearcher,
    pbd: Arc<dyn DiscoveryPbd>,
    plan_store: Arc<dyn DiscoveryPlanStore>,
    config: DiscoveryConfig,
    rate_limiter: RateLimiter,
}

impl DiscoveryLoop {
    pub fn new(
        searcher: ContentSearcher,
        pbd: Arc<dyn DiscoveryPbd>,
        plan_store: Arc<dyn DiscoveryPlanStore>,
        config: DiscoveryConfig,
    ) -> Self {
        let rate_limiter = RateLimiter::new(config.candidate_delay_range_ms);
        Self {
            searcher,
            pbd,
            plan_store,
            config,
            rate_limiter,
        }
    }

    pub async fn run(&mut self, query: &str) -> BrowserResult<DiscoveryStats> {
        let start = Instant::now();
        let engine = self.searcher.search_engine();
        let mut stats = DiscoveryStats::new(query, &engine.to_string(), self.config.dry_run);

        let candidates = self.searcher.search(query).await?;
        stats.candidates_found = candidates.len();
        info!(
            search_engine = %engine,
            candidates = stats.candidates_found,
            dry_run = self.config.dry_run,
            "discovery search completed"
        );

        for candidate in candidates {
            if stats.plans_created >= self.config.max_plans_per_run {
                break;
            }

            if stats.candidates_processed > 0 {
                let waited = self.rate_limiter.wait().await;
                stats.total_wait_ms += waited;
                if self.config.debug {
                    debug!(delay_ms = waited, url = %candidate.url, "rate limiting before candidate");
                }
            }

            match self.process_candidate(&candidate).await {
                Ok(Some(plan_id)) => {
                    stats.plans_created += 1;
                    if self.config.debug {
                        debug!(plan_id = %plan_id, url = %candidate.url, "plan created from discovery");
                    }
                }
                Ok(None) => {
                    if self.config.debug {
                        debug!(url = %candidate.url, "dry-run candidate processed");
                    }
                }
                Err(err) => {
                    let message = format!("{}: {}", candidate.url, err);
                    stats.errors.push(message.clone());
                    warn!(url = %candidate.url, error = %err, "discovery candidate failed");
                    if self.config.stop_on_first_error {
                        return Err(err);
                    }
                }
            }

            stats.candidates_processed += 1;
        }

        stats.duration_secs = start.elapsed().as_secs();
        info!(
            query = %stats.query,
            plans = stats.plans_created,
            processed = stats.candidates_processed,
            duration = stats.duration_secs,
            errors = stats.errors.len(),
            "discovery loop finished"
        );

        Ok(stats)
    }

    async fn process_candidate(&self, candidate: &Candidate) -> BrowserResult<Option<String>> {
        let outcome = self.pbd.collect(&candidate.url).await?;
        if outcome.capture.kind == BrowserCaptureKind::Unknown {
            return Err(BrowserError::Unexpected(
                "no playable media manifest captured".to_string(),
            ));
        }

        if self.config.dry_run {
            return Ok(None);
        }

        let plan = self
            .plan_store
            .create_from_outcome(candidate, &outcome)
            .await
            .map_err(|err| BrowserError::Unexpected(err.to_string()))?;
        Ok(Some(plan.plan_id))
    }
}

struct RateLimiter {
    range: (u64, u64),
}

impl RateLimiter {
    fn new(range: (u64, u64)) -> Self {
        Self { range }
    }

    async fn wait(&mut self) -> u64 {
        if self.range.0 == 0 && self.range.1 == 0 {
            return 0;
        }
        let mut rng = rand::thread_rng();
        let lower = self.range.0.min(self.range.1);
        let upper = self.range.0.max(self.range.1);
        let delay = rng.gen_range(lower..=upper);
        sleep(Duration::from_millis(delay)).await;
        delay
    }
}

#[async_trait(?Send)]
pub trait DiscoveryPbd: Send + Sync {
    async fn collect(&self, url: &str) -> BrowserResult<PbdOutcome>;
}

pub struct BrowserPbdRunner {
    automation: Arc<BrowserAutomation>,
    playbook: Arc<PlayBeforeDownload>,
}

impl BrowserPbdRunner {
    pub fn new(automation: Arc<BrowserAutomation>, playbook: Arc<PlayBeforeDownload>) -> Self {
        Self {
            automation,
            playbook,
        }
    }
}

#[async_trait(?Send)]
impl DiscoveryPbd for BrowserPbdRunner {
    async fn collect(&self, url: &str) -> BrowserResult<PbdOutcome> {
        self.playbook.collect(&self.automation, url).await
    }
}

#[async_trait]
pub trait DiscoveryPlanStore: Send + Sync {
    async fn create_from_outcome(
        &self,
        candidate: &Candidate,
        outcome: &PbdOutcome,
    ) -> PlanResult<Plan>;
}

#[async_trait]
impl DiscoveryPlanStore for SqlitePlanStore {
    async fn create_from_outcome(
        &self,
        candidate: &Candidate,
        outcome: &PbdOutcome,
    ) -> PlanResult<Plan> {
        let store = self.clone();
        let candidate = candidate.clone();
        let outcome = outcome.clone();
        tokio::task::spawn_blocking(move || store.create_plan_from_discovery(&candidate, &outcome))
            .await
            .map_err(|err| PlanError::Io(std::io::Error::other(err)))?
    }
}
