use std::future::Future;

use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::models::Plan;
use super::store::SqlitePlanStore;
use super::{PlanResult, PlanStatus};

#[derive(Debug, Clone)]
pub struct RealizerConfig {
    pub poll_interval: Duration,
    pub max_attempts: usize,
}

impl Default for RealizerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(10),
            max_attempts: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RealizationOutcome {
    Downloaded { mark_ready: bool },
    Retry { note: String },
    Failed { note: String },
}

#[derive(Debug)]
pub struct Realizer {
    store: SqlitePlanStore,
    config: RealizerConfig,
}

impl Realizer {
    pub fn new(store: SqlitePlanStore, config: RealizerConfig) -> Self {
        Self { store, config }
    }

    pub async fn run_loop<F, Fut>(&self, mut handler: F) -> PlanResult<()>
    where
        F: FnMut(Plan) -> Fut,
        Fut: Future<Output = PlanResult<RealizationOutcome>>,
    {
        loop {
            let processed = self.tick(&mut handler).await?;
            if !processed {
                sleep(self.config.poll_interval).await;
            }
        }
    }

    pub async fn tick<F, Fut>(&self, handler: &mut F) -> PlanResult<bool>
    where
        F: FnMut(Plan) -> Fut,
        Fut: Future<Output = PlanResult<RealizationOutcome>>,
    {
        let Some(plan) = self.store.reserve_next()? else {
            return Ok(false);
        };
        info!(plan_id = %plan.plan_id, "realizer reservou plano");
        let outcome = handler(plan.clone()).await?;
        self.handle_outcome(plan, outcome)?;
        Ok(true)
    }

    fn handle_outcome(&self, plan: Plan, outcome: RealizationOutcome) -> PlanResult<()> {
        match outcome {
            RealizationOutcome::Downloaded { mark_ready } => {
                self.store.complete_download(&plan.plan_id)?;
                self.store.record_attempt(
                    &plan.plan_id,
                    Some(PlanStatus::InProgress),
                    Some(PlanStatus::Downloaded),
                    "download concluído",
                )?;
                if mark_ready {
                    self.store.finalize_ready(&plan.plan_id)?;
                }
                self.store.reset_failures(&plan.plan_id)?;
                info!(plan_id = %plan.plan_id, "plan marcado como download concluído");
            }
            RealizationOutcome::Retry { note } => {
                self.store.increment_failure(&plan.plan_id)?;
                self.store.record_attempt(
                    &plan.plan_id,
                    Some(PlanStatus::InProgress),
                    Some(PlanStatus::Selected),
                    note,
                )?;
                if plan.failure_count as usize + 1 >= self.config.max_attempts {
                    warn!(plan_id = %plan.plan_id, "falha máxima atingida, marcando como failed");
                    self.store
                        .fail_plan(&plan.plan_id, "limite de tentativas excedido")?;
                } else {
                    self.store
                        .update_status(&plan.plan_id, PlanStatus::Selected)?;
                }
            }
            RealizationOutcome::Failed { note } => {
                warn!(plan_id = %plan.plan_id, "plan falhou definitivamente");
                self.store.fail_plan(&plan.plan_id, note)?;
            }
        }
        Ok(())
    }
}
