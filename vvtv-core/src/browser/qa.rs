use std::sync::Arc;
use std::time::{Duration, Instant};

use super::automation::BrowserLauncher;
use super::error::BrowserResult;
use super::metrics::BrowserMetrics;
use super::pbd::{PbdOutcome, PlayBeforeDownload};

#[derive(Debug, Clone)]
pub struct SmokeTestResult {
    pub url: String,
    pub success: bool,
    pub capture: Option<PbdOutcome>,
    pub duration: Duration,
    pub warnings: Vec<String>,
    pub metrics: BrowserMetrics,
}

#[derive(Debug, Clone)]
pub struct QaScriptResult {
    pub scenario: QaScenario,
    pub result: SmokeTestResult,
}

#[derive(Debug, Clone)]
pub enum QaScenario {
    Smoke { url: String },
}

pub struct BrowserQaRunner {
    launcher: Arc<BrowserLauncher>,
    playbook: PlayBeforeDownload,
}

impl BrowserQaRunner {
    pub fn new(launcher: BrowserLauncher) -> Self {
        let config = Arc::new(launcher.config().clone());
        let playbook = PlayBeforeDownload::new(Arc::clone(&config));
        Self {
            launcher: Arc::new(launcher),
            playbook,
        }
    }

    pub async fn run_smoke(&self, url: &str) -> BrowserResult<SmokeTestResult> {
        let start = Instant::now();
        let automation = self.launcher.launch().await?;
        let metrics_snapshot = automation.metrics();
        let outcome = self.playbook.collect(&automation, url).await;
        let shutdown_result = automation.shutdown().await;
        shutdown_result?;
        let outcome = outcome?;
        Ok(SmokeTestResult {
            url: url.to_string(),
            success: true,
            capture: Some(outcome),
            duration: start.elapsed(),
            warnings: Vec::new(),
            metrics: metrics_snapshot,
        })
    }

    pub async fn run(&self, scenario: QaScenario) -> BrowserResult<QaScriptResult> {
        match &scenario {
            QaScenario::Smoke { url } => {
                let result = self.run_smoke(url).await?;
                Ok(QaScriptResult { scenario, result })
            }
        }
    }
}
