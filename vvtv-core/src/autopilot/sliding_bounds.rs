use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for sliding bounds operations
pub type SlidingBoundsResult<T> = std::result::Result<T, SlidingBoundsError>;

/// Errors that can occur during sliding bounds operations
#[derive(Debug, Error)]
pub enum SlidingBoundsError {
    #[error("parameter not found: {0}")]
    ParameterNotFound(String),
    #[error("invalid bounds: {0}")]
    InvalidBounds(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Dynamic parameter bounds that adapt based on performance
#[derive(Debug, Clone)]
pub struct SlidingBounds {
    bounds: HashMap<String, ParameterBounds>,
    history: VecDeque<BoundsAdjustment>,
    config: SlidingBoundsConfig,
    config_path: Option<PathBuf>,
    bounds_path: Option<PathBuf>,
}

/// Serializable state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingBoundsState {
    pub bounds: HashMap<String, ParameterBounds>,
    pub history: Vec<BoundsAdjustment>,
    pub config: SlidingBoundsConfig,
    pub last_saved: DateTime<Utc>,
}

/// Bounds for a specific parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterBounds {
    pub parameter_name: String,
    pub min_value: f64,
    pub max_value: f64,
    pub current_value: f64,
    pub stability_days: u32,
    pub rollback_count: u32,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub total_adjustments: u32,
    pub last_expansion: Option<DateTime<Utc>>,
    pub last_contraction: Option<DateTime<Utc>>,
    pub performance_score: f64, // 0.0 to 1.0, higher is better
    pub change_history: VecDeque<ParameterChange>,
}

/// Record of parameter value changes for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChange {
    pub timestamp: DateTime<Utc>,
    pub old_value: f64,
    pub new_value: f64,
    pub change_reason: String,
    pub success: Option<bool>, // None = pending, Some(true/false) = measured result
}

/// Statistics for a parameter's performance and behavior
#[derive(Debug, Clone, Serialize)]
pub struct ParameterStats {
    pub parameter_name: String,
    pub current_bounds: (f64, f64),
    pub current_value: f64,
    pub stability_days: u32,
    pub rollback_count: u32,
    pub total_adjustments: u32,
    pub performance_score: f64,
    pub success_rate: f64,
    pub avg_change_magnitude: f64,
    pub total_changes: usize,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Analysis result for bounds expansion eligibility
#[derive(Debug, Clone)]
struct ExpansionAnalysis {
    pub should_expand: bool,
    pub stability_score: f64,
    pub performance_trend: f64,
    pub confidence: f64,
    pub stability_period: u32,
    pub reason: String,
}

/// Result of expansion calculation
#[derive(Debug, Clone)]
struct ExpansionResult {
    pub new_min: f64,
    pub new_max: f64,
    pub effective_rate: f64,
    pub strategy: ExpansionStrategy,
}

/// Expansion strategies for different parameter types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpansionStrategy {
    /// Expand symmetrically around the center point
    Symmetric,
    /// Expand more toward the current value's side
    BiasedToCurrent,
    /// Use smaller, more conservative expansion
    Conservative,
}

/// Recommendation for parameter bounds expansion
#[derive(Debug, Clone, Serialize)]
pub struct ExpansionRecommendation {
    pub parameter_name: String,
    pub current_bounds: (f64, f64),
    pub recommended_bounds: (f64, f64),
    pub expansion_rate: f64,
    pub confidence: f64,
    pub reason: String,
    pub strategy: ExpansionStrategy,
    pub should_expand: bool,
}

/// Severity levels for parameter failures
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureSeverity {
    /// Minor issues, small impact
    Minor,
    /// Moderate issues, noticeable impact
    Moderate,
    /// Severe issues, significant impact
    Severe,
    /// Critical issues, system-threatening impact
    Critical,
}

/// Anti-windup expansion strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AntiWindupStrategy {
    /// Expand symmetrically around center
    CenterExpansion,
    /// Expand more toward current value
    BiasedExpansion,
    /// Conservative expansion with safety margins
    SafeExpansion,
}

/// Analysis of bounds oscillation patterns
#[derive(Debug, Clone, Serialize)]
pub struct OscillationAnalysis {
    pub is_oscillating: bool,
    pub oscillation_frequency: f64,
    pub oscillation_amplitude: f64,
    pub recommendation: OscillationRecommendation,
    pub confidence: f64,
}

/// Recommendations for handling oscillation
#[derive(Debug, Clone, Serialize)]
pub enum OscillationRecommendation {
    /// Continue normal operation
    Continue,
    /// Monitor for patterns
    Monitor,
    /// Reduce adjustment rates
    ReduceRate,
    /// Pause adjustments temporarily
    Pause,
}

/// Context information for parameter failures
#[derive(Debug, Clone)]
pub struct FailureContext {
    pub severity: FailureSeverity,
    pub impact_description: String,
}

/// Analysis result for contraction decisions
#[derive(Debug, Clone)]
pub struct ContractionAnalysis {
    pub should_contract: bool,
    pub recommended_rate: f64,
    pub impact_score: f64,
    pub failure_pattern: FailurePattern,
    pub confidence: f64,
    pub reason: String,
}

/// Patterns of parameter failures
#[derive(Debug, Clone, Serialize)]
pub enum FailurePattern {
    /// Single isolated failure
    Isolated,
    /// Occasional sporadic failures
    Sporadic,
    /// Failures clustered in time
    Clustered,
    /// Regular recurring failures
    Recurring,
    /// Persistent chronic failures
    Chronic,
}

/// Recommendation for parameter bounds contraction
#[derive(Debug, Clone, Serialize)]
pub struct ContractionRecommendation {
    pub parameter_name: String,
    pub current_bounds: (f64, f64),
    pub should_contract: bool,
    pub recommended_rate: f64,
    pub confidence: f64,
    pub reason: String,
    pub rollback_count: u32,
    pub performance_score: f64,
}

/// Comprehensive validation result for parameter changes
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<ConstraintViolation>,
    pub warnings: Vec<ConstraintWarning>,
    pub adjusted_value: Option<f64>,
    pub confidence: f64,
}

/// Validation result for multiple parameter changes
#[derive(Debug, Clone, Serialize)]
pub struct SetValidationResult {
    pub is_valid: bool,
    pub individual_results: Vec<(String, ValidationResult)>,
    pub set_violations: Vec<ConstraintViolation>,
    pub set_warnings: Vec<ConstraintWarning>,
    pub confidence: f64,
}

/// Constraint violation details
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintViolation {
    pub constraint_type: ConstraintType,
    pub severity: ViolationSeverity,
    pub message: String,
    pub suggested_fix: Option<String>,
}

/// Constraint warning details
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintWarning {
    pub warning_type: WarningType,
    pub message: String,
    pub recommendation: String,
}

/// Types of constraints that can be violated
#[derive(Debug, Clone, Serialize)]
pub enum ConstraintType {
    SlidingBounds,
    AbsoluteBounds,
    BusinessLogic,
    Safety,
    RateLimit,
    Performance,
}

/// Severity levels for constraint violations
#[derive(Debug, Clone, Serialize)]
pub enum ViolationSeverity {
    Warning,
    Error,
    Critical,
}

/// Types of warnings
#[derive(Debug, Clone, Serialize)]
pub enum WarningType {
    Performance,
    Quality,
    Configuration,
    RateLimit,
}

/// Configuration for sliding bounds behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingBoundsConfig {
    pub expansion_rate_per_week: f64, // 0.05 = 5%
    pub contraction_rate_after_rollback: f64, // 0.25 = 25%
    pub stability_threshold_days: u32, // 7 days
    pub rollback_threshold: u32, // 3 rollbacks
    pub anti_windup_enabled: bool,
}

impl Default for SlidingBoundsConfig {
    fn default() -> Self {
        Self {
            expansion_rate_per_week: 0.05,
            contraction_rate_after_rollback: 0.25,
            stability_threshold_days: 7,
            rollback_threshold: 3,
            anti_windup_enabled: true,
        }
    }
}

/// Record of bounds adjustments for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BoundsAdjustment {
    Expansion { 
        parameter: String, 
        old_range: (f64, f64), 
        new_range: (f64, f64),
        expansion_rate: f64,
        stability_days: u32,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    Contraction { 
        parameter: String, 
        old_range: (f64, f64), 
        new_range: (f64, f64),
        contraction_rate: f64,
        rollback_count: u32,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    AntiWindup { 
        parameter: String, 
        old_range: (f64, f64),
        new_range: (f64, f64),
        reason: String,
        timestamp: DateTime<Utc>,
    },
    Reset {
        parameter: String,
        old_range: (f64, f64),
        new_range: (f64, f64),
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

/// Comprehensive bounds adjustment report for analysis and debugging
#[derive(Debug, Clone, Serialize)]
pub struct BoundsAdjustmentReport {
    pub report_timestamp: DateTime<Utc>,
    pub report_period: (DateTime<Utc>, DateTime<Utc>),
    pub total_adjustments: usize,
    pub adjustments_by_type: HashMap<String, usize>,
    pub adjustments_by_parameter: HashMap<String, usize>,
    pub parameter_summaries: Vec<ParameterAdjustmentSummary>,
    pub stability_analysis: StabilityAnalysis,
    pub recommendations: Vec<BoundsRecommendation>,
}

/// Summary of adjustments for a specific parameter
#[derive(Debug, Clone, Serialize)]
pub struct ParameterAdjustmentSummary {
    pub parameter_name: String,
    pub total_adjustments: usize,
    pub expansions: usize,
    pub contractions: usize,
    pub anti_windup_events: usize,
    pub resets: usize,
    pub current_bounds: (f64, f64),
    pub bounds_range_history: Vec<f64>, // Range sizes over time
    pub stability_score: f64, // 0.0 to 1.0, higher is more stable
    pub last_adjustment: Option<DateTime<Utc>>,
    pub adjustment_frequency: f64, // Adjustments per week
}

/// Analysis of overall bounds stability across all parameters
#[derive(Debug, Clone, Serialize)]
pub struct StabilityAnalysis {
    pub overall_stability_score: f64,
    pub most_stable_parameters: Vec<String>,
    pub least_stable_parameters: Vec<String>,
    pub oscillating_parameters: Vec<String>,
    pub parameters_needing_attention: Vec<String>,
    pub stability_trend: StabilityTrend,
}

/// Trend analysis for bounds stability
#[derive(Debug, Clone, Serialize)]
pub enum StabilityTrend {
    Improving,
    Stable,
    Degrading,
    Oscillating,
}

/// Recommendations for bounds management
#[derive(Debug, Clone, Serialize)]
pub struct BoundsRecommendation {
    pub recommendation_type: RecommendationType,
    pub parameter_name: String,
    pub priority: RecommendationPriority,
    pub description: String,
    pub suggested_action: String,
    pub confidence: f64,
}

/// Types of bounds recommendations
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationType {
    ExpandBounds,
    ContractBounds,
    ResetBounds,
    IncreaseStabilityPeriod,
    DecreaseExpansionRate,
    EnableAntiWindup,
    InvestigateOscillation,
}

/// Priority levels for recommendations
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// Historical bounds adjustment data for export and analysis
#[derive(Debug, Clone, Serialize)]
pub struct BoundsAdjustmentHistory {
    pub export_timestamp: DateTime<Utc>,
    pub adjustments: Vec<BoundsAdjustment>,
    pub parameter_metadata: HashMap<String, ParameterMetadata>,
    pub configuration_history: Vec<ConfigurationChange>,
}

/// Metadata about parameter bounds configuration
#[derive(Debug, Clone, Serialize)]
pub struct ParameterMetadata {
    pub parameter_name: String,
    pub initial_bounds: (f64, f64),
    pub absolute_min: f64,
    pub absolute_max: f64,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub total_lifetime_adjustments: usize,
}

/// Record of configuration changes that affect bounds behavior
#[derive(Debug, Clone, Serialize)]
pub struct ConfigurationChange {
    pub timestamp: DateTime<Utc>,
    pub change_type: ConfigurationChangeType,
    pub old_value: String,
    pub new_value: String,
    pub reason: String,
}

/// Types of configuration changes
#[derive(Debug, Clone, Serialize)]
pub enum ConfigurationChangeType {
    ExpansionRate,
    ContractionRate,
    StabilityThreshold,
    AntiWindupEnabled,
    MaxHistorySize,
    Other(String),
}

impl SlidingBounds {
    /// Create new sliding bounds with default parameter bounds
    pub fn new(config: SlidingBoundsConfig) -> Self {
        let mut bounds = HashMap::new();
        let now = Utc::now();
        
        // Initialize default bounds for known parameters
        bounds.insert("selection_temperature".to_string(), ParameterBounds {
            parameter_name: "selection_temperature".to_string(),
            min_value: 0.1,
            max_value: 2.0,
            current_value: 0.85,
            stability_days: 0,
            rollback_count: 0,
            last_updated: now,
            created_at: now,
            total_adjustments: 0,
            last_expansion: None,
            last_contraction: None,
            performance_score: 0.5,
            change_history: VecDeque::new(),
        });
        
        bounds.insert("selection_top_k".to_string(), ParameterBounds {
            parameter_name: "selection_top_k".to_string(),
            min_value: 1.0,
            max_value: 50.0,
            current_value: 12.0,
            stability_days: 0,
            rollback_count: 0,
            last_updated: now,
            created_at: now,
            total_adjustments: 0,
            last_expansion: None,
            last_contraction: None,
            performance_score: 0.5,
            change_history: VecDeque::new(),
        });
        
        bounds.insert("plan_selection_bias".to_string(), ParameterBounds {
            parameter_name: "plan_selection_bias".to_string(),
            min_value: -0.20,
            max_value: 0.20,
            current_value: 0.0,
            stability_days: 0,
            rollback_count: 0,
            last_updated: now,
            created_at: now,
            total_adjustments: 0,
            last_expansion: None,
            last_contraction: None,
            performance_score: 0.5,
            change_history: VecDeque::new(),
        });

        bounds.insert("curator_confidence_threshold".to_string(), ParameterBounds {
            parameter_name: "curator_confidence_threshold".to_string(),
            min_value: 0.3,
            max_value: 0.9,
            current_value: 0.62,
            stability_days: 0,
            rollback_count: 0,
            last_updated: now,
            created_at: now,
            total_adjustments: 0,
            last_expansion: None,
            last_contraction: None,
            performance_score: 0.5,
            change_history: VecDeque::new(),
        });

        Self {
            bounds,
            history: VecDeque::new(),
            config,
            config_path: None,
            bounds_path: None,
        }
    }

    /// Create sliding bounds with persistence paths
    pub fn with_persistence(
        config: SlidingBoundsConfig,
        config_path: PathBuf,
        bounds_path: PathBuf,
    ) -> SlidingBoundsResult<Self> {
        let mut sliding_bounds = Self::new(config);
        sliding_bounds.config_path = Some(config_path);
        sliding_bounds.bounds_path = Some(bounds_path);
        
        // Try to load existing bounds state
        if let Err(e) = sliding_bounds.load_bounds_state() {
            tracing::warn!(
                target: "sliding_bounds",
                error = %e,
                "failed to load existing bounds state, using defaults"
            );
        }
        
        Ok(sliding_bounds)
    }

    /// Load bounds state from file
    pub fn load_bounds_state(&mut self) -> SlidingBoundsResult<()> {
        if let Some(ref bounds_path) = self.bounds_path {
            if bounds_path.exists() {
                let content = fs::read_to_string(bounds_path)?;
                let state: SlidingBoundsState = serde_json::from_str(&content)?;
                
                self.bounds = state.bounds;
                self.history = state.history.into();
                self.config = state.config;
                
                tracing::info!(
                    target: "sliding_bounds",
                    bounds_count = self.bounds.len(),
                    history_count = self.history.len(),
                    "loaded bounds state from file"
                );
            }
        }
        Ok(())
    }

    /// Save bounds state to file
    pub fn save_bounds_state(&self) -> SlidingBoundsResult<()> {
        if let Some(ref bounds_path) = self.bounds_path {
            let state = SlidingBoundsState {
                bounds: self.bounds.clone(),
                history: self.history.iter().cloned().collect(),
                config: self.config.clone(),
                last_saved: Utc::now(),
            };
            
            let content = serde_json::to_string_pretty(&state)?;
            
            // Ensure parent directory exists
            if let Some(parent) = bounds_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            // Atomic write
            let temp_path = bounds_path.with_extension("tmp");
            fs::write(&temp_path, content)?;
            fs::rename(&temp_path, bounds_path)?;
            
            tracing::debug!(
                target: "sliding_bounds",
                path = %bounds_path.display(),
                "saved bounds state to file"
            );
        }
        Ok(())
    }

    /// Load configuration from file
    pub fn load_config_from_file<P: AsRef<Path>>(path: P) -> SlidingBoundsResult<SlidingBoundsConfig> {
        let content = fs::read_to_string(path)?;
        let config: SlidingBoundsConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_config_to_file<P: AsRef<Path>>(&self, path: P) -> SlidingBoundsResult<()> {
        let content = serde_yaml::to_string(&self.config)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Validate if a parameter change is within bounds
    pub fn validate_change(&self, parameter: &str, new_value: f64) -> SlidingBoundsResult<bool> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        if new_value < bounds.min_value || new_value > bounds.max_value {
            return Err(SlidingBoundsError::InvalidBounds(format!(
                "Parameter {} value {} is outside bounds [{}, {}]",
                parameter, new_value, bounds.min_value, bounds.max_value
            )));
        }

        // Additional validation for specific parameters
        match parameter {
            "selection_temperature" => {
                if new_value < 0.1 || new_value > 2.0 {
                    return Err(SlidingBoundsError::InvalidBounds(
                        "Temperature must be between 0.1 and 2.0".to_string()
                    ));
                }
            }
            "plan_selection_bias" => {
                let daily_change = (new_value - bounds.current_value).abs();
                if daily_change > 0.05 {
                    return Err(SlidingBoundsError::InvalidBounds(
                        "Daily bias change cannot exceed 0.05".to_string()
                    ));
                }
            }
            "curator_confidence_threshold" => {
                if new_value < 0.0 || new_value > 1.0 {
                    return Err(SlidingBoundsError::InvalidBounds(
                        "Confidence threshold must be between 0.0 and 1.0".to_string()
                    ));
                }
            }
            _ => {}
        }

        Ok(true)
    }

    /// Expand bounds for parameters that have been stable
    pub fn expand_bounds_for_stable_parameters(&mut self) -> Vec<BoundsAdjustment> {
        let mut adjustments = Vec::new();
        let now = Utc::now();

        for (param_name, bounds) in self.bounds.iter_mut() {
            if bounds.stability_days >= self.config.stability_threshold_days {
                let old_range = (bounds.min_value, bounds.max_value);
                
                // Calculate expansion rate based on performance score
                let performance_multiplier = 0.5 + bounds.performance_score * 0.5; // 0.5 to 1.0
                let adjusted_expansion_rate = self.config.expansion_rate_per_week * performance_multiplier;
                let expansion_factor = 1.0 + adjusted_expansion_rate;
                
                // Expand bounds symmetrically around current value
                let current_range = bounds.max_value - bounds.min_value;
                let new_range = current_range * expansion_factor;
                let center = (bounds.min_value + bounds.max_value) / 2.0;
                
                bounds.min_value = center - new_range / 2.0;
                bounds.max_value = center + new_range / 2.0;
                bounds.last_updated = now;
                bounds.last_expansion = Some(now);
                bounds.stability_days = 0; // Reset stability counter
                bounds.total_adjustments += 1;
                
                let adjustment = BoundsAdjustment::Expansion {
                    parameter: param_name.clone(),
                    old_range,
                    new_range: (bounds.min_value, bounds.max_value),
                    expansion_rate: adjusted_expansion_rate,
                    stability_days: self.config.stability_threshold_days,
                    reason: format!("Stable for {} days, performance score: {:.2}", 
                                  self.config.stability_threshold_days, bounds.performance_score),
                    timestamp: now,
                };
                
                adjustments.push(adjustment.clone());
                self.history.push_back(adjustment);

                tracing::info!(
                    target: "sliding_bounds",
                    parameter = %param_name,
                    old_range = ?old_range,
                    new_range = ?(bounds.min_value, bounds.max_value),
                    expansion_rate = adjusted_expansion_rate,
                    performance_score = bounds.performance_score,
                    "expanded parameter bounds"
                );
            }
        }

        // Limit history size
        while self.history.len() > 1000 {
            self.history.pop_front();
        }

        // Auto-save if persistence is enabled
        if !adjustments.is_empty() {
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after expansion"
                );
            }
        }

        adjustments
    }

    /// Advanced bounds expansion with sophisticated stability analysis
    pub fn expand_bounds_with_analysis(&mut self) -> SlidingBoundsResult<Vec<BoundsAdjustment>> {
        let mut adjustments = Vec::new();
        let now = Utc::now();

        // First, collect all expansion decisions without borrowing mutably
        let mut expansion_decisions = Vec::new();
        for (param_name, bounds) in &self.bounds {
            let expansion_analysis = self.analyze_expansion_eligibility(bounds)?;
            
            if expansion_analysis.should_expand {
                let expansion_result = self.calculate_optimal_expansion(bounds, &expansion_analysis)?;
                expansion_decisions.push((param_name.clone(), expansion_analysis, expansion_result));
            }
        }

        // Now apply the expansions
        for (param_name, expansion_analysis, expansion_result) in expansion_decisions {
            if let Some(bounds) = self.bounds.get_mut(&param_name) {
                let old_range = (bounds.min_value, bounds.max_value);
                
                bounds.min_value = expansion_result.new_min;
                bounds.max_value = expansion_result.new_max;
                bounds.last_updated = now;
                bounds.last_expansion = Some(now);
                bounds.stability_days = 0; // Reset stability counter
                bounds.total_adjustments += 1;
                
                let adjustment = BoundsAdjustment::Expansion {
                    parameter: param_name.clone(),
                    old_range,
                    new_range: (bounds.min_value, bounds.max_value),
                    expansion_rate: expansion_result.effective_rate,
                    stability_days: expansion_analysis.stability_period,
                    reason: expansion_analysis.reason.clone(),
                    timestamp: now,
                };
                
                adjustments.push(adjustment.clone());
                self.history.push_back(adjustment);

                tracing::info!(
                    target: "sliding_bounds",
                    parameter = %param_name,
                    old_range = ?old_range,
                    new_range = ?(bounds.min_value, bounds.max_value),
                    expansion_rate = expansion_result.effective_rate,
                    confidence = expansion_analysis.confidence,
                    stability_score = expansion_analysis.stability_score,
                    "expanded parameter bounds with analysis"
                );
            }
        }

        // Limit history size
        while self.history.len() > 1000 {
            self.history.pop_front();
        }

        // Auto-save if persistence is enabled
        if !adjustments.is_empty() {
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after expansion analysis"
                );
            }
        }

        Ok(adjustments)
    }

    /// Analyze if a parameter is eligible for bounds expansion
    fn analyze_expansion_eligibility(&self, bounds: &ParameterBounds) -> SlidingBoundsResult<ExpansionAnalysis> {
        let stability_score = self.calculate_stability_score(bounds);
        let performance_trend = self.calculate_performance_trend(bounds);
        let confidence = self.calculate_expansion_confidence(bounds);
        
        // Check basic stability requirement
        let meets_stability_threshold = bounds.stability_days >= self.config.stability_threshold_days;
        
        // Check performance requirements
        let performance_acceptable = bounds.performance_score >= 0.6; // 60% success rate minimum
        
        // Check that we haven't expanded too recently
        let expansion_cooldown_ok = if let Some(last_expansion) = bounds.last_expansion {
            let days_since_expansion = (Utc::now() - last_expansion).num_days();
            days_since_expansion >= 3 // At least 3 days since last expansion
        } else {
            true // Never expanded before
        };
        
        // Check that parameter is actually being used (has recent changes)
        let has_recent_activity = !bounds.change_history.is_empty() && 
            bounds.change_history.iter().any(|change| {
                (Utc::now() - change.timestamp).num_days() <= 7
            });
        
        let should_expand = meets_stability_threshold && 
                           performance_acceptable && 
                           expansion_cooldown_ok && 
                           has_recent_activity &&
                           confidence >= 0.7;
        
        let reason = if should_expand {
            format!(
                "Stable for {} days, performance: {:.1}%, confidence: {:.1}%, trend: {:.2}",
                bounds.stability_days,
                bounds.performance_score * 100.0,
                confidence * 100.0,
                performance_trend
            )
        } else {
            let mut reasons = Vec::new();
            if !meets_stability_threshold {
                reasons.push(format!("insufficient stability ({}/{})", bounds.stability_days, self.config.stability_threshold_days));
            }
            if !performance_acceptable {
                reasons.push(format!("low performance ({:.1}%)", bounds.performance_score * 100.0));
            }
            if !expansion_cooldown_ok {
                reasons.push("recent expansion".to_string());
            }
            if !has_recent_activity {
                reasons.push("no recent activity".to_string());
            }
            if confidence < 0.7 {
                reasons.push(format!("low confidence ({:.1}%)", confidence * 100.0));
            }
            format!("expansion blocked: {}", reasons.join(", "))
        };

        Ok(ExpansionAnalysis {
            should_expand,
            stability_score,
            performance_trend,
            confidence,
            stability_period: bounds.stability_days,
            reason,
        })
    }

    /// Calculate stability score based on parameter behavior
    fn calculate_stability_score(&self, bounds: &ParameterBounds) -> f64 {
        if bounds.change_history.is_empty() {
            return 0.5; // Neutral score for no data
        }

        // Calculate variance in parameter changes
        let changes: Vec<f64> = bounds.change_history.iter()
            .map(|change| (change.new_value - change.old_value).abs())
            .collect();
        
        if changes.is_empty() {
            return 0.5;
        }

        let mean_change = changes.iter().sum::<f64>() / changes.len() as f64;
        let variance = changes.iter()
            .map(|change| (change - mean_change).powi(2))
            .sum::<f64>() / changes.len() as f64;
        
        let coefficient_of_variation = if mean_change > 0.0 {
            variance.sqrt() / mean_change
        } else {
            0.0
        };
        
        // Lower CV = higher stability
        let stability_from_variance = 1.0 / (1.0 + coefficient_of_variation);
        
        // Factor in success rate
        let successful_changes = bounds.change_history.iter()
            .filter(|change| change.success == Some(true))
            .count();
        let total_measured = bounds.change_history.iter()
            .filter(|change| change.success.is_some())
            .count();
        
        let success_rate = if total_measured > 0 {
            successful_changes as f64 / total_measured as f64
        } else {
            0.5
        };
        
        // Combine variance-based stability with success rate
        (stability_from_variance * 0.6 + success_rate * 0.4).clamp(0.0, 1.0)
    }

    /// Calculate performance trend over recent changes
    fn calculate_performance_trend(&self, bounds: &ParameterBounds) -> f64 {
        let recent_changes: Vec<&ParameterChange> = bounds.change_history.iter()
            .rev() // Most recent first
            .take(10) // Last 10 changes
            .collect();
        
        if recent_changes.len() < 3 {
            return 0.0; // Not enough data for trend
        }

        // Calculate trend in success rate over time
        let mut trend_sum = 0.0;
        let mut trend_count = 0;
        
        for window in recent_changes.windows(3) {
            let older_success = window[2].success.map(|s| if s { 1.0 } else { 0.0 }).unwrap_or(0.5);
            let newer_success = window[0].success.map(|s| if s { 1.0 } else { 0.0 }).unwrap_or(0.5);
            
            trend_sum += newer_success - older_success;
            trend_count += 1;
        }
        
        if trend_count > 0 {
            trend_sum / trend_count as f64
        } else {
            0.0
        }
    }

    /// Calculate confidence in expansion decision
    fn calculate_expansion_confidence(&self, bounds: &ParameterBounds) -> f64 {
        let mut confidence_factors = Vec::new();
        
        // Factor 1: Amount of historical data
        let data_factor = (bounds.change_history.len() as f64 / 20.0).min(1.0); // Max confidence at 20+ changes
        confidence_factors.push(data_factor * 0.3);
        
        // Factor 2: Consistency of performance
        let performance_consistency = if bounds.change_history.len() >= 5 {
            let recent_performance: Vec<f64> = bounds.change_history.iter()
                .rev()
                .take(5)
                .filter_map(|change| change.success.map(|s| if s { 1.0 } else { 0.0 }))
                .collect();
            
            if !recent_performance.is_empty() {
                let mean = recent_performance.iter().sum::<f64>() / recent_performance.len() as f64;
                let variance = recent_performance.iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f64>() / recent_performance.len() as f64;
                1.0 - variance // Lower variance = higher consistency
            } else {
                0.5
            }
        } else {
            0.5
        };
        confidence_factors.push(performance_consistency * 0.4);
        
        // Factor 3: Time since last rollback
        let rollback_factor = if bounds.rollback_count == 0 {
            1.0
        } else {
            // Reduce confidence based on recent rollbacks
            (1.0 - (bounds.rollback_count as f64 * 0.2)).max(0.2)
        };
        confidence_factors.push(rollback_factor * 0.3);
        
        confidence_factors.iter().sum::<f64>().clamp(0.0, 1.0)
    }

    /// Calculate optimal expansion parameters
    fn calculate_optimal_expansion(&self, bounds: &ParameterBounds, analysis: &ExpansionAnalysis) -> SlidingBoundsResult<ExpansionResult> {
        // Base expansion rate from configuration
        let base_rate = self.config.expansion_rate_per_week;
        
        // Adjust rate based on performance and confidence
        let performance_multiplier = 0.5 + bounds.performance_score * 0.5; // 0.5 to 1.0
        let confidence_multiplier = 0.3 + analysis.confidence * 0.7; // 0.3 to 1.0
        let stability_multiplier = 0.4 + analysis.stability_score * 0.6; // 0.4 to 1.0
        
        let effective_rate = base_rate * performance_multiplier * confidence_multiplier * stability_multiplier;
        
        // Calculate expansion based on parameter type and current usage
        let expansion_strategy = self.determine_expansion_strategy(&bounds.parameter_name, bounds);
        
        let (new_min, new_max) = match expansion_strategy {
            ExpansionStrategy::Symmetric => {
                // Expand symmetrically around current center
                let current_range = bounds.max_value - bounds.min_value;
                let new_range = current_range * (1.0 + effective_rate);
                let center = (bounds.min_value + bounds.max_value) / 2.0;
                (center - new_range / 2.0, center + new_range / 2.0)
            }
            ExpansionStrategy::BiasedToCurrent => {
                // Expand more in the direction of current value
                let current_range = bounds.max_value - bounds.min_value;
                let expansion_amount = current_range * effective_rate;
                
                // Determine bias based on where current value sits in range
                let position_ratio = (bounds.current_value - bounds.min_value) / current_range;
                let upper_expansion = expansion_amount * position_ratio;
                let lower_expansion = expansion_amount * (1.0 - position_ratio);
                
                (bounds.min_value - lower_expansion, bounds.max_value + upper_expansion)
            }
            ExpansionStrategy::Conservative => {
                // Smaller, more conservative expansion
                let conservative_rate = effective_rate * 0.5;
                let current_range = bounds.max_value - bounds.min_value;
                let new_range = current_range * (1.0 + conservative_rate);
                let center = (bounds.min_value + bounds.max_value) / 2.0;
                (center - new_range / 2.0, center + new_range / 2.0)
            }
        };
        
        Ok(ExpansionResult {
            new_min,
            new_max,
            effective_rate,
            strategy: expansion_strategy,
        })
    }

    /// Determine the best expansion strategy for a parameter
    fn determine_expansion_strategy(&self, parameter_name: &str, bounds: &ParameterBounds) -> ExpansionStrategy {
        match parameter_name {
            "selection_temperature" => {
                // Temperature benefits from symmetric expansion
                ExpansionStrategy::Symmetric
            }
            "selection_top_k" => {
                // Top-k should expand conservatively as it affects performance significantly
                ExpansionStrategy::Conservative
            }
            "plan_selection_bias" => {
                // Bias should expand toward current usage patterns
                ExpansionStrategy::BiasedToCurrent
            }
            "curator_confidence_threshold" => {
                // Confidence threshold should expand conservatively
                ExpansionStrategy::Conservative
            }
            _ => {
                // Default to conservative for unknown parameters
                if bounds.performance_score > 0.8 {
                    ExpansionStrategy::Symmetric
                } else {
                    ExpansionStrategy::Conservative
                }
            }
        }
    }

    /// Get expansion recommendations without applying them
    pub fn get_expansion_recommendations(&self) -> SlidingBoundsResult<Vec<ExpansionRecommendation>> {
        let mut recommendations = Vec::new();
        
        for (param_name, bounds) in &self.bounds {
            let analysis = self.analyze_expansion_eligibility(bounds)?;
            
            let recommendation = if analysis.should_expand {
                let expansion_result = self.calculate_optimal_expansion(bounds, &analysis)?;
                ExpansionRecommendation {
                    parameter_name: param_name.clone(),
                    current_bounds: (bounds.min_value, bounds.max_value),
                    recommended_bounds: (expansion_result.new_min, expansion_result.new_max),
                    expansion_rate: expansion_result.effective_rate,
                    confidence: analysis.confidence,
                    reason: analysis.reason,
                    strategy: expansion_result.strategy,
                    should_expand: true,
                }
            } else {
                ExpansionRecommendation {
                    parameter_name: param_name.clone(),
                    current_bounds: (bounds.min_value, bounds.max_value),
                    recommended_bounds: (bounds.min_value, bounds.max_value),
                    expansion_rate: 0.0,
                    confidence: analysis.confidence,
                    reason: analysis.reason,
                    strategy: ExpansionStrategy::Conservative,
                    should_expand: false,
                }
            };
            
            recommendations.push(recommendation);
        }
        
        Ok(recommendations)
    }

    /// Perform gradual expansion over multiple cycles
    pub fn gradual_expansion_cycle(&mut self, max_parameters_per_cycle: usize) -> SlidingBoundsResult<Vec<BoundsAdjustment>> {
        let recommendations = self.get_expansion_recommendations()?;
        
        // Sort by confidence and select top candidates
        let mut candidates: Vec<_> = recommendations.into_iter()
            .filter(|rec| rec.should_expand)
            .collect();
        
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(max_parameters_per_cycle);
        
        let mut adjustments = Vec::new();
        let now = Utc::now();
        
        for candidate in candidates {
            if let Some(bounds) = self.bounds.get_mut(&candidate.parameter_name) {
                let old_range = (bounds.min_value, bounds.max_value);
                
                bounds.min_value = candidate.recommended_bounds.0;
                bounds.max_value = candidate.recommended_bounds.1;
                bounds.last_updated = now;
                bounds.last_expansion = Some(now);
                bounds.stability_days = 0;
                bounds.total_adjustments += 1;
                
                let adjustment = BoundsAdjustment::Expansion {
                    parameter: candidate.parameter_name.clone(),
                    old_range,
                    new_range: candidate.recommended_bounds,
                    expansion_rate: candidate.expansion_rate,
                    stability_days: bounds.stability_days,
                    reason: format!("Gradual expansion: {}", candidate.reason),
                    timestamp: now,
                };
                
                adjustments.push(adjustment.clone());
                self.history.push_back(adjustment);
                
                tracing::info!(
                    target: "sliding_bounds",
                    parameter = %candidate.parameter_name,
                    old_range = ?old_range,
                    new_range = ?candidate.recommended_bounds,
                    confidence = candidate.confidence,
                    strategy = ?candidate.strategy,
                    "performed gradual expansion"
                );
            }
        }
        
        // Auto-save if persistence is enabled
        if !adjustments.is_empty() {
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after gradual expansion"
                );
            }
        }
        
        Ok(adjustments)
    }

    /// Calculate expansion rate based on historical performance
    pub fn calculate_adaptive_expansion_rate(&self, parameter: &str) -> SlidingBoundsResult<f64> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;
        
        let base_rate = self.config.expansion_rate_per_week;
        
        // Adjust based on historical success
        let success_multiplier = if bounds.change_history.len() >= 5 {
            let recent_successes = bounds.change_history.iter()
                .rev()
                .take(10)
                .filter(|change| change.success == Some(true))
                .count();
            let recent_total = bounds.change_history.iter()
                .rev()
                .take(10)
                .filter(|change| change.success.is_some())
                .count();
            
            if recent_total > 0 {
                let success_rate = recent_successes as f64 / recent_total as f64;
                0.5 + success_rate * 0.5 // 0.5 to 1.0 multiplier
            } else {
                0.75 // Default moderate multiplier
            }
        } else {
            0.75 // Conservative for new parameters
        };
        
        // Adjust based on stability
        let stability_multiplier = if bounds.stability_days >= self.config.stability_threshold_days * 2 {
            1.2 // Bonus for extra stability
        } else if bounds.stability_days >= self.config.stability_threshold_days {
            1.0 // Normal rate
        } else {
            0.5 // Reduced rate for less stable parameters
        };
        
        // Adjust based on recent rollbacks
        let rollback_penalty = if bounds.rollback_count > 0 {
            1.0 - (bounds.rollback_count as f64 * 0.1).min(0.5)
        } else {
            1.0
        };
        
        let adaptive_rate = base_rate * success_multiplier * stability_multiplier * rollback_penalty;
        
        Ok(adaptive_rate.clamp(0.01, 0.15)) // Limit between 1% and 15%
    }

    /// Get expansion history for a parameter
    pub fn get_expansion_history(&self, parameter: &str) -> Vec<&BoundsAdjustment> {
        self.history.iter()
            .filter(|adjustment| {
                match adjustment {
                    BoundsAdjustment::Expansion { parameter: param, .. } => param == parameter,
                    _ => false,
                }
            })
            .collect()
    }

    /// Calculate time since last expansion for a parameter
    pub fn time_since_last_expansion(&self, parameter: &str) -> SlidingBoundsResult<Option<chrono::Duration>> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;
        
        Ok(bounds.last_expansion.map(|last| Utc::now() - last))
    }

    /// Check if parameter is ready for expansion based on cooldown
    pub fn is_expansion_ready(&self, parameter: &str, cooldown_days: i64) -> SlidingBoundsResult<bool> {
        match self.time_since_last_expansion(parameter)? {
            Some(duration) => Ok(duration.num_days() >= cooldown_days),
            None => Ok(true), // Never expanded, so ready
        }
    }

    /// Advanced bounds contraction with exponential backoff
    pub fn contract_bounds_with_backoff(&mut self, parameter: &str, failure_severity: FailureSeverity) -> SlidingBoundsResult<BoundsAdjustment> {
        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        bounds.rollback_count += 1;
        let now = Utc::now();

        // Calculate contraction based on failure severity and rollback history
        let base_contraction_rate = match failure_severity {
            FailureSeverity::Minor => self.config.contraction_rate_after_rollback * 0.5,
            FailureSeverity::Moderate => self.config.contraction_rate_after_rollback,
            FailureSeverity::Severe => self.config.contraction_rate_after_rollback * 1.5,
            FailureSeverity::Critical => self.config.contraction_rate_after_rollback * 2.0,
        };

        // Apply exponential backoff based on consecutive rollbacks
        let backoff_multiplier = if bounds.rollback_count >= self.config.rollback_threshold {
            1.0 + (bounds.rollback_count - self.config.rollback_threshold) as f64 * 0.2
        } else {
            1.0
        };

        let effective_contraction_rate = (base_contraction_rate * backoff_multiplier).min(0.8); // Max 80% contraction

        if bounds.rollback_count >= self.config.rollback_threshold {
            let old_range = (bounds.min_value, bounds.max_value);
            
            // Apply contraction with performance consideration
            let performance_factor = 1.0 - bounds.performance_score * 0.3; // Reduce contraction for better performing params
            let final_contraction_rate = effective_contraction_rate * performance_factor;
            
            let contraction_factor = 1.0 - final_contraction_rate;
            let current_range = bounds.max_value - bounds.min_value;
            let new_range = current_range * contraction_factor;
            let center = (bounds.min_value + bounds.max_value) / 2.0;
            
            bounds.min_value = center - new_range / 2.0;
            bounds.max_value = center + new_range / 2.0;
            bounds.last_updated = now;
            bounds.last_contraction = Some(now);
            bounds.rollback_count = 0; // Reset rollback counter after contraction
            bounds.total_adjustments += 1;
            
            let adjustment = BoundsAdjustment::Contraction {
                parameter: parameter.to_string(),
                old_range,
                new_range: (bounds.min_value, bounds.max_value),
                contraction_rate: final_contraction_rate,
                rollback_count: self.config.rollback_threshold,
                reason: format!("Rollback threshold reached ({}), severity: {:?}, backoff: {:.2}x, performance: {:.2}", 
                              self.config.rollback_threshold, failure_severity, backoff_multiplier, bounds.performance_score),
                timestamp: now,
            };
            
            self.history.push_back(adjustment.clone());

            tracing::warn!(
                target: "sliding_bounds",
                parameter = %parameter,
                old_range = ?old_range,
                new_range = ?(bounds.min_value, bounds.max_value),
                contraction_rate = final_contraction_rate,
                severity = ?failure_severity,
                backoff_multiplier = backoff_multiplier,

                "contracted parameter bounds with exponential backoff"
            );

            // Auto-save if persistence is enabled
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after contraction"
                );
            }

            Ok(adjustment)
        } else {
            // Record rollback without contracting yet
            bounds.last_updated = now;
            let adjustment = BoundsAdjustment::AntiWindup {
                parameter: parameter.to_string(),
                old_range: (bounds.min_value, bounds.max_value),
                new_range: (bounds.min_value, bounds.max_value),
                reason: format!("Rollback recorded ({}/{}), severity: {:?}", 
                              bounds.rollback_count, self.config.rollback_threshold, failure_severity),
                timestamp: now,
            };

            tracing::debug!(
                target: "sliding_bounds",
                parameter = %parameter,
                rollback_count = bounds.rollback_count,
                threshold = self.config.rollback_threshold,
                severity = ?failure_severity,
                "recorded rollback with severity, threshold not yet reached"
            );

            Ok(adjustment)
        }
    }

    /// Enhanced anti-windup protection with adaptive minimum bounds
    pub fn apply_enhanced_anti_windup_protection(&mut self, parameter: &str) -> SlidingBoundsResult<Option<BoundsAdjustment>> {
        if !self.config.anti_windup_enabled {
            return Ok(None);
        }

        // First, get immutable reference to calculate values
        let (adaptive_min_range, expansion_strategy, current_range, old_range) = {
            let bounds = self.bounds.get(parameter)
                .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;
            
            let adaptive_min_range = self.calculate_adaptive_minimum_range(parameter, bounds)?;
            let expansion_strategy = self.determine_anti_windup_strategy(parameter, bounds);
            let current_range = bounds.max_value - bounds.min_value;
            let old_range = (bounds.min_value, bounds.max_value);
            
            (adaptive_min_range, expansion_strategy, current_range, old_range)
        };

        if current_range < adaptive_min_range {
            // Now get mutable reference to apply changes
            let bounds = self.bounds.get_mut(parameter).unwrap();
            
            let (new_min, new_max) = match expansion_strategy {
                AntiWindupStrategy::CenterExpansion => {
                    let center = (bounds.min_value + bounds.max_value) / 2.0;
                    (center - adaptive_min_range / 2.0, center + adaptive_min_range / 2.0)
                }
                AntiWindupStrategy::BiasedExpansion => {
                    // Expand more toward the current value
                    let position_ratio = (bounds.current_value - bounds.min_value) / current_range;
                    let lower_expansion = adaptive_min_range * (1.0 - position_ratio);
                    let upper_expansion = adaptive_min_range * position_ratio;
                    (bounds.current_value - lower_expansion, bounds.current_value + upper_expansion)
                }
                AntiWindupStrategy::SafeExpansion => {
                    // Conservative expansion with safety margins
                    let safety_margin = adaptive_min_range * 0.1;
                    let center = (bounds.min_value + bounds.max_value) / 2.0;
                    (center - (adaptive_min_range + safety_margin) / 2.0, 
                     center + (adaptive_min_range + safety_margin) / 2.0)
                }
            };

            bounds.min_value = new_min;
            bounds.max_value = new_max;
            bounds.last_updated = Utc::now();
            bounds.total_adjustments += 1;

            let adjustment = BoundsAdjustment::AntiWindup {
                parameter: parameter.to_string(),
                old_range,
                new_range: (new_min, new_max),
                reason: format!("Enhanced anti-windup: range {:.4} < min {:.4}, strategy: {:?}", 
                              current_range, adaptive_min_range, expansion_strategy),
                timestamp: Utc::now(),
            };
            
            self.history.push_back(adjustment.clone());

            tracing::info!(
                target: "sliding_bounds",
                parameter = %parameter,
                old_range = ?old_range,
                new_range = ?(new_min, new_max),
                adaptive_min_range = adaptive_min_range,
                strategy = ?expansion_strategy,
                "applied enhanced anti-windup protection"
            );

            // Auto-save if persistence is enabled
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after anti-windup"
                );
            }

            Ok(Some(adjustment))
        } else {
            Ok(None)
        }
    }

    /// Calculate adaptive minimum range based on parameter characteristics
    fn calculate_adaptive_minimum_range(&self, parameter: &str, bounds: &ParameterBounds) -> SlidingBoundsResult<f64> {
        // Base minimum ranges for different parameter types
        let base_min_range = match parameter {
            "selection_temperature" => 0.1,
            "selection_top_k" => 2.0,
            "plan_selection_bias" => 0.02,
            "curator_confidence_threshold" => 0.05,
            _ => 0.1, // Default
        };

        // Adjust based on parameter usage patterns
        let usage_multiplier = if !bounds.change_history.is_empty() {
            let avg_change = bounds.change_history.iter()
                .map(|change| (change.new_value - change.old_value).abs())
                .sum::<f64>() / bounds.change_history.len() as f64;
            
            // Larger typical changes need larger minimum ranges
            1.0 + (avg_change / base_min_range).min(2.0)
        } else {
            1.0
        };

        // Adjust based on performance score
        let performance_multiplier = if bounds.performance_score > 0.8 {
            1.2 // Allow slightly larger ranges for well-performing parameters
        } else if bounds.performance_score < 0.4 {
            0.8 // Tighter ranges for poorly performing parameters
        } else {
            1.0
        };

        // Adjust based on rollback history
        let rollback_multiplier = if bounds.rollback_count > 0 {
            0.9 - (bounds.rollback_count as f64 * 0.1).min(0.3) // Reduce range for problematic parameters
        } else {
            1.0
        };

        let adaptive_range = base_min_range * usage_multiplier * performance_multiplier * rollback_multiplier;
        Ok(adaptive_range.max(base_min_range * 0.5)) // Never go below 50% of base minimum
    }

    /// Determine anti-windup expansion strategy
    fn determine_anti_windup_strategy(&self, parameter: &str, bounds: &ParameterBounds) -> AntiWindupStrategy {
        match parameter {
            "selection_temperature" => {
                // Temperature should expand around center for balanced exploration
                AntiWindupStrategy::CenterExpansion
            }
            "selection_top_k" => {
                // Top-k should expand conservatively with safety margins
                AntiWindupStrategy::SafeExpansion
            }
            "plan_selection_bias" => {
                // Bias should expand toward current usage
                AntiWindupStrategy::BiasedExpansion
            }
            "curator_confidence_threshold" => {
                // Confidence threshold should expand safely
                AntiWindupStrategy::SafeExpansion
            }
            _ => {
                // Default strategy based on performance
                if bounds.performance_score > 0.7 {
                    AntiWindupStrategy::BiasedExpansion
                } else {
                    AntiWindupStrategy::SafeExpansion
                }
            }
        }
    }

    /// Detect and prevent bounds oscillation
    pub fn detect_bounds_oscillation(&self, parameter: &str) -> SlidingBoundsResult<OscillationAnalysis> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        // Analyze recent bounds adjustments for oscillation patterns
        let recent_adjustments: Vec<&BoundsAdjustment> = self.history.iter()
            .rev()
            .take(10)
            .filter(|adj| {
                match adj {
                    BoundsAdjustment::Expansion { parameter: param, .. } |
                    BoundsAdjustment::Contraction { parameter: param, .. } => param == parameter,
                    _ => false,
                }
            })
            .collect();

        if recent_adjustments.len() < 4 {
            return Ok(OscillationAnalysis {
                is_oscillating: false,
                oscillation_frequency: 0.0,
                oscillation_amplitude: 0.0,
                recommendation: OscillationRecommendation::Continue,
                confidence: 0.0,
            });
        }

        // Check for alternating expansion/contraction pattern
        let mut expansion_count = 0;
        let mut contraction_count = 0;
        let mut alternating_pattern = true;
        let mut last_was_expansion = None;

        for adjustment in &recent_adjustments {
            let is_expansion = matches!(adjustment, BoundsAdjustment::Expansion { .. });
            
            if is_expansion {
                expansion_count += 1;
            } else {
                contraction_count += 1;
            }

            if let Some(last_expansion) = last_was_expansion {
                if last_expansion == is_expansion {
                    alternating_pattern = false;
                }
            }
            last_was_expansion = Some(is_expansion);
        }

        let is_oscillating = alternating_pattern && expansion_count > 1 && contraction_count > 1;
        
        let oscillation_frequency = if is_oscillating {
            recent_adjustments.len() as f64 / 10.0 // Frequency over last 10 adjustments
        } else {
            0.0
        };

        // Calculate oscillation amplitude (range of bounds changes)
        let mut range_sizes = Vec::new();
        for adjustment in &recent_adjustments {
            let range_size = match adjustment {
                BoundsAdjustment::Expansion { old_range, new_range, .. } |
                BoundsAdjustment::Contraction { old_range, new_range, .. } => {
                    (new_range.1 - new_range.0) - (old_range.1 - old_range.0)
                }
                _ => 0.0,
            };
            range_sizes.push(range_size.abs());
        }

        let oscillation_amplitude = if !range_sizes.is_empty() {
            range_sizes.iter().sum::<f64>() / range_sizes.len() as f64
        } else {
            0.0
        };

        let recommendation = if is_oscillating {
            if oscillation_frequency > 0.6 {
                OscillationRecommendation::Pause
            } else if oscillation_amplitude > (bounds.max_value - bounds.min_value) * 0.1 {
                OscillationRecommendation::ReduceRate
            } else {
                OscillationRecommendation::Monitor
            }
        } else {
            OscillationRecommendation::Continue
        };

        let confidence = if recent_adjustments.len() >= 6 {
            0.8
        } else {
            0.5
        };

        Ok(OscillationAnalysis {
            is_oscillating,
            oscillation_frequency,
            oscillation_amplitude,
            recommendation,
            confidence,
        })
    }

    /// Apply oscillation prevention measures
    pub fn apply_oscillation_prevention(&mut self, parameter: &str) -> SlidingBoundsResult<Option<BoundsAdjustment>> {
        let oscillation_analysis = self.detect_bounds_oscillation(parameter)?;
        
        if !oscillation_analysis.is_oscillating {
            return Ok(None);
        }

        match oscillation_analysis.recommendation {
            OscillationRecommendation::Pause => {
                // Temporarily disable bounds adjustments for this parameter
                tracing::warn!(
                    target: "sliding_bounds",
                    parameter = %parameter,
                    frequency = oscillation_analysis.oscillation_frequency,
                    "detected high-frequency oscillation, pausing bounds adjustments"
                );
                // This would be implemented by setting a flag or timestamp
                Ok(None)
            }
            OscillationRecommendation::ReduceRate => {
                // Reduce expansion/contraction rates
                tracing::info!(
                    target: "sliding_bounds",
                    parameter = %parameter,
                    amplitude = oscillation_analysis.oscillation_amplitude,
                    "detected oscillation, reducing adjustment rates"
                );
                // This would modify the config rates temporarily
                Ok(None)
            }
            OscillationRecommendation::Monitor => {
                tracing::debug!(
                    target: "sliding_bounds",
                    parameter = %parameter,
                    "monitoring parameter for oscillation patterns"
                );
                Ok(None)
            }
            OscillationRecommendation::Continue => {
                Ok(None)
            }
        }
    }

    /// Analyze contraction necessity and calculate optimal contraction
    pub fn analyze_contraction_necessity(&self, parameter: &str, failure_context: &FailureContext) -> SlidingBoundsResult<ContractionAnalysis> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        // Calculate failure impact score
        let impact_score = self.calculate_failure_impact(bounds, failure_context);
        
        // Analyze failure pattern
        let failure_pattern = self.analyze_failure_pattern(bounds, failure_context);
        
        // Calculate recommended contraction rate
        let recommended_rate = self.calculate_optimal_contraction_rate(bounds, failure_context, impact_score);
        
        // Determine if contraction is necessary
        let should_contract = bounds.rollback_count >= self.config.rollback_threshold ||
                             failure_context.severity == FailureSeverity::Critical ||
                             impact_score > 0.7;

        let confidence = self.calculate_contraction_confidence(bounds, failure_context, &failure_pattern);

        let reason = format!(
            "Rollbacks: {}/{}, severity: {:?}, impact: {:.2}, pattern: {:?}",
            bounds.rollback_count, self.config.rollback_threshold,
            failure_context.severity, impact_score, failure_pattern
        );

        Ok(ContractionAnalysis {
            should_contract,
            recommended_rate,
            impact_score,
            failure_pattern,
            confidence,
            reason,
        })
    }

    /// Calculate failure impact score
    fn calculate_failure_impact(&self, bounds: &ParameterBounds, failure_context: &FailureContext) -> f64 {
        let mut impact_factors = Vec::new();

        // Severity impact
        let severity_impact = match failure_context.severity {
            FailureSeverity::Minor => 0.2,
            FailureSeverity::Moderate => 0.4,
            FailureSeverity::Severe => 0.7,
            FailureSeverity::Critical => 1.0,
        };
        impact_factors.push(severity_impact * 0.4);

        // Frequency impact (more frequent failures = higher impact)
        let frequency_impact = (bounds.rollback_count as f64 / 10.0).min(1.0);
        impact_factors.push(frequency_impact * 0.3);

        // Performance degradation impact
        let performance_impact = 1.0 - bounds.performance_score;
        impact_factors.push(performance_impact * 0.3);

        impact_factors.iter().sum::<f64>().clamp(0.0, 1.0)
    }

    /// Analyze failure patterns to determine appropriate response
    fn analyze_failure_pattern(&self, bounds: &ParameterBounds, _failure_context: &FailureContext) -> FailurePattern {
        if bounds.rollback_count >= 5 {
            FailurePattern::Chronic
        } else if bounds.rollback_count >= 3 {
            FailurePattern::Recurring
        } else if bounds.change_history.len() >= 3 {
            // Check if failures are clustered in time
            let recent_failures = bounds.change_history.iter()
                .rev()
                .take(5)
                .filter(|change| change.success == Some(false))
                .count();
            
            if recent_failures >= 3 {
                FailurePattern::Clustered
            } else {
                FailurePattern::Sporadic
            }
        } else {
            FailurePattern::Isolated
        }
    }

    /// Calculate optimal contraction rate based on multiple factors
    fn calculate_optimal_contraction_rate(&self, bounds: &ParameterBounds, failure_context: &FailureContext, impact_score: f64) -> f64 {
        let base_rate = self.config.contraction_rate_after_rollback;
        
        // Adjust by severity
        let severity_multiplier = match failure_context.severity {
            FailureSeverity::Minor => 0.5,
            FailureSeverity::Moderate => 1.0,
            FailureSeverity::Severe => 1.5,
            FailureSeverity::Critical => 2.0,
        };

        // Adjust by rollback frequency
        let frequency_multiplier = 1.0 + (bounds.rollback_count as f64 * 0.1).min(1.0);
        
        // Adjust by impact score
        let impact_multiplier = 0.5 + impact_score * 0.5;
        
        // Adjust by performance (reduce contraction for better performing parameters)
        let performance_factor = 1.0 - bounds.performance_score * 0.2;

        let optimal_rate = base_rate * severity_multiplier * frequency_multiplier * impact_multiplier * performance_factor;
        
        optimal_rate.clamp(0.05, 0.8) // Between 5% and 80% contraction
    }

    /// Calculate confidence in contraction decision
    fn calculate_contraction_confidence(&self, bounds: &ParameterBounds, failure_context: &FailureContext, failure_pattern: &FailurePattern) -> f64 {
        let mut confidence_factors = Vec::new();

        // Data quality factor
        let data_factor = (bounds.change_history.len() as f64 / 10.0).min(1.0);
        confidence_factors.push(data_factor * 0.3);

        // Severity certainty
        let severity_certainty = match failure_context.severity {
            FailureSeverity::Critical => 1.0,
            FailureSeverity::Severe => 0.8,
            FailureSeverity::Moderate => 0.6,
            FailureSeverity::Minor => 0.4,
        };
        confidence_factors.push(severity_certainty * 0.4);

        // Pattern clarity
        let pattern_clarity = match failure_pattern {
            FailurePattern::Chronic => 1.0,
            FailurePattern::Recurring => 0.8,
            FailurePattern::Clustered => 0.6,
            FailurePattern::Sporadic => 0.4,
            FailurePattern::Isolated => 0.2,
        };
        confidence_factors.push(pattern_clarity * 0.3);

        confidence_factors.iter().sum::<f64>().clamp(0.0, 1.0)
    }

    /// Get contraction recommendations for all parameters
    pub fn get_contraction_recommendations(&self) -> SlidingBoundsResult<Vec<ContractionRecommendation>> {
        let mut recommendations = Vec::new();
        
        for (param_name, bounds) in &self.bounds {
            if bounds.rollback_count > 0 {
                // Create a moderate failure context for analysis
                let failure_context = FailureContext {
                    severity: if bounds.rollback_count >= self.config.rollback_threshold {
                        FailureSeverity::Moderate
                    } else {
                        FailureSeverity::Minor
                    },
                    impact_description: format!("Parameter has {} rollbacks", bounds.rollback_count),
                };

                let analysis = self.analyze_contraction_necessity(param_name, &failure_context)?;
                
                recommendations.push(ContractionRecommendation {
                    parameter_name: param_name.clone(),
                    current_bounds: (bounds.min_value, bounds.max_value),
                    should_contract: analysis.should_contract,
                    recommended_rate: analysis.recommended_rate,
                    confidence: analysis.confidence,
                    reason: analysis.reason,
                    rollback_count: bounds.rollback_count,
                    performance_score: bounds.performance_score,
                });
            }
        }
        
        Ok(recommendations)
    }

    /// Contract bounds after rollback events
    pub fn contract_bounds_after_rollback(&mut self, parameter: &str) -> SlidingBoundsResult<BoundsAdjustment> {
        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        bounds.rollback_count += 1;
        let now = Utc::now();

        if bounds.rollback_count >= self.config.rollback_threshold {
            let old_range = (bounds.min_value, bounds.max_value);
            
            // Contract bounds by the configured rate, adjusted by performance
            let performance_penalty = 1.0 - bounds.performance_score * 0.3; // Reduce contraction for better performing params
            let adjusted_contraction_rate = self.config.contraction_rate_after_rollback * performance_penalty;
            let contraction_factor = 1.0 - adjusted_contraction_rate;
            let current_range = bounds.max_value - bounds.min_value;
            let new_range = current_range * contraction_factor;
            let center = (bounds.min_value + bounds.max_value) / 2.0;
            
            bounds.min_value = center - new_range / 2.0;
            bounds.max_value = center + new_range / 2.0;
            bounds.last_updated = now;
            bounds.last_contraction = Some(now);
            bounds.rollback_count = 0; // Reset rollback counter
            bounds.total_adjustments += 1;
            
            let adjustment = BoundsAdjustment::Contraction {
                parameter: parameter.to_string(),
                old_range,
                new_range: (bounds.min_value, bounds.max_value),
                contraction_rate: adjusted_contraction_rate,
                rollback_count: self.config.rollback_threshold,
                reason: format!("Rollback threshold reached ({}), performance score: {:.2}", 
                              self.config.rollback_threshold, bounds.performance_score),
                timestamp: now,
            };
            
            self.history.push_back(adjustment.clone());

            tracing::warn!(
                target: "sliding_bounds",
                parameter = %parameter,
                old_range = ?old_range,
                new_range = ?(bounds.min_value, bounds.max_value),
                contraction_rate = adjusted_contraction_rate,
                rollback_count = self.config.rollback_threshold,
                "contracted parameter bounds after rollbacks"
            );

            // Auto-save if persistence is enabled
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after contraction"
                );
            }

            Ok(adjustment)
        } else {
            // Just record the rollback without contracting bounds yet
            bounds.last_updated = now;
            let adjustment = BoundsAdjustment::AntiWindup {
                parameter: parameter.to_string(),
                old_range: (bounds.min_value, bounds.max_value),
                new_range: (bounds.min_value, bounds.max_value), // No change yet
                reason: format!("Rollback recorded ({}/{})", bounds.rollback_count, self.config.rollback_threshold),
                timestamp: now,
            };

            tracing::debug!(
                target: "sliding_bounds",
                parameter = %parameter,
                rollback_count = bounds.rollback_count,
                threshold = self.config.rollback_threshold,
                "recorded rollback, threshold not yet reached"
            );

            Ok(adjustment)
        }
    }

    /// Apply anti-windup protection to prevent excessive bounds tightening
    pub fn apply_anti_windup_protection(&mut self, parameter: &str) -> SlidingBoundsResult<Option<BoundsAdjustment>> {
        if !self.config.anti_windup_enabled {
            return Ok(None);
        }

        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        // Check if bounds have become too tight
        let current_range = bounds.max_value - bounds.min_value;
        let min_allowed_range = match parameter {
            "selection_temperature" => 0.1, // Minimum range of 0.1
            "selection_top_k" => 2.0, // Minimum range of 2
            "plan_selection_bias" => 0.02, // Minimum range of 0.02
            "curator_confidence_threshold" => 0.05, // Minimum range of 0.05
            _ => current_range * 0.1, // 10% of current range as minimum
        };

        if current_range < min_allowed_range {
            let old_range = (bounds.min_value, bounds.max_value);
            let center = (bounds.min_value + bounds.max_value) / 2.0;
            bounds.min_value = center - min_allowed_range / 2.0;
            bounds.max_value = center + min_allowed_range / 2.0;
            bounds.last_updated = Utc::now();
            bounds.total_adjustments += 1;

            let adjustment = BoundsAdjustment::AntiWindup {
                parameter: parameter.to_string(),
                old_range,
                new_range: (bounds.min_value, bounds.max_value),
                reason: format!("Bounds expanded to prevent excessive tightening (range was {:.4}, min allowed {:.4})", 
                              current_range, min_allowed_range),
                timestamp: Utc::now(),
            };
            
            self.history.push_back(adjustment.clone());

            tracing::info!(
                target: "sliding_bounds",
                parameter = %parameter,
                old_range = ?old_range,
                new_range = ?(bounds.min_value, bounds.max_value),
                min_allowed_range = min_allowed_range,
                "applied anti-windup protection"
            );

            // Auto-save if persistence is enabled
            if let Err(e) = self.save_bounds_state() {
                tracing::warn!(
                    target: "sliding_bounds",
                    error = %e,
                    "failed to save bounds state after anti-windup"
                );
            }

            Ok(Some(adjustment))
        } else {
            Ok(None)
        }
    }

    /// Reset parameter bounds to default values
    pub fn reset_parameter_bounds(&mut self, parameter: &str, reason: &str) -> SlidingBoundsResult<BoundsAdjustment> {
        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        let old_range = (bounds.min_value, bounds.max_value);
        let now = Utc::now();

        // Reset to default bounds based on parameter type
        let (new_min, new_max, new_current) = match parameter {
            "selection_temperature" => (0.1, 2.0, 0.85),
            "selection_top_k" => (1.0, 50.0, 12.0),
            "plan_selection_bias" => (-0.20, 0.20, 0.0),
            "curator_confidence_threshold" => (0.3, 0.9, 0.62),
            _ => return Err(SlidingBoundsError::Configuration(
                format!("No default bounds defined for parameter: {}", parameter)
            )),
        };

        bounds.min_value = new_min;
        bounds.max_value = new_max;
        bounds.current_value = new_current;
        bounds.stability_days = 0;
        bounds.rollback_count = 0;
        bounds.last_updated = now;
        bounds.total_adjustments += 1;
        bounds.performance_score = 0.5; // Reset to neutral
        bounds.change_history.clear();

        let adjustment = BoundsAdjustment::Reset {
            parameter: parameter.to_string(),
            old_range,
            new_range: (new_min, new_max),
            reason: reason.to_string(),
            timestamp: now,
        };

        self.history.push_back(adjustment.clone());

        tracing::warn!(
            target: "sliding_bounds",
            parameter = %parameter,
            old_range = ?old_range,
            new_range = ?(new_min, new_max),
            reason = %reason,
            "reset parameter bounds to defaults"
        );

        Ok(adjustment)
    }

    /// Get parameter statistics
    pub fn get_parameter_stats(&self, parameter: &str) -> SlidingBoundsResult<ParameterStats> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        let successful_changes = bounds.change_history.iter()
            .filter(|change| change.success == Some(true))
            .count();

        let total_changes = bounds.change_history.iter()
            .filter(|change| change.success.is_some())
            .count();

        let success_rate = if total_changes > 0 {
            successful_changes as f64 / total_changes as f64
        } else {
            0.0
        };

        let avg_change_magnitude = if !bounds.change_history.is_empty() {
            bounds.change_history.iter()
                .map(|change| (change.new_value - change.old_value).abs())
                .sum::<f64>() / bounds.change_history.len() as f64
        } else {
            0.0
        };

        Ok(ParameterStats {
            parameter_name: parameter.to_string(),
            current_bounds: (bounds.min_value, bounds.max_value),
            current_value: bounds.current_value,
            stability_days: bounds.stability_days,
            rollback_count: bounds.rollback_count,
            total_adjustments: bounds.total_adjustments,
            performance_score: bounds.performance_score,
            success_rate,
            avg_change_magnitude,
            total_changes: bounds.change_history.len(),
            last_updated: bounds.last_updated,
            created_at: bounds.created_at,
        })
    }

    /// Update parameter current value and stability tracking
    pub fn update_parameter_value(&mut self, parameter: &str, new_value: f64) -> SlidingBoundsResult<()> {
        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        let old_value = bounds.current_value;
        let now = Utc::now();

        // Check if value is stable (small change)
        let change_threshold = match parameter {
            "selection_temperature" => 0.01,
            "selection_top_k" => 1.0,
            "plan_selection_bias" => 0.005,
            "curator_confidence_threshold" => 0.01,
            _ => 0.01,
        };

        let is_stable_change = (new_value - old_value).abs() < change_threshold;

        if is_stable_change {
            bounds.stability_days += 1;
        } else {
            bounds.stability_days = 0; // Reset stability counter on significant change
        }

        // Record the parameter change
        let change_record = ParameterChange {
            timestamp: now,
            old_value,
            new_value,
            change_reason: if is_stable_change { "stable_adjustment" } else { "significant_change" }.to_string(),
            success: None, // Will be updated later when impact is measured
        };

        bounds.change_history.push_back(change_record);
        
        // Limit change history size
        while bounds.change_history.len() > 100 {
            bounds.change_history.pop_front();
        }

        bounds.current_value = new_value;
        bounds.last_updated = now;

        let stability_days = bounds.stability_days;

        tracing::debug!(
            target: "sliding_bounds",
            parameter = parameter,
            old_value = old_value,
            new_value = new_value,
            stability_days = stability_days,
            is_stable = is_stable_change,
            "updated parameter value"
        );

        // Auto-save if persistence is enabled (after releasing the mutable borrow)
        if let Err(e) = self.save_bounds_state() {
            tracing::warn!(
                target: "sliding_bounds",
                parameter = parameter,
                error = %e,
                "failed to save bounds state after parameter update"
            );
        }

        Ok(())
    }

    /// Update parameter performance score based on success/failure
    pub fn update_parameter_performance(&mut self, parameter: &str, success: bool, impact_score: f64) -> SlidingBoundsResult<()> {
        let bounds = self.bounds.get_mut(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        // Update the most recent change record with success status
        if let Some(latest_change) = bounds.change_history.back_mut() {
            latest_change.success = Some(success);
        }

        // Update performance score using exponential moving average
        let alpha = 0.1; // Learning rate
        let new_score = if success { 
            (1.0 + impact_score).min(1.0) // Positive impact boosts score
        } else { 
            (0.0 + impact_score).max(0.0) // Negative impact reduces score
        };
        
        bounds.performance_score = bounds.performance_score * (1.0 - alpha) + new_score * alpha;
        bounds.performance_score = bounds.performance_score.clamp(0.0, 1.0);

        tracing::debug!(
            target: "sliding_bounds",
            parameter = parameter,
            success = success,
            impact_score = impact_score,
            new_performance_score = bounds.performance_score,
            "updated parameter performance"
        );

        Ok(())
    }

    /// Add a new parameter with initial bounds
    pub fn add_parameter(&mut self, parameter_name: String, min_value: f64, max_value: f64, initial_value: f64) -> SlidingBoundsResult<()> {
        if min_value >= max_value {
            return Err(SlidingBoundsError::InvalidBounds(
                format!("min_value ({}) must be less than max_value ({})", min_value, max_value)
            ));
        }

        if initial_value < min_value || initial_value > max_value {
            return Err(SlidingBoundsError::InvalidBounds(
                format!("initial_value ({}) must be between min_value ({}) and max_value ({})", 
                       initial_value, min_value, max_value)
            ));
        }

        let now = Utc::now();
        let bounds = ParameterBounds {
            parameter_name: parameter_name.clone(),
            min_value,
            max_value,
            current_value: initial_value,
            stability_days: 0,
            rollback_count: 0,
            last_updated: now,
            created_at: now,
            total_adjustments: 0,
            last_expansion: None,
            last_contraction: None,
            performance_score: 0.5,
            change_history: VecDeque::new(),
        };

        self.bounds.insert(parameter_name.clone(), bounds);

        tracing::info!(
            target: "sliding_bounds",
            parameter = %parameter_name,
            min_value = min_value,
            max_value = max_value,
            initial_value = initial_value,
            "added new parameter bounds"
        );

        Ok(())
    }

    /// Remove a parameter (use with caution)
    pub fn remove_parameter(&mut self, parameter: &str) -> SlidingBoundsResult<ParameterBounds> {
        self.bounds.remove(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))
    }

    /// Get current bounds for a parameter
    pub fn get_bounds(&self, parameter: &str) -> Option<&ParameterBounds> {
        self.bounds.get(parameter)
    }

    /// Get all parameter bounds
    pub fn get_all_bounds(&self) -> &HashMap<String, ParameterBounds> {
        &self.bounds
    }

    /// Get bounds adjustment history
    pub fn get_adjustment_history(&self) -> &VecDeque<BoundsAdjustment> {
        &self.history
    }

    /// Generate comprehensive bounds adjustment report for a given time period
    pub fn generate_adjustment_report(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> BoundsAdjustmentReport {
        let now = Utc::now();
        
        // Filter adjustments within the time period
        let period_adjustments: Vec<&BoundsAdjustment> = self.history.iter()
            .filter(|adj| {
                let timestamp = match adj {
                    BoundsAdjustment::Expansion { timestamp, .. } |
                    BoundsAdjustment::Contraction { timestamp, .. } |
                    BoundsAdjustment::AntiWindup { timestamp, .. } |
                    BoundsAdjustment::Reset { timestamp, .. } => *timestamp,
                };
                timestamp >= start_time && timestamp <= end_time
            })
            .collect();

        // Count adjustments by type
        let mut adjustments_by_type = HashMap::new();
        let mut adjustments_by_parameter = HashMap::new();

        for adjustment in &period_adjustments {
            let (adj_type, parameter) = match adjustment {
                BoundsAdjustment::Expansion { parameter, .. } => ("expansion", parameter),
                BoundsAdjustment::Contraction { parameter, .. } => ("contraction", parameter),
                BoundsAdjustment::AntiWindup { parameter, .. } => ("anti_windup", parameter),
                BoundsAdjustment::Reset { parameter, .. } => ("reset", parameter),
            };

            *adjustments_by_type.entry(adj_type.to_string()).or_insert(0) += 1;
            *adjustments_by_parameter.entry(parameter.clone()).or_insert(0) += 1;
        }

        // Generate parameter summaries
        let parameter_summaries = self.generate_parameter_summaries(&period_adjustments, start_time, end_time);

        // Analyze stability
        let stability_analysis = self.analyze_bounds_stability(&period_adjustments);

        // Generate recommendations
        let recommendations = self.generate_bounds_recommendations(&parameter_summaries, &stability_analysis);

        BoundsAdjustmentReport {
            report_timestamp: now,
            report_period: (start_time, end_time),
            total_adjustments: period_adjustments.len(),
            adjustments_by_type,
            adjustments_by_parameter,
            parameter_summaries,
            stability_analysis,
            recommendations,
        }
    }

    /// Generate parameter-specific adjustment summaries
    fn generate_parameter_summaries(&self, adjustments: &[&BoundsAdjustment], start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Vec<ParameterAdjustmentSummary> {
        let mut summaries = Vec::new();
        let period_days = (end_time - start_time).num_days() as f64;

        for (param_name, bounds) in &self.bounds {
            let param_adjustments: Vec<&BoundsAdjustment> = adjustments.iter()
                .filter(|adj| {
                    match adj {
                        BoundsAdjustment::Expansion { parameter, .. } |
                        BoundsAdjustment::Contraction { parameter, .. } |
                        BoundsAdjustment::AntiWindup { parameter, .. } |
                        BoundsAdjustment::Reset { parameter, .. } => parameter == param_name,
                    }
                })
                .copied()
                .collect();

            let mut expansions = 0;
            let mut contractions = 0;
            let mut anti_windup_events = 0;
            let mut resets = 0;
            let mut bounds_range_history = Vec::new();
            let mut last_adjustment = None;

            for adjustment in &param_adjustments {
                match adjustment {
                    BoundsAdjustment::Expansion { timestamp, new_range, .. } => {
                        expansions += 1;
                        bounds_range_history.push(new_range.1 - new_range.0);
                        last_adjustment = Some(*timestamp);
                    }
                    BoundsAdjustment::Contraction { timestamp, new_range, .. } => {
                        contractions += 1;
                        bounds_range_history.push(new_range.1 - new_range.0);
                        last_adjustment = Some(*timestamp);
                    }
                    BoundsAdjustment::AntiWindup { timestamp, new_range, .. } => {
                        anti_windup_events += 1;
                        bounds_range_history.push(new_range.1 - new_range.0);
                        last_adjustment = Some(*timestamp);
                    }
                    BoundsAdjustment::Reset { timestamp, new_range, .. } => {
                        resets += 1;
                        bounds_range_history.push(new_range.1 - new_range.0);
                        last_adjustment = Some(*timestamp);
                    }
                }
            }

            // Calculate stability score (0.0 to 1.0, higher is more stable)
            let stability_score = self.calculate_parameter_stability_score(param_name, &param_adjustments);

            // Calculate adjustment frequency (adjustments per week)
            let adjustment_frequency = if period_days > 0.0 {
                (param_adjustments.len() as f64) * 7.0 / period_days
            } else {
                0.0
            };

            summaries.push(ParameterAdjustmentSummary {
                parameter_name: param_name.clone(),
                total_adjustments: param_adjustments.len(),
                expansions,
                contractions,
                anti_windup_events,
                resets,
                current_bounds: (bounds.min_value, bounds.max_value),
                bounds_range_history,
                stability_score,
                last_adjustment,
                adjustment_frequency,
            });
        }

        summaries
    }

    /// Calculate stability score for a parameter based on its adjustment history
    fn calculate_parameter_stability_score(&self, _parameter: &str, adjustments: &[&BoundsAdjustment]) -> f64 {
        if adjustments.is_empty() {
            return 1.0; // No adjustments = perfectly stable
        }

        let mut stability_factors = Vec::new();

        // Factor 1: Frequency of adjustments (fewer = more stable)
        let adjustment_frequency = adjustments.len() as f64;
        let frequency_score = (1.0 / (1.0 + adjustment_frequency * 0.1)).max(0.0);
        stability_factors.push(frequency_score);

        // Factor 2: Type of adjustments (expansions are better than contractions)
        let expansions = adjustments.iter().filter(|adj| matches!(adj, BoundsAdjustment::Expansion { .. })).count();
        let contractions = adjustments.iter().filter(|adj| matches!(adj, BoundsAdjustment::Contraction { .. })).count();
        let anti_windup = adjustments.iter().filter(|adj| matches!(adj, BoundsAdjustment::AntiWindup { .. })).count();
        let resets = adjustments.iter().filter(|adj| matches!(adj, BoundsAdjustment::Reset { .. })).count();

        let type_score = if adjustments.len() > 0 {
            let expansion_ratio = expansions as f64 / adjustments.len() as f64;
            let contraction_penalty = (contractions + resets) as f64 / adjustments.len() as f64;
            let anti_windup_penalty = anti_windup as f64 / adjustments.len() as f64;
            
            (expansion_ratio - contraction_penalty * 0.5 - anti_windup_penalty * 0.3).max(0.0)
        } else {
            1.0
        };
        stability_factors.push(type_score);

        // Factor 3: Oscillation detection (alternating expansions/contractions)
        let oscillation_penalty = self.detect_oscillation_in_adjustments(adjustments);
        stability_factors.push(1.0 - oscillation_penalty);

        // Factor 4: Time since last adjustment (longer = more stable)
        if let Some(last_adj) = adjustments.last() {
            let last_timestamp = match last_adj {
                BoundsAdjustment::Expansion { timestamp, .. } |
                BoundsAdjustment::Contraction { timestamp, .. } |
                BoundsAdjustment::AntiWindup { timestamp, .. } |
                BoundsAdjustment::Reset { timestamp, .. } => *timestamp,
            };
            let days_since_last = (Utc::now() - last_timestamp).num_days() as f64;
            let time_score = (days_since_last / 30.0).min(1.0); // Max score after 30 days
            stability_factors.push(time_score);
        }

        // Calculate weighted average
        stability_factors.iter().sum::<f64>() / stability_factors.len() as f64
    }

    /// Detect oscillation patterns in adjustment history
    fn detect_oscillation_in_adjustments(&self, adjustments: &[&BoundsAdjustment]) -> f64 {
        if adjustments.len() < 4 {
            return 0.0; // Need at least 4 adjustments to detect oscillation
        }

        let mut pattern = Vec::new();
        for adjustment in adjustments {
            match adjustment {
                BoundsAdjustment::Expansion { .. } => pattern.push(1),
                BoundsAdjustment::Contraction { .. } => pattern.push(-1),
                BoundsAdjustment::AntiWindup { .. } => pattern.push(0),
                BoundsAdjustment::Reset { .. } => pattern.push(-2),
            }
        }

        // Look for alternating patterns
        let mut alternations = 0;
        for i in 1..pattern.len() {
            if pattern[i] != pattern[i-1] && pattern[i] != 0 && pattern[i-1] != 0 {
                alternations += 1;
            }
        }

        // High alternation rate indicates oscillation
        let alternation_rate = alternations as f64 / (pattern.len() - 1) as f64;
        if alternation_rate > 0.6 {
            alternation_rate
        } else {
            0.0
        }
    }

    /// Analyze overall bounds stability across all parameters
    fn analyze_bounds_stability(&self, adjustments: &[&BoundsAdjustment]) -> StabilityAnalysis {
        let mut parameter_stability_scores = HashMap::new();
        
        // Calculate stability scores for all parameters
        for param_name in self.bounds.keys() {
            let param_adjustments: Vec<&BoundsAdjustment> = adjustments.iter()
                .filter(|adj| {
                    match adj {
                        BoundsAdjustment::Expansion { parameter, .. } |
                        BoundsAdjustment::Contraction { parameter, .. } |
                        BoundsAdjustment::AntiWindup { parameter, .. } |
                        BoundsAdjustment::Reset { parameter, .. } => parameter == param_name,
                    }
                })
                .copied()
                .collect();

            let stability_score = self.calculate_parameter_stability_score(param_name, &param_adjustments);
            parameter_stability_scores.insert(param_name.clone(), stability_score);
        }

        // Calculate overall stability score
        let overall_stability_score = if parameter_stability_scores.is_empty() {
            1.0
        } else {
            parameter_stability_scores.values().sum::<f64>() / parameter_stability_scores.len() as f64
        };

        // Sort parameters by stability
        let mut sorted_params: Vec<(String, f64)> = parameter_stability_scores.into_iter().collect();
        sorted_params.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let most_stable_parameters = sorted_params.iter()
            .take(3)
            .filter(|(_, score)| *score > 0.8)
            .map(|(name, _)| name.clone())
            .collect();

        let least_stable_parameters = sorted_params.iter()
            .rev()
            .take(3)
            .filter(|(_, score)| *score < 0.5)
            .map(|(name, _)| name.clone())
            .collect();

        // Detect oscillating parameters
        let oscillating_parameters: Vec<String> = sorted_params.iter()
            .filter(|(name, _)| {
                let param_adjustments: Vec<&BoundsAdjustment> = adjustments.iter()
                    .filter(|adj| {
                        match adj {
                            BoundsAdjustment::Expansion { parameter, .. } |
                            BoundsAdjustment::Contraction { parameter, .. } |
                            BoundsAdjustment::AntiWindup { parameter, .. } |
                            BoundsAdjustment::Reset { parameter, .. } => parameter == name,
                        }
                    })
                    .copied()
                    .collect();
                self.detect_oscillation_in_adjustments(&param_adjustments) > 0.5
            })
            .map(|(name, _)| name.clone())
            .collect();

        // Parameters needing attention (low stability or high adjustment frequency)
        let parameters_needing_attention = sorted_params.iter()
            .filter(|(name, score)| {
                *score < 0.6 || {
                    let param_adjustments: Vec<&BoundsAdjustment> = adjustments.iter()
                        .filter(|adj| {
                            match adj {
                                BoundsAdjustment::Expansion { parameter, .. } |
                                BoundsAdjustment::Contraction { parameter, .. } |
                                BoundsAdjustment::AntiWindup { parameter, .. } |
                                BoundsAdjustment::Reset { parameter, .. } => parameter == name,
                            }
                        })
                        .copied()
                        .collect();
                    param_adjustments.len() > 5 // More than 5 adjustments in period
                }
            })
            .map(|(name, _)| name.clone())
            .collect();

        // Determine stability trend
        let stability_trend = if overall_stability_score > 0.8 {
            StabilityTrend::Stable
        } else if overall_stability_score > 0.6 {
            if oscillating_parameters.len() > 2 {
                StabilityTrend::Oscillating
            } else {
                StabilityTrend::Improving
            }
        } else {
            StabilityTrend::Degrading
        };

        StabilityAnalysis {
            overall_stability_score,
            most_stable_parameters,
            least_stable_parameters,
            oscillating_parameters,
            parameters_needing_attention,
            stability_trend,
        }
    }

    /// Generate recommendations based on bounds analysis
    fn generate_bounds_recommendations(&self, parameter_summaries: &[ParameterAdjustmentSummary], stability_analysis: &StabilityAnalysis) -> Vec<BoundsRecommendation> {
        let mut recommendations = Vec::new();

        // Recommendations for oscillating parameters
        for param_name in &stability_analysis.oscillating_parameters {
            recommendations.push(BoundsRecommendation {
                recommendation_type: RecommendationType::InvestigateOscillation,
                parameter_name: param_name.clone(),
                priority: RecommendationPriority::High,
                description: format!("Parameter {} shows oscillating bounds adjustments", param_name),
                suggested_action: "Review parameter behavior and consider increasing stability period or enabling anti-windup protection".to_string(),
                confidence: 0.8,
            });
        }

        // Recommendations for unstable parameters
        for param_name in &stability_analysis.least_stable_parameters {
            if let Some(summary) = parameter_summaries.iter().find(|s| &s.parameter_name == param_name) {
                if summary.contractions > summary.expansions {
                    recommendations.push(BoundsRecommendation {
                        recommendation_type: RecommendationType::ResetBounds,
                        parameter_name: param_name.clone(),
                        priority: RecommendationPriority::Medium,
                        description: format!("Parameter {} has more contractions than expansions", param_name),
                        suggested_action: "Consider resetting bounds to default values and monitoring".to_string(),
                        confidence: 0.7,
                    });
                }

                if summary.adjustment_frequency > 2.0 {
                    recommendations.push(BoundsRecommendation {
                        recommendation_type: RecommendationType::IncreaseStabilityPeriod,
                        parameter_name: param_name.clone(),
                        priority: RecommendationPriority::Medium,
                        description: format!("Parameter {} has high adjustment frequency ({:.1} per week)", param_name, summary.adjustment_frequency),
                        suggested_action: "Increase stability period requirement before allowing bounds expansion".to_string(),
                        confidence: 0.6,
                    });
                }
            }
        }

        // Recommendations for stable parameters that could be expanded
        for param_name in &stability_analysis.most_stable_parameters {
            if let Some(summary) = parameter_summaries.iter().find(|s| &s.parameter_name == param_name) {
                if summary.total_adjustments == 0 && summary.stability_score > 0.9 {
                    recommendations.push(BoundsRecommendation {
                        recommendation_type: RecommendationType::ExpandBounds,
                        parameter_name: param_name.clone(),
                        priority: RecommendationPriority::Low,
                        description: format!("Parameter {} is very stable with no recent adjustments", param_name),
                        suggested_action: "Consider allowing bounds expansion to explore better parameter values".to_string(),
                        confidence: 0.5,
                    });
                }
            }
        }

        // Global recommendations based on overall stability
        match stability_analysis.stability_trend {
            StabilityTrend::Degrading => {
                recommendations.push(BoundsRecommendation {
                    recommendation_type: RecommendationType::DecreaseExpansionRate,
                    parameter_name: "global".to_string(),
                    priority: RecommendationPriority::High,
                    description: "Overall bounds stability is degrading".to_string(),
                    suggested_action: "Decrease expansion rate and increase stability requirements".to_string(),
                    confidence: 0.8,
                });
            }
            StabilityTrend::Oscillating => {
                recommendations.push(BoundsRecommendation {
                    recommendation_type: RecommendationType::EnableAntiWindup,
                    parameter_name: "global".to_string(),
                    priority: RecommendationPriority::High,
                    description: "Multiple parameters showing oscillating behavior".to_string(),
                    suggested_action: "Enable anti-windup protection globally and review bounds adjustment policies".to_string(),
                    confidence: 0.9,
                });
            }
            _ => {}
        }

        recommendations
    }

    /// Export bounds adjustment history for external analysis
    pub fn export_adjustment_history(&self, include_metadata: bool) -> BoundsAdjustmentHistory {
        let now = Utc::now();
        let adjustments = self.history.iter().cloned().collect();

        let parameter_metadata = if include_metadata {
            self.bounds.iter().map(|(name, bounds)| {
                (name.clone(), ParameterMetadata {
                    parameter_name: name.clone(),
                    initial_bounds: (bounds.min_value, bounds.max_value), // Current bounds as proxy
                    absolute_min: bounds.min_value - (bounds.max_value - bounds.min_value) * 0.5,
                    absolute_max: bounds.max_value + (bounds.max_value - bounds.min_value) * 0.5,
                    created_at: bounds.created_at,
                    last_modified: bounds.last_updated,
                    total_lifetime_adjustments: bounds.total_adjustments as usize,
                })
            }).collect()
        } else {
            HashMap::new()
        };

        BoundsAdjustmentHistory {
            export_timestamp: now,
            adjustments,
            parameter_metadata,
            configuration_history: Vec::new(), // TODO: Implement configuration change tracking
        }
    }

    /// Get adjustment history for a specific parameter within a time range
    pub fn get_parameter_adjustment_history(&self, parameter: &str, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Vec<BoundsAdjustment> {
        self.history.iter()
            .filter(|adj| {
                let (adj_param, timestamp) = match adj {
                    BoundsAdjustment::Expansion { parameter, timestamp, .. } => (parameter, *timestamp),
                    BoundsAdjustment::Contraction { parameter, timestamp, .. } => (parameter, *timestamp),
                    BoundsAdjustment::AntiWindup { parameter, timestamp, .. } => (parameter, *timestamp),
                    BoundsAdjustment::Reset { parameter, timestamp, .. } => (parameter, *timestamp),
                };
                adj_param == parameter && timestamp >= start_time && timestamp <= end_time
            })
            .cloned()
            .collect()
    }

    /// Generate weekly incident triage report for bounds adjustments
    pub fn generate_weekly_triage_report(&self) -> BoundsAdjustmentReport {
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        self.generate_adjustment_report(week_ago, now)
    }

    /// Clean up old adjustment history based on retention policy
    pub fn cleanup_old_history(&mut self, retention_days: u32) -> usize {
        let cutoff_time = Utc::now() - chrono::Duration::days(retention_days as i64);
        let initial_count = self.history.len();

        self.history.retain(|adjustment| {
            let timestamp = match adjustment {
                BoundsAdjustment::Expansion { timestamp, .. } |
                BoundsAdjustment::Contraction { timestamp, .. } |
                BoundsAdjustment::AntiWindup { timestamp, .. } |
                BoundsAdjustment::Reset { timestamp, .. } => *timestamp,
            };
            timestamp >= cutoff_time
        });

        initial_count - self.history.len()
    }

    /// Save bounds adjustment report to file
    pub fn save_adjustment_report(&self, report: &BoundsAdjustmentReport, file_path: &Path) -> SlidingBoundsResult<()> {
        let json_content = serde_json::to_string_pretty(report)
            .map_err(|e| SlidingBoundsError::Configuration(format!("Failed to serialize report: {}", e)))?;

        fs::write(file_path, json_content)
            .map_err(|e| SlidingBoundsError::Configuration(format!("Failed to write report file: {}", e)))?;

        Ok(())
    }

    /// Export bounds state for debugging and analysis
    pub fn export_bounds_state(&self) -> serde_json::Value {
        serde_json::json!({
            "bounds": self.bounds,
            "config": self.config,
            "history_count": self.history.len(),
            "last_adjustments": self.history.iter().rev().take(5).collect::<Vec<_>>()
        })
    }

    /// Comprehensive parameter validation with detailed constraint checking
    pub fn validate_change_comprehensive(&self, parameter: &str, new_value: f64) -> SlidingBoundsResult<ValidationResult> {
        let bounds = self.bounds.get(parameter)
            .ok_or_else(|| SlidingBoundsError::ParameterNotFound(parameter.to_string()))?;

        let mut validation_result = ValidationResult {
            is_valid: true,
            violations: Vec::new(),
            warnings: Vec::new(),
            adjusted_value: None,
            confidence: 1.0,
        };

        // 1. Bounds validation
        self.validate_bounds_constraints(parameter, new_value, bounds, &mut validation_result)?;

        // 2. Business logic constraints
        self.validate_business_constraints(parameter, new_value, bounds, &mut validation_result)?;

        // 3. Safety constraints
        self.validate_safety_constraints(parameter, new_value, bounds, &mut validation_result)?;

        // Calculate overall confidence based on violations and warnings
        validation_result.confidence = self.calculate_validation_confidence(&validation_result);

        // Determine if change is valid overall
        validation_result.is_valid = validation_result.violations.is_empty();

        Ok(validation_result)
    }

    /// Validate bounds constraints
    fn validate_bounds_constraints(
        &self,
        parameter: &str,
        new_value: f64,
        bounds: &ParameterBounds,
        result: &mut ValidationResult,
    ) -> SlidingBoundsResult<()> {
        // Check sliding bounds
        if new_value < bounds.min_value {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::SlidingBounds,
                severity: ViolationSeverity::Error,
                message: format!("Value {} below minimum bound {}", new_value, bounds.min_value),
                suggested_fix: Some(format!("Use minimum value: {}", bounds.min_value)),
            });
            result.adjusted_value = Some(bounds.min_value);
        } else if new_value > bounds.max_value {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::SlidingBounds,
                severity: ViolationSeverity::Error,
                message: format!("Value {} above maximum bound {}", new_value, bounds.max_value),
                suggested_fix: Some(format!("Use maximum value: {}", bounds.max_value)),
            });
            result.adjusted_value = Some(bounds.max_value);
        }

        // Check absolute bounds (hard limits that never change)
        let (absolute_min, absolute_max) = self.get_absolute_bounds(parameter);
        if new_value < absolute_min {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::AbsoluteBounds,
                severity: ViolationSeverity::Critical,
                message: format!("Value {} below absolute minimum {}", new_value, absolute_min),
                suggested_fix: Some(format!("Use absolute minimum: {}", absolute_min)),
            });
            result.adjusted_value = Some(absolute_min);
        } else if new_value > absolute_max {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::AbsoluteBounds,
                severity: ViolationSeverity::Critical,
                message: format!("Value {} above absolute maximum {}", new_value, absolute_max),
                suggested_fix: Some(format!("Use absolute maximum: {}", absolute_max)),
            });
            result.adjusted_value = Some(absolute_max);
        }

        Ok(())
    }

    /// Get absolute bounds for a parameter (hard limits)
    fn get_absolute_bounds(&self, parameter: &str) -> (f64, f64) {
        match parameter {
            "selection_temperature" => (0.01, 5.0), // Absolute physical limits
            "selection_top_k" => (1.0, 100.0),
            "curator_confidence_threshold" => (0.0, 1.0),
            "plan_selection_bias" => (-0.5, 0.5),
            _ => (f64::NEG_INFINITY, f64::INFINITY), // No absolute limits for unknown parameters
        }
    }

    /// Validate business logic constraints
    fn validate_business_constraints(
        &self,
        parameter: &str,
        new_value: f64,
        _bounds: &ParameterBounds,
        result: &mut ValidationResult,
    ) -> SlidingBoundsResult<()> {
        match parameter {
            "selection_temperature" => {
                // Temperature must be positive
                if new_value <= 0.0 {
                    result.violations.push(ConstraintViolation {
                        constraint_type: ConstraintType::BusinessLogic,
                        severity: ViolationSeverity::Error,
                        message: "Temperature must be positive".to_string(),
                        suggested_fix: Some("Use minimum temperature: 0.1".to_string()),
                    });
                }
            }
            "selection_top_k" => {
                // Top-k must be integer
                if new_value.fract() != 0.0 {
                    result.violations.push(ConstraintViolation {
                        constraint_type: ConstraintType::BusinessLogic,
                        severity: ViolationSeverity::Error,
                        message: "Top-k must be an integer".to_string(),
                        suggested_fix: Some(format!("Use rounded value: {}", new_value.round())),
                    });
                    result.adjusted_value = Some(new_value.round());
                }

                // Top-k must be at least 1
                if new_value < 1.0 {
                    result.violations.push(ConstraintViolation {
                        constraint_type: ConstraintType::BusinessLogic,
                        severity: ViolationSeverity::Error,
                        message: "Top-k must be at least 1".to_string(),
                        suggested_fix: Some("Use minimum top-k: 1".to_string()),
                    });
                }
            }
            "curator_confidence_threshold" => {
                // Threshold must be between 0 and 1
                if new_value < 0.0 || new_value > 1.0 {
                    result.violations.push(ConstraintViolation {
                        constraint_type: ConstraintType::BusinessLogic,
                        severity: ViolationSeverity::Error,
                        message: "Confidence threshold must be between 0 and 1".to_string(),
                        suggested_fix: Some(format!("Use clamped value: {}", new_value.clamp(0.0, 1.0))),
                    });
                    result.adjusted_value = Some(new_value.clamp(0.0, 1.0));
                }
            }
            "plan_selection_bias" => {
                // Daily change limit
                if new_value.abs() > 0.05 {
                    result.violations.push(ConstraintViolation {
                        constraint_type: ConstraintType::RateLimit,
                        severity: ViolationSeverity::Error,
                        message: format!("Daily bias change {} exceeds limit 0.05", new_value.abs()),
                        suggested_fix: Some("Limit daily bias change to 0.05".to_string()),
                    });
                }
            }
            _ => {
                // Unknown parameter - add warning
                result.warnings.push(ConstraintWarning {
                    warning_type: WarningType::Configuration,
                    message: format!("Unknown parameter '{}' - validation may be incomplete", parameter),
                    recommendation: "Verify parameter name and add specific validation rules".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate safety constraints
    fn validate_safety_constraints(
        &self,
        parameter: &str,
        new_value: f64,
        bounds: &ParameterBounds,
        result: &mut ValidationResult,
    ) -> SlidingBoundsResult<()> {
        // Check for NaN or infinite values
        if !new_value.is_finite() {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::Safety,
                severity: ViolationSeverity::Critical,
                message: "Value must be finite (not NaN or infinite)".to_string(),
                suggested_fix: Some(format!("Use current value: {}", bounds.current_value)),
            });
            result.adjusted_value = Some(bounds.current_value);
            return Ok(());
        }

        // Check maximum change rate (safety limit)
        let max_change_pct = match parameter {
            "selection_temperature" => 0.20, // 20% max change
            "selection_top_k" => 0.30, // 30% max change
            "curator_confidence_threshold" => 0.15, // 15% max change
            "plan_selection_bias" => 2.0, // Allow larger relative changes for small values
            _ => 0.25, // 25% default
        };

        let change_pct = if bounds.current_value != 0.0 {
            ((new_value - bounds.current_value) / bounds.current_value).abs()
        } else {
            new_value.abs()
        };

        if change_pct > max_change_pct {
            result.violations.push(ConstraintViolation {
                constraint_type: ConstraintType::Safety,
                severity: ViolationSeverity::Warning,
                message: format!(
                    "Change magnitude {:.1}% exceeds safety limit {:.1}%",
                    change_pct * 100.0,
                    max_change_pct * 100.0
                ),
                suggested_fix: Some(format!(
                    "Limit change to: {}",
                    bounds.current_value * (1.0 + max_change_pct * new_value.signum())
                )),
            });
        }

        Ok(())
    }

    /// Calculate validation confidence based on violations and warnings
    fn calculate_validation_confidence(&self, result: &ValidationResult) -> f64 {
        let mut confidence = 1.0;

        // Reduce confidence based on violations
        for violation in &result.violations {
            let penalty = match violation.severity {
                ViolationSeverity::Critical => 0.8,
                ViolationSeverity::Error => 0.3,
                ViolationSeverity::Warning => 0.1,
            };
            confidence -= penalty;
        }

        // Reduce confidence based on warnings
        confidence -= result.warnings.len() as f64 * 0.05;

        confidence.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_bounds_creation() {
        let config = SlidingBoundsConfig::default();
        let bounds = SlidingBounds::new(config);
        
        assert!(bounds.get_bounds("selection_temperature").is_some());
        assert!(bounds.get_bounds("selection_top_k").is_some());
        assert!(bounds.get_bounds("plan_selection_bias").is_some());
    }

    #[test]
    fn test_parameter_validation() {
        let config = SlidingBoundsConfig::default();
        let bounds = SlidingBounds::new(config);
        
        // Valid values
        assert!(bounds.validate_change("selection_temperature", 0.85).is_ok());
        assert!(bounds.validate_change("selection_top_k", 12.0).is_ok());
        
        // Invalid values
        assert!(bounds.validate_change("selection_temperature", 0.05).is_err());
        assert!(bounds.validate_change("selection_temperature", 3.0).is_err());
    }

    #[test]
    fn test_bounds_expansion() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Set stability days to trigger expansion
        if let Some(temp_bounds) = bounds.bounds.get_mut("selection_temperature") {
            temp_bounds.stability_days = 7;
        }
        
        let adjustments = bounds.expand_bounds_for_stable_parameters();
        assert_eq!(adjustments.len(), 1);
        
        match &adjustments[0] {
            BoundsAdjustment::Expansion { parameter, .. } => {
                assert_eq!(parameter, "selection_temperature");
            }
            _ => panic!("Expected expansion adjustment"),
        }
    }

    #[test]
    fn test_bounds_contraction() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Trigger rollback threshold
        for _ in 0..3 {
            let _ = bounds.contract_bounds_after_rollback("selection_temperature");
        }
        
        let adjustment = bounds.contract_bounds_after_rollback("selection_temperature").unwrap();
        match adjustment {
            BoundsAdjustment::Contraction { parameter, .. } => {
                assert_eq!(parameter, "selection_temperature");
            }
            _ => panic!("Expected contraction adjustment"),
        }
    }

    #[test]
    fn test_daily_bias_change_limit() {
        let config = SlidingBoundsConfig::default();
        let bounds = SlidingBounds::new(config);
        
        // Large daily change should be rejected
        assert!(bounds.validate_change("plan_selection_bias", 0.10).is_err());
        
        // Small daily change should be accepted
        assert!(bounds.validate_change("plan_selection_bias", 0.03).is_ok());
    }

    #[test]
    fn test_bounds_adjustment_reporting() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Add some test adjustments
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        
        // Simulate some bounds adjustments
        let _ = bounds.expand_bounds_for_stable_parameters();
        let _ = bounds.contract_bounds_after_rollback("selection_temperature");
        
        // Generate report
        let report = bounds.generate_adjustment_report(week_ago, now);
        
        assert!(report.total_adjustments > 0);
        assert!(!report.parameter_summaries.is_empty());
        assert!(report.stability_analysis.overall_stability_score >= 0.0);
        assert!(report.stability_analysis.overall_stability_score <= 1.0);
    }

    #[test]
    fn test_parameter_stability_calculation() {
        let config = SlidingBoundsConfig::default();
        let bounds = SlidingBounds::new(config);
        
        // Test with no adjustments (should be perfectly stable)
        let empty_adjustments = Vec::new();
        let stability_score = bounds.calculate_parameter_stability_score("test_param", &empty_adjustments);
        assert_eq!(stability_score, 1.0);
        
        // Test with mixed adjustments
        let now = Utc::now();
        let expansion = BoundsAdjustment::Expansion {
            parameter: "test_param".to_string(),
            old_range: (0.5, 1.5),
            new_range: (0.4, 1.6),
            expansion_rate: 0.05,
            stability_days: 7,
            reason: "stable performance".to_string(),
            timestamp: now,
        };
        
        let contraction = BoundsAdjustment::Contraction {
            parameter: "test_param".to_string(),
            old_range: (0.4, 1.6),
            new_range: (0.5, 1.5),
            contraction_rate: 0.25,
            rollback_count: 1,
            reason: "rollback detected".to_string(),
            timestamp: now,
        };
        
        let mixed_adjustments = vec![&expansion, &contraction];
        let stability_score = bounds.calculate_parameter_stability_score("test_param", &mixed_adjustments);
        assert!(stability_score >= 0.0 && stability_score <= 1.0);
    }

    #[test]
    fn test_oscillation_detection() {
        let config = SlidingBoundsConfig::default();
        let bounds = SlidingBounds::new(config);
        
        let now = Utc::now();
        
        // Create alternating expansion/contraction pattern (oscillation)
        let adjustments = vec![
            BoundsAdjustment::Expansion {
                parameter: "test_param".to_string(),
                old_range: (0.5, 1.5),
                new_range: (0.4, 1.6),
                expansion_rate: 0.05,
                stability_days: 7,
                reason: "expansion".to_string(),
                timestamp: now,
            },
            BoundsAdjustment::Contraction {
                parameter: "test_param".to_string(),
                old_range: (0.4, 1.6),
                new_range: (0.5, 1.5),
                contraction_rate: 0.25,
                rollback_count: 1,
                reason: "contraction".to_string(),
                timestamp: now,
            },
            BoundsAdjustment::Expansion {
                parameter: "test_param".to_string(),
                old_range: (0.5, 1.5),
                new_range: (0.4, 1.6),
                expansion_rate: 0.05,
                stability_days: 7,
                reason: "expansion".to_string(),
                timestamp: now,
            },
            BoundsAdjustment::Contraction {
                parameter: "test_param".to_string(),
                old_range: (0.4, 1.6),
                new_range: (0.5, 1.5),
                contraction_rate: 0.25,
                rollback_count: 2,
                reason: "contraction".to_string(),
                timestamp: now,
            },
        ];
        
        let adjustment_refs: Vec<&BoundsAdjustment> = adjustments.iter().collect();
        let oscillation_penalty = bounds.detect_oscillation_in_adjustments(&adjustment_refs);
        
        // Should detect oscillation (high alternation rate)
        assert!(oscillation_penalty > 0.5);
    }

    #[test]
    fn test_weekly_triage_report() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Add some adjustments
        let _ = bounds.expand_bounds_for_stable_parameters();
        
        // Generate weekly triage report
        let report = bounds.generate_weekly_triage_report();
        
        assert!(report.report_period.1 > report.report_period.0);
        assert!(!report.parameter_summaries.is_empty());
        
        // Check that report period is approximately one week
        let period_duration = report.report_period.1 - report.report_period.0;
        assert!(period_duration.num_days() >= 6 && period_duration.num_days() <= 8);
    }

    #[test]
    fn test_adjustment_history_export() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Add some adjustments
        let _ = bounds.expand_bounds_for_stable_parameters();
        
        // Export history with metadata
        let history = bounds.export_adjustment_history(true);
        
        assert!(!history.adjustments.is_empty());
        assert!(!history.parameter_metadata.is_empty());
        
        // Export history without metadata
        let history_no_metadata = bounds.export_adjustment_history(false);
        
        assert!(!history_no_metadata.adjustments.is_empty());
        assert!(history_no_metadata.parameter_metadata.is_empty());
    }

    #[test]
    fn test_parameter_adjustment_history_filtering() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        let two_weeks_ago = now - chrono::Duration::days(14);
        
        // Add some adjustments
        let _ = bounds.expand_bounds_for_stable_parameters();
        
        // Get history for specific parameter and time range
        let param_history = bounds.get_parameter_adjustment_history(
            "selection_temperature", 
            two_weeks_ago, 
            now
        );
        
        // Should contain adjustments for the specified parameter within the time range
        for adjustment in &param_history {
            match adjustment {
                BoundsAdjustment::Expansion { parameter, timestamp, .. } |
                BoundsAdjustment::Contraction { parameter, timestamp, .. } |
                BoundsAdjustment::AntiWindup { parameter, timestamp, .. } |
                BoundsAdjustment::Reset { parameter, timestamp, .. } => {
                    assert_eq!(parameter, "selection_temperature");
                    assert!(*timestamp >= two_weeks_ago && *timestamp <= now);
                }
            }
        }
    }

    #[test]
    fn test_history_cleanup() {
        let config = SlidingBoundsConfig::default();
        let mut bounds = SlidingBounds::new(config);
        
        // Add some adjustments
        let _ = bounds.expand_bounds_for_stable_parameters();
        let initial_count = bounds.get_adjustment_history().len();
        
        // Clean up history older than 30 days (should not remove recent adjustments)
        let removed_count = bounds.cleanup_old_history(30);
        
        assert_eq!(removed_count, 0); // No old adjustments to remove
        assert_eq!(bounds.get_adjustment_history().len(), initial_count);
        
        // Clean up history older than 0 days (should remove all adjustments)
        let removed_count = bounds.cleanup_old_history(0);
        
        assert!(removed_count > 0);
        assert!(bounds.get_adjustment_history().len() < initial_count);
    }
}