use std::fs;
use std::path::Path;

use chrono::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result alias for business logic operations.
pub type Result<T> = std::result::Result<T, BusinessLogicError>;

/// Errors that can occur while loading or validating the business logic card.
#[derive(Debug, Error)]
pub enum BusinessLogicError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid business logic: {0}")]
    Invalid(String),
}

/// Describes the deterministic knobs defined by the owner card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessLogic {
    pub policy_version: String,
    pub env: String,
    pub knobs: Knobs,
    #[serde(default)]
    pub scheduling: Scheduling,
    pub selection: Selection,
    #[serde(default)]
    pub exploration: Exploration,
    #[serde(default)]
    pub autopilot: Autopilot,
    #[serde(default)]
    pub kpis: Kpis,
}

impl BusinessLogic {
    /// Load the business logic card from a YAML file and validates it.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut logic: Self = serde_yaml::from_str(&content)?;
        logic.validate()?;
        Ok(logic)
    }

    /// Validate invariants required by the planner.
    pub fn validate(&mut self) -> Result<()> {
        if !(self.knobs.plan_selection_bias >= -0.20 && self.knobs.plan_selection_bias <= 0.20) {
            return Err(BusinessLogicError::Invalid(
                "plan_selection_bias must be within [-0.20, 0.20]".into(),
            ));
        }
        if self.selection.temperature <= 0.0 {
            return Err(BusinessLogicError::Invalid(
                "selection.temperature must be > 0".into(),
            ));
        }
        if self.selection.top_k == Some(0) {
            return Err(BusinessLogicError::Invalid(
                "selection.top_k must be greater than zero".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.exploration.epsilon) {
            return Err(BusinessLogicError::Invalid(
                "exploration.epsilon must be within [0, 1]".into(),
            ));
        }
        if let Some(max_variation) = self.autopilot.max_daily_variation {
            if max_variation < 0.0 {
                return Err(BusinessLogicError::Invalid(
                    "autopilot.max_daily_variation must be >= 0".into(),
                ));
            }
        }
        if self.scheduling.slot_duration_minutes == 0 {
            return Err(BusinessLogicError::Invalid(
                "scheduling.slot_duration_minutes must be >= 1".into(),
            ));
        }
        Ok(())
    }

    /// Returns the configured selection temperature.
    pub fn selection_temperature(&self) -> f64 {
        self.selection.temperature
    }

    /// Returns the configured selection method.
    pub fn selection_method(&self) -> SelectionMethod {
        self.selection.method
    }

    /// Returns the number of items to sample; falls back to the planner batch size when missing.
    pub fn selection_top_k(&self, default: usize) -> usize {
        self.selection.top_k.unwrap_or(default).max(1)
    }

    /// Returns the global selection bias applied to all scores prior to selection.
    pub fn plan_selection_bias(&self) -> f64 {
        self.knobs.plan_selection_bias
    }

    /// Returns the configured seed strategy.
    pub fn seed_strategy(&self) -> SeedStrategy {
        self.selection.seed_strategy
    }

    /// Returns the slot duration as a chrono `Duration`.
    pub fn slot_duration(&self) -> Duration {
        Duration::minutes(self.scheduling.slot_duration_minutes as i64)
    }

    /// Global seed combined with the slot hash.
    pub fn global_seed(&self) -> u64 {
        self.scheduling.global_seed.unwrap_or(42)
    }

    /// Optional window identifier persisted in the database.
    pub fn window_id(&self) -> u64 {
        self.scheduling.window_id.unwrap_or(0)
    }

    /// Whether curator applies are globally locked.
    pub fn curator_locked(&self) -> bool {
        self.scheduling.lock_curator_applies
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knobs {
    pub boost_bucket: String,
    pub music_mood_focus: Vec<String>,
    pub interstitials_ratio: f64,
    pub plan_selection_bias: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scheduling {
    #[serde(default = "Scheduling::default_slot_duration")]
    pub slot_duration_minutes: u32,
    #[serde(default)]
    pub window_id: Option<u64>,
    #[serde(default)]
    pub global_seed: Option<u64>,
    #[serde(default)]
    pub lock_curator_applies: bool,
}

impl Scheduling {
    fn default_slot_duration() -> u32 {
        15
    }
}

impl Default for Scheduling {
    fn default() -> Self {
        Self {
            slot_duration_minutes: Self::default_slot_duration(),
            window_id: None,
            global_seed: None,
            lock_curator_applies: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub method: SelectionMethod,
    #[serde(default = "Selection::default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub seed_strategy: SeedStrategy,
}

impl Selection {
    const fn default_temperature() -> f64 {
        0.85
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Exploration {
    #[serde(default = "Exploration::default_epsilon")]
    pub epsilon: f64,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

impl Exploration {
    const fn default_epsilon() -> f64 {
        0.1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Autopilot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub max_daily_variation: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Kpis {
    #[serde(default)]
    pub primary: Vec<String>,
    #[serde(default)]
    pub secondary: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMethod {
    GumbelTopK,
    Softmax,
    Greedy,
    EpsilonGreedy,
}

impl Default for SelectionMethod {
    fn default() -> Self {
        SelectionMethod::GumbelTopK
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedStrategy {
    SlotHash,
    Global,
    Window,
}

impl Default for SeedStrategy {
    fn default() -> Self {
        SeedStrategy::SlotHash
    }
}
