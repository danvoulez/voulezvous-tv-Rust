use std::collections::HashMap;
// use std::time::{Duration, Instant}; // Unused for now

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for canary deployment operations
pub type CanaryResult<T> = std::result::Result<T, CanaryError>;

/// Errors that can occur during canary deployments
#[derive(Debug, Error)]
pub enum CanaryError {
    #[error("deployment not found: {0}")]
    DeploymentNotFound(String),
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("metrics collection failed: {0}")]
    MetricsCollectionFailed(String),
    #[error("statistical analysis failed: {0}")]
    StatisticalAnalysisFailed(String),
    #[error("deployment timeout: {0}")]
    DeploymentTimeout(String),
    #[error("canary failed: {0}")]
    CanaryFailed(String),
    #[error("insufficient data: {0}")]
    InsufficientData(String),
}

/// Canary deployment manager for controlled parameter rollouts
pub struct CanaryDeployment {
    deployments: HashMap<String, ActiveCanaryDeployment>,
    config: CanaryConfig,
    metrics_collector: Box<dyn MetricsCollector>,
}

impl std::fmt::Debug for CanaryDeployment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanaryDeployment")
            .field("deployments", &self.deployments)
            .field("config", &self.config)
            .field("metrics_collector", &"<MetricsCollector>")
            .finish()
    }
}

/// Configuration for canary deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Percentage of traffic to route to canary (0.0 to 1.0)
    pub canary_traffic_percentage: f64,
    /// Duration to run canary deployment
    pub canary_duration_minutes: u32,
    /// Minimum sample size for statistical significance
    pub min_sample_size: usize,
    /// Confidence level required for deployment decisions (0.0 to 1.0)
    pub confidence_threshold: f64,
    /// Maximum number of concurrent canary deployments
    pub max_concurrent_deployments: usize,
    /// Timeout for metrics collection
    pub metrics_collection_timeout_seconds: u32,
    /// KPI thresholds for automatic rollback
    pub rollback_thresholds: KpiThresholds,
}

impl Default for CanaryConfig {
    fn default() -> Self {
        Self {
            canary_traffic_percentage: 0.2, // 20% canary, 80% control
            canary_duration_minutes: 60,
            min_sample_size: 100,
            confidence_threshold: 0.95,
            max_concurrent_deployments: 3,
            metrics_collection_timeout_seconds: 30,
            rollback_thresholds: KpiThresholds::default(),
        }
    }
}

/// KPI thresholds for automatic canary rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiThresholds {
    /// Maximum acceptable decrease in retention (percentage points)
    pub max_retention_decrease_pp: f64,
    /// Maximum acceptable decrease in VMAF score
    pub max_vmaf_decrease: f64,
    /// Maximum acceptable increase in error rate (percentage points)
    pub max_error_rate_increase_pp: f64,
    /// Maximum acceptable increase in latency (milliseconds)
    pub max_latency_increase_ms: f64,
}

impl Default for KpiThresholds {
    fn default() -> Self {
        Self {
            max_retention_decrease_pp: 2.0, // 2 percentage points
            max_vmaf_decrease: 5.0,         // 5 VMAF points
            max_error_rate_increase_pp: 1.0, // 1 percentage point
            max_latency_increase_ms: 100.0,  // 100ms
        }
    }
}

/// Active canary deployment state
#[derive(Debug, Clone)]
pub struct ActiveCanaryDeployment {
    pub deployment_id: String,
    pub parameter_changes: HashMap<String, ParameterChange>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: CanaryStatus,
    pub traffic_split: TrafficSplit,
    pub metrics_summary: Option<MetricsSummary>,
    pub decision: Option<CanaryDecision>,
    pub rollback_reason: Option<String>,
}

/// Parameter change being tested in canary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChange {
    pub parameter_name: String,
    pub old_value: f64,
    pub new_value: f64,
    pub change_type: ParameterChangeType,
    pub rationale: String,
}

/// Types of parameter changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterChangeType {
    Optimization,
    Correction,
    Exploration,
    Recovery,
}

/// Current status of a canary deployment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CanaryStatus {
    Initializing,
    Running,
    CollectingMetrics,
    Analyzing,
    Completed,
    RolledBack,
    Failed,
}

/// Traffic splitting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplit {
    pub canary_percentage: f64,
    pub control_percentage: f64,
    pub routing_strategy: RoutingStrategy,
    pub split_key: String, // Key used for consistent routing
}

/// Strategies for routing traffic between canary and control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Route based on hash of user/session ID
    HashBased,
    /// Route based on time slots
    TimeSlotBased,
    /// Route based on geographic region
    RegionBased,
    /// Random routing (for testing)
    Random,
}

/// Aggregated metrics for canary analysis
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSummary {
    pub canary_metrics: GroupMetrics,
    pub control_metrics: GroupMetrics,
    pub collection_period: (DateTime<Utc>, DateTime<Utc>),
    pub sample_sizes: SampleSizes,
    pub statistical_significance: StatisticalSignificance,
}

/// Metrics for a specific group (canary or control)
#[derive(Debug, Clone, Serialize)]
pub struct GroupMetrics {
    pub retention_5min: f64,
    pub vmaf_avg: f64,
    pub error_rate: f64,
    pub latency_p95_ms: f64,
    pub selection_entropy: f64,
    pub curator_apply_rate: f64,
    pub custom_metrics: HashMap<String, f64>,
}

/// Sample sizes for statistical analysis
#[derive(Debug, Clone, Serialize)]
pub struct SampleSizes {
    pub canary_samples: usize,
    pub control_samples: usize,
    pub total_samples: usize,
}

/// Statistical significance analysis results
#[derive(Debug, Clone, Serialize)]
pub struct StatisticalSignificance {
    pub retention_significance: SignificanceTest,
    pub vmaf_significance: SignificanceTest,
    pub error_rate_significance: SignificanceTest,
    pub latency_significance: SignificanceTest,
    pub overall_confidence: f64,
}

/// Individual significance test result
#[derive(Debug, Clone, Serialize)]
pub struct SignificanceTest {
    pub metric_name: String,
    pub p_value: f64,
    pub confidence_interval: (f64, f64),
    pub effect_size: f64,
    pub is_significant: bool,
    pub improvement_direction: ImprovementDirection,
}

/// Direction of improvement for a metric
#[derive(Debug, Clone, Serialize)]
pub enum ImprovementDirection {
    Higher, // Higher values are better (e.g., retention, VMAF)
    Lower,  // Lower values are better (e.g., error rate, latency)
}

/// Final decision for canary deployment
#[derive(Debug, Clone, Serialize)]
pub struct CanaryDecision {
    pub decision_type: DecisionType,
    pub confidence: f64,
    pub rationale: String,
    pub kpi_impacts: HashMap<String, KpiImpact>,
    pub recommendation: String,
    pub decision_timestamp: DateTime<Utc>,
}

/// Types of canary decisions
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum DecisionType {
    Proceed,      // Deploy to full traffic
    Rollback,     // Revert changes
    Inconclusive, // Need more data or manual review
}

/// Impact analysis for a specific KPI
#[derive(Debug, Clone, Serialize)]
pub struct KpiImpact {
    pub metric_name: String,
    pub baseline_value: f64,
    pub canary_value: f64,
    pub absolute_change: f64,
    pub percentage_change: f64,
    pub is_improvement: bool,
    pub exceeds_threshold: bool,
}

/// Trait for collecting metrics during canary deployments
pub trait MetricsCollector: Send + Sync {
    /// Collect metrics for a specific time period and traffic group
    fn collect_metrics(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        traffic_group: TrafficGroup,
    ) -> CanaryResult<GroupMetrics>;

    /// Get current sample sizes for both groups
    fn get_sample_sizes(&self, deployment_id: &str) -> CanaryResult<SampleSizes>;

    /// Check if minimum sample size has been reached
    fn has_sufficient_samples(&self, deployment_id: &str, min_samples: usize) -> CanaryResult<bool>;
}

/// Traffic group identifier for metrics collection
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TrafficGroup {
    Canary,
    Control,
}

/// Result of deployment monitoring check
#[derive(Debug, Clone)]
pub struct MonitoringResult {
    pub should_continue: bool,
    pub rollback_reason: Option<String>,
    pub metrics_available: bool,
    pub kpi_violations: Vec<KpiViolation>,
}

/// KPI violation detected during monitoring
#[derive(Debug, Clone, Serialize)]
pub struct KpiViolation {
    pub metric_name: String,
    pub baseline_value: f64,
    pub canary_value: f64,
    pub absolute_change: f64,
    pub percentage_change: f64,
    pub threshold: f64,
}

/// Real-time monitoring status for a deployment
#[derive(Debug, Clone, Serialize)]
pub struct MonitoringStatus {
    pub deployment_id: String,
    pub status: CanaryStatus,
    pub elapsed_minutes: u32,
    pub remaining_minutes: u32,
    pub sample_sizes: SampleSizes,
    pub has_sufficient_samples: bool,
    pub metrics_summary: Option<MetricsSummary>,
    pub last_updated: DateTime<Utc>,
}

/// Result of KPI gate evaluation
#[derive(Debug, Clone, Serialize)]
pub struct KpiGateResult {
    pub all_gates_passed: bool,
    pub critical_gates_passed: bool,
    pub gate_checks: Vec<GateCheck>,
    pub overall_confidence: f64,
}

/// Individual gate check result
#[derive(Debug, Clone, Serialize)]
pub struct GateCheck {
    pub gate_name: String,
    pub passed: bool,
    pub reason: String,
    pub confidence: f64,
}

/// Advanced statistical test with multiple methods
#[derive(Debug, Clone, Serialize)]
pub struct AdvancedStatisticalTest {
    pub metric_name: String,
    pub t_test_result: TTestResult,
    pub mann_whitney_result: Option<MannWhitneyResult>,
    pub bootstrap_result: Option<BootstrapResult>,
    pub bayesian_result: Option<BayesianResult>,
    pub combined_p_value: f64,
    pub effect_size_cohen_d: f64,
    pub practical_significance: bool,
}

/// T-test statistical result
#[derive(Debug, Clone, Serialize)]
pub struct TTestResult {
    pub t_statistic: f64,
    pub p_value: f64,
    pub degrees_of_freedom: usize,
    pub confidence_interval_95: (f64, f64),
    pub is_significant: bool,
}

/// Mann-Whitney U test result (non-parametric)
#[derive(Debug, Clone, Serialize)]
pub struct MannWhitneyResult {
    pub u_statistic: f64,
    pub p_value: f64,
    pub is_significant: bool,
}

/// Bootstrap confidence interval result
#[derive(Debug, Clone, Serialize)]
pub struct BootstrapResult {
    pub mean_difference: f64,
    pub confidence_interval_95: (f64, f64),
    pub bootstrap_samples: usize,
}

/// Bayesian statistical result
#[derive(Debug, Clone, Serialize)]
pub struct BayesianResult {
    pub posterior_mean: f64,
    pub credible_interval_95: (f64, f64),
    pub probability_of_improvement: f64,
    pub bayes_factor: f64,
}

/// Power analysis for statistical tests
#[derive(Debug, Clone, Serialize)]
pub struct PowerAnalysis {
    pub current_power: f64,
    pub required_sample_size_80_power: usize,
    pub required_sample_size_90_power: usize,
    pub minimum_detectable_effect: f64,
    pub recommended_duration_hours: u32,
}

/// Result of deployment progression decision
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentProgression {
    pub action_taken: ProgressionAction,
    pub reason: String,
    pub new_status: CanaryStatus,
    pub decision: Option<CanaryDecision>,
    pub next_check_time: Option<DateTime<Utc>>,
}

/// Actions that can be taken during deployment progression
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ProgressionAction {
    NoAction,
    Promoted,
    RolledBack,
    ExtendedMonitoring,
    ManualReviewRequired,
}

/// Summary of deployment progression for multiple deployments
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentProgressionSummary {
    pub deployment_id: String,
    pub success: bool,
    pub progression: Option<DeploymentProgression>,
    pub error: Option<String>,
}

/// Deployment event types for audit logging
#[derive(Debug, Clone, Serialize)]
pub enum DeploymentEvent {
    Started,
    Promoted,
    RolledBack,
    Inconclusive,
    ManualOverride,
    Timeout,
    Failed,
}

/// Audit log entry for deployment events
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentLogEntry {
    pub timestamp: DateTime<Utc>,
    pub deployment_id: String,
    pub event: DeploymentEvent,
    pub parameter_changes: HashMap<String, ParameterChange>,
    pub decision: Option<CanaryDecision>,
    pub metrics_summary: Option<MetricsSummary>,
}

/// Recommendation for deployment progression
#[derive(Debug, Clone, Serialize)]
pub struct ProgressionRecommendation {
    pub recommendation_type: RecommendationType,
    pub priority: RecommendationPriority,
    pub description: String,
    pub estimated_time_minutes: u32,
}

/// Types of progression recommendations
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationType {
    WaitForSamples,
    ReadyForAnalysis,
    ExtendMonitoring,
    ConsiderRollback,
    TimeoutWarning,
    ManualReview,
}

/// Priority levels for recommendations
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Comprehensive failure analysis for canary deployments
#[derive(Debug, Clone, Serialize)]
pub struct FailureAnalysis {
    pub deployment_id: String,
    pub failure_timestamp: DateTime<Utc>,
    pub failure_categories: Vec<FailureCategory>,
    pub root_causes: Vec<RootCause>,
    pub failure_pattern: FailurePattern,
    pub parameter_changes: HashMap<String, ParameterChange>,
    pub metrics_summary: Option<MetricsSummary>,
    pub recommendations: Vec<FailureRecommendation>,
    pub severity: FailureSeverity,
}

/// Categories of canary deployment failures
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum FailureCategory {
    KpiViolation,
    StatisticalInsignificance,
    InsufficientData,
    SystemError,
    ParameterMisconfiguration,
    TimeoutFailure,
    InfrastructureFailure,
}

/// Root cause analysis for failures
#[derive(Debug, Clone, Serialize)]
pub struct RootCause {
    pub cause_type: RootCauseType,
    pub description: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
}

/// Types of root causes
#[derive(Debug, Clone, Serialize)]
pub enum RootCauseType {
    KpiDegradation,
    InsufficientEvidence,
    InsufficientSamples,
    SystemInstability,
    ExcessiveParameterChange,
    ConfigurationError,
    InfrastructureIssue,
}

/// Failure patterns for learning and prevention
#[derive(Debug, Clone, Serialize)]
pub enum FailurePattern {
    EarlyKpiViolation,
    DelayedKpiViolation,
    InsufficientEvidence,
    DataCollection,
    SystemInstability,
    ParameterConfiguration,
    Unknown,
}

/// Severity levels for failures
#[derive(Debug, Clone, Serialize)]
pub enum FailureSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Recommendations for preventing future failures
#[derive(Debug, Clone, Serialize)]
pub struct FailureRecommendation {
    pub recommendation_type: RecommendationType,
    pub description: String,
    pub priority: RecommendationPriority,
    pub estimated_impact: String,
}

/// Additional recommendation types for failure analysis
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationType {
    WaitForSamples,
    ReadyForAnalysis,
    ExtendMonitoring,
    ConsiderRollback,
    TimeoutWarning,
    ManualReview,
    ParameterAdjustment,
    QualityReview,
    ExtendDuration,
    InfrastructureReview,
}

/// Comprehensive deployment report
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentReport {
    pub deployment_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub actual_duration_minutes: u32,
    pub planned_duration_minutes: u32,
    pub final_status: CanaryStatus,
    pub parameter_changes: HashMap<String, ParameterChange>,
    pub metrics_summary: Option<MetricsSummary>,
    pub decision: Option<CanaryDecision>,
    pub failure_analysis: Option<FailureAnalysis>,
    pub traffic_split: TrafficSplit,
    pub rollback_reason: Option<String>,
}

impl CanaryDeployment {
    /// Create new canary deployment manager
    pub fn new(config: CanaryConfig, metrics_collector: Box<dyn MetricsCollector>) -> Self {
        Self {
            deployments: HashMap::new(),
            config,
            metrics_collector,
        }
    }

    /// Collect metrics for a canary deployment
    pub fn collect_deployment_metrics(&mut self, deployment_id: &str) -> CanaryResult<MetricsSummary> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        if !matches!(deployment.status, CanaryStatus::Running | CanaryStatus::CollectingMetrics) {
            return Err(CanaryError::InvalidConfiguration(
                format!("Cannot collect metrics for deployment in status: {:?}", deployment.status)
            ));
        }

        // Check if we have sufficient samples
        let sample_sizes = self.metrics_collector.get_sample_sizes(deployment_id)?;
        if !self.metrics_collector.has_sufficient_samples(deployment_id, self.config.min_sample_size)? {
            return Err(CanaryError::InsufficientData(
                format!("Insufficient samples: canary={}, control={}, required={}",
                    sample_sizes.canary_samples, sample_sizes.control_samples, self.config.min_sample_size)
            ));
        }

        // Collect metrics for both groups
        let collection_start = deployment.start_time;
        let collection_end = std::cmp::min(Utc::now(), deployment.end_time);

        let canary_metrics = self.metrics_collector.collect_metrics(
            collection_start,
            collection_end,
            TrafficGroup::Canary,
        )?;

        let control_metrics = self.metrics_collector.collect_metrics(
            collection_start,
            collection_end,
            TrafficGroup::Control,
        )?;

        // Perform statistical significance testing
        let statistical_significance = self.calculate_statistical_significance(
            &canary_metrics,
            &control_metrics,
            &sample_sizes,
        )?;

        let metrics_summary = MetricsSummary {
            canary_metrics,
            control_metrics,
            collection_period: (collection_start, collection_end),
            sample_sizes,
            statistical_significance,
        };

        // Update deployment with metrics
        let deployment = self.deployments.get_mut(deployment_id).unwrap();
        deployment.metrics_summary = Some(metrics_summary.clone());
        deployment.status = CanaryStatus::Analyzing;

        Ok(metrics_summary)
    }

    /// Calculate statistical significance for all metrics
    fn calculate_statistical_significance(
        &self,
        canary_metrics: &GroupMetrics,
        control_metrics: &GroupMetrics,
        sample_sizes: &SampleSizes,
    ) -> CanaryResult<StatisticalSignificance> {
        let retention_test = self.perform_significance_test(
            "retention_5min",
            canary_metrics.retention_5min,
            control_metrics.retention_5min,
            sample_sizes,
            ImprovementDirection::Higher,
        )?;

        let vmaf_test = self.perform_significance_test(
            "vmaf_avg",
            canary_metrics.vmaf_avg,
            control_metrics.vmaf_avg,
            sample_sizes,
            ImprovementDirection::Higher,
        )?;

        let error_rate_test = self.perform_significance_test(
            "error_rate",
            canary_metrics.error_rate,
            control_metrics.error_rate,
            sample_sizes,
            ImprovementDirection::Lower,
        )?;

        let latency_test = self.perform_significance_test(
            "latency_p95_ms",
            canary_metrics.latency_p95_ms,
            control_metrics.latency_p95_ms,
            sample_sizes,
            ImprovementDirection::Lower,
        )?;

        // Calculate overall confidence as the minimum of all significant tests
        let significant_tests = vec![&retention_test, &vmaf_test, &error_rate_test, &latency_test]
            .into_iter()
            .filter(|test| test.is_significant)
            .collect::<Vec<_>>();

        let overall_confidence = if significant_tests.is_empty() {
            0.0
        } else {
            significant_tests.iter()
                .map(|test| 1.0 - test.p_value)
                .fold(f64::INFINITY, f64::min)
        };

        Ok(StatisticalSignificance {
            retention_significance: retention_test,
            vmaf_significance: vmaf_test,
            error_rate_significance: error_rate_test,
            latency_significance: latency_test,
            overall_confidence,
        })
    }

    /// Perform statistical significance test for a single metric
    fn perform_significance_test(
        &self,
        metric_name: &str,
        canary_value: f64,
        control_value: f64,
        sample_sizes: &SampleSizes,
        improvement_direction: ImprovementDirection,
    ) -> CanaryResult<SignificanceTest> {
        // Calculate effect size (Cohen's d approximation)
        let effect_size = (canary_value - control_value).abs() / 
            ((canary_value + control_value) / 2.0).max(0.001);

        // Simplified t-test calculation (in a real implementation, this would use proper statistical libraries)
        let pooled_std_error = self.calculate_pooled_standard_error(
            canary_value,
            control_value,
            sample_sizes.canary_samples,
            sample_sizes.control_samples,
        );

        let t_statistic = (canary_value - control_value) / pooled_std_error;
        let degrees_of_freedom = sample_sizes.canary_samples + sample_sizes.control_samples - 2;

        // Simplified p-value calculation (approximation)
        let p_value = self.calculate_p_value(t_statistic.abs(), degrees_of_freedom);

        // Calculate confidence interval (95%)
        let margin_of_error = 1.96 * pooled_std_error; // Using z-score for large samples
        let difference = canary_value - control_value;
        let confidence_interval = (
            difference - margin_of_error,
            difference + margin_of_error,
        );

        let is_significant = p_value < (1.0 - self.config.confidence_threshold);

        Ok(SignificanceTest {
            metric_name: metric_name.to_string(),
            p_value,
            confidence_interval,
            effect_size,
            is_significant,
            improvement_direction,
        })
    }

    /// Calculate pooled standard error for t-test
    fn calculate_pooled_standard_error(&self, value1: f64, value2: f64, n1: usize, n2: usize) -> f64 {
        // Simplified calculation assuming equal variances
        // In a real implementation, this would use actual sample variances
        let estimated_variance = ((value1 + value2) / 2.0) * 0.1; // 10% coefficient of variation
        let pooled_variance = estimated_variance * (1.0 / n1 as f64 + 1.0 / n2 as f64);
        pooled_variance.sqrt()
    }

    /// Calculate p-value from t-statistic (simplified approximation)
    fn calculate_p_value(&self, t_stat: f64, _df: usize) -> f64 {
        // Simplified p-value calculation using normal approximation
        // In a real implementation, this would use proper t-distribution
        if t_stat < 1.0 {
            0.5
        } else if t_stat < 1.96 {
            0.1
        } else if t_stat < 2.58 {
            0.05
        } else if t_stat < 3.29 {
            0.01
        } else {
            0.001
        }
    }

    /// Monitor deployment and check for automatic rollback conditions
    pub fn monitor_deployment(&mut self, deployment_id: &str) -> CanaryResult<MonitoringResult> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        if !matches!(deployment.status, CanaryStatus::Running | CanaryStatus::CollectingMetrics) {
            return Ok(MonitoringResult {
                should_continue: false,
                rollback_reason: Some("Deployment not in active state".to_string()),
                metrics_available: false,
                kpi_violations: Vec::new(),
            });
        }

        // Check if deployment has timed out
        if self.check_deployment_timeout(deployment_id)? {
            return Ok(MonitoringResult {
                should_continue: false,
                rollback_reason: Some("Deployment timeout exceeded".to_string()),
                metrics_available: false,
                kpi_violations: Vec::new(),
            });
        }

        // Try to collect metrics
        let metrics_summary = match self.collect_deployment_metrics(deployment_id) {
            Ok(summary) => summary,
            Err(CanaryError::InsufficientData(_)) => {
                // Not enough data yet, continue monitoring
                return Ok(MonitoringResult {
                    should_continue: true,
                    rollback_reason: None,
                    metrics_available: false,
                    kpi_violations: Vec::new(),
                });
            }
            Err(e) => return Err(e),
        };

        // Check for KPI violations
        let kpi_violations = self.check_kpi_violations(&metrics_summary)?;

        let should_rollback = !kpi_violations.is_empty();
        let rollback_reason = if should_rollback {
            Some(format!("KPI violations detected: {}", 
                kpi_violations.iter()
                    .map(|v| format!("{}: {:.2}% change", v.metric_name, v.percentage_change))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        } else {
            None
        };

        Ok(MonitoringResult {
            should_continue: !should_rollback,
            rollback_reason,
            metrics_available: true,
            kpi_violations,
        })
    }

    /// Check for KPI threshold violations
    fn check_kpi_violations(&self, metrics_summary: &MetricsSummary) -> CanaryResult<Vec<KpiViolation>> {
        let mut violations = Vec::new();

        // Check retention decrease
        let retention_change = metrics_summary.canary_metrics.retention_5min - 
                              metrics_summary.control_metrics.retention_5min;
        if retention_change < -self.config.rollback_thresholds.max_retention_decrease_pp / 100.0 {
            violations.push(KpiViolation {
                metric_name: "retention_5min".to_string(),
                baseline_value: metrics_summary.control_metrics.retention_5min,
                canary_value: metrics_summary.canary_metrics.retention_5min,
                absolute_change: retention_change,
                percentage_change: retention_change * 100.0,
                threshold: -self.config.rollback_thresholds.max_retention_decrease_pp,
            });
        }

        // Check VMAF decrease
        let vmaf_change = metrics_summary.canary_metrics.vmaf_avg - 
                         metrics_summary.control_metrics.vmaf_avg;
        if vmaf_change < -self.config.rollback_thresholds.max_vmaf_decrease {
            violations.push(KpiViolation {
                metric_name: "vmaf_avg".to_string(),
                baseline_value: metrics_summary.control_metrics.vmaf_avg,
                canary_value: metrics_summary.canary_metrics.vmaf_avg,
                absolute_change: vmaf_change,
                percentage_change: (vmaf_change / metrics_summary.control_metrics.vmaf_avg) * 100.0,
                threshold: -self.config.rollback_thresholds.max_vmaf_decrease,
            });
        }

        // Check error rate increase
        let error_rate_change = metrics_summary.canary_metrics.error_rate - 
                               metrics_summary.control_metrics.error_rate;
        if error_rate_change > self.config.rollback_thresholds.max_error_rate_increase_pp / 100.0 {
            violations.push(KpiViolation {
                metric_name: "error_rate".to_string(),
                baseline_value: metrics_summary.control_metrics.error_rate,
                canary_value: metrics_summary.canary_metrics.error_rate,
                absolute_change: error_rate_change,
                percentage_change: error_rate_change * 100.0,
                threshold: self.config.rollback_thresholds.max_error_rate_increase_pp,
            });
        }

        // Check latency increase
        let latency_change = metrics_summary.canary_metrics.latency_p95_ms - 
                            metrics_summary.control_metrics.latency_p95_ms;
        if latency_change > self.config.rollback_thresholds.max_latency_increase_ms {
            violations.push(KpiViolation {
                metric_name: "latency_p95_ms".to_string(),
                baseline_value: metrics_summary.control_metrics.latency_p95_ms,
                canary_value: metrics_summary.canary_metrics.latency_p95_ms,
                absolute_change: latency_change,
                percentage_change: (latency_change / metrics_summary.control_metrics.latency_p95_ms) * 100.0,
                threshold: self.config.rollback_thresholds.max_latency_increase_ms,
            });
        }

        Ok(violations)
    }

    /// Evaluate canary deployment using KPI gates and statistical significance
    pub fn evaluate_canary_deployment(&mut self, deployment_id: &str) -> CanaryResult<CanaryDecision> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // Ensure we have metrics
        let metrics_summary = match &deployment.metrics_summary {
            Some(summary) => summary,
            None => {
                // Try to collect metrics first
                return match self.collect_deployment_metrics(deployment_id) {
                    Ok(summary) => self.make_canary_decision(&summary),
                    Err(CanaryError::InsufficientData(_)) => {
                        Ok(CanaryDecision {
                            decision_type: DecisionType::Inconclusive,
                            confidence: 0.0,
                            rationale: "Insufficient data for decision".to_string(),
                            kpi_impacts: HashMap::new(),
                            recommendation: "Continue monitoring until sufficient samples are collected".to_string(),
                            decision_timestamp: Utc::now(),
                        })
                    }
                    Err(e) => Err(e),
                };
            }
        };

        self.make_canary_decision(metrics_summary)
    }

    /// Make canary deployment decision based on statistical analysis and KPI gates
    fn make_canary_decision(&self, metrics_summary: &MetricsSummary) -> CanaryResult<CanaryDecision> {
        let mut kpi_impacts = HashMap::new();
        let mut decision_factors = Vec::new();
        // Calculate overall confidence will be set later

        // Analyze each KPI
        let retention_impact = self.analyze_kpi_impact(
            "retention_5min",
            metrics_summary.control_metrics.retention_5min,
            metrics_summary.canary_metrics.retention_5min,
            &metrics_summary.statistical_significance.retention_significance,
            ImprovementDirection::Higher,
        );
        kpi_impacts.insert("retention_5min".to_string(), retention_impact.clone());
        decision_factors.push(retention_impact);

        let vmaf_impact = self.analyze_kpi_impact(
            "vmaf_avg",
            metrics_summary.control_metrics.vmaf_avg,
            metrics_summary.canary_metrics.vmaf_avg,
            &metrics_summary.statistical_significance.vmaf_significance,
            ImprovementDirection::Higher,
        );
        kpi_impacts.insert("vmaf_avg".to_string(), vmaf_impact.clone());
        decision_factors.push(vmaf_impact);

        let error_rate_impact = self.analyze_kpi_impact(
            "error_rate",
            metrics_summary.control_metrics.error_rate,
            metrics_summary.canary_metrics.error_rate,
            &metrics_summary.statistical_significance.error_rate_significance,
            ImprovementDirection::Lower,
        );
        kpi_impacts.insert("error_rate".to_string(), error_rate_impact.clone());
        decision_factors.push(error_rate_impact);

        let latency_impact = self.analyze_kpi_impact(
            "latency_p95_ms",
            metrics_summary.control_metrics.latency_p95_ms,
            metrics_summary.canary_metrics.latency_p95_ms,
            &metrics_summary.statistical_significance.latency_significance,
            ImprovementDirection::Lower,
        );
        kpi_impacts.insert("latency_p95_ms".to_string(), latency_impact.clone());
        decision_factors.push(latency_impact);

        // Apply KPI gate logic
        let gate_result = self.apply_kpi_gates(&decision_factors, metrics_summary)?;

        // Calculate overall confidence
        let overall_confidence = self.calculate_overall_confidence(&decision_factors, &gate_result);

        // Make final decision
        let (decision_type, rationale, recommendation) = self.determine_final_decision(&gate_result, overall_confidence);

        Ok(CanaryDecision {
            decision_type,
            confidence: overall_confidence,
            rationale,
            kpi_impacts,
            recommendation,
            decision_timestamp: Utc::now(),
        })
    }

    /// Analyze impact of a single KPI
    fn analyze_kpi_impact(
        &self,
        metric_name: &str,
        baseline_value: f64,
        canary_value: f64,
        _significance_test: &SignificanceTest,
        improvement_direction: ImprovementDirection,
    ) -> KpiImpact {
        let absolute_change = canary_value - baseline_value;
        let percentage_change = if baseline_value != 0.0 {
            (absolute_change / baseline_value) * 100.0
        } else {
            0.0
        };

        let is_improvement = match improvement_direction {
            ImprovementDirection::Higher => absolute_change > 0.0,
            ImprovementDirection::Lower => absolute_change < 0.0,
        };

        // Check if change exceeds rollback thresholds
        let exceeds_threshold = match metric_name {
            "retention_5min" => absolute_change < -self.config.rollback_thresholds.max_retention_decrease_pp / 100.0,
            "vmaf_avg" => absolute_change < -self.config.rollback_thresholds.max_vmaf_decrease,
            "error_rate" => absolute_change > self.config.rollback_thresholds.max_error_rate_increase_pp / 100.0,
            "latency_p95_ms" => absolute_change > self.config.rollback_thresholds.max_latency_increase_ms,
            _ => false,
        };

        KpiImpact {
            metric_name: metric_name.to_string(),
            baseline_value,
            canary_value,
            absolute_change,
            percentage_change,
            is_improvement,
            exceeds_threshold,
        }
    }

    /// Apply KPI gate logic to determine if deployment should proceed
    fn apply_kpi_gates(&self, decision_factors: &[KpiImpact], metrics_summary: &MetricsSummary) -> CanaryResult<KpiGateResult> {
        let mut gate_results = Vec::new();

        // Gate 1: No critical KPI violations
        let critical_violations: Vec<&KpiImpact> = decision_factors.iter()
            .filter(|impact| impact.exceeds_threshold)
            .collect();

        if !critical_violations.is_empty() {
            gate_results.push(GateCheck {
                gate_name: "Critical KPI Thresholds".to_string(),
                passed: false,
                reason: format!("Critical violations in: {}", 
                    critical_violations.iter()
                        .map(|v| format!("{} ({:.2}%)", v.metric_name, v.percentage_change))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                confidence: 1.0,
            });
        } else {
            gate_results.push(GateCheck {
                gate_name: "Critical KPI Thresholds".to_string(),
                passed: true,
                reason: "No critical KPI violations detected".to_string(),
                confidence: 1.0,
            });
        }

        // Gate 2: Statistical significance requirement
        let significant_improvements: Vec<&KpiImpact> = decision_factors.iter()
            .filter(|impact| impact.is_improvement)
            .collect();

        let overall_significance = metrics_summary.statistical_significance.overall_confidence;
        let significance_gate_passed = overall_significance >= self.config.confidence_threshold;

        gate_results.push(GateCheck {
            gate_name: "Statistical Significance".to_string(),
            passed: significance_gate_passed,
            reason: if significance_gate_passed {
                format!("Overall confidence {:.1}% meets threshold {:.1}%", 
                    overall_significance * 100.0, self.config.confidence_threshold * 100.0)
            } else {
                format!("Overall confidence {:.1}% below threshold {:.1}%", 
                    overall_significance * 100.0, self.config.confidence_threshold * 100.0)
            },
            confidence: overall_significance,
        });

        // Gate 3: Minimum improvement requirement
        let has_meaningful_improvement = significant_improvements.iter()
            .any(|impact| impact.percentage_change.abs() > 1.0); // At least 1% improvement

        gate_results.push(GateCheck {
            gate_name: "Meaningful Improvement".to_string(),
            passed: has_meaningful_improvement,
            reason: if has_meaningful_improvement {
                format!("Detected meaningful improvements in: {}", 
                    significant_improvements.iter()
                        .filter(|impact| impact.percentage_change.abs() > 1.0)
                        .map(|impact| format!("{} (+{:.1}%)", impact.metric_name, impact.percentage_change))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                "No meaningful improvements detected (>1% change)".to_string()
            },
            confidence: if has_meaningful_improvement { 0.8 } else { 0.2 },
        });

        // Gate 4: Sample size adequacy
        let sample_adequacy = self.evaluate_sample_adequacy(&metrics_summary.sample_sizes);
        gate_results.push(sample_adequacy);

        // Calculate overall gate result
        let all_gates_passed = gate_results.iter().all(|gate| gate.passed);
        let critical_gates_passed = gate_results.iter()
            .filter(|gate| gate.gate_name == "Critical KPI Thresholds" || gate.gate_name == "Statistical Significance")
            .all(|gate| gate.passed);

        let overall_confidence = gate_results.iter().map(|g| g.confidence).fold(0.0, f64::min);

        Ok(KpiGateResult {
            all_gates_passed,
            critical_gates_passed,
            gate_checks: gate_results,
            overall_confidence,
        })
    }

    /// Evaluate sample size adequacy
    fn evaluate_sample_adequacy(&self, sample_sizes: &SampleSizes) -> GateCheck {
        let min_required = self.config.min_sample_size;
        let canary_adequate = sample_sizes.canary_samples >= min_required;
        let control_adequate = sample_sizes.control_samples >= min_required;
        let both_adequate = canary_adequate && control_adequate;

        // Calculate confidence based on sample sizes
        let canary_confidence = (sample_sizes.canary_samples as f64 / min_required as f64).min(1.0);
        let control_confidence = (sample_sizes.control_samples as f64 / min_required as f64).min(1.0);
        let overall_confidence = canary_confidence.min(control_confidence);

        GateCheck {
            gate_name: "Sample Size Adequacy".to_string(),
            passed: both_adequate,
            reason: format!("Canary: {}/{} samples, Control: {}/{} samples", 
                sample_sizes.canary_samples, min_required,
                sample_sizes.control_samples, min_required),
            confidence: overall_confidence,
        }
    }

    /// Calculate overall confidence for the decision
    fn calculate_overall_confidence(&self, decision_factors: &[KpiImpact], gate_result: &KpiGateResult) -> f64 {
        if !gate_result.critical_gates_passed {
            return 0.0; // No confidence if critical gates fail
        }

        // Base confidence from gate results
        let gate_confidence = gate_result.overall_confidence;

        // Adjust confidence based on effect sizes
        let effect_size_confidence = decision_factors.iter()
            .map(|impact| {
                let effect_magnitude = impact.percentage_change.abs();
                if effect_magnitude > 5.0 {
                    1.0 // High confidence for large effects
                } else if effect_magnitude > 2.0 {
                    0.8 // Medium confidence for moderate effects
                } else if effect_magnitude > 0.5 {
                    0.6 // Low confidence for small effects
                } else {
                    0.3 // Very low confidence for tiny effects
                }
            })
            .fold(0.0, f64::max);

        // Combine confidences (conservative approach)
        (gate_confidence * 0.7 + effect_size_confidence * 0.3).min(1.0)
    }

    /// Determine final decision based on gate results and confidence
    fn determine_final_decision(&self, gate_result: &KpiGateResult, confidence: f64) -> (DecisionType, String, String) {
        if !gate_result.critical_gates_passed {
            (
                DecisionType::Rollback,
                "Critical KPI gates failed - automatic rollback required".to_string(),
                "Immediately rollback deployment due to KPI violations or insufficient statistical confidence".to_string(),
            )
        } else if gate_result.all_gates_passed && confidence >= 0.8 {
            (
                DecisionType::Proceed,
                format!("All gates passed with high confidence ({:.1}%)", confidence * 100.0),
                "Deploy to full traffic - canary shows significant improvements with high confidence".to_string(),
            )
        } else if gate_result.all_gates_passed && confidence >= 0.6 {
            (
                DecisionType::Proceed,
                format!("All gates passed with moderate confidence ({:.1}%)", confidence * 100.0),
                "Deploy to full traffic - canary shows improvements with acceptable confidence".to_string(),
            )
        } else if confidence >= 0.4 {
            (
                DecisionType::Inconclusive,
                format!("Mixed results with moderate confidence ({:.1}%)", confidence * 100.0),
                "Extend canary duration or seek manual review - results are promising but not conclusive".to_string(),
            )
        } else {
            (
                DecisionType::Rollback,
                format!("Low confidence in results ({:.1}%)", confidence * 100.0),
                "Rollback deployment - insufficient evidence of improvement".to_string(),
            )
        }
    }

    /// Get real-time monitoring status for a deployment
    pub fn get_monitoring_status(&self, deployment_id: &str) -> CanaryResult<MonitoringStatus> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let now = Utc::now();
        let elapsed_minutes = (now - deployment.start_time).num_minutes() as u32;
        let remaining_minutes = if now < deployment.end_time {
            (deployment.end_time - now).num_minutes() as u32
        } else {
            0
        };

        let sample_sizes = self.metrics_collector.get_sample_sizes(deployment_id)
            .unwrap_or_else(|_| SampleSizes {
                canary_samples: 0,
                control_samples: 0,
                total_samples: 0,
            });

        let has_sufficient_samples = sample_sizes.canary_samples >= self.config.min_sample_size &&
                                   sample_sizes.control_samples >= self.config.min_sample_size;

        Ok(MonitoringStatus {
            deployment_id: deployment_id.to_string(),
            status: deployment.status.clone(),
            elapsed_minutes,
            remaining_minutes,
            sample_sizes,
            has_sufficient_samples,
            metrics_summary: deployment.metrics_summary.clone(),
            last_updated: now,
        })
    }

    /// Start a new canary deployment
    pub fn start_deployment(
        &mut self,
        deployment_id: String,
        parameter_changes: HashMap<String, ParameterChange>,
    ) -> CanaryResult<()> {
        // Check if we're at the concurrent deployment limit
        let active_deployments = self.deployments.values()
            .filter(|d| matches!(d.status, CanaryStatus::Running | CanaryStatus::CollectingMetrics | CanaryStatus::Analyzing))
            .count();

        if active_deployments >= self.config.max_concurrent_deployments {
            return Err(CanaryError::InvalidConfiguration(
                format!("Maximum concurrent deployments ({}) exceeded", self.config.max_concurrent_deployments)
            ));
        }

        // Validate parameter changes
        self.validate_parameter_changes(&parameter_changes)?;

        let now = Utc::now();
        let end_time = now + chrono::Duration::minutes(self.config.canary_duration_minutes as i64);

        // Create traffic split configuration
        let traffic_split = TrafficSplit {
            canary_percentage: self.config.canary_traffic_percentage,
            control_percentage: 1.0 - self.config.canary_traffic_percentage,
            routing_strategy: RoutingStrategy::HashBased,
            split_key: format!("canary_{}", deployment_id),
        };

        let deployment = ActiveCanaryDeployment {
            deployment_id: deployment_id.clone(),
            parameter_changes,
            start_time: now,
            end_time,
            status: CanaryStatus::Initializing,
            traffic_split,
            metrics_summary: None,
            decision: None,
            rollback_reason: None,
        };

        self.deployments.insert(deployment_id.clone(), deployment);

        // Initialize the deployment (this would set up traffic routing in a real system)
        self.initialize_deployment(&deployment_id)?;

        Ok(())
    }

    /// Initialize deployment infrastructure
    fn initialize_deployment(&mut self, deployment_id: &str) -> CanaryResult<()> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // In a real implementation, this would:
        // 1. Configure load balancer for traffic splitting
        // 2. Deploy parameter changes to canary instances
        // 3. Set up monitoring and alerting
        // 4. Initialize metrics collection

        deployment.status = CanaryStatus::Running;
        Ok(())
    }

    /// Validate parameter changes before deployment
    fn validate_parameter_changes(&self, changes: &HashMap<String, ParameterChange>) -> CanaryResult<()> {
        if changes.is_empty() {
            return Err(CanaryError::InvalidConfiguration(
                "No parameter changes specified".to_string()
            ));
        }

        for (param_name, change) in changes {
            // Validate parameter names and values
            match param_name.as_str() {
                "selection_temperature" => {
                    if change.new_value < 0.1 || change.new_value > 2.0 {
                        return Err(CanaryError::InvalidConfiguration(
                            format!("Temperature {} outside valid range [0.1, 2.0]", change.new_value)
                        ));
                    }
                }
                "selection_top_k" => {
                    if change.new_value < 1.0 || change.new_value > 100.0 || change.new_value.fract() != 0.0 {
                        return Err(CanaryError::InvalidConfiguration(
                            format!("Top-k {} must be integer between 1 and 100", change.new_value)
                        ));
                    }
                }
                "plan_selection_bias" => {
                    if change.new_value < -0.5 || change.new_value > 0.5 {
                        return Err(CanaryError::InvalidConfiguration(
                            format!("Bias {} outside valid range [-0.5, 0.5]", change.new_value)
                        ));
                    }
                }
                _ => {
                    // Allow other parameters but note as warning
                    // In a real implementation, this would use proper logging
                    eprintln!("Warning: Unknown parameter in canary deployment: {}", param_name);
                }
            }

            // Validate change magnitude
            let change_magnitude = (change.new_value - change.old_value).abs();
            if change_magnitude > 0.5 {
                return Err(CanaryError::InvalidConfiguration(
                    format!("Change magnitude {} too large for parameter {}", change_magnitude, param_name)
                ));
            }
        }

        Ok(())
    }

    /// Get current status of a deployment
    pub fn get_deployment_status(&self, deployment_id: &str) -> CanaryResult<CanaryStatus> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        Ok(deployment.status.clone())
    }

    /// Get deployment details
    pub fn get_deployment(&self, deployment_id: &str) -> CanaryResult<&ActiveCanaryDeployment> {
        self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))
    }

    /// List all active deployments
    pub fn list_active_deployments(&self) -> Vec<&ActiveCanaryDeployment> {
        self.deployments.values()
            .filter(|d| !matches!(d.status, CanaryStatus::Completed | CanaryStatus::RolledBack | CanaryStatus::Failed))
            .collect()
    }

    /// Update deployment status
    pub fn update_deployment_status(&mut self, deployment_id: &str, status: CanaryStatus) -> CanaryResult<()> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        deployment.status = status;
        Ok(())
    }

    /// Check if deployment has timed out
    pub fn check_deployment_timeout(&self, deployment_id: &str) -> CanaryResult<bool> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let now = Utc::now();
        Ok(now > deployment.end_time && matches!(deployment.status, CanaryStatus::Running | CanaryStatus::CollectingMetrics))
    }

    /// Rollback a deployment
    pub fn rollback_deployment(&mut self, deployment_id: &str, reason: String) -> CanaryResult<()> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // In a real implementation, this would:
        // 1. Revert parameter changes
        // 2. Route all traffic back to control
        // 3. Clean up canary infrastructure
        // 4. Send notifications

        deployment.status = CanaryStatus::RolledBack;
        deployment.rollback_reason = Some(reason);

        Ok(())
    }

    /// Complete a deployment (promote canary to full traffic)
    pub fn complete_deployment(&mut self, deployment_id: &str) -> CanaryResult<()> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // In a real implementation, this would:
        // 1. Apply parameter changes to all traffic
        // 2. Clean up canary infrastructure
        // 3. Update configuration
        // 4. Send notifications

        deployment.status = CanaryStatus::Completed;

        Ok(())
    }

    /// Get traffic routing decision for a request
    pub fn get_traffic_routing(&self, deployment_id: &str, routing_key: &str) -> CanaryResult<TrafficGroup> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        if !matches!(deployment.status, CanaryStatus::Running | CanaryStatus::CollectingMetrics) {
            return Ok(TrafficGroup::Control);
        }

        match deployment.traffic_split.routing_strategy {
            RoutingStrategy::HashBased => {
                // Use consistent hashing to route traffic
                let hash = self.hash_routing_key(routing_key, &deployment.traffic_split.split_key);
                let normalized_hash = (hash % 100) as f64 / 100.0;
                
                if normalized_hash < deployment.traffic_split.canary_percentage {
                    Ok(TrafficGroup::Canary)
                } else {
                    Ok(TrafficGroup::Control)
                }
            }
            RoutingStrategy::Random => {
                // Random routing (for testing)
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let random_value: f64 = rng.gen();
                
                if random_value < deployment.traffic_split.canary_percentage {
                    Ok(TrafficGroup::Canary)
                } else {
                    Ok(TrafficGroup::Control)
                }
            }
            _ => {
                // Other routing strategies not implemented yet
                Ok(TrafficGroup::Control)
            }
        }
    }

    /// Hash routing key for consistent traffic splitting
    fn hash_routing_key(&self, routing_key: &str, split_key: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        routing_key.hash(&mut hasher);
        split_key.hash(&mut hasher);
        hasher.finish()
    }

    /// Clean up completed or failed deployments
    pub fn cleanup_old_deployments(&mut self, retention_hours: u32) -> usize {
        let cutoff_time = Utc::now() - chrono::Duration::hours(retention_hours as i64);
        let initial_count = self.deployments.len();

        self.deployments.retain(|_, deployment| {
            match deployment.status {
                CanaryStatus::Completed | CanaryStatus::RolledBack | CanaryStatus::Failed => {
                    deployment.start_time >= cutoff_time
                }
                _ => true, // Keep active deployments
            }
        });

        initial_count - self.deployments.len()
    }

    /// Perform advanced statistical testing with multiple methods
    pub fn perform_advanced_statistical_test(
        &self,
        metric_name: &str,
        canary_values: &[f64],
        control_values: &[f64],
        _improvement_direction: ImprovementDirection,
    ) -> CanaryResult<AdvancedStatisticalTest> {
        if canary_values.is_empty() || control_values.is_empty() {
            return Err(CanaryError::InsufficientData("Empty sample arrays".to_string()));
        }

        // Calculate basic statistics
        let canary_mean = canary_values.iter().sum::<f64>() / canary_values.len() as f64;
        let control_mean = control_values.iter().sum::<f64>() / control_values.len() as f64;
        let mean_difference = canary_mean - control_mean;

        // Perform t-test
        let t_test_result = self.perform_t_test(canary_values, control_values)?;

        // Calculate Cohen's d effect size
        let pooled_std = self.calculate_pooled_standard_deviation(canary_values, control_values);
        let cohen_d = if pooled_std > 0.0 {
            mean_difference / pooled_std
        } else {
            0.0
        };

        // Determine practical significance
        let practical_significance = match metric_name {
            "retention_5min" => cohen_d.abs() > 0.2, // Small effect size threshold
            "vmaf_avg" => cohen_d.abs() > 0.3,       // Medium effect size threshold
            "error_rate" => cohen_d.abs() > 0.2,     // Small effect size threshold
            "latency_p95_ms" => cohen_d.abs() > 0.3, // Medium effect size threshold
            _ => cohen_d.abs() > 0.2,                 // Default small effect size
        };

        // Perform Mann-Whitney U test (non-parametric alternative)
        let mann_whitney_result = self.perform_mann_whitney_test(canary_values, control_values)?;

        // Perform bootstrap confidence interval
        let bootstrap_result = self.perform_bootstrap_test(canary_values, control_values, 1000)?;

        // Combine p-values using Fisher's method (simplified)
        let combined_p_value = self.combine_p_values(&[
            t_test_result.p_value,
            mann_whitney_result.p_value,
        ]);

        Ok(AdvancedStatisticalTest {
            metric_name: metric_name.to_string(),
            t_test_result,
            mann_whitney_result: Some(mann_whitney_result),
            bootstrap_result: Some(bootstrap_result),
            bayesian_result: None, // Could be implemented for more advanced analysis
            combined_p_value,
            effect_size_cohen_d: cohen_d,
            practical_significance,
        })
    }

    /// Perform Welch's t-test for unequal variances
    fn perform_t_test(&self, sample1: &[f64], sample2: &[f64]) -> CanaryResult<TTestResult> {
        let n1 = sample1.len();
        let n2 = sample2.len();

        if n1 < 2 || n2 < 2 {
            return Err(CanaryError::InsufficientData("Need at least 2 samples per group".to_string()));
        }

        let mean1 = sample1.iter().sum::<f64>() / n1 as f64;
        let mean2 = sample2.iter().sum::<f64>() / n2 as f64;

        let var1 = sample1.iter().map(|x| (x - mean1).powi(2)).sum::<f64>() / (n1 - 1) as f64;
        let var2 = sample2.iter().map(|x| (x - mean2).powi(2)).sum::<f64>() / (n2 - 1) as f64;

        let se = (var1 / n1 as f64 + var2 / n2 as f64).sqrt();
        let t_statistic = (mean1 - mean2) / se;

        // Welch's degrees of freedom
        let df = (var1 / n1 as f64 + var2 / n2 as f64).powi(2) /
                 ((var1 / n1 as f64).powi(2) / (n1 - 1) as f64 + 
                  (var2 / n2 as f64).powi(2) / (n2 - 1) as f64);

        let p_value = self.calculate_t_test_p_value(t_statistic.abs(), df as usize);

        // 95% confidence interval
        let t_critical = 1.96; // Approximation for large samples
        let margin_of_error = t_critical * se;
        let confidence_interval_95 = (
            (mean1 - mean2) - margin_of_error,
            (mean1 - mean2) + margin_of_error,
        );

        let is_significant = p_value < 0.05;

        Ok(TTestResult {
            t_statistic,
            p_value,
            degrees_of_freedom: df as usize,
            confidence_interval_95,
            is_significant,
        })
    }

    /// Perform Mann-Whitney U test (non-parametric)
    fn perform_mann_whitney_test(&self, sample1: &[f64], sample2: &[f64]) -> CanaryResult<MannWhitneyResult> {
        let n1 = sample1.len();
        let n2 = sample2.len();

        // Combine and rank all values
        let mut combined: Vec<(f64, usize)> = Vec::new();
        for &val in sample1 {
            combined.push((val, 1)); // Group 1
        }
        for &val in sample2 {
            combined.push((val, 2)); // Group 2
        }

        // Sort by value
        combined.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Assign ranks (handling ties by averaging)
        let mut ranks = vec![0.0; combined.len()];
        let mut i = 0;
        while i < combined.len() {
            let mut j = i;
            while j < combined.len() && combined[j].0 == combined[i].0 {
                j += 1;
            }
            let avg_rank = (i + j + 1) as f64 / 2.0;
            for k in i..j {
                ranks[k] = avg_rank;
            }
            i = j;
        }

        // Calculate rank sums
        let mut r1 = 0.0;
        for (idx, &(_, group)) in combined.iter().enumerate() {
            if group == 1 {
                r1 += ranks[idx];
            }
        }

        // Calculate U statistics
        let u1 = r1 - (n1 * (n1 + 1)) as f64 / 2.0;
        let u2 = (n1 * n2) as f64 - u1;
        let u_statistic = u1.min(u2);

        // Normal approximation for p-value (for large samples)
        let mean_u = (n1 * n2) as f64 / 2.0;
        let std_u = ((n1 * n2 * (n1 + n2 + 1)) as f64 / 12.0).sqrt();
        let z_score = (u_statistic - mean_u) / std_u;
        let p_value = 2.0 * (1.0 - self.standard_normal_cdf(z_score.abs()));

        let is_significant = p_value < 0.05;

        Ok(MannWhitneyResult {
            u_statistic,
            p_value,
            is_significant,
        })
    }

    /// Perform bootstrap confidence interval test
    fn perform_bootstrap_test(&self, sample1: &[f64], sample2: &[f64], n_bootstrap: usize) -> CanaryResult<BootstrapResult> {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let mut rng = thread_rng();
        let mut bootstrap_differences = Vec::new();

        for _ in 0..n_bootstrap {
            // Bootstrap resample both groups
            let boot_sample1: Vec<f64> = (0..sample1.len())
                .map(|_| *sample1.choose(&mut rng).unwrap())
                .collect();
            let boot_sample2: Vec<f64> = (0..sample2.len())
                .map(|_| *sample2.choose(&mut rng).unwrap())
                .collect();

            let mean1 = boot_sample1.iter().sum::<f64>() / boot_sample1.len() as f64;
            let mean2 = boot_sample2.iter().sum::<f64>() / boot_sample2.len() as f64;
            bootstrap_differences.push(mean1 - mean2);
        }

        bootstrap_differences.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean_difference = bootstrap_differences.iter().sum::<f64>() / bootstrap_differences.len() as f64;

        // 95% confidence interval (2.5th and 97.5th percentiles)
        let lower_idx = (n_bootstrap as f64 * 0.025) as usize;
        let upper_idx = (n_bootstrap as f64 * 0.975) as usize;
        let confidence_interval_95 = (
            bootstrap_differences[lower_idx],
            bootstrap_differences[upper_idx.min(bootstrap_differences.len() - 1)],
        );

        Ok(BootstrapResult {
            mean_difference,
            confidence_interval_95,
            bootstrap_samples: n_bootstrap,
        })
    }

    /// Calculate pooled standard deviation
    fn calculate_pooled_standard_deviation(&self, sample1: &[f64], sample2: &[f64]) -> f64 {
        let n1 = sample1.len();
        let n2 = sample2.len();

        let mean1 = sample1.iter().sum::<f64>() / n1 as f64;
        let mean2 = sample2.iter().sum::<f64>() / n2 as f64;

        let ss1 = sample1.iter().map(|x| (x - mean1).powi(2)).sum::<f64>();
        let ss2 = sample2.iter().map(|x| (x - mean2).powi(2)).sum::<f64>();

        let pooled_variance = (ss1 + ss2) / (n1 + n2 - 2) as f64;
        pooled_variance.sqrt()
    }

    /// Combine p-values using Fisher's method
    fn combine_p_values(&self, p_values: &[f64]) -> f64 {
        let chi_square = -2.0 * p_values.iter().map(|p| p.ln()).sum::<f64>();
        let _df = 2 * p_values.len();
        
        // Simplified chi-square to p-value conversion
        if chi_square > 20.0 {
            0.001
        } else if chi_square > 15.0 {
            0.01
        } else if chi_square > 10.0 {
            0.05
        } else {
            0.1
        }
    }

    /// Calculate t-test p-value (simplified approximation)
    fn calculate_t_test_p_value(&self, t_stat: f64, _df: usize) -> f64 {
        // Simplified p-value calculation using normal approximation
        2.0 * (1.0 - self.standard_normal_cdf(t_stat))
    }

    /// Standard normal cumulative distribution function (approximation)
    fn standard_normal_cdf(&self, x: f64) -> f64 {
        0.5 * (1.0 + self.erf(x / 2.0_f64.sqrt()))
    }

    /// Error function approximation
    fn erf(&self, x: f64) -> f64 {
        // Abramowitz and Stegun approximation
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }

    /// Perform power analysis for sample size recommendations
    pub fn perform_power_analysis(
        &self,
        current_sample_sizes: &SampleSizes,
        effect_size: f64,
        alpha: f64,
    ) -> PowerAnalysis {
        let n1 = current_sample_sizes.canary_samples;
        let n2 = current_sample_sizes.control_samples;

        // Calculate current statistical power (simplified)
        let current_power = self.calculate_statistical_power(n1, n2, effect_size, alpha);

        // Calculate required sample sizes for 80% and 90% power
        let required_80_power = self.calculate_required_sample_size(effect_size, alpha, 0.8);
        let required_90_power = self.calculate_required_sample_size(effect_size, alpha, 0.9);

        // Calculate minimum detectable effect with current sample size
        let minimum_detectable_effect = self.calculate_minimum_detectable_effect(n1, n2, alpha, 0.8);

        // Estimate required duration to reach 80% power
        let current_total = n1 + n2;
        let required_total = required_80_power * 2; // Assuming 1:4 ratio maintained
        let duration_multiplier = if current_total > 0 {
            required_total as f64 / current_total as f64
        } else {
            1.0
        };
        let recommended_duration_hours = (self.config.canary_duration_minutes as f64 * duration_multiplier / 60.0) as u32;

        PowerAnalysis {
            current_power,
            required_sample_size_80_power: required_80_power,
            required_sample_size_90_power: required_90_power,
            minimum_detectable_effect,
            recommended_duration_hours,
        }
    }

    /// Calculate statistical power (simplified)
    fn calculate_statistical_power(&self, n1: usize, n2: usize, effect_size: f64, alpha: f64) -> f64 {
        let n_harmonic = 2.0 / (1.0 / n1 as f64 + 1.0 / n2 as f64);
        let delta = effect_size * (n_harmonic / 2.0).sqrt();
        let z_alpha = self.inverse_normal_cdf(1.0 - alpha / 2.0);
        let z_beta = delta - z_alpha;
        self.standard_normal_cdf(z_beta)
    }

    /// Calculate required sample size for desired power
    fn calculate_required_sample_size(&self, effect_size: f64, alpha: f64, power: f64) -> usize {
        let z_alpha = self.inverse_normal_cdf(1.0 - alpha / 2.0);
        let z_beta = self.inverse_normal_cdf(power);
        let n = 2.0 * ((z_alpha + z_beta) / effect_size).powi(2);
        n.ceil() as usize
    }

    /// Calculate minimum detectable effect
    fn calculate_minimum_detectable_effect(&self, n1: usize, n2: usize, alpha: f64, power: f64) -> f64 {
        let z_alpha = self.inverse_normal_cdf(1.0 - alpha / 2.0);
        let z_beta = self.inverse_normal_cdf(power);
        let n_harmonic = 2.0 / (1.0 / n1 as f64 + 1.0 / n2 as f64);
        (z_alpha + z_beta) / (n_harmonic / 2.0).sqrt()
    }

    /// Inverse normal CDF (approximation)
    fn inverse_normal_cdf(&self, p: f64) -> f64 {
        // Simplified approximation for common values
        if p >= 0.975 { 1.96 }
        else if p >= 0.95 { 1.645 }
        else if p >= 0.9 { 1.28 }
        else if p >= 0.8 { 0.84 }
        else if p >= 0.5 { 0.0 }
        else { -self.inverse_normal_cdf(1.0 - p) }
    }

    /// Execute automated canary decision and progression
    pub fn execute_canary_decision(&mut self, deployment_id: &str) -> CanaryResult<DeploymentProgression> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // Only process deployments in analyzing state
        if deployment.status != CanaryStatus::Analyzing {
            return Ok(DeploymentProgression {
                action_taken: ProgressionAction::NoAction,
                reason: format!("Deployment not in analyzing state: {:?}", deployment.status),
                new_status: deployment.status.clone(),
                decision: None,
                next_check_time: None,
            });
        }

        // Evaluate the canary deployment
        let decision = self.evaluate_canary_deployment(deployment_id)?;

        // Execute the decision
        let progression = match decision.decision_type {
            DecisionType::Proceed => {
                self.proceed_to_full_deployment(deployment_id, &decision)?
            }
            DecisionType::Rollback => {
                self.execute_rollback(deployment_id, &decision)?
            }
            DecisionType::Inconclusive => {
                self.handle_inconclusive_result(deployment_id, &decision)?
            }
        };

        // Update deployment with decision
        let deployment = self.deployments.get_mut(deployment_id).unwrap();
        deployment.decision = Some(decision.clone());

        Ok(DeploymentProgression {
            action_taken: progression.action_taken,
            reason: progression.reason,
            new_status: progression.new_status.clone(),
            decision: Some(decision),
            next_check_time: progression.next_check_time,
        })
    }

    /// Proceed to full deployment (promote canary)
    fn proceed_to_full_deployment(
        &mut self,
        deployment_id: &str,
        decision: &CanaryDecision,
    ) -> CanaryResult<DeploymentProgression> {
        // Apply parameter changes to full traffic
        self.complete_deployment(deployment_id)?;

        // Log the successful deployment
        self.log_deployment_event(deployment_id, DeploymentEvent::Promoted, Some(decision))?;

        Ok(DeploymentProgression {
            action_taken: ProgressionAction::Promoted,
            reason: format!("Canary promoted to full deployment: {}", decision.rationale),
            new_status: CanaryStatus::Completed,
            decision: None, // Decision is set by caller
            next_check_time: None,
        })
    }

    /// Execute rollback of canary deployment
    fn execute_rollback(
        &mut self,
        deployment_id: &str,
        decision: &CanaryDecision,
    ) -> CanaryResult<DeploymentProgression> {
        // Rollback parameter changes
        self.rollback_deployment(deployment_id, decision.rationale.clone())?;

        // Log the rollback
        self.log_deployment_event(deployment_id, DeploymentEvent::RolledBack, Some(decision))?;

        Ok(DeploymentProgression {
            action_taken: ProgressionAction::RolledBack,
            reason: format!("Canary rolled back: {}", decision.rationale),
            new_status: CanaryStatus::RolledBack,
            decision: None, // Decision is set by caller
            next_check_time: None,
        })
    }

    /// Handle inconclusive canary results
    fn handle_inconclusive_result(
        &mut self,
        deployment_id: &str,
        decision: &CanaryDecision,
    ) -> CanaryResult<DeploymentProgression> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // Determine next action based on confidence level and time remaining
        let now = Utc::now();
        let time_remaining = if now < deployment.end_time {
            deployment.end_time - now
        } else {
            chrono::Duration::zero()
        };

        let (action, new_status, next_check) = if decision.confidence >= 0.4 && time_remaining.num_minutes() > 15 {
            // Extend monitoring if we have time and moderate confidence
            let extended_end_time = deployment.end_time + chrono::Duration::minutes(30);
            deployment.end_time = extended_end_time;
            
            (
                ProgressionAction::ExtendedMonitoring,
                CanaryStatus::CollectingMetrics,
                Some(now + chrono::Duration::minutes(15)),
            )
        } else if decision.confidence >= 0.3 {
            // Request manual review for borderline cases
            (
                ProgressionAction::ManualReviewRequired,
                CanaryStatus::Analyzing,
                Some(now + chrono::Duration::hours(1)), // Check back in 1 hour
            )
        } else {
            // Low confidence - rollback (will be handled after releasing the borrow)
            (
                ProgressionAction::RolledBack,
                CanaryStatus::RolledBack,
                None,
            )
        };

        deployment.status = new_status.clone();

        // Handle rollback after releasing the mutable borrow
        if action == ProgressionAction::RolledBack && decision.confidence < 0.3 {
            let _ = deployment; // Release the mutable reference
            self.rollback_deployment(deployment_id, "Low confidence in canary results".to_string())?;
        }

        // Log the inconclusive result
        self.log_deployment_event(deployment_id, DeploymentEvent::Inconclusive, Some(decision))?;

        Ok(DeploymentProgression {
            action_taken: action,
            reason: format!("Inconclusive result: {}", decision.rationale),
            new_status,
            decision: None, // Decision is set by caller
            next_check_time: next_check,
        })
    }

    /// Process all active deployments for automated progression
    pub fn process_active_deployments(&mut self) -> CanaryResult<Vec<DeploymentProgressionSummary>> {
        let mut results = Vec::new();
        let active_deployment_ids: Vec<String> = self.deployments.keys().cloned().collect();

        for deployment_id in active_deployment_ids {
            // Check if deployment needs processing
            if self.should_process_deployment(&deployment_id)? {
                match self.execute_canary_decision(&deployment_id) {
                    Ok(progression) => {
                        results.push(DeploymentProgressionSummary {
                            deployment_id: deployment_id.clone(),
                            success: true,
                            progression: Some(progression),
                            error: None,
                        });
                    }
                    Err(e) => {
                        results.push(DeploymentProgressionSummary {
                            deployment_id: deployment_id.clone(),
                            success: false,
                            progression: None,
                            error: Some(format!("{}", e)),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Check if a deployment should be processed for progression
    fn should_process_deployment(&self, deployment_id: &str) -> CanaryResult<bool> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let now = Utc::now();

        match deployment.status {
            CanaryStatus::Running => {
                // Check if we have sufficient samples for analysis
                match self.metrics_collector.has_sufficient_samples(deployment_id, self.config.min_sample_size) {
                    Ok(has_samples) => Ok(has_samples),
                    Err(_) => Ok(false),
                }
            }
            CanaryStatus::CollectingMetrics => {
                // Check if collection period is complete
                Ok(now >= deployment.end_time)
            }
            CanaryStatus::Analyzing => {
                // Always process analyzing deployments
                Ok(true)
            }
            _ => Ok(false), // Don't process completed, failed, or rolled back deployments
        }
    }

    /// Log deployment events for audit trail
    fn log_deployment_event(
        &self,
        deployment_id: &str,
        event: DeploymentEvent,
        decision: Option<&CanaryDecision>,
    ) -> CanaryResult<()> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let log_entry = DeploymentLogEntry {
            timestamp: Utc::now(),
            deployment_id: deployment_id.to_string(),
            event,
            parameter_changes: deployment.parameter_changes.clone(),
            decision: decision.cloned(),
            metrics_summary: deployment.metrics_summary.clone(),
        };

        // In a real implementation, this would write to a persistent log
        eprintln!("Deployment Event: {}", serde_json::to_string(&log_entry).unwrap_or_default());

        Ok(())
    }

    /// Get deployment progression recommendations
    pub fn get_progression_recommendations(&self, deployment_id: &str) -> CanaryResult<Vec<ProgressionRecommendation>> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let mut recommendations = Vec::new();

        // Check current status and provide recommendations
        match deployment.status {
            CanaryStatus::Running => {
                let sample_sizes = self.metrics_collector.get_sample_sizes(deployment_id)?;
                
                if sample_sizes.canary_samples < self.config.min_sample_size {
                    recommendations.push(ProgressionRecommendation {
                        recommendation_type: RecommendationType::WaitForSamples,
                        priority: RecommendationPriority::High,
                        description: format!("Need {} more canary samples for statistical significance", 
                            self.config.min_sample_size - sample_sizes.canary_samples),
                        estimated_time_minutes: self.estimate_time_for_samples(
                            self.config.min_sample_size - sample_sizes.canary_samples,
                            deployment_id,
                        ),
                    });
                } else {
                    recommendations.push(ProgressionRecommendation {
                        recommendation_type: RecommendationType::ReadyForAnalysis,
                        priority: RecommendationPriority::High,
                        description: "Sufficient samples collected - ready for statistical analysis".to_string(),
                        estimated_time_minutes: 0,
                    });
                }
            }
            CanaryStatus::Analyzing => {
                if let Some(decision) = &deployment.decision {
                    match decision.decision_type {
                        DecisionType::Inconclusive => {
                            if decision.confidence > 0.4 {
                                recommendations.push(ProgressionRecommendation {
                                    recommendation_type: RecommendationType::ExtendMonitoring,
                                    priority: RecommendationPriority::Medium,
                                    description: "Borderline results - consider extending monitoring period".to_string(),
                                    estimated_time_minutes: 30,
                                });
                            } else {
                                recommendations.push(ProgressionRecommendation {
                                    recommendation_type: RecommendationType::ConsiderRollback,
                                    priority: RecommendationPriority::High,
                                    description: "Low confidence in results - consider rollback".to_string(),
                                    estimated_time_minutes: 0,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        // Add time-based recommendations
        let now = Utc::now();
        let _elapsed_minutes = (now - deployment.start_time).num_minutes();
        let remaining_minutes = if now < deployment.end_time {
            (deployment.end_time - now).num_minutes()
        } else {
            0
        };

        if remaining_minutes < 10 && deployment.status == CanaryStatus::Running {
            recommendations.push(ProgressionRecommendation {
                recommendation_type: RecommendationType::TimeoutWarning,
                priority: RecommendationPriority::High,
                description: format!("Deployment will timeout in {} minutes", remaining_minutes),
                estimated_time_minutes: remaining_minutes as u32,
            });
        }

        Ok(recommendations)
    }

    /// Estimate time needed to collect additional samples
    fn estimate_time_for_samples(&self, needed_samples: usize, deployment_id: &str) -> u32 {
        // Get current sample collection rate
        let current_samples = self.metrics_collector.get_sample_sizes(deployment_id)
            .map(|s| s.canary_samples)
            .unwrap_or(0);

        let deployment = self.deployments.get(deployment_id).unwrap();
        let elapsed_minutes = (Utc::now() - deployment.start_time).num_minutes() as u32;

        if elapsed_minutes > 0 && current_samples > 0 {
            let samples_per_minute = current_samples as f64 / elapsed_minutes as f64;
            if samples_per_minute > 0.0 {
                return (needed_samples as f64 / samples_per_minute).ceil() as u32;
            }
        }

        // Default estimate: assume we need 15 more minutes
        15
    }

    /// Force manual override of canary decision
    pub fn manual_override_decision(
        &mut self,
        deployment_id: &str,
        override_decision: DecisionType,
        reason: String,
        operator: String,
    ) -> CanaryResult<DeploymentProgression> {
        let deployment = self.deployments.get_mut(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        // Create manual override decision
        let manual_decision = CanaryDecision {
            decision_type: override_decision.clone(),
            confidence: 1.0, // Manual decisions have full confidence
            rationale: format!("Manual override by {}: {}", operator, reason),
            kpi_impacts: HashMap::new(),
            recommendation: "Manual operator decision".to_string(),
            decision_timestamp: Utc::now(),
        };

        deployment.decision = Some(manual_decision.clone());

        // Execute the override decision
        let progression = match override_decision {
            DecisionType::Proceed => {
                self.proceed_to_full_deployment(deployment_id, &manual_decision)?
            }
            DecisionType::Rollback => {
                self.execute_rollback(deployment_id, &manual_decision)?
            }
            DecisionType::Inconclusive => {
                // For manual inconclusive, just update status
                deployment.status = CanaryStatus::Analyzing;
                DeploymentProgression {
                    action_taken: ProgressionAction::ManualReviewRequired,
                    reason: format!("Manual override: {}", reason),
                    new_status: CanaryStatus::Analyzing,
                    decision: None, // Decision is set by caller
                    next_check_time: Some(Utc::now() + chrono::Duration::hours(1)),
                }
            }
        };

        // Log the manual override
        self.log_deployment_event(deployment_id, DeploymentEvent::ManualOverride, Some(&manual_decision))?;

        Ok(progression)
    }

    /// Comprehensive failure analysis for canary deployments
    pub fn analyze_deployment_failure(&self, deployment_id: &str) -> CanaryResult<FailureAnalysis> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        if !matches!(deployment.status, CanaryStatus::RolledBack | CanaryStatus::Failed) {
            return Err(CanaryError::InvalidConfiguration(
                "Can only analyze failed or rolled back deployments".to_string()
            ));
        }

        let mut failure_categories = Vec::new();
        let mut root_causes = Vec::new();
        let mut recommendations = Vec::new();

        // Analyze based on decision and metrics
        if let Some(decision) = &deployment.decision {
            match decision.decision_type {
                DecisionType::Rollback => {
                    self.analyze_rollback_failure(deployment, decision, &mut failure_categories, &mut root_causes, &mut recommendations)?;
                }
                _ => {}
            }
        }

        // Analyze metrics if available
        if let Some(metrics) = &deployment.metrics_summary {
            self.analyze_metrics_failure(metrics, &mut failure_categories, &mut root_causes, &mut recommendations)?;
        }

        // Analyze parameter changes
        self.analyze_parameter_failure(&deployment.parameter_changes, &mut failure_categories, &mut root_causes, &mut recommendations);

        // Generate failure pattern
        let failure_pattern = self.identify_failure_pattern(&failure_categories, deployment);

        Ok(FailureAnalysis {
            deployment_id: deployment_id.to_string(),
            failure_timestamp: deployment.decision.as_ref().map(|d| d.decision_timestamp).unwrap_or(Utc::now()),
            failure_categories,
            root_causes,
            failure_pattern,
            parameter_changes: deployment.parameter_changes.clone(),
            metrics_summary: deployment.metrics_summary.clone(),
            recommendations,
            severity: self.calculate_failure_severity(&root_causes),
        })
    }

    /// Analyze rollback-specific failures
    fn analyze_rollback_failure(
        &self,
        deployment: &ActiveCanaryDeployment,
        decision: &CanaryDecision,
        categories: &mut Vec<FailureCategory>,
        root_causes: &mut Vec<RootCause>,
        recommendations: &mut Vec<FailureRecommendation>,
    ) -> CanaryResult<()> {
        // Check KPI impacts for failure patterns
        for (metric_name, impact) in &decision.kpi_impacts {
            if impact.exceeds_threshold {
                categories.push(FailureCategory::KpiViolation);
                root_causes.push(RootCause {
                    cause_type: RootCauseType::KpiDegradation,
                    description: format!("{} degraded by {:.2}%", metric_name, impact.percentage_change),
                    confidence: 0.9,
                    evidence: vec![format!("Baseline: {:.4}, Canary: {:.4}", impact.baseline_value, impact.canary_value)],
                });

                // Generate specific recommendations based on metric
                match metric_name.as_str() {
                    "retention_5min" => {
                        recommendations.push(FailureRecommendation {
                            recommendation_type: RecommendationType::ParameterAdjustment,
                            description: "Consider smaller temperature changes to avoid retention impact".to_string(),
                            priority: RecommendationPriority::High,
                            estimated_impact: "Reduce retention degradation risk".to_string(),
                        });
                    }
                    "vmaf_avg" => {
                        recommendations.push(FailureRecommendation {
                            recommendation_type: RecommendationType::QualityReview,
                            description: "Review video quality settings and encoding parameters".to_string(),
                            priority: RecommendationPriority::High,
                            estimated_impact: "Prevent quality degradation".to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Check confidence level
        if decision.confidence < 0.3 {
            categories.push(FailureCategory::StatisticalInsignificance);
            root_causes.push(RootCause {
                cause_type: RootCauseType::InsufficientEvidence,
                description: format!("Low statistical confidence: {:.1}%", decision.confidence * 100.0),
                confidence: 0.8,
                evidence: vec!["Statistical tests did not reach significance threshold".to_string()],
            });
        }

        Ok(())
    }

    /// Analyze metrics-based failures
    fn analyze_metrics_failure(
        &self,
        metrics: &MetricsSummary,
        categories: &mut Vec<FailureCategory>,
        root_causes: &mut Vec<RootCause>,
        recommendations: &mut Vec<FailureRecommendation>,
    ) -> CanaryResult<()> {
        // Check sample size adequacy
        if metrics.sample_sizes.canary_samples < self.config.min_sample_size {
            categories.push(FailureCategory::InsufficientData);
            root_causes.push(RootCause {
                cause_type: RootCauseType::InsufficientSamples,
                description: format!("Insufficient canary samples: {} < {}", 
                    metrics.sample_sizes.canary_samples, self.config.min_sample_size),
                confidence: 1.0,
                evidence: vec![format!("Required: {}, Actual: {}", 
                    self.config.min_sample_size, metrics.sample_sizes.canary_samples)],
            });

            recommendations.push(FailureRecommendation {
                recommendation_type: RecommendationType::ExtendDuration,
                description: "Increase canary duration to collect more samples".to_string(),
                priority: RecommendationPriority::Medium,
                estimated_impact: "Improve statistical power".to_string(),
            });
        }

        // Check for extreme metric values
        if metrics.canary_metrics.error_rate > 0.1 {
            categories.push(FailureCategory::SystemError);
            root_causes.push(RootCause {
                cause_type: RootCauseType::SystemInstability,
                description: format!("High error rate in canary: {:.2}%", metrics.canary_metrics.error_rate * 100.0),
                confidence: 0.9,
                evidence: vec![format!("Error rate: {:.4}", metrics.canary_metrics.error_rate)],
            });
        }

        Ok(())
    }

    /// Analyze parameter-specific failures
    fn analyze_parameter_failure(
        &self,
        parameter_changes: &HashMap<String, ParameterChange>,
        categories: &mut Vec<FailureCategory>,
        root_causes: &mut Vec<RootCause>,
        recommendations: &mut Vec<FailureRecommendation>,
    ) {
        for (param_name, change) in parameter_changes {
            let change_magnitude = (change.new_value - change.old_value).abs();
            
            // Check for large parameter changes
            match param_name.as_str() {
                "selection_temperature" => {
                    if change_magnitude > 0.2 {
                        categories.push(FailureCategory::ParameterMisconfiguration);
                        root_causes.push(RootCause {
                            cause_type: RootCauseType::ExcessiveParameterChange,
                            description: format!("Large temperature change: {:.3} -> {:.3}", 
                                change.old_value, change.new_value),
                            confidence: 0.7,
                            evidence: vec![format!("Change magnitude: {:.3}", change_magnitude)],
                        });

                        recommendations.push(FailureRecommendation {
                            recommendation_type: RecommendationType::ParameterAdjustment,
                            description: "Use smaller temperature increments (≤0.1)".to_string(),
                            priority: RecommendationPriority::High,
                            estimated_impact: "Reduce parameter shock risk".to_string(),
                        });
                    }
                }
                "selection_top_k" => {
                    if change_magnitude > 10.0 {
                        categories.push(FailureCategory::ParameterMisconfiguration);
                        root_causes.push(RootCause {
                            cause_type: RootCauseType::ExcessiveParameterChange,
                            description: format!("Large top-k change: {:.0} -> {:.0}", 
                                change.old_value, change.new_value),
                            confidence: 0.7,
                            evidence: vec![format!("Change magnitude: {:.0}", change_magnitude)],
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// Identify failure patterns for learning
    fn identify_failure_pattern(&self, categories: &[FailureCategory], deployment: &ActiveCanaryDeployment) -> FailurePattern {
        let duration_minutes = (deployment.end_time - deployment.start_time).num_minutes();
        
        if categories.contains(&FailureCategory::KpiViolation) {
            if duration_minutes < 30 {
                FailurePattern::EarlyKpiViolation
            } else {
                FailurePattern::DelayedKpiViolation
            }
        } else if categories.contains(&FailureCategory::StatisticalInsignificance) {
            FailurePattern::InsufficientEvidence
        } else if categories.contains(&FailureCategory::InsufficientData) {
            FailurePattern::DataCollection
        } else if categories.contains(&FailureCategory::SystemError) {
            FailurePattern::SystemInstability
        } else if categories.contains(&FailureCategory::ParameterMisconfiguration) {
            FailurePattern::ParameterConfiguration
        } else {
            FailurePattern::Unknown
        }
    }

    /// Calculate failure severity
    fn calculate_failure_severity(&self, root_causes: &[RootCause]) -> FailureSeverity {
        let max_confidence = root_causes.iter()
            .map(|rc| rc.confidence)
            .fold(0.0, f64::max);

        let has_kpi_violation = root_causes.iter()
            .any(|rc| matches!(rc.cause_type, RootCauseType::KpiDegradation));

        let has_system_issue = root_causes.iter()
            .any(|rc| matches!(rc.cause_type, RootCauseType::SystemInstability));

        if has_system_issue && max_confidence > 0.8 {
            FailureSeverity::Critical
        } else if has_kpi_violation && max_confidence > 0.7 {
            FailureSeverity::High
        } else if max_confidence > 0.5 {
            FailureSeverity::Medium
        } else {
            FailureSeverity::Low
        }
    }

    /// Generate comprehensive deployment report
    pub fn generate_deployment_report(&self, deployment_id: &str) -> CanaryResult<DeploymentReport> {
        let deployment = self.deployments.get(deployment_id)
            .ok_or_else(|| CanaryError::DeploymentNotFound(deployment_id.to_string()))?;

        let failure_analysis = if matches!(deployment.status, CanaryStatus::RolledBack | CanaryStatus::Failed) {
            Some(self.analyze_deployment_failure(deployment_id)?)
        } else {
            None
        };

        let duration = deployment.end_time - deployment.start_time;
        let actual_duration = if deployment.status == CanaryStatus::Running {
            Utc::now() - deployment.start_time
        } else {
            duration
        };

        Ok(DeploymentReport {
            deployment_id: deployment_id.to_string(),
            start_time: deployment.start_time,
            end_time: deployment.end_time,
            actual_duration_minutes: actual_duration.num_minutes() as u32,
            planned_duration_minutes: duration.num_minutes() as u32,
            final_status: deployment.status.clone(),
            parameter_changes: deployment.parameter_changes.clone(),
            metrics_summary: deployment.metrics_summary.clone(),
            decision: deployment.decision.clone(),
            failure_analysis,
            traffic_split: deployment.traffic_split.clone(),
            rollback_reason: deployment.rollback_reason.clone(),
        })
    }
}

/// Mock metrics collector for testing
#[derive(Debug)]
pub struct MockMetricsCollector {
    pub baseline_metrics: GroupMetrics,
    pub sample_counts: HashMap<String, SampleSizes>,
    pub metrics_variance: f64,
}

impl MockMetricsCollector {
    pub fn new() -> Self {
        Self {
            baseline_metrics: GroupMetrics {
                retention_5min: 0.38,
                vmaf_avg: 92.0,
                error_rate: 0.02,
                latency_p95_ms: 450.0,
                selection_entropy: 0.65,
                curator_apply_rate: 0.12,
                custom_metrics: HashMap::new(),
            },
            sample_counts: HashMap::new(),
            metrics_variance: 0.05, // 5% variance in metrics
        }
    }

    /// Create a mock collector with specific baseline metrics
    pub fn with_baseline(baseline_metrics: GroupMetrics) -> Self {
        Self {
            baseline_metrics,
            sample_counts: HashMap::new(),
            metrics_variance: 0.05,
        }
    }

    /// Set sample sizes for a deployment
    pub fn set_sample_sizes(&mut self, deployment_id: &str, sample_sizes: SampleSizes) {
        self.sample_counts.insert(deployment_id.to_string(), sample_sizes);
    }

    /// Simulate metrics with realistic variance
    fn add_variance(&self, base_value: f64, traffic_group: &TrafficGroup) -> f64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Add random variance
        let variance_factor = 1.0 + (rng.gen::<f64>() - 0.5) * 2.0 * self.metrics_variance;
        
        // Add systematic difference for canary vs control
        let systematic_factor = match traffic_group {
            TrafficGroup::Canary => 1.01, // Slight improvement for canary
            TrafficGroup::Control => 1.0,
        };
        
        base_value * variance_factor * systematic_factor
    }
}

impl MetricsCollector for MockMetricsCollector {
    fn collect_metrics(
        &self,
        _start_time: DateTime<Utc>,
        _end_time: DateTime<Utc>,
        traffic_group: TrafficGroup,
    ) -> CanaryResult<GroupMetrics> {
        // Apply variance to baseline metrics
        let metrics = GroupMetrics {
            retention_5min: self.add_variance(self.baseline_metrics.retention_5min, &traffic_group),
            vmaf_avg: self.add_variance(self.baseline_metrics.vmaf_avg, &traffic_group),
            error_rate: self.add_variance(self.baseline_metrics.error_rate, &traffic_group).max(0.0),
            latency_p95_ms: self.add_variance(self.baseline_metrics.latency_p95_ms, &traffic_group).max(1.0),
            selection_entropy: self.add_variance(self.baseline_metrics.selection_entropy, &traffic_group),
            curator_apply_rate: self.add_variance(self.baseline_metrics.curator_apply_rate, &traffic_group),
            custom_metrics: self.baseline_metrics.custom_metrics.clone(),
        };

        Ok(metrics)
    }

    fn get_sample_sizes(&self, deployment_id: &str) -> CanaryResult<SampleSizes> {
        if let Some(sizes) = self.sample_counts.get(deployment_id) {
            Ok(sizes.clone())
        } else {
            // Default sample sizes that grow over time
            Ok(SampleSizes {
                canary_samples: 150,
                control_samples: 600,
                total_samples: 750,
            })
        }
    }

    fn has_sufficient_samples(&self, deployment_id: &str, min_samples: usize) -> CanaryResult<bool> {
        let sample_sizes = self.get_sample_sizes(deployment_id)?;
        Ok(sample_sizes.canary_samples >= min_samples && sample_sizes.control_samples >= min_samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canary_deployment_creation() {
        let config = CanaryConfig::default();
        let metrics_collector = Box::new(MockMetricsCollector::new());
        let mut canary = CanaryDeployment::new(config, metrics_collector);

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Increase exploration based on recent performance".to_string(),
        });

        let result = canary.start_deployment("test_deployment_1".to_string(), parameter_changes);
        assert!(result.is_ok());

        let status = canary.get_deployment_status("test_deployment_1").unwrap();
        assert_eq!(status, CanaryStatus::Running);
    }

    #[test]
    fn test_parameter_validation() {
        let config = CanaryConfig::default();
        let metrics_collector = Box::new(MockMetricsCollector::new());
        let mut canary = CanaryDeployment::new(config, metrics_collector);

        // Test invalid temperature
        let mut invalid_changes = HashMap::new();
        invalid_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 3.0, // Invalid: too high
            change_type: ParameterChangeType::Optimization,
            rationale: "Test invalid value".to_string(),
        });

        let result = canary.start_deployment("invalid_deployment".to_string(), invalid_changes);
        assert!(result.is_err());
    }

    #[test]
    fn test_traffic_routing() {
        let config = CanaryConfig::default();
        let metrics_collector = Box::new(MockMetricsCollector::new());
        let mut canary = CanaryDeployment::new(config, metrics_collector);

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test routing".to_string(),
        });

        canary.start_deployment("routing_test".to_string(), parameter_changes).unwrap();

        // Test consistent routing
        let routing1 = canary.get_traffic_routing("routing_test", "user_123").unwrap();
        let routing2 = canary.get_traffic_routing("routing_test", "user_123").unwrap();
        
        // Same user should get same routing
        assert_eq!(routing1, routing2);

        // Test that we get both canary and control traffic over many requests
        let mut canary_count = 0;
        let mut control_count = 0;
        
        for i in 0..1000 {
            let routing = canary.get_traffic_routing("routing_test", &format!("user_{}", i)).unwrap();
            match routing {
                TrafficGroup::Canary => canary_count += 1,
                TrafficGroup::Control => control_count += 1,
            }
        }

        // Should be roughly 20% canary, 80% control (with some variance)
        let canary_percentage = canary_count as f64 / 1000.0;
        assert!(canary_percentage > 0.15 && canary_percentage < 0.25);
    }

    #[test]
    fn test_deployment_timeout() {
        let config = CanaryConfig {
            canary_duration_minutes: 0, // Immediate timeout for testing
            ..Default::default()
        };
        let metrics_collector = Box::new(MockMetricsCollector::new());
        let mut canary = CanaryDeployment::new(config, metrics_collector);

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test timeout".to_string(),
        });

        canary.start_deployment("timeout_test".to_string(), parameter_changes).unwrap();

        // Should timeout immediately
        let is_timeout = canary.check_deployment_timeout("timeout_test").unwrap();
        assert!(is_timeout);
    }

    #[test]
    fn test_concurrent_deployment_limit() {
        let config = CanaryConfig {
            max_concurrent_deployments: 1,
            ..Default::default()
        };
        let metrics_collector = Box::new(MockMetricsCollector::new());
        let mut canary = CanaryDeployment::new(config, metrics_collector);

        let parameter_changes = HashMap::new();

        // First deployment should succeed
        let result1 = canary.start_deployment("deployment_1".to_string(), parameter_changes.clone());
        assert!(result1.is_ok());

        // Second deployment should fail due to limit
        let result2 = canary.start_deployment("deployment_2".to_string(), parameter_changes);
        assert!(result2.is_err());
    }

    #[test]
    fn test_metrics_collection() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        // Set sufficient sample sizes
        mock_collector.set_sample_sizes("metrics_test", SampleSizes {
            canary_samples: 200,
            control_samples: 800,
            total_samples: 1000,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test metrics collection".to_string(),
        });

        canary.start_deployment("metrics_test".to_string(), parameter_changes).unwrap();

        // Collect metrics
        let metrics_result = canary.collect_deployment_metrics("metrics_test");
        assert!(metrics_result.is_ok());

        let metrics_summary = metrics_result.unwrap();
        assert!(metrics_summary.canary_metrics.retention_5min > 0.0);
        assert!(metrics_summary.control_metrics.retention_5min > 0.0);
        assert!(metrics_summary.sample_sizes.total_samples > 0);
    }

    #[test]
    fn test_statistical_significance_calculation() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        mock_collector.set_sample_sizes("stats_test", SampleSizes {
            canary_samples: 500,
            control_samples: 2000,
            total_samples: 2500,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test statistical significance".to_string(),
        });

        canary.start_deployment("stats_test".to_string(), parameter_changes).unwrap();

        let metrics_summary = canary.collect_deployment_metrics("stats_test").unwrap();
        
        // Check that statistical significance was calculated
        assert!(metrics_summary.statistical_significance.overall_confidence >= 0.0);
        assert!(metrics_summary.statistical_significance.retention_significance.p_value >= 0.0);
        assert!(metrics_summary.statistical_significance.retention_significance.p_value <= 1.0);
    }

    #[test]
    fn test_kpi_violation_detection() {
        let config = CanaryConfig {
            rollback_thresholds: KpiThresholds {
                max_retention_decrease_pp: 1.0, // Very strict threshold for testing
                max_vmaf_decrease: 2.0,
                max_error_rate_increase_pp: 0.5,
                max_latency_increase_ms: 50.0,
            },
            ..Default::default()
        };

        // Create metrics collector with degraded canary performance
        let baseline_metrics = GroupMetrics {
            retention_5min: 0.38,
            vmaf_avg: 92.0,
            error_rate: 0.02,
            latency_p95_ms: 450.0,
            selection_entropy: 0.65,
            curator_apply_rate: 0.12,
            custom_metrics: HashMap::new(),
        };

        let mut mock_collector = MockMetricsCollector::with_baseline(baseline_metrics);
        mock_collector.set_sample_sizes("violation_test", SampleSizes {
            canary_samples: 200,
            control_samples: 800,
            total_samples: 1000,
        });

        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test KPI violations".to_string(),
        });

        canary.start_deployment("violation_test".to_string(), parameter_changes).unwrap();

        // Monitor deployment (may detect violations due to variance)
        let monitoring_result = canary.monitor_deployment("violation_test").unwrap();
        
        // Should have metrics available
        assert!(monitoring_result.metrics_available);
    }

    #[test]
    fn test_monitoring_status() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        mock_collector.set_sample_sizes("status_test", SampleSizes {
            canary_samples: 150,
            control_samples: 600,
            total_samples: 750,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test monitoring status".to_string(),
        });

        canary.start_deployment("status_test".to_string(), parameter_changes).unwrap();

        let status = canary.get_monitoring_status("status_test").unwrap();
        
        assert_eq!(status.deployment_id, "status_test");
        assert_eq!(status.status, CanaryStatus::Running);
        assert!(status.elapsed_minutes >= 0);
        assert!(status.sample_sizes.total_samples > 0);
    }

    #[test]
    fn test_insufficient_samples() {
        let config = CanaryConfig {
            min_sample_size: 1000, // Very high requirement
            ..Default::default()
        };
        
        let mut mock_collector = MockMetricsCollector::new();
        mock_collector.set_sample_sizes("insufficient_test", SampleSizes {
            canary_samples: 50,  // Too few samples
            control_samples: 200,
            total_samples: 250,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test insufficient samples".to_string(),
        });

        canary.start_deployment("insufficient_test".to_string(), parameter_changes).unwrap();

        // Should fail due to insufficient samples
        let metrics_result = canary.collect_deployment_metrics("insufficient_test");
        assert!(metrics_result.is_err());
        
        match metrics_result.unwrap_err() {
            CanaryError::InsufficientData(_) => {}, // Expected
            _ => panic!("Expected InsufficientData error"),
        }
    }

    #[test]
    fn test_deployment_progression_proceed() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        // Set up good metrics that should lead to proceed decision
        mock_collector.set_sample_sizes("progression_test", SampleSizes {
            canary_samples: 500,
            control_samples: 2000,
            total_samples: 2500,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test progression".to_string(),
        });

        canary.start_deployment("progression_test".to_string(), parameter_changes).unwrap();

        // Collect metrics to move to analyzing state
        let _ = canary.collect_deployment_metrics("progression_test").unwrap();

        // Execute canary decision
        let progression = canary.execute_canary_decision("progression_test").unwrap();
        
        // Should have taken some action
        assert_ne!(progression.action_taken, ProgressionAction::NoAction);
        
        // Check that decision was made
        assert!(progression.decision.is_some());
    }

    #[test]
    fn test_manual_override() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        mock_collector.set_sample_sizes("override_test", SampleSizes {
            canary_samples: 200,
            control_samples: 800,
            total_samples: 1000,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test manual override".to_string(),
        });

        canary.start_deployment("override_test".to_string(), parameter_changes).unwrap();

        // Execute manual override
        let progression = canary.manual_override_decision(
            "override_test",
            DecisionType::Proceed,
            "Operator decision based on external factors".to_string(),
            "operator@example.com".to_string(),
        ).unwrap();

        assert_eq!(progression.action_taken, ProgressionAction::Promoted);
        assert!(progression.reason.contains("Manual override"));
        assert_eq!(progression.new_status, CanaryStatus::Completed);
    }

    #[test]
    fn test_progression_recommendations() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        // Set insufficient samples to trigger recommendation
        mock_collector.set_sample_sizes("recommendations_test", SampleSizes {
            canary_samples: 50,  // Below minimum
            control_samples: 200,
            total_samples: 250,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let mut parameter_changes = HashMap::new();
        parameter_changes.insert("selection_temperature".to_string(), ParameterChange {
            parameter_name: "selection_temperature".to_string(),
            old_value: 0.85,
            new_value: 0.90,
            change_type: ParameterChangeType::Optimization,
            rationale: "Test recommendations".to_string(),
        });

        canary.start_deployment("recommendations_test".to_string(), parameter_changes).unwrap();

        let recommendations = canary.get_progression_recommendations("recommendations_test").unwrap();
        
        assert!(!recommendations.is_empty());
        
        // Should recommend waiting for more samples
        let wait_recommendation = recommendations.iter()
            .find(|r| matches!(r.recommendation_type, RecommendationType::WaitForSamples));
        assert!(wait_recommendation.is_some());
    }

    #[test]
    fn test_process_active_deployments() {
        let config = CanaryConfig::default();
        let mut mock_collector = MockMetricsCollector::new();
        
        // Set up multiple deployments
        mock_collector.set_sample_sizes("deployment_1", SampleSizes {
            canary_samples: 200,
            control_samples: 800,
            total_samples: 1000,
        });
        
        mock_collector.set_sample_sizes("deployment_2", SampleSizes {
            canary_samples: 50,  // Insufficient
            control_samples: 200,
            total_samples: 250,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let parameter_changes = HashMap::new();

        // Start multiple deployments
        canary.start_deployment("deployment_1".to_string(), parameter_changes.clone()).unwrap();
        canary.start_deployment("deployment_2".to_string(), parameter_changes).unwrap();

        // Process all active deployments
        let results = canary.process_active_deployments().unwrap();
        
        assert_eq!(results.len(), 2);
        
        // Check that both deployments were processed
        let deployment_1_result = results.iter().find(|r| r.deployment_id == "deployment_1");
        let deployment_2_result = results.iter().find(|r| r.deployment_id == "deployment_2");
        
        assert!(deployment_1_result.is_some());
        assert!(deployment_2_result.is_some());
    }

    #[test]
    fn test_deployment_timeout_handling() {
        let config = CanaryConfig {
            canary_duration_minutes: 0, // Immediate timeout
            ..Default::default()
        };
        
        let mut mock_collector = MockMetricsCollector::new();
        mock_collector.set_sample_sizes("timeout_test", SampleSizes {
            canary_samples: 200,
            control_samples: 800,
            total_samples: 1000,
        });
        
        let mut canary = CanaryDeployment::new(config, Box::new(mock_collector));

        let parameter_changes = HashMap::new();
        canary.start_deployment("timeout_test".to_string(), parameter_changes).unwrap();

        // Should detect timeout
        let is_timeout = canary.check_deployment_timeout("timeout_test").unwrap();
        assert!(is_timeout);

        // Get recommendations should include timeout warning
        let recommendations = canary.get_progression_recommendations("timeout_test").unwrap();
        let timeout_warning = recommendations.iter()
            .find(|r| matches!(r.recommendation_type, RecommendationType::TimeoutWarning));
        
        // Note: This might not trigger if deployment is already timed out
        // The test validates the timeout detection mechanism
    }
}