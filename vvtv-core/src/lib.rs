#![allow(
    clippy::default_constructed_unit_structs,
    clippy::redundant_closure,
    clippy::let_and_return,
    clippy::needless_question_mark,
    clippy::manual_strip,
    clippy::field_reassign_with_default,
    clippy::unnecessary_cast,
    clippy::result_large_err
)]

pub mod broadcaster;
pub mod browser;
pub mod business_logic;
pub mod compliance;
pub mod config;
pub mod curation;
pub mod distribution;
pub mod error;
pub mod incident;
pub mod llm;
pub mod monetization;
pub mod monitor;
pub mod plan;
pub mod autopilot;
pub mod processor;
pub mod quality;
pub mod queue;
mod sqlite;
pub mod test_framework;

pub use broadcaster::{
    failover::{FailoverError, FailoverManager},
    watchdog::{Watchdog, WatchdogAction, WatchdogError, WatchdogReport},
    Broadcaster, BroadcasterError, BroadcasterEvent, BroadcasterPaths, CommandExecutor,
    SystemCommandExecutor,
};
pub use browser::{
    BrowserAutomation, BrowserCapture, BrowserCaptureKind, BrowserError, BrowserEvent,
    BrowserLauncher, BrowserMetrics, BrowserPbdRunner, BrowserProfile, BrowserQaRunner,
    BrowserResult, BrowserSearchSessionFactory, ContentSearcher, DiscoveryConfig, DiscoveryLoop,
    DiscoveryPbd, DiscoveryPlanStore, DiscoveryStats, HumanMotionController, MetadataExtractor,
    PbdOutcome, PlayBeforeDownload, PlaybackValidation, ProfileManager, QaDashboard,
    QaMetricsStore, QaStatistics, SearchConfig, SearchEngine, SearchResultRaw, SearchSession,
    SearchSessionFactory, SessionRecorder, SessionRecorderConfig, SmokeMode, SmokeTestOptions,
    SmokeTestResult,
};
pub use business_logic::{
    Autopilot as BusinessAutopilot, BusinessLogic, BusinessLogicError, Exploration,
    Knobs as BusinessKnobs, Kpis as BusinessKpis, Result as BusinessLogicResult,
    Scheduling as BusinessScheduling, SeedStrategy, Selection as BusinessSelection,
    SelectionMethod,
};
pub use compliance::{
    ComplianceError, ComplianceResult, ComplianceSuite, ComplianceSuiteConfig, ComplianceSummary,
    ConsentLogEntry, CsamHashEntry, CsamScanFinding, CsamScanReport, CsamScanner,
    DrmDetectionConfig, DrmScanFinding, DrmScanReport, DrmScanner, LicenseAuditFinding,
    LicenseAuditFindingKind, LicenseAuditReport, LicenseAuditSummary, LicenseAuditor,
};
pub use config::{
    load_broadcaster_config, load_browser_config, load_processor_config, load_vvtv_config,
    BroadcasterConfig, BrowserConfig, ConfigBundle, ProcessorConfig, VvtvConfig,
};
pub use curation::{
    CuratorDecision, CuratorEvaluation, CuratorSignal, CuratorVigilante, CuratorVigilanteConfig,
    TokenBucket,
};
pub use distribution::{
    cdn::{
        BackupCdnConfig, BackupCdnManager, BackupSyncReport, CdnCoordinator, CdnError, CdnMetrics,
        PrimaryCdnConfig, PrimaryCdnManager,
    },
    edge::{
        EdgeBufferStatus, EdgeConfig, EdgeError, EdgeLatencyRecord, EdgeOrchestrator, EdgeSummary,
        SegmentSnapshot,
    },
    replicator::{
        ReplicationCheckReport, ReplicationError, ReplicationManager, ReplicationReport,
        ReplicationSyncReport,
    },
    security::{DistributionSecurity, DistributionSecurityError, SecurityConfig, SegmentToken},
    DistributionCycleReport, DistributionError, DistributionManager,
};
pub use error::{ConfigError, Result};
pub use incident::{
    DispatchAction, DispatchStatus, IncidentChannel, IncidentCommunicationsConfig,
    IncidentDispatch, IncidentError, IncidentHistoryRecord, IncidentHistoryWriter,
    IncidentNotification, IncidentNotifier, IncidentReport, IncidentSeverity,
    IncidentTimelineEntry, SeverityRouting,
};
pub use llm::{
    CircuitBreakerConfig, LlmAction, LlmHookKind, LlmHookOutcome, LlmInvocation,
    LlmInvocationResult, LlmOrchestrator, LlmResultMode,
};
pub use monetization::{
    AdaptiveError, AdaptiveProgrammer, AdaptiveReport, AdaptiveResult, AdaptiveUpdate,
    AudienceError, AudienceMetrics, AudienceReport, AudienceResult, AudienceSnapshot,
    AudienceStore, AudienceStoreBuilder, DashboardArtifacts, DashboardError, DashboardResult,
    DesireVector, EconomyError, EconomyEvent, EconomyEventType, EconomyResult, EconomyStore,
    EconomyStoreBuilder, EconomySummary, LedgerExport, MicroSpotContract, MicroSpotInjection,
    MicroSpotManager, MonetizationDashboard, NewEconomyEvent, NewViewerSession, SpotsError,
    SpotsResult, ViewerSession,
};
pub use monitor::{
    AlertChannel, AlertCondition, AlertEngine, AlertNotification, AlertRule, AlertSeverity,
    AlertState, AlertStateType, BusinessMetric, BusinessMetricType, Dashboard, DashboardAlert,
    DashboardConfig, DashboardFormat, DashboardGenerator, DashboardPanel, DataPoint,
    EnhancedDashboardGenerator, LiveQcSample, LiveQualityCollector, LogAlertChannel, MetricRecord,
    MetricSnapshot, MetricSeries, MetricsStore, MonitorError, PanelType, QcReportGenerator,
    TemplateEngine, Threshold, ThresholdCondition, TimeRange, VisualReviewPanel,
};
pub use plan::{
    Plan, PlanAdaptiveUpdate, PlanAuditFinding, PlanAuditKind, PlanBlacklistEntry, PlanError,
    PlanImportRecord, PlanMetrics, PlanResult, PlanSelectionDecision, PlanStatus, Planner,
    PlannerConfig, PlannerEvent, RealizationOutcome, Realizer, RealizerConfig, SqlitePlanStore,
    SqlitePlanStoreBuilder,
};
pub use processor::{
    Processor, ProcessorError, ProcessorReport, ProcessorResult, StagingPaths, MASTER_PLAYLIST_NAME,
};
pub use quality::{
    QualityAction, QualityActionKind, QualityAnalyzer, QualityReport, QualityResult,
    QualityThresholds, SignatureProfile,
};
pub use queue::{
    PlayoutQueueStore, PlayoutQueueStoreBuilder, QueueEntry, QueueError, QueueFilter, QueueItem,
    QueueMetrics, QueueResult, QueueSelectionPolicy, QueueStatus, QueueSummary,
};
pub use test_framework::{
    BenchmarkComparison, BenchmarkResult, CanaryTestResult, DailyStabilityMetric, DriftTestResult,
    EndToEndTestResult, GoldenSelectionTest, GoldenTestCase, GoldenTestResult,
    IntegrationTestConfig, IntegrationTestSuite, IntegrationTestSuiteResult, LlmStressTest,
    PerformanceBenchmarks, StressTestConfig, StressTestResult, TestEnvironment,
};
pub use autopilot::{
    AutopilotConfig, AutopilotCycle, AutopilotEngine, AutopilotError, AutopilotEventType,
    AutopilotLogEntry, AutopilotLogger, AutopilotMetrics, AutopilotResult, AutopilotStatus,
    BoundsAdjustment, ChangeType, ChangeValidation, CycleStatus, DailyScheduler, DataQuality,
    ExpectedImpact, LogLevel, LoggingConfig, LoggingStats, MetricsAnalysis, OptimizationAlgorithm,
    OptimizationOpportunity, OptimizationRecord, OptimizationStats, OptimizerConfig,
    ParameterBounds, ParameterChange, ParameterOptimizer, ParameterStats, PredictionAccuracyRecord,
    BoundsParameterChange, ExpansionRecommendation, ExpansionStrategy, AntiWindupStrategy,
    ContractionRecommendation, FailurePattern, FailureSeverity, OscillationAnalysis,
    OscillationRecommendation, BoundsAdjustmentHistory, BoundsAdjustmentReport,
    BoundsRecommendation, ConfigurationChange, ConfigurationChangeType,
    ParameterAdjustmentSummary, ParameterMetadata, RecommendationPriority, RecommendationType,
    StabilityAnalysis, StabilityTrend, ActiveCanaryDeployment, AdvancedStatisticalTest, 
    BayesianResult, BootstrapResult, CanaryConfig, CanaryDecision, CanaryDeployment, CanaryError, 
    CanaryResult, CanaryStatus, DecisionType, DeploymentEvent, DeploymentLogEntry, DeploymentProgression, 
    DeploymentProgressionSummary, GateCheck, GroupMetrics, ImprovementDirection, KpiGateResult, 
    KpiImpact, KpiThresholds, KpiViolation, MannWhitneyResult, MetricsCollector, MetricsSummary, 
    MockMetricsCollector, MonitoringResult, MonitoringStatus, CanaryParameterChange, ParameterChangeType, 
    PowerAnalysis, ProgressionAction, ProgressionRecommendation, RoutingStrategy, SampleSizes, 
    SignificanceTest, StatisticalSignificance, TTestResult, TrafficGroup, TrafficSplit,
    BoundsCheckResult, BusinessRule, BusinessRuleType, ChangeLimit, ConstraintCheckResult,
    DependencyCheckResult, ParameterConstraints, ParameterDataType, ParameterDependency,
    ParameterValidator, RateLimitCheckResult, SafetyCheckResult, SafetyRule, SafetyRuleType,
    SafetySeverity, ValidationConfig, ValidationError, ValidationReport, ValidationResult,
    ValidationStats, ValidationWarning, WarningType, WarningSeverity,
    SchedulerConfig, SchedulerError, SchedulerState, SchedulerStatus, SlidingBounds,
    SlidingBoundsConfig, SlidingBoundsError, SlidingBoundsResult, SlidingBoundsState,
    TrendAnalysis, TrendDirection, ValidationResults, DeploymentResult,
};
