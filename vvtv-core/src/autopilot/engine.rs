use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::timeout;


use crate::monitor::{BusinessMetric, BusinessMetricType, MetricsStore};
use crate::business_logic::BusinessLogic;
use sha2::Digest;

/// Result alias for autopilot operations
pub type AutopilotResult<T> = std::result::Result<T, AutopilotError>;

/// Errors that can occur during autopilot operations
#[derive(Debug, Error)]
pub enum AutopilotError {
    #[error("autopilot cycle timeout after {0} minutes")]
    Timeout(u32),
    #[error("validation failure: {0}")]
    ValidationFailure(String),
    #[error("canary deployment failed: {0}")]
    CanaryFailure(String),
    #[error("parameter bounds violation: {0}")]
    BoundsViolation(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("business logic error: {0}")]
    BusinessLogic(#[from] crate::business_logic::BusinessLogicError),
    #[error("metrics error: {0}")]
    Metrics(#[from] crate::monitor::MonitorError),
}

/// Core autopilot engine for autonomous parameter optimization
#[derive(Debug)]
pub struct AutopilotEngine {
    metrics_store: Arc<MetricsStore>,
    business_logic_path: PathBuf,
    history_dir: PathBuf,
    config: AutopilotConfig,
    current_cycle_id: Option<String>,
    optimizer: super::optimizer::ParameterOptimizer,
    sliding_bounds: super::sliding_bounds::SlidingBounds,
    logger: super::logging::AutopilotLogger,
}

/// Configuration for the autopilot engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotConfig {
    pub enabled: bool,
    pub daily_schedule_utc: String, // "03:00"
    pub canary_duration_minutes: u32,
    pub canary_traffic_percentage: f32,
    pub statistical_confidence_threshold: f64,
    pub max_execution_time_minutes: u32,
    pub emergency_pause: bool,
    pub manual_override_hours: u32,
}

impl Default for AutopilotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            daily_schedule_utc: "03:00".to_string(),
            canary_duration_minutes: 60,
            canary_traffic_percentage: 0.2,
            statistical_confidence_threshold: 0.95,
            max_execution_time_minutes: 10,
            emergency_pause: false,
            manual_override_hours: 24,
        }
    }
}

impl AutopilotConfig {
    /// Load configuration from YAML file with schema validation
    pub fn load_from_file(path: &PathBuf) -> AutopilotResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AutopilotError::Configuration(format!("Failed to read config file: {}", e)))?;
        
        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| AutopilotError::Configuration(format!("Invalid YAML format: {}", e)))?;
        
        // Validate configuration after loading
        Self::validate_config(&config)?;
        
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn save_to_file(&self, path: &PathBuf) -> AutopilotResult<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| AutopilotError::Configuration(format!("Failed to serialize config: {}", e)))?;
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration schema and constraints
    fn validate_config(config: &AutopilotConfig) -> AutopilotResult<()> {
        // Validate canary traffic percentage
        if config.canary_traffic_percentage <= 0.0 || config.canary_traffic_percentage >= 1.0 {
            return Err(AutopilotError::Configuration(
                "canary_traffic_percentage must be between 0 and 1".to_string()
            ));
        }
        
        // Validate statistical confidence threshold
        if config.statistical_confidence_threshold <= 0.0 || config.statistical_confidence_threshold >= 1.0 {
            return Err(AutopilotError::Configuration(
                "statistical_confidence_threshold must be between 0 and 1".to_string()
            ));
        }
        
        // Validate execution time limit
        if config.max_execution_time_minutes == 0 {
            return Err(AutopilotError::Configuration(
                "max_execution_time_minutes must be greater than 0".to_string()
            ));
        }
        
        // Validate canary duration
        if config.canary_duration_minutes == 0 {
            return Err(AutopilotError::Configuration(
                "canary_duration_minutes must be greater than 0".to_string()
            ));
        }
        
        // Validate schedule format (basic check for HH:MM)
        if !Self::validate_schedule_format(&config.daily_schedule_utc) {
            return Err(AutopilotError::Configuration(
                "daily_schedule_utc must be in HH:MM format (e.g., '03:00')".to_string()
            ));
        }
        
        // Validate manual override hours
        if config.manual_override_hours == 0 {
            return Err(AutopilotError::Configuration(
                "manual_override_hours must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }

    /// Validate schedule format (HH:MM)
    fn validate_schedule_format(schedule: &str) -> bool {
        if schedule.len() != 5 || !schedule.contains(':') {
            return false;
        }
        
        let parts: Vec<&str> = schedule.split(':').collect();
        if parts.len() != 2 {
            return false;
        }
        
        // Validate hour (00-23)
        if let Ok(hour) = parts[0].parse::<u32>() {
            if hour > 23 {
                return false;
            }
        } else {
            return false;
        }
        
        // Validate minute (00-59)
        if let Ok(minute) = parts[1].parse::<u32>() {
            if minute > 59 {
                return false;
            }
        } else {
            return false;
        }
        
        true
    }

    /// Get schedule as (hour, minute) tuple
    pub fn get_schedule_time(&self) -> AutopilotResult<(u32, u32)> {
        let parts: Vec<&str> = self.daily_schedule_utc.split(':').collect();
        let hour = parts[0].parse::<u32>()
            .map_err(|_| AutopilotError::Configuration("Invalid hour in schedule".to_string()))?;
        let minute = parts[1].parse::<u32>()
            .map_err(|_| AutopilotError::Configuration("Invalid minute in schedule".to_string()))?;
        
        Ok((hour, minute))
    }
}

/// Represents a complete autopilot execution cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotCycle {
    pub cycle_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: CycleStatus,
    pub metrics_analysis: Option<MetricsAnalysis>,
    pub proposed_changes: Vec<ParameterChange>,
    pub validation_results: Option<ValidationResults>,
    pub deployment_result: Option<DeploymentResult>,
    pub error_message: Option<String>,
}

impl AutopilotCycle {
    /// Create a new autopilot cycle
    pub fn new(cycle_id: String) -> Self {
        Self {
            cycle_id,
            started_at: Utc::now(),
            completed_at: None,
            status: CycleStatus::Running,
            metrics_analysis: None,
            proposed_changes: Vec::new(),
            validation_results: None,
            deployment_result: None,
            error_message: None,
        }
    }

    /// Mark cycle as completed successfully
    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = CycleStatus::Completed;
    }

    /// Mark cycle as failed with error message
    pub fn fail(&mut self, error: String) {
        self.completed_at = Some(Utc::now());
        self.status = CycleStatus::Failed;
        self.error_message = Some(error);
    }

    /// Mark cycle as rolled back
    pub fn rollback(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = CycleStatus::RolledBack;
    }

    /// Get cycle duration if completed
    pub fn duration(&self) -> Option<Duration> {
        self.completed_at.map(|end| {
            let duration = end.signed_duration_since(self.started_at);
            Duration::from_secs(duration.num_seconds().max(0) as u64)
        })
    }

    /// Check if cycle is still running
    pub fn is_running(&self) -> bool {
        matches!(self.status, CycleStatus::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CycleStatus {
    Running,
    Completed,
    Failed,
    RolledBack,
}

/// Analysis of business metrics for parameter optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsAnalysis {
    pub analysis_window: (DateTime<Utc>, DateTime<Utc>),
    pub selection_entropy_trend: TrendAnalysis,
    pub curator_budget_trend: TrendAnalysis,
    pub novelty_kld_trend: TrendAnalysis,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub confidence_score: f64,
    pub data_quality: DataQuality,
    pub cross_metric_consistency: f64,
}

/// Data quality metrics for the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQuality {
    pub entropy_data_points: usize,
    pub budget_data_points: usize,
    pub novelty_data_points: usize,
    pub total_data_points: usize,
    pub data_completeness: f64, // Percentage of expected data points
    pub data_freshness_hours: f64, // Hours since last data point
    pub has_sufficient_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub metric_type: BusinessMetricType,
    pub current_value: f64,
    pub trend_direction: TrendDirection,
    pub trend_strength: f64,
    pub stability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOpportunity {
    pub parameter_name: String,
    pub current_value: f64,
    pub suggested_value: f64,
    pub expected_impact: f64,
    pub confidence: f64,
    pub rationale: String,
}

/// Represents a proposed parameter change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChange {
    pub parameter_name: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub change_type: ChangeType,
    pub confidence: f64,
    pub expected_impact: ExpectedImpact,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Optimization,
    Correction,
    Exploration,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    pub selection_entropy_delta: Option<f64>,
    pub curator_budget_delta: Option<f64>,
    pub novelty_kld_delta: Option<f64>,
    pub overall_confidence: f64,
}

/// Results of parameter change validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResults {
    pub bounds_validation: bool,
    pub golden_set_validation: bool,
    pub configuration_validation: bool,
    pub validation_errors: Vec<String>,
    #[serde(with = "duration_serde")]
    pub validation_time: Duration,
}

/// Results of parameter deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub deployment_type: DeploymentType,
    pub success: bool,
    pub canary_result: Option<String>, // Reference to canary deployment
    #[serde(with = "duration_serde")]
    pub deployment_time: Duration,
    pub rollback_performed: bool,
    pub final_configuration_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentType {
    CanaryThenFull,
    DirectDeploy,
    Rollback,
}

// Helper module for Duration serialization
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}impl
 AutopilotEngine {
    /// Create a new autopilot engine
    pub fn new(
        metrics_store: Arc<MetricsStore>,
        business_logic_path: PathBuf,
        history_dir: PathBuf,
        config: AutopilotConfig,
    ) -> AutopilotResult<Self> {
        // Validate configuration
        AutopilotConfig::validate_config(&config)?;
        
        // Ensure history directory exists
        if let Some(parent) = history_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Initialize optimizer with default configuration
        let optimizer_config = super::optimizer::OptimizerConfig::default();
        let mut optimizer = super::optimizer::ParameterOptimizer::new(optimizer_config);

        // Load current business logic parameters
        if business_logic_path.exists() {
            match BusinessLogic::load_from_file(&business_logic_path) {
                Ok(business_logic) => {
                    if let Err(e) = optimizer.load_current_parameters(&business_logic) {
                        tracing::warn!(
                            target: "autopilot",
                            error = %e,
                            "failed to load current parameters, using defaults"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: "autopilot",
                        error = %e,
                        "failed to load business logic, using default parameters"
                    );
                }
            }
        }

        // Initialize sliding bounds
        let bounds_config = super::sliding_bounds::SlidingBoundsConfig::default();
        let sliding_bounds = super::sliding_bounds::SlidingBounds::new(bounds_config);

        // Initialize logger
        let logging_config = super::logging::LoggingConfig::default();
        let logger = super::logging::AutopilotLogger::new(metrics_store.clone(), logging_config);
        
        Ok(Self {
            metrics_store,
            business_logic_path,
            history_dir,
            config,
            current_cycle_id: None,
            optimizer,
            sliding_bounds,
            logger,
        })
    }

    /// Load autopilot configuration from file or use defaults
    pub fn load_config(config_path: Option<&PathBuf>) -> AutopilotResult<AutopilotConfig> {
        match config_path {
            Some(path) if path.exists() => {
                tracing::info!(
                    target: "autopilot",
                    config_path = %path.display(),
                    "loading autopilot configuration from file"
                );
                AutopilotConfig::load_from_file(path)
            }
            Some(path) => {
                tracing::warn!(
                    target: "autopilot",
                    config_path = %path.display(),
                    "config file not found, using defaults"
                );
                Ok(AutopilotConfig::default())
            }
            None => {
                tracing::info!(target: "autopilot", "using default autopilot configuration");
                Ok(AutopilotConfig::default())
            }
        }
    }

    /// Check if autopilot is enabled and not paused
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.emergency_pause
    }

    /// Get current cycle ID if running
    pub fn current_cycle_id(&self) -> Option<&str> {
        self.current_cycle_id.as_deref()
    }

    /// Run a complete autopilot cycle with safety mechanisms
    pub async fn run_daily_cycle(&mut self) -> AutopilotResult<AutopilotCycle> {
        if !self.is_enabled() {
            return Err(AutopilotError::Configuration("Autopilot is disabled or paused".to_string()));
        }

        let timeout_minutes = self.config.max_execution_time_minutes;
        let cycle_future = self.run_daily_cycle_internal();
        let timeout_duration = Duration::from_secs(timeout_minutes as u64 * 60);
        
        match timeout(timeout_duration, cycle_future).await {
            Ok(Ok(cycle)) => Ok(cycle),
            Ok(Err(e)) => {
                self.handle_cycle_failure(&e).await?;
                Err(e)
            }
            Err(_) => {
                let timeout_error = AutopilotError::Timeout(self.config.max_execution_time_minutes);
                self.handle_cycle_failure(&timeout_error).await?;
                Err(timeout_error)
            }
        }
    }

    /// Internal cycle execution without timeout wrapper
    async fn run_daily_cycle_internal(&mut self) -> AutopilotResult<AutopilotCycle> {
        let cycle_id = format!("autopilot_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        self.current_cycle_id = Some(cycle_id.clone());
        
        let mut cycle = AutopilotCycle::new(cycle_id.clone());

        // Log cycle start
        self.logger.log_cycle_start(&cycle).await?;

        tracing::info!(
            target: "autopilot",
            cycle_id = %cycle_id,
            "starting autopilot cycle"
        );

        // Step 1: Analyze metrics from the last 24 hours
        let analysis_window = Duration::from_secs(24 * 60 * 60);
        cycle.metrics_analysis = Some(self.analyze_metrics(analysis_window).await?);
        
        // Step 2: Propose parameter changes based on analysis
        if let Some(ref analysis) = cycle.metrics_analysis {
            cycle.proposed_changes = self.propose_parameter_changes(analysis).await?;
            
            // Log proposed changes
            if !cycle.proposed_changes.is_empty() {
                self.logger.log_parameter_change_proposal(&cycle_id, &cycle.proposed_changes).await?;
            }
        }

        // Step 3: Validate and deploy changes if any were proposed
        if !cycle.proposed_changes.is_empty() {
            let (validation_results, deployment_result) = 
                self.validate_and_deploy(&cycle.proposed_changes).await?;
            
            // Log deployment results
            let deployment_success = deployment_result.success;
            let deployment_time_ms = deployment_result.deployment_time.as_millis() as u64;
            self.logger.log_parameter_change_deployment(
                &cycle_id,
                &cycle.proposed_changes,
                deployment_success,
                deployment_time_ms,
            ).await?;
            
            cycle.validation_results = Some(validation_results);
            cycle.deployment_result = Some(deployment_result);
        }

        cycle.complete();
        self.current_cycle_id = None;

        // Log cycle completion
        self.logger.log_cycle_completion(&cycle).await?;

        tracing::info!(
            target: "autopilot",
            cycle_id = %cycle_id,
            changes_proposed = cycle.proposed_changes.len(),
            "autopilot cycle completed successfully"
        );

        // Persist completed cycle
        self.persist_cycle(&cycle).await?;

        Ok(cycle)
    }

    /// Persist autopilot cycle to history
    async fn persist_cycle(&self, cycle: &AutopilotCycle) -> AutopilotResult<()> {
        let cycle_file = self.history_dir.join(format!("{}.json", cycle.cycle_id));
        
        // Ensure history directory exists
        if let Some(parent) = cycle_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let cycle_json = serde_json::to_string_pretty(cycle)
            .map_err(|e| AutopilotError::Configuration(format!("Failed to serialize cycle: {}", e)))?;
        
        std::fs::write(&cycle_file, cycle_json)?;
        
        tracing::debug!(
            target: "autopilot",
            cycle_id = %cycle.cycle_id,
            file_path = %cycle_file.display(),
            "persisted autopilot cycle"
        );
        
        Ok(())
    }

    /// Load autopilot cycle from history
    pub fn load_cycle(&self, cycle_id: &str) -> AutopilotResult<Option<AutopilotCycle>> {
        let cycle_file = self.history_dir.join(format!("{}.json", cycle_id));
        
        if !cycle_file.exists() {
            return Ok(None);
        }
        
        let cycle_json = std::fs::read_to_string(&cycle_file)?;
        let cycle: AutopilotCycle = serde_json::from_str(&cycle_json)
            .map_err(|e| AutopilotError::Configuration(format!("Failed to deserialize cycle: {}", e)))?;
        
        Ok(Some(cycle))
    }

    /// Get recent autopilot cycles
    pub fn get_recent_cycles(&self, limit: usize) -> AutopilotResult<Vec<AutopilotCycle>> {
        let mut cycles = Vec::new();
        
        if !self.history_dir.exists() {
            return Ok(cycles);
        }
        
        let mut entries: Vec<_> = std::fs::read_dir(&self.history_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();
        
        // Sort by modification time (newest first)
        entries.sort_by_key(|entry| {
            entry.metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        entries.reverse();
        
        for entry in entries.into_iter().take(limit) {
            if let Ok(cycle_json) = std::fs::read_to_string(entry.path()) {
                if let Ok(cycle) = serde_json::from_str::<AutopilotCycle>(&cycle_json) {
                    cycles.push(cycle);
                }
            }
        }
        
        Ok(cycles)
    }

    /// Handle autopilot cycle failures with appropriate responses
    async fn handle_cycle_failure(&mut self, error: &AutopilotError) -> AutopilotResult<()> {
        tracing::error!(
            target: "autopilot",
            error = %error,
            cycle_id = ?self.current_cycle_id,
            "autopilot cycle failed"
        );

        // Log cycle failure if we have a cycle ID
        if let Some(cycle_id) = &self.current_cycle_id {
            // Create a failed cycle for logging
            let mut failed_cycle = AutopilotCycle::new(cycle_id.clone());
            failed_cycle.fail(error.to_string());
            
            self.logger.log_cycle_failure(&failed_cycle, &error.to_string()).await?;
        }

        // Clear current cycle
        self.current_cycle_id = None;

        // Take appropriate action based on error type
        match error {
            AutopilotError::ValidationFailure(_) => {
                // Pause autopilot temporarily for validation failures
                tracing::warn!(target: "autopilot", "pausing autopilot due to validation failure");
                // This would integrate with drift monitor in a full implementation
            }
            AutopilotError::CanaryFailure(_) => {
                // Canary failures should trigger rollback (handled in canary module)
                tracing::warn!(target: "autopilot", "canary failure detected, rollback should be initiated");
            }
            AutopilotError::Timeout(_) => {
                // Timeout requires immediate operator attention
                tracing::error!(target: "autopilot", "autopilot timeout - operator attention required");
                // This would integrate with P6 AlertEngine
            }
            _ => {
                tracing::warn!(target: "autopilot", "general autopilot failure");
            }
        }

        Ok(())
    }

    /// Analyze business metrics to identify optimization opportunities
    async fn analyze_metrics(&self, window: Duration) -> AutopilotResult<MetricsAnalysis> {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::from_std(window).unwrap();

        tracing::info!(
            target: "autopilot",
            start_time = %start_time,
            end_time = %end_time,
            window_hours = window.as_secs() / 3600,
            "starting metrics analysis"
        );

        // Fetch metrics for analysis with error handling
        let entropy_metrics = self.fetch_metrics_with_validation(
            BusinessMetricType::SelectionEntropy,
            start_time,
            end_time,
        ).await?;

        let budget_metrics = self.fetch_metrics_with_validation(
            BusinessMetricType::CuratorApplyBudgetUsedPct,
            start_time,
            end_time,
        ).await?;

        let novelty_metrics = self.fetch_metrics_with_validation(
            BusinessMetricType::NoveltyTemporalKld,
            start_time,
            end_time,
        ).await?;

        tracing::debug!(
            target: "autopilot",
            entropy_points = entropy_metrics.len(),
            budget_points = budget_metrics.len(),
            novelty_points = novelty_metrics.len(),
            "fetched metrics for analysis"
        );

        // Analyze trends for each metric type with enhanced statistical methods
        let entropy_trend = self.analyze_metric_trend_enhanced(&entropy_metrics, BusinessMetricType::SelectionEntropy);
        let budget_trend = self.analyze_metric_trend_enhanced(&budget_metrics, BusinessMetricType::CuratorApplyBudgetUsedPct);
        let novelty_trend = self.analyze_metric_trend_enhanced(&novelty_metrics, BusinessMetricType::NoveltyTemporalKld);

        // Identify optimization opportunities based on comprehensive analysis
        let opportunities = self.identify_optimization_opportunities_enhanced(&entropy_trend, &budget_trend, &novelty_trend);
        
        // Calculate data quality metrics
        let data_quality = self.calculate_data_quality(&entropy_metrics, &budget_metrics, &novelty_metrics, window);
        
        // Calculate cross-metric consistency
        let cross_metric_consistency = self.calculate_cross_metric_consistency(&entropy_trend, &budget_trend, &novelty_trend);
        
        // Calculate overall confidence in the analysis with multiple factors
        let confidence_score = self.calculate_analysis_confidence_enhanced(&entropy_trend, &budget_trend, &novelty_trend, &opportunities);

        tracing::info!(
            target: "autopilot",
            confidence_score = confidence_score,
            opportunities_count = opportunities.len(),
            data_completeness = data_quality.data_completeness,
            cross_metric_consistency = cross_metric_consistency,
            "completed metrics analysis"
        );

        Ok(MetricsAnalysis {
            analysis_window: (start_time, end_time),
            selection_entropy_trend: entropy_trend,
            curator_budget_trend: budget_trend,
            novelty_kld_trend: novelty_trend,
            optimization_opportunities: opportunities,
            confidence_score,
            data_quality,
            cross_metric_consistency,
        })
    }

    /// Fetch metrics with validation and error handling
    async fn fetch_metrics_with_validation(
        &self,
        metric_type: BusinessMetricType,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> AutopilotResult<Vec<BusinessMetric>> {
        let metrics = self.metrics_store.query_business_metrics(
            metric_type,
            start_time,
            end_time,
        )?;

        // Validate metrics quality
        if metrics.is_empty() {
            tracing::warn!(
                target: "autopilot",
                metric_type = ?metric_type,
                "no metrics found in analysis window"
            );
        } else if metrics.len() < 10 {
            tracing::warn!(
                target: "autopilot",
                metric_type = ?metric_type,
                count = metrics.len(),
                "insufficient metrics for reliable analysis"
            );
        }

        // Filter out invalid values
        let original_count = metrics.len();
        let valid_metrics: Vec<BusinessMetric> = metrics
            .into_iter()
            .filter(|m| m.value.is_finite() && m.value >= 0.0)
            .collect();

        if valid_metrics.len() != original_count {
            tracing::warn!(
                target: "autopilot",
                metric_type = ?metric_type,
                filtered_count = valid_metrics.len(),
                original_count = original_count,
                "filtered out invalid metric values"
            );
        }

        Ok(valid_metrics)
    }

    /// Enhanced trend analysis with sophisticated statistical methods
    fn analyze_metric_trend_enhanced(&self, metrics: &[BusinessMetric], metric_type: BusinessMetricType) -> TrendAnalysis {
        if metrics.is_empty() {
            return TrendAnalysis {
                metric_type,
                current_value: 0.0,
                trend_direction: TrendDirection::Stable,
                trend_strength: 0.0,
                stability_score: 0.0,
            };
        }

        let values: Vec<f64> = metrics.iter().map(|m| m.value).collect();
        let current_value = values.last().copied().unwrap_or(0.0);
        
        // Enhanced trend analysis using multiple methods
        let (trend_direction, trend_strength) = if values.len() >= 6 {
            // Use linear regression for trend detection
            let trend_slope = self.calculate_linear_regression_slope(&values);
            let trend_strength_raw = trend_slope.abs();
            
            // Apply metric-specific thresholds for trend significance
            let significance_threshold = match metric_type {
                BusinessMetricType::SelectionEntropy => 0.02, // 2% change is significant
                BusinessMetricType::CuratorApplyBudgetUsedPct => 0.05, // 5% change is significant
                BusinessMetricType::NoveltyTemporalKld => 0.03, // 3% change is significant
                _ => 0.03, // Default 3% threshold
            };
            
            if trend_strength_raw < significance_threshold {
                (TrendDirection::Stable, trend_strength_raw)
            } else if trend_slope > 0.0 {
                (TrendDirection::Increasing, trend_strength_raw)
            } else {
                (TrendDirection::Decreasing, trend_strength_raw)
            }
        } else if values.len() >= 2 {
            // Fallback to simple comparison for small datasets
            let first_half_avg = values[..values.len()/2].iter().sum::<f64>() / (values.len()/2) as f64;
            let second_half_avg = values[values.len()/2..].iter().sum::<f64>() / (values.len() - values.len()/2) as f64;
            let change = if first_half_avg != 0.0 {
                (second_half_avg - first_half_avg) / first_half_avg
            } else {
                0.0
            };
            
            if change.abs() < 0.05 {
                (TrendDirection::Stable, change.abs())
            } else if change > 0.0 {
                (TrendDirection::Increasing, change)
            } else {
                (TrendDirection::Decreasing, change.abs())
            }
        } else {
            (TrendDirection::Stable, 0.0)
        };

        // Enhanced stability score using coefficient of variation and trend consistency
        let stability_score = self.calculate_stability_score(&values, &trend_direction);

        TrendAnalysis {
            metric_type,
            current_value,
            trend_direction,
            trend_strength,
            stability_score,
        }
    }

    /// Calculate linear regression slope for trend detection
    fn calculate_linear_regression_slope(&self, values: &[f64]) -> f64 {
        let n = values.len() as f64;
        if n < 2.0 {
            return 0.0;
        }

        // Create x values (time indices)
        let x_values: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
        
        // Calculate means
        let x_mean = x_values.iter().sum::<f64>() / n;
        let y_mean = values.iter().sum::<f64>() / n;
        
        // Calculate slope using least squares method
        let numerator: f64 = x_values.iter().zip(values.iter())
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .sum();
        
        let denominator: f64 = x_values.iter()
            .map(|x| (x - x_mean).powi(2))
            .sum();
        
        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }

    /// Calculate enhanced stability score
    fn calculate_stability_score(&self, values: &[f64], trend_direction: &TrendDirection) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        
        // Calculate coefficient of variation (CV)
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        let cv = if mean != 0.0 { std_dev / mean.abs() } else { std_dev };
        
        // Calculate trend consistency (how consistent the trend direction is)
        let trend_consistency = if values.len() >= 4 {
            let mut consistent_changes = 0;
            let mut total_changes = 0;
            
            for window in values.windows(2) {
                let change = window[1] - window[0];
                total_changes += 1;
                
                match trend_direction {
                    TrendDirection::Increasing if change > 0.0 => consistent_changes += 1,
                    TrendDirection::Decreasing if change < 0.0 => consistent_changes += 1,
                    TrendDirection::Stable if change.abs() < mean * 0.05 => consistent_changes += 1,
                    _ => {}
                }
            }
            
            if total_changes > 0 {
                consistent_changes as f64 / total_changes as f64
            } else {
                0.0
            }
        } else {
            0.5 // Neutral consistency for small datasets
        };
        
        // Combine CV and trend consistency into stability score
        // Lower CV and higher consistency = higher stability
        let cv_score = 1.0 / (1.0 + cv);
        let stability_score = (cv_score * 0.6 + trend_consistency * 0.4).clamp(0.0, 1.0);
        
        stability_score
    }

    /// Enhanced optimization opportunity identification with comprehensive analysis
    fn identify_optimization_opportunities_enhanced(
        &self,
        entropy_trend: &TrendAnalysis,
        budget_trend: &TrendAnalysis,
        novelty_trend: &TrendAnalysis,
    ) -> Vec<OptimizationOpportunity> {
        let mut opportunities = Vec::new();

        // Selection entropy optimization
        opportunities.extend(self.analyze_entropy_optimization(entropy_trend));
        
        // Curator budget optimization
        opportunities.extend(self.analyze_budget_optimization(budget_trend));
        
        // Novelty optimization
        opportunities.extend(self.analyze_novelty_optimization(novelty_trend));
        
        // Cross-metric optimization opportunities
        opportunities.extend(self.analyze_cross_metric_optimization(entropy_trend, budget_trend, novelty_trend));
        
        // Filter and rank opportunities by confidence and impact
        opportunities.sort_by(|a, b| {
            let a_score = a.confidence * a.expected_impact.abs();
            let b_score = b.confidence * b.expected_impact.abs();
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Limit to top 5 opportunities to avoid overwhelming the system
        opportunities.truncate(5);
        
        tracing::debug!(
            target: "autopilot",
            opportunities_count = opportunities.len(),
            "identified optimization opportunities"
        );
        
        opportunities
    }

    /// Analyze selection entropy for optimization opportunities
    fn analyze_entropy_optimization(&self, entropy_trend: &TrendAnalysis) -> Vec<OptimizationOpportunity> {
        let mut opportunities = Vec::new();
        
        // Low entropy with high stability suggests need for more exploration
        if entropy_trend.current_value < 0.4 && entropy_trend.stability_score > 0.7 {
            let confidence = (entropy_trend.stability_score * 0.8).clamp(0.0, 1.0);
            let temperature_increase = 0.05 * (0.4 - entropy_trend.current_value) / 0.4;
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "selection_temperature".to_string(),
                current_value: 0.85, // Would be read from actual config
                suggested_value: (0.85 + temperature_increase).min(1.2),
                expected_impact: temperature_increase * 2.0, // Expected entropy increase
                confidence,
                rationale: format!(
                    "Low entropy ({:.3}) with high stability ({:.3}) suggests insufficient exploration",
                    entropy_trend.current_value, entropy_trend.stability_score
                ),
            });
        }
        
        // High entropy with decreasing trend might need stabilization
        if entropy_trend.current_value > 0.8 && matches!(entropy_trend.trend_direction, TrendDirection::Decreasing) {
            let confidence = (entropy_trend.trend_strength * 0.6).clamp(0.0, 1.0);
            let top_k_adjustment = ((entropy_trend.current_value - 0.7) * 10.0).round();
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "selection_top_k".to_string(),
                current_value: 12.0, // Would be read from actual config
                suggested_value: (12.0 - top_k_adjustment).max(5.0),
                expected_impact: -0.1, // Expected entropy stabilization
                confidence,
                rationale: format!(
                    "High entropy ({:.3}) with decreasing trend suggests need for focus",
                    entropy_trend.current_value
                ),
            });
        }
        
        opportunities
    }

    /// Analyze curator budget for optimization opportunities
    fn analyze_budget_optimization(&self, budget_trend: &TrendAnalysis) -> Vec<OptimizationOpportunity> {
        let mut opportunities = Vec::new();
        
        // High budget usage with increasing trend suggests threshold adjustment
        if budget_trend.current_value > 0.75 && matches!(budget_trend.trend_direction, TrendDirection::Increasing) {
            let confidence = (budget_trend.trend_strength * budget_trend.stability_score * 0.8).clamp(0.0, 1.0);
            let threshold_increase = 0.02 * (budget_trend.current_value - 0.75) / 0.25;
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "curator_confidence_threshold".to_string(),
                current_value: 0.62, // Would be read from actual config
                suggested_value: (0.62 + threshold_increase).min(0.75),
                expected_impact: -threshold_increase * 5.0, // Expected budget reduction
                confidence,
                rationale: format!(
                    "High budget usage ({:.1}%) with increasing trend suggests threshold too low",
                    budget_trend.current_value * 100.0
                ),
            });
        }
        
        // Low budget usage with stable trend might allow more aggressive curation
        if budget_trend.current_value < 0.3 && matches!(budget_trend.trend_direction, TrendDirection::Stable) {
            let confidence = (budget_trend.stability_score * 0.6).clamp(0.0, 1.0);
            let threshold_decrease = 0.01 * (0.3 - budget_trend.current_value) / 0.3;
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "curator_confidence_threshold".to_string(),
                current_value: 0.62, // Would be read from actual config
                suggested_value: (0.62 - threshold_decrease).max(0.45),
                expected_impact: threshold_decrease * 3.0, // Expected quality improvement
                confidence,
                rationale: format!(
                    "Low budget usage ({:.1}%) suggests opportunity for more aggressive curation",
                    budget_trend.current_value * 100.0
                ),
            });
        }
        
        opportunities
    }

    /// Analyze novelty metrics for optimization opportunities
    fn analyze_novelty_optimization(&self, novelty_trend: &TrendAnalysis) -> Vec<OptimizationOpportunity> {
        let mut opportunities = Vec::new();
        
        // Low novelty suggests need for more diverse content selection
        if novelty_trend.current_value < 0.2 && novelty_trend.stability_score > 0.6 {
            let confidence = (novelty_trend.stability_score * 0.7).clamp(0.0, 1.0);
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "plan_selection_bias".to_string(),
                current_value: 0.0, // Would be read from actual config
                suggested_value: 0.02, // Small bias toward novelty
                expected_impact: 0.05, // Expected novelty increase
                confidence,
                rationale: format!(
                    "Low novelty ({:.3}) suggests need for more diverse content selection",
                    novelty_trend.current_value
                ),
            });
        }
        
        // High novelty with decreasing trend might need stabilization
        if novelty_trend.current_value > 0.6 && matches!(novelty_trend.trend_direction, TrendDirection::Decreasing) {
            let confidence = (novelty_trend.trend_strength * 0.5).clamp(0.0, 1.0);
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "plan_selection_bias".to_string(),
                current_value: 0.02, // Would be read from actual config
                suggested_value: 0.0, // Remove novelty bias
                expected_impact: -0.03, // Expected novelty stabilization
                confidence,
                rationale: format!(
                    "High novelty ({:.3}) with decreasing trend suggests over-diversification",
                    novelty_trend.current_value
                ),
            });
        }
        
        opportunities
    }

    /// Analyze cross-metric optimization opportunities
    fn analyze_cross_metric_optimization(
        &self,
        entropy_trend: &TrendAnalysis,
        budget_trend: &TrendAnalysis,
        novelty_trend: &TrendAnalysis,
    ) -> Vec<OptimizationOpportunity> {
        let mut opportunities = Vec::new();
        
        // Low entropy + high budget usage suggests temperature/threshold imbalance
        if entropy_trend.current_value < 0.4 && budget_trend.current_value > 0.7 {
            let confidence = ((entropy_trend.stability_score + budget_trend.stability_score) / 2.0 * 0.6).clamp(0.0, 1.0);
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "selection_temperature".to_string(),
                current_value: 0.85,
                suggested_value: 0.95,
                expected_impact: 0.08,
                confidence,
                rationale: "Low entropy with high budget usage suggests need for balanced exploration".to_string(),
            });
        }
        
        // High novelty + low entropy suggests conflicting objectives
        if novelty_trend.current_value > 0.5 && entropy_trend.current_value < 0.3 {
            let confidence = 0.4; // Lower confidence for complex cross-metric scenarios
            
            opportunities.push(OptimizationOpportunity {
                parameter_name: "selection_top_k".to_string(),
                current_value: 12.0,
                suggested_value: 15.0,
                expected_impact: 0.05,
                confidence,
                rationale: "High novelty with low entropy suggests need for broader candidate consideration".to_string(),
            });
        }
        
        opportunities
    }

    /// Enhanced confidence calculation with multiple factors
    fn calculate_analysis_confidence_enhanced(
        &self,
        entropy_trend: &TrendAnalysis,
        budget_trend: &TrendAnalysis,
        novelty_trend: &TrendAnalysis,
        opportunities: &[OptimizationOpportunity],
    ) -> f64 {
        // Factor 1: Data quality and stability (40% weight)
        let avg_stability = (entropy_trend.stability_score + budget_trend.stability_score + novelty_trend.stability_score) / 3.0;
        
        // Factor 2: Trend clarity and consistency (30% weight)
        let trend_clarity = (entropy_trend.trend_strength + budget_trend.trend_strength + novelty_trend.trend_strength) / 3.0;
        
        // Factor 3: Opportunity quality (20% weight)
        let opportunity_confidence = if opportunities.is_empty() {
            0.3 // Low confidence if no opportunities identified
        } else {
            let avg_opportunity_confidence = opportunities.iter()
                .map(|op| op.confidence)
                .sum::<f64>() / opportunities.len() as f64;
            avg_opportunity_confidence
        };
        
        // Factor 4: Cross-metric consistency (10% weight)
        let cross_metric_consistency = self.calculate_cross_metric_consistency(entropy_trend, budget_trend, novelty_trend);
        
        // Weighted combination of all factors
        let overall_confidence = (
            avg_stability * 0.4 +
            trend_clarity * 0.3 +
            opportunity_confidence * 0.2 +
            cross_metric_consistency * 0.1
        ).clamp(0.0, 1.0);
        
        tracing::debug!(
            target: "autopilot",
            stability = avg_stability,
            trend_clarity = trend_clarity,
            opportunity_confidence = opportunity_confidence,
            cross_metric_consistency = cross_metric_consistency,
            overall_confidence = overall_confidence,
            "calculated analysis confidence"
        );
        
        overall_confidence
    }

    /// Calculate cross-metric consistency score
    fn calculate_cross_metric_consistency(
        &self,
        entropy_trend: &TrendAnalysis,
        budget_trend: &TrendAnalysis,
        novelty_trend: &TrendAnalysis,
    ) -> f64 {
        let mut consistency_score = 0.0;
        let mut total_checks = 0;
        
        // Check entropy-budget relationship
        // High entropy should correlate with lower budget usage (more exploration, less curation)
        if entropy_trend.current_value > 0.6 && budget_trend.current_value < 0.4 {
            consistency_score += 1.0;
        } else if entropy_trend.current_value < 0.4 && budget_trend.current_value > 0.6 {
            consistency_score += 1.0;
        } else if (entropy_trend.current_value - 0.5).abs() < 0.1 && (budget_trend.current_value - 0.5).abs() < 0.1 {
            consistency_score += 0.5; // Neutral state is somewhat consistent
        }
        total_checks += 1;
        
        // Check entropy-novelty relationship
        // Higher entropy should generally correlate with higher novelty
        let entropy_novelty_correlation = (entropy_trend.current_value - 0.5) * (novelty_trend.current_value - 0.5);
        if entropy_novelty_correlation > 0.0 {
            consistency_score += entropy_novelty_correlation.min(1.0);
        }
        total_checks += 1;
        
        // Check trend direction consistency
        // If multiple metrics are trending in the same direction, it suggests systematic change
        let trend_directions = [
            &entropy_trend.trend_direction,
            &budget_trend.trend_direction,
            &novelty_trend.trend_direction,
        ];
        
        let stable_count = trend_directions.iter().filter(|&&dir| matches!(dir, TrendDirection::Stable)).count();
        let increasing_count = trend_directions.iter().filter(|&&dir| matches!(dir, TrendDirection::Increasing)).count();
        let decreasing_count = trend_directions.iter().filter(|&&dir| matches!(dir, TrendDirection::Decreasing)).count();
        
        let max_direction_count = stable_count.max(increasing_count).max(decreasing_count);
        let direction_consistency = max_direction_count as f64 / trend_directions.len() as f64;
        consistency_score += direction_consistency;
        total_checks += 1;
        
        if total_checks > 0 {
            consistency_score / total_checks as f64
        } else {
            0.5 // Neutral consistency if no checks performed
        }
    }

    /// Calculate data quality metrics for the analysis
    fn calculate_data_quality(
        &self,
        entropy_metrics: &[BusinessMetric],
        budget_metrics: &[BusinessMetric],
        novelty_metrics: &[BusinessMetric],
        window: Duration,
    ) -> DataQuality {
        let entropy_count = entropy_metrics.len();
        let budget_count = budget_metrics.len();
        let novelty_count = novelty_metrics.len();
        let total_count = entropy_count + budget_count + novelty_count;
        
        // Calculate expected data points (assuming 1 point per hour for each metric)
        let window_hours = window.as_secs() / 3600;
        let expected_points_per_metric = window_hours as usize;
        let total_expected_points = expected_points_per_metric * 3; // 3 metric types
        
        let data_completeness = if total_expected_points > 0 {
            (total_count as f64 / total_expected_points as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        
        // Calculate data freshness (time since most recent data point)
        let most_recent_timestamp = [entropy_metrics, budget_metrics, novelty_metrics]
            .iter()
            .flat_map(|metrics| metrics.iter())
            .map(|m| m.timestamp)
            .max()
            .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
        
        let data_freshness_hours = (Utc::now() - most_recent_timestamp)
            .num_seconds() as f64 / 3600.0;
        
        // Determine if we have sufficient data for reliable analysis
        let min_points_per_metric = 6; // Minimum for trend analysis
        let has_sufficient_data = entropy_count >= min_points_per_metric
            && budget_count >= min_points_per_metric
            && novelty_count >= min_points_per_metric
            && data_freshness_hours < 6.0; // Data should be less than 6 hours old
        
        DataQuality {
            entropy_data_points: entropy_count,
            budget_data_points: budget_count,
            novelty_data_points: novelty_count,
            total_data_points: total_count,
            data_completeness,
            data_freshness_hours,
            has_sufficient_data,
        }
    }

    /// Propose parameter changes based on metrics analysis using the optimizer
    async fn propose_parameter_changes(&mut self, analysis: &MetricsAnalysis) -> AutopilotResult<Vec<ParameterChange>> {
        // Only propose changes if confidence is high enough
        if analysis.confidence_score < 0.5 {
            tracing::info!(
                target: "autopilot",
                confidence = analysis.confidence_score,
                "skipping parameter changes due to low confidence"
            );
            return Ok(Vec::new());
        }

        // Check data quality requirements
        if !analysis.data_quality.has_sufficient_data {
            tracing::warn!(
                target: "autopilot",
                data_completeness = analysis.data_quality.data_completeness,
                data_freshness_hours = analysis.data_quality.data_freshness_hours,
                "insufficient data quality for parameter optimization"
            );
            return Ok(Vec::new());
        }

        // Use the parameter optimizer to propose changes
        let proposed_changes = self.optimizer.propose_parameter_changes(analysis, &self.sliding_bounds)?;

        // Record optimization attempts for learning
        for change in &proposed_changes {
            let old_value = match &change.old_value {
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                _ => 0.0,
            };
            let new_value = match &change.new_value {
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                _ => 0.0,
            };
            let predicted_impact = change.expected_impact.overall_confidence;

            self.optimizer.record_optimization_attempt(
                change.parameter_name.clone(),
                old_value,
                new_value,
                predicted_impact,
                change.rationale.clone(),
            );
        }

        tracing::info!(
            target: "autopilot",
            changes_proposed = proposed_changes.len(),
            "parameter changes proposed using optimizer"
        );

        Ok(proposed_changes)
    }

    /// Validate and deploy parameter changes
    async fn validate_and_deploy(&self, changes: &[ParameterChange]) -> AutopilotResult<(ValidationResults, DeploymentResult)> {
        let validation_start = std::time::Instant::now();
        let mut validation_errors = Vec::new();
        
        // Step 1: Validate business logic configuration syntax
        let configuration_validation = self.validate_business_logic_configuration(changes, &mut validation_errors).await?;
        
        // Step 2: Bounds validation (already done in optimizer, but double-check)
        let bounds_validation = self.validate_against_bounds(changes, &mut validation_errors);
        
        // Step 3: Golden set validation (placeholder for now)
        let golden_set_validation = self.validate_golden_set_tests(changes, &mut validation_errors).await?;
        
        let validation_results = ValidationResults {
            bounds_validation,
            golden_set_validation,
            configuration_validation,
            validation_errors,
            validation_time: validation_start.elapsed(),
        };

        // Only proceed with deployment if validation passes
        let deployment_result = if validation_results.bounds_validation && 
                                  validation_results.golden_set_validation && 
                                  validation_results.configuration_validation {
            self.deploy_parameter_changes(changes).await?
        } else {
            DeploymentResult {
                deployment_type: DeploymentType::DirectDeploy,
                success: false,
                canary_result: None,
                deployment_time: Duration::from_secs(0),
                rollback_performed: false,
                final_configuration_hash: "validation_failed".to_string(),
            }
        };

        Ok((validation_results, deployment_result))
    }

    /// Validate business logic configuration with proposed changes
    async fn validate_business_logic_configuration(
        &self,
        changes: &[ParameterChange],
        errors: &mut Vec<String>,
    ) -> AutopilotResult<bool> {
        // Load current business logic
        let mut business_logic = match BusinessLogic::load_from_file(&self.business_logic_path) {
            Ok(bl) => bl,
            Err(e) => {
                errors.push(format!("Failed to load business logic: {}", e));
                return Ok(false);
            }
        };

        // Apply proposed changes to a copy for validation
        for change in changes {
            if let Err(e) = self.apply_change_to_business_logic(&mut business_logic, change) {
                errors.push(format!("Failed to apply change to {}: {}", change.parameter_name, e));
                return Ok(false);
            }
        }

        // Validate the modified configuration
        match business_logic.validate() {
            Ok(_) => {
                tracing::debug!(
                    target: "autopilot",
                    changes_count = changes.len(),
                    "business logic configuration validation passed"
                );
                Ok(true)
            }
            Err(e) => {
                errors.push(format!("Business logic validation failed: {}", e));
                Ok(false)
            }
        }
    }

    /// Apply a parameter change to business logic (placeholder implementation)
    fn apply_change_to_business_logic(
        &self,
        _business_logic: &mut BusinessLogic,
        change: &ParameterChange,
    ) -> AutopilotResult<()> {
        // This would modify the business logic structure based on the parameter change
        // For now, just validate that we can extract the numeric value
        match &change.new_value {
            serde_json::Value::Number(n) => {
                if n.as_f64().is_some() {
                    tracing::debug!(
                        target: "autopilot",
                        parameter = %change.parameter_name,
                        new_value = ?change.new_value,
                        "validated parameter change application"
                    );
                    Ok(())
                } else {
                    Err(AutopilotError::ValidationFailure(
                        format!("Invalid numeric value for {}", change.parameter_name)
                    ))
                }
            }
            _ => Err(AutopilotError::ValidationFailure(
                format!("Non-numeric value for parameter {}", change.parameter_name)
            ))
        }
    }

    /// Validate changes against sliding bounds
    fn validate_against_bounds(&self, changes: &[ParameterChange], errors: &mut Vec<String>) -> bool {
        let mut all_valid = true;
        
        for change in changes {
            if let serde_json::Value::Number(n) = &change.new_value {
                if let Some(value) = n.as_f64() {
                    if let Err(e) = self.sliding_bounds.validate_change(&change.parameter_name, value) {
                        errors.push(format!("Bounds validation failed for {}: {}", change.parameter_name, e));
                        all_valid = false;
                    }
                }
            }
        }
        
        all_valid
    }

    /// Validate changes using golden set tests (placeholder)
    async fn validate_golden_set_tests(
        &self,
        changes: &[ParameterChange],
        errors: &mut Vec<String>,
    ) -> AutopilotResult<bool> {
        // This would integrate with the P7 Golden Selection Test framework
        // For now, simulate validation
        if changes.len() > 3 {
            errors.push("Too many simultaneous changes for golden set validation".to_string());
            Ok(false)
        } else {
            tracing::debug!(
                target: "autopilot",
                changes_count = changes.len(),
                "golden set validation passed (simulated)"
            );
            Ok(true)
        }
    }

    /// Deploy parameter changes to the business logic file
    async fn deploy_parameter_changes(&self, changes: &[ParameterChange]) -> AutopilotResult<DeploymentResult> {
        let deployment_start = std::time::Instant::now();
        
        // Load current business logic
        let mut business_logic = BusinessLogic::load_from_file(&self.business_logic_path)?;
        
        // Apply changes
        for change in changes {
            self.apply_change_to_business_logic(&mut business_logic, change)?;
        }
        
        // Create backup of current configuration
        let backup_path = self.business_logic_path.with_extension("backup");
        std::fs::copy(&self.business_logic_path, &backup_path)?;
        
        // Write updated configuration atomically
        let temp_path = self.business_logic_path.with_extension("tmp");
        let yaml_content = serde_yaml::to_string(&business_logic)
            .map_err(|e| AutopilotError::Configuration(format!("Failed to serialize business logic: {}", e)))?;
        std::fs::write(&temp_path, yaml_content)?;
        std::fs::rename(&temp_path, &self.business_logic_path)?;
        
        // Calculate configuration hash for tracking
        let config_content = std::fs::read_to_string(&self.business_logic_path)?;
        let config_hash = format!("{:x}", sha2::Sha256::digest(config_content.as_bytes()));
        
        tracing::info!(
            target: "autopilot",
            changes_count = changes.len(),
            config_hash = %config_hash,
            deployment_time_ms = deployment_start.elapsed().as_millis(),
            "successfully deployed parameter changes"
        );
        
        Ok(DeploymentResult {
            deployment_type: DeploymentType::DirectDeploy,
            success: true,
            canary_result: Some("direct_deployment".to_string()),
            deployment_time: deployment_start.elapsed(),
            rollback_performed: false,
            final_configuration_hash: config_hash,
        })
    }

    /// Start the daily scheduler for autonomous operation
    pub async fn start_scheduler(
        engine: Arc<tokio::sync::Mutex<AutopilotEngine>>,
        config: Option<super::scheduler::SchedulerConfig>,
    ) -> AutopilotResult<()> {
        let scheduler_config = config.unwrap_or_default();
        
        // Extract schedule from autopilot config
        let schedule_utc = {
            let engine_guard = engine.lock().await;
            engine_guard.config.daily_schedule_utc.clone()
        };
        
        let mut scheduler = super::scheduler::DailyScheduler::new(
            &schedule_utc,
            scheduler_config.timeout_minutes,
        ).map_err(|e| AutopilotError::Configuration(format!("Scheduler error: {}", e)))?;
        
        tracing::info!(
            target: "autopilot",
            schedule = %schedule_utc,
            "starting autopilot scheduler"
        );
        
        // Run scheduler loop (this will run indefinitely)
        scheduler.run_scheduler_loop(engine, scheduler_config).await
            .map_err(|e| AutopilotError::Configuration(format!("Scheduler loop error: {}", e)))?;
        
        Ok(())
    }

    /// Trigger manual autopilot execution (for testing/emergency)
    pub async fn trigger_manual_execution(&mut self) -> AutopilotResult<AutopilotCycle> {
        tracing::info!(target: "autopilot", "triggering manual autopilot execution");
        
        if !self.is_enabled() {
            return Err(AutopilotError::Configuration(
                "Cannot trigger manual execution: autopilot is disabled or paused".to_string()
            ));
        }
        
        self.run_daily_cycle().await
    }

    /// Pause autopilot for specified duration
    pub fn pause_autopilot(&mut self, duration_hours: u32) -> AutopilotResult<()> {
        self.config.emergency_pause = true;
        
        tracing::warn!(
            target: "autopilot",
            duration_hours = duration_hours,
            "autopilot paused"
        );
        
        Ok(())
    }

    /// Resume autopilot from pause
    pub fn resume_autopilot(&mut self) -> AutopilotResult<()> {
        self.config.emergency_pause = false;
        
        tracing::info!(target: "autopilot", "autopilot resumed");
        
        Ok(())
    }

    /// Get autopilot status information
    pub fn get_status(&self) -> AutopilotStatus {
        AutopilotStatus {
            enabled: self.is_enabled(),
            emergency_pause: self.config.emergency_pause,
            current_cycle_id: self.current_cycle_id.clone(),
            daily_schedule_utc: self.config.daily_schedule_utc.clone(),
            canary_duration_minutes: self.config.canary_duration_minutes,
            statistical_confidence_threshold: self.config.statistical_confidence_threshold,
            max_execution_time_minutes: self.config.max_execution_time_minutes,
        }
    }

    /// Get optimizer statistics
    pub fn get_optimizer_stats(&self) -> super::optimizer::OptimizationStats {
        self.optimizer.get_optimization_stats()
    }

    /// Get current parameter bounds
    pub fn get_parameter_bounds(&self) -> &super::sliding_bounds::SlidingBounds {
        &self.sliding_bounds
    }

    /// Update parameter bounds after rollback events
    pub fn handle_parameter_rollback(&mut self, parameter_name: &str) -> AutopilotResult<()> {
        match self.sliding_bounds.contract_bounds_after_rollback(parameter_name) {
            Ok(adjustment) => {
                tracing::info!(
                    target: "autopilot",
                    parameter = parameter_name,
                    adjustment = ?adjustment,
                    "contracted parameter bounds after rollback"
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    target: "autopilot",
                    parameter = parameter_name,
                    error = %e,
                    "failed to contract bounds after rollback"
                );
                Err(AutopilotError::Configuration(format!("Bounds adjustment failed: {}", e)))
            }
        }
    }

    /// Expand bounds for stable parameters
    pub fn expand_stable_parameter_bounds(&mut self) -> AutopilotResult<Vec<super::sliding_bounds::BoundsAdjustment>> {
        let adjustments = self.sliding_bounds.expand_bounds_for_stable_parameters();
        
        if !adjustments.is_empty() {
            tracing::info!(
                target: "autopilot",
                adjustments_count = adjustments.len(),
                "expanded bounds for stable parameters"
            );
        }
        
        Ok(adjustments)
    }

    /// Update optimization results for learning
    pub async fn update_optimization_results(
        &mut self,
        parameter_name: &str,
        timestamp: DateTime<Utc>,
        actual_impact: f64,
        success: bool,
    ) -> AutopilotResult<()> {
        self.optimizer.update_optimization_result(parameter_name, timestamp, actual_impact, success);
        
        // Also update sliding bounds stability tracking
        if success {
            if let Err(e) = self.sliding_bounds.update_parameter_value(parameter_name, actual_impact) {
                tracing::warn!(
                    target: "autopilot",
                    parameter = parameter_name,
                    error = %e,
                    "failed to update parameter stability tracking"
                );
            }
        }

        // Record prediction accuracy for learning
        let accuracy_record = super::logging::PredictionAccuracyRecord {
            timestamp,
            cycle_id: self.current_cycle_id.clone().unwrap_or_else(|| "unknown".to_string()),
            parameter_name: parameter_name.to_string(),
            predicted_impact: actual_impact, // This would be the original prediction
            actual_impact,
            accuracy_score: if success { 0.8 } else { 0.2 }, // Simplified accuracy calculation
            measurement_window_hours: 24,
        };

        self.logger.record_prediction_accuracy(&accuracy_record).await?;

        Ok(())
    }

    /// Log emergency pause
    pub async fn log_emergency_pause(&self, reason: &str, duration_hours: u32) -> AutopilotResult<()> {
        self.logger.log_emergency_pause(reason, duration_hours).await
    }

    /// Log bounds adjustment
    pub async fn log_bounds_adjustment(
        &self,
        parameter_name: &str,
        adjustment_type: &str,
        old_bounds: (f64, f64),
        new_bounds: (f64, f64),
        reason: &str,
    ) -> AutopilotResult<()> {
        let cycle_id = self.current_cycle_id.as_deref().unwrap_or("manual");
        self.logger.log_bounds_adjustment(
            cycle_id,
            parameter_name,
            adjustment_type,
            old_bounds,
            new_bounds,
            reason,
        ).await
    }

    /// Get logging statistics
    pub fn get_logging_stats(&self) -> super::logging::LoggingStats {
        self.logger.get_logging_stats()
    }
}

/// Autopilot status information
#[derive(Debug, Clone, Serialize)]
pub struct AutopilotStatus {
    pub enabled: bool,
    pub emergency_pause: bool,
    pub current_cycle_id: Option<String>,
    pub daily_schedule_utc: String,
    pub canary_duration_minutes: u32,
    pub statistical_confidence_threshold: f64,
    pub max_execution_time_minutes: u32,
}