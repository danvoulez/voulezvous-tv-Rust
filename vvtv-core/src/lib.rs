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
pub mod config;
pub mod error;
pub mod monitor;
pub mod plan;
pub mod processor;
pub mod quality;
pub mod queue;
mod sqlite;

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
pub use config::{
    load_broadcaster_config, load_browser_config, load_processor_config, load_vvtv_config,
    BroadcasterConfig, BrowserConfig, ConfigBundle, ProcessorConfig, VvtvConfig,
};
pub use error::{ConfigError, Result};
pub use monitor::{
    DashboardGenerator, LiveQcSample, LiveQualityCollector, MetricRecord, MetricSnapshot,
    MetricsStore, MonitorError, QcReportGenerator, VisualReviewPanel,
};
pub use plan::{
    Plan, PlanAuditFinding, PlanAuditKind, PlanBlacklistEntry, PlanError, PlanImportRecord,
    PlanMetrics, PlanResult, PlanSelectionDecision, PlanStatus, Planner, PlannerConfig,
    PlannerEvent, RealizationOutcome, Realizer, RealizerConfig, SqlitePlanStore,
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
