mod automation;
mod discovery_loop;
mod error;
mod error_handler;
mod fingerprint;
mod human;
mod ip_rotator;
mod metadata;
mod metrics;
mod pbd;
mod profile;
mod qa;
mod retry;
mod searcher;

pub use automation::{BrowserAutomation, BrowserEvent, BrowserLauncher, LaunchOverrides};
pub use discovery_loop::{
    BrowserPbdRunner, DiscoveryConfig, DiscoveryLoop, DiscoveryPbd, DiscoveryPlanStore,
    DiscoveryStats,
};
pub use error::{BrowserError, BrowserResult};
pub use error_handler::{
    map_category, telemetry_failure, telemetry_run, AutomationTelemetry, BrowserErrorCategory,
    FailureContext, RemediationAction, RunContext,
};
pub use fingerprint::FingerprintMasker;
pub use human::{HumanMotionController, HumanMotionPlan, MotionEvent, MotionPhase};
pub use ip_rotator::{CommandExecutor, IpRotator, SystemCommandExecutor};
pub use metadata::{ContentMetadata, MetadataExtractor, NormalizedTag};
pub use metrics::BrowserMetrics;
pub use pbd::{
    BrowserCapture, BrowserCaptureKind, CollectOptions, PbdArtifacts, PbdOutcome,
    PlayBeforeDownload, PlaybackValidation,
};
pub use profile::{BrowserProfile, ProfileManager};
pub use qa::{
    BrowserQaRunner, QaDashboard, QaMetricsStore, QaScenario, QaScriptResult, QaStatistics,
    SessionRecorder, SessionRecorderConfig, SessionRecordingHandle, SmokeMode, SmokeTestOptions,
    SmokeTestResult,
};
pub use retry::{RetryOutcome, RetryPolicy};
pub use searcher::{
    BrowserSearchSessionFactory, Candidate, ContentSearcher, SearchConfig, SearchEngine,
    SearchResultRaw, SearchSession, SearchSessionFactory,
};
