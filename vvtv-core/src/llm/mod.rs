use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::warn;

use crate::plan::Plan;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("hook handler error: {0}")]
    Handler(String),
    #[error("hook {0:?} not configured")]
    HookMissing(LlmHookKind),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmResultMode {
    AdviceOnly,
    Apply,
}

impl Default for LlmResultMode {
    fn default() -> Self {
        LlmResultMode::AdviceOnly
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LlmHookKind {
    ExpandQueries,
    SiteTactics,
    RerankCandidates,
    RecoveryPlan,
    EnrichMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAction {
    pub source: String,
    pub model: String,
    pub reason: String,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
}

impl LlmAction {
    pub fn advice(source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            model: "fallback".into(),
            reason: reason.into(),
            confidence: 0.0,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmInvocation {
    pub plan_id: String,
    pub score: f64,
    pub rationale: String,
    pub tags: Vec<String>,
    pub kind: String,
}

impl LlmInvocation {
    pub fn from_plan(plan: &Plan, score: f64, rationale: impl Into<String>) -> Self {
        Self {
            plan_id: plan.plan_id.clone(),
            score,
            rationale: rationale.into(),
            tags: plan.tags.clone(),
            kind: plan.kind.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmHookOutcome {
    pub action: LlmAction,
    #[serde(default)]
    pub mode: LlmResultMode,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmHookRequest {
    pub hook: LlmHookKind,
    pub deadline_ms: u64,
    pub allowed_actions: Vec<String>,
    pub budget_tokens: usize,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct LlmInvocationResult {
    pub action: LlmAction,
    pub mode: LlmResultMode,
    pub order: Vec<String>,
    pub payload: Value,
}

#[async_trait]
pub trait LlmHookHandler: Send + Sync {
    async fn handle(&self, request: LlmHookRequest) -> Result<LlmHookOutcome, LlmError>;
}

pub struct HttpLlmHandler {
    client: reqwest::Client,
    endpoint: String,
}

impl HttpLlmHandler {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
        }
    }
}

#[async_trait]
impl LlmHookHandler for HttpLlmHandler {
    async fn handle(&self, request: LlmHookRequest) -> Result<LlmHookOutcome, LlmError> {
        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(LlmError::Handler(format!(
                "unexpected status {}",
                response.status()
            )));
        }
        Ok(response.json::<LlmHookOutcome>().await?)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    pub window_size: usize,
    pub failure_threshold: f64,
    pub open_for: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            failure_threshold: 0.10,
            open_for: Duration::from_secs(300),
        }
    }
}

struct CircuitBreaker {
    config: CircuitBreakerConfig,
    outcomes: VecDeque<bool>,
    state: CircuitState,
    open_until: Option<Instant>,
}

enum CircuitState {
    Closed,
    HalfOpen,
    Open,
}

enum CircuitDecision {
    Proceed,
    ShortCircuit,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            outcomes: VecDeque::with_capacity(config.window_size),
            state: CircuitState::Closed,
            open_until: None,
        }
    }

    fn before_call(&mut self, now: Instant) -> CircuitDecision {
        match self.state {
            CircuitState::Closed => CircuitDecision::Proceed,
            CircuitState::HalfOpen => CircuitDecision::Proceed,
            CircuitState::Open => {
                if let Some(until) = self.open_until {
                    if now >= until {
                        self.state = CircuitState::HalfOpen;
                        CircuitDecision::Proceed
                    } else {
                        CircuitDecision::ShortCircuit
                    }
                } else {
                    CircuitDecision::Proceed
                }
            }
        }
    }

    fn record(&mut self, now: Instant, success: bool) {
        match self.state {
            CircuitState::HalfOpen => {
                if success {
                    self.state = CircuitState::Closed;
                    self.outcomes.clear();
                } else {
                    self.trip(now);
                }
            }
            CircuitState::Closed => {
                self.push_outcome(success);
                if self.should_trip() {
                    self.trip(now);
                }
            }
            CircuitState::Open => {
                if success {
                    self.state = CircuitState::HalfOpen;
                } else {
                    self.trip(now);
                }
            }
        }
    }

    fn push_outcome(&mut self, success: bool) {
        if self.outcomes.len() == self.config.window_size {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back(success);
    }

    fn should_trip(&self) -> bool {
        if self.outcomes.is_empty() {
            return false;
        }
        let failures = self.outcomes.iter().filter(|outcome| !**outcome).count();
        let rate = failures as f64 / self.outcomes.len() as f64;
        rate > self.config.failure_threshold
    }

    fn trip(&mut self, now: Instant) {
        self.state = CircuitState::Open;
        self.open_until = Some(now + self.config.open_for);
    }
}

pub struct LlmHook {
    kind: LlmHookKind,
    handler: Arc<dyn LlmHookHandler>,
    breaker: CircuitBreaker,
    allowed_actions: Vec<String>,
    budget_tokens: usize,
    deadline: Duration,
}

impl LlmHook {
    pub fn new(
        kind: LlmHookKind,
        handler: Arc<dyn LlmHookHandler>,
        deadline: Duration,
        allowed_actions: Vec<String>,
        budget_tokens: usize,
        breaker: CircuitBreakerConfig,
    ) -> Self {
        Self {
            kind,
            handler,
            breaker: CircuitBreaker::new(breaker),
            allowed_actions,
            budget_tokens,
            deadline,
        }
    }

    fn fallback(&self, reason: &str) -> LlmHookOutcome {
        LlmHookOutcome {
            action: LlmAction::advice(format!("{:?}", self.kind), reason),
            mode: LlmResultMode::AdviceOnly,
            payload: Value::Null,
        }
    }

    async fn invoke(&mut self, payload: Value) -> LlmHookOutcome {
        let now = Instant::now();
        match self.breaker.before_call(now) {
            CircuitDecision::ShortCircuit => {
                return self.fallback("circuit_open");
            }
            CircuitDecision::Proceed => {}
        }

        let request = LlmHookRequest {
            hook: self.kind,
            deadline_ms: self.deadline.as_millis().max(1) as u64,
            allowed_actions: self.allowed_actions.clone(),
            budget_tokens: self.budget_tokens,
            payload,
        };

        let fut = self.handler.handle(request);
        match timeout(self.deadline, fut).await {
            Ok(Ok(outcome)) => {
                self.breaker.record(now, true);
                outcome
            }
            Ok(Err(err)) => {
                warn!(target: "llm", hook = ?self.kind, "handler error: {err}");
                self.breaker.record(now, false);
                self.fallback("handler_error")
            }
            Err(_) => {
                warn!(target: "llm", hook = ?self.kind, "timeout after {:?}", self.deadline);
                self.breaker.record(now, false);
                self.fallback("timeout")
            }
        }
    }
}

pub struct LlmOrchestrator {
    hooks: HashMap<LlmHookKind, Mutex<LlmHook>>,
}

impl LlmOrchestrator {
    pub fn new(hooks: Vec<LlmHook>) -> Self {
        let mut map = HashMap::new();
        for hook in hooks {
            map.insert(hook.kind, Mutex::new(hook));
        }
        Self { hooks: map }
    }

    pub fn is_enabled(&self, kind: LlmHookKind) -> bool {
        self.hooks.contains_key(&kind)
    }

    async fn invoke(&self, kind: LlmHookKind, payload: Value) -> Result<LlmHookOutcome, LlmError> {
        if let Some(hook) = self.hooks.get(&kind) {
            let mut guard = hook.lock().await;
            Ok(guard.invoke(payload).await)
        } else {
            Err(LlmError::HookMissing(kind))
        }
    }

    pub async fn rerank_candidates(
        &self,
        candidates: Vec<LlmInvocation>,
    ) -> Result<LlmInvocationResult, LlmError> {
        let payload = serde_json::json!({
            "candidates": candidates
                .iter()
                .map(|candidate| serde_json::json!({
                    "plan_id": candidate.plan_id,
                    "score": candidate.score,
                    "rationale": candidate.rationale,
                    "tags": candidate.tags,
                    "kind": candidate.kind,
                }))
                .collect::<Vec<_>>()
        });
        match self.invoke(LlmHookKind::RerankCandidates, payload).await {
            Ok(outcome) => {
                let order = outcome
                    .payload
                    .get("order")
                    .and_then(|value| value.as_array())
                    .map(|array| {
                        array
                            .iter()
                            .filter_map(|value| value.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Ok(LlmInvocationResult {
                    action: outcome.action,
                    mode: outcome.mode,
                    order,
                    payload: outcome.payload,
                })
            }
            Err(LlmError::HookMissing(kind)) => Ok(LlmInvocationResult {
                action: LlmAction::advice(format!("{:?}", kind), "hook_disabled"),
                mode: LlmResultMode::AdviceOnly,
                order: Vec::new(),
                payload: Value::Null,
            }),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticHandler {
        outcome: LlmHookOutcome,
    }

    #[async_trait]
    impl LlmHookHandler for StaticHandler {
        async fn handle(&self, _request: LlmHookRequest) -> Result<LlmHookOutcome, LlmError> {
            Ok(self.outcome.clone())
        }
    }

    #[tokio::test]
    async fn circuit_breaker_short_circuits() {
        let handler = Arc::new(StaticHandler {
            outcome: LlmHookOutcome {
                action: LlmAction::advice("test", "ok"),
                mode: LlmResultMode::Apply,
                payload: serde_json::json!({"order": ["a", "b"]}),
            },
        });

        let mut hook = LlmHook::new(
            LlmHookKind::RerankCandidates,
            handler,
            Duration::from_millis(10),
            vec!["apply".into()],
            512,
            CircuitBreakerConfig::default(),
        );

        // Trip the breaker manually by recording failures.
        for _ in 0..60 {
            hook.breaker.record(Instant::now(), false);
        }

        let outcome = hook.invoke(Value::Null).await;
        assert_eq!(outcome.mode, LlmResultMode::AdviceOnly);
        assert_eq!(outcome.action.reason, "circuit_open");
    }

    #[tokio::test]
    async fn orchestrator_rerank_parses_order() {
        let handler = Arc::new(StaticHandler {
            outcome: LlmHookOutcome {
                action: LlmAction::advice("rerank", "applied"),
                mode: LlmResultMode::Apply,
                payload: serde_json::json!({"order": ["b", "a"]}),
            },
        });

        let hook = LlmHook::new(
            LlmHookKind::RerankCandidates,
            handler,
            Duration::from_millis(10),
            vec!["apply".into()],
            128,
            CircuitBreakerConfig::default(),
        );
        let orchestrator = LlmOrchestrator::new(vec![hook]);
        let candidates = vec![
            LlmInvocation {
                plan_id: "a".into(),
                score: 1.0,
                rationale: "r1".into(),
                tags: vec![],
                kind: "music".into(),
            },
            LlmInvocation {
                plan_id: "b".into(),
                score: 0.5,
                rationale: "r2".into(),
                tags: vec![],
                kind: "talk".into(),
            },
        ];
        let result = orchestrator.rerank_candidates(candidates).await.unwrap();
        assert_eq!(result.mode, LlmResultMode::Apply);
        assert_eq!(result.order, vec!["b", "a"]);
    }
}
