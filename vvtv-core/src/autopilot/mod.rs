//! Autopilot system for autonomous parameter optimization
//! 
//! This module implements P4 (Autopilot D+1 & Drift Guards) which provides:
//! - Daily automated parameter optimization based on business metrics
//! - Sliding bounds safety system with dynamic constraints
//! - Canary deployment system with statistical validation
//! - Anti-drift monitoring and protection mechanisms
//! - Parameter history and versioning with rollback capabilities
//! - Incident triage and learning system

pub mod canary;
pub mod engine;
pub mod logging;
pub mod optimizer;
pub mod scheduler;
pub mod sliding_bounds;
pub mod validation;

pub use engine::{
    AutopilotEngine, AutopilotConfig, AutopilotCycle, AutopilotError, AutopilotResult,
    AutopilotStatus, CycleStatus, DataQuality, MetricsAnalysis, OptimizationOpportunity,
    ParameterChange, TrendAnalysis, TrendDirection, ValidationResults, DeploymentResult, 
    ChangeType, ExpectedImpact,
};

pub use scheduler::{
    DailyScheduler, SchedulerConfig, SchedulerState, SchedulerStatus, SchedulerError,
};

pub use optimizer::{
    ChangeValidation, OptimizationAlgorithm, OptimizationRecord, OptimizationStats,
    OptimizerConfig, ParameterOptimizer,
};

pub use logging::{
    AutopilotEventType, AutopilotLogEntry, AutopilotLogger, AutopilotMetrics,
    LogLevel, LoggingConfig, LoggingStats, PredictionAccuracyRecord,
};

pub use sliding_bounds::{
    AntiWindupStrategy, BoundsAdjustment, BoundsAdjustmentHistory, BoundsAdjustmentReport,
    BoundsRecommendation, ConfigurationChange, ConfigurationChangeType, ContractionRecommendation, 
    ExpansionRecommendation, ExpansionStrategy, FailurePattern, FailureSeverity, 
    OscillationAnalysis, OscillationRecommendation, ParameterAdjustmentSummary, ParameterBounds, 
    ParameterChange as BoundsParameterChange, ParameterMetadata, ParameterStats, 
    RecommendationPriority, RecommendationType, SlidingBounds, SlidingBoundsConfig, 
    SlidingBoundsError, SlidingBoundsResult, SlidingBoundsState, StabilityAnalysis, 
    StabilityTrend,
};

pub use canary::{
    ActiveCanaryDeployment, AdvancedStatisticalTest, BayesianResult, BootstrapResult, CanaryConfig, 
    CanaryDecision, CanaryDeployment, CanaryError, CanaryResult, CanaryStatus, DecisionType, 
    DeploymentEvent, DeploymentLogEntry, DeploymentProgression, DeploymentProgressionSummary,
    GateCheck, GroupMetrics, ImprovementDirection, KpiGateResult, KpiImpact, KpiThresholds, 
    KpiViolation, MannWhitneyResult, MetricsCollector, MetricsSummary, MockMetricsCollector, 
    MonitoringResult, MonitoringStatus, ParameterChange as CanaryParameterChange, ParameterChangeType, 
    PowerAnalysis, ProgressionAction, ProgressionRecommendation, RoutingStrategy, SampleSizes, 
    SignificanceTest, StatisticalSignificance, 
    TTestResult, TrafficGroup, TrafficSplit,
};

pub use validation::{
    BoundsCheckResult, BusinessRule, BusinessRuleType, ChangeLimit, ConstraintCheckResult,
    DependencyCheckResult, ParameterConstraints, ParameterDataType, ParameterDependency,
    ParameterValidator, RateLimitCheckResult, SafetyCheckResult, SafetyRule, SafetyRuleType,
    SafetySeverity, ValidationConfig, ValidationError, ValidationReport, ValidationResult,
    ValidationStats, ValidationWarning, WarningType, WarningSeverity,
};