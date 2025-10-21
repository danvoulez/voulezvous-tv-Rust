use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig as ChromiumConfig};
use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{
    AddScriptToEvaluateOnNewDocumentParams, NavigateParams,
};
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use chromiumoxide::handler::viewport::Viewport as ChromiumViewport;
use chromiumoxide::page::Page;
use futures::StreamExt;
use rand::{seq::SliceRandom, Rng};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::config::{BrowserConfig, ViewportSection};

use super::error::{BrowserError, BrowserResult};
use super::error_handler::AutomationTelemetry;
use super::fingerprint::FingerprintMasker;
use super::ip_rotator::IpRotator;
use super::metrics::BrowserMetrics;
use super::profile::{BrowserProfile, ProfileManager};
use super::retry::RetryPolicy;

#[derive(Debug, Clone)]
pub struct ViewportSpec {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LaunchOverrides {
    pub headless: Option<bool>,
}

#[derive(Debug)]
pub struct BrowserLauncher {
    config: Arc<BrowserConfig>,
    profiles: ProfileManager,
    proxy_pool: ProxyPool,
    fingerprint: Arc<FingerprintMasker>,
    telemetry: Arc<AutomationTelemetry>,
    retry_policy: RetryPolicy,
    ip_rotator: Option<Arc<AsyncMutex<IpRotator>>>,
}

impl Clone for BrowserLauncher {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            profiles: self.profiles.clone(),
            proxy_pool: self.proxy_pool.clone(),
            fingerprint: Arc::clone(&self.fingerprint),
            telemetry: Arc::clone(&self.telemetry),
            retry_policy: self.retry_policy.clone(),
            ip_rotator: self.ip_rotator.as_ref().map(Arc::clone),
        }
    }
}

#[derive(Debug, Clone)]
struct ProxyPool {
    entries: Vec<String>,
}

impl ProxyPool {
    fn new(config: &BrowserConfig) -> Self {
        let mut entries = std::env::var("VVTV_BROWSER_PROXIES")
            .unwrap_or_default()
            .split(',')
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        if entries.is_empty() && config.proxy.enabled {
            if let Ok(path) = std::env::var("VVTV_BROWSER_PROXY_FILE") {
                if let Ok(contents) = std::fs::read_to_string(path) {
                    entries.extend(
                        contents
                            .lines()
                            .map(|line| line.trim().to_string())
                            .filter(|value| !value.is_empty()),
                    );
                }
            }
        }

        Self { entries }
    }

    fn next(&self) -> Option<String> {
        if self.entries.is_empty() {
            None
        } else {
            let mut rng = rand::thread_rng();
            self.entries.choose(&mut rng).cloned()
        }
    }
}

impl BrowserLauncher {
    pub fn new(config: BrowserConfig, profiles: ProfileManager) -> BrowserResult<Self> {
        let proxy_pool = ProxyPool::new(&config);
        let config = Arc::new(config);
        let fingerprint = Arc::new(FingerprintMasker::new(config.fingerprint.clone()));
        let failure_log = PathBuf::from(&config.observability.failure_log);
        let metrics_db = PathBuf::from(&config.observability.metrics_db);
        let telemetry = Arc::new(AutomationTelemetry::new(failure_log, metrics_db)?);
        let retry_policy = RetryPolicy::new(config.retry.clone());
        let ip_rotator = if config.ip_rotation.enabled && !config.ip_rotation.exit_nodes.is_empty()
        {
            let rotator = IpRotator::new(config.ip_rotation.clone(), Arc::clone(&telemetry))?;
            Some(Arc::new(AsyncMutex::new(rotator)))
        } else {
            None
        };
        Ok(Self {
            config,
            profiles,
            proxy_pool,
            fingerprint,
            telemetry,
            retry_policy,
            ip_rotator,
        })
    }

    pub fn telemetry(&self) -> Arc<AutomationTelemetry> {
        Arc::clone(&self.telemetry)
    }

    pub fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy.clone()
    }

    pub fn ip_rotator(&self) -> Option<Arc<AsyncMutex<IpRotator>>> {
        self.ip_rotator.as_ref().map(Arc::clone)
    }

    pub fn fingerprint(&self) -> Arc<FingerprintMasker> {
        Arc::clone(&self.fingerprint)
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    pub fn profile_manager(&self) -> &ProfileManager {
        &self.profiles
    }

    pub async fn launch(&self) -> BrowserResult<BrowserAutomation> {
        self.launch_with_overrides(LaunchOverrides::default()).await
    }

    pub async fn launch_with_overrides(
        &self,
        overrides: LaunchOverrides,
    ) -> BrowserResult<BrowserAutomation> {
        self.profiles.cleanup_expired()?;
        let profile = self.profiles.allocate()?;
        let viewport = self.select_viewport();
        let user_agent = self.select_user_agent();
        let proxy = if self.config.proxy.enabled {
            self.proxy_pool.next()
        } else {
            None
        };
        let headless = overrides.headless.unwrap_or(self.config.chromium.headless);
        let chromium_config = self.build_chromium_config(
            &profile,
            &viewport,
            &user_agent,
            proxy.as_deref(),
            headless,
        )?;
        info!(
            profile = %profile.id(),
            ua = %user_agent,
            width = viewport.width,
            height = viewport.height,
            headless,
            "Launching Chromium instance"
        );

        let (browser, mut handler) = Browser::launch(chromium_config)
            .await
            .map_err(|err| BrowserError::Launch(err.to_string()))?;

        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    debug!(error = %err, "Chromium handler reported error");
                }
            }
        });

        profile.touch().await?;

        Ok(BrowserAutomation {
            browser,
            profile,
            handler_task: Some(handler_task),
            config: Arc::clone(&self.config),
            metrics: Arc::new(Mutex::new(BrowserMetrics::default())),
            viewport,
            user_agent,
            proxy,
            fingerprint: Arc::clone(&self.fingerprint),
        })
    }

    fn select_viewport(&self) -> ViewportSpec {
        let ViewportSection {
            resolutions,
            jitter_pixels,
            device_scale_factor,
        } = &self.config.viewport;

        let mut rng = rand::thread_rng();
        let base = resolutions.choose(&mut rng).cloned().unwrap_or([1366, 768]);
        let jitter = *jitter_pixels as i32;
        let width = (base[0] as i32 + rng.gen_range(-jitter..=jitter)).clamp(640, 2560) as u32;
        let height = (base[1] as i32 + rng.gen_range(-jitter..=jitter)).clamp(480, 1600) as u32;
        let scale = rng.gen_range(device_scale_factor[0]..=device_scale_factor[1]) as f64;
        ViewportSpec {
            width,
            height,
            device_scale_factor: scale,
        }
    }

    fn select_user_agent(&self) -> String {
        let mut rng = rand::thread_rng();
        if self.config.user_agents.pool.is_empty() {
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_4) AppleWebKit/605.1.15 (KHTML, like Gecko)"
                .to_string()
        } else {
            self.config
                .user_agents
                .pool
                .choose(&mut rng)
                .cloned()
                .unwrap_or_else(|| self.config.user_agents.pool[0].clone())
        }
    }

    fn build_chromium_config(
        &self,
        profile: &BrowserProfile,
        viewport: &ViewportSpec,
        user_agent: &str,
        proxy: Option<&str>,
        headless: bool,
    ) -> BrowserResult<ChromiumConfig> {
        let mut builder = ChromiumConfig::builder()
            .chrome_executable(&self.config.chromium.executable_path)
            .user_data_dir(profile.path())
            .viewport(ChromiumViewport {
                width: viewport.width,
                height: viewport.height,
                device_scale_factor: Some(viewport.device_scale_factor),
                emulating_mobile: false,
                is_landscape: viewport.width >= viewport.height,
                has_touch: false,
            });

        if !headless {
            builder = builder.with_head();
        }
        if !self.config.chromium.sandbox {
            builder = builder.no_sandbox();
        }
        if let Some(timeout) = self.config.chromium.tab_timeout_seconds {
            builder = builder.request_timeout(Duration::from_secs(timeout));
        }

        let mut args = vec![
            format!("--user-agent={user_agent}"),
            format!("--window-size={},{}", viewport.width, viewport.height),
        ];

        if self.config.chromium.disable_gpu {
            args.push("--disable-gpu".into());
        }
        if self.config.flags.mute_audio {
            args.push("--mute-audio".into());
        }
        if !self.config.flags.autoplay_policy.is_empty() {
            args.push(format!(
                "--autoplay-policy={}",
                self.config.flags.autoplay_policy
            ));
        }
        if let Some(lang) = &self.config.flags.lang {
            args.push(format!("--lang={lang}"));
        }
        if let Some(proxy) = proxy {
            args.push(format!("--proxy-server={proxy}"));
        }
        for feature in &self.config.flags.disable_blink_features {
            args.push(format!("--disable-blink-features={feature}"));
        }
        if self.config.flags.no_first_run {
            args.push("--no-first-run".into());
        }
        if self.config.flags.disable_automation_controlled {
            args.push("--disable-features=AutomationControlled".into());
        }
        if let Some(accept) = &self.config.flags.accept_language {
            args.push(format!("--accept-lang={accept}"));
        }
        args.push("--disable-background-timer-throttling".into());
        args.push("--password-store=basic".into());

        builder = builder.args(args);

        builder.build().map_err(BrowserError::Configuration)
    }
}

#[derive(Debug)]
pub struct BrowserAutomation {
    browser: Browser,
    profile: BrowserProfile,
    handler_task: Option<JoinHandle<()>>,
    config: Arc<BrowserConfig>,
    metrics: Arc<Mutex<BrowserMetrics>>,
    viewport: ViewportSpec,
    user_agent: String,
    proxy: Option<String>,
    fingerprint: Arc<FingerprintMasker>,
}

impl BrowserAutomation {
    pub fn profile(&self) -> &BrowserProfile {
        &self.profile
    }

    pub fn metrics(&self) -> BrowserMetrics {
        self.metrics.lock().unwrap().clone()
    }

    pub(crate) fn metrics_handle(&self) -> Arc<Mutex<BrowserMetrics>> {
        Arc::clone(&self.metrics)
    }

    pub fn viewport(&self) -> &ViewportSpec {
        &self.viewport
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn proxy(&self) -> Option<&str> {
        self.proxy.as_deref()
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    pub async fn new_context(&self) -> BrowserResult<BrowserContext> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.record_page_open();
        }
        let params = CreateTargetParams::new("about:blank");
        let page = self.browser.new_page(params).await?;
        self.configure_page(&page).await?;
        Ok(BrowserContext {
            page,
            metrics: Arc::clone(&self.metrics),
            user_agent: self.user_agent.clone(),
            viewport: self.viewport.clone(),
        })
    }

    pub async fn shutdown(mut self) -> BrowserResult<()> {
        info!(profile = %self.profile.id(), "Shutting down Chromium instance");
        if let Err(err) = self.browser.close().await {
            warn!(error = %err, "Failed to close browser gracefully");
        }
        if let Some(handle) = self.handler_task.take() {
            if let Err(err) = handle.await {
                warn!(error = %err, "Browser handler join error");
            }
        }
        Ok(())
    }

    async fn configure_page(&self, page: &Page) -> BrowserResult<()> {
        page.enable_stealth_mode_with_agent(&self.user_agent)
            .await?;

        let mut params_builder =
            SetUserAgentOverrideParams::builder().user_agent(self.user_agent.clone());
        if let Some(accept) = &self.config.flags.accept_language {
            params_builder = params_builder.accept_language(accept.clone());
        }
        let params = params_builder
            .build()
            .map_err(BrowserError::Configuration)?;
        page.set_user_agent(params).await?;

        if let Some(lang) = &self.config.flags.lang {
            let languages_script = format!(
                "Object.defineProperty(navigator, 'language', {{ get: () => '{lang}' }});\nObject.defineProperty(navigator, 'languages', {{ get: () => ['{lang}', 'en-US'] }});"
            );
            page.evaluate_on_new_document(
                AddScriptToEvaluateOnNewDocumentParams::builder()
                    .source(languages_script)
                    .build()
                    .map_err(BrowserError::Configuration)?,
            )
            .await?;
        }

        let network_hook = r#"
(() => {
    const bucket = [];
    const push = (entry) => {
        try {
            bucket.push(entry);
        } catch (_) {
            // ignore
        }
    };
    Object.defineProperty(window, '__vvtvCapturedRequests', {
        value: bucket,
        writable: false,
        configurable: false,
    });

    const originalFetch = window.fetch;
    window.fetch = async (...args) => {
        const response = await originalFetch(...args);
        try {
            const request = args[0];
            const url = typeof request === 'string' ? request : request.url;
            push({ url: String(url || ''), type: 'fetch', status: response.status });
        } catch (_) {}
        return response;
    };

    const OriginalXHR = window.XMLHttpRequest;
    window.XMLHttpRequest = function() {
        const xhr = new OriginalXHR();
        let url = '';
        let method = 'GET';
        const open = xhr.open;
        xhr.open = function(m, u) {
            method = m || 'GET';
            url = u || '';
            return open.apply(xhr, arguments);
        };
        xhr.addEventListener('loadend', function() {
            push({ url: String(url || ''), type: 'xhr', status: xhr.status, method });
        });
        return xhr;
    };
})();
"#;

        page.evaluate_on_new_document(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(network_hook)
                .build()
                .map_err(BrowserError::Configuration)?,
        )
        .await?;
        self.fingerprint.apply(page).await?;
        Ok(())
    }
}

impl Drop for BrowserAutomation {
    fn drop(&mut self) {
        if let Some(handle) = &self.handler_task {
            if !handle.is_finished() {
                warn!(
                    profile = %self.profile.id(),
                    "BrowserAutomation dropped without explicit shutdown"
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct BrowserContext {
    page: Page,
    metrics: Arc<Mutex<BrowserMetrics>>,
    user_agent: String,
    viewport: ViewportSpec,
}

impl BrowserContext {
    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn viewport(&self) -> &ViewportSpec {
        &self.viewport
    }

    pub async fn goto(&self, url: &str) -> BrowserResult<()> {
        let params = NavigateParams::builder()
            .url(url)
            .build()
            .map_err(BrowserError::Configuration)?;
        self.page.goto(params).await?;
        self.page.wait_for_navigation().await?;
        Ok(())
    }

    pub fn metrics(&self) -> BrowserMetrics {
        self.metrics.lock().unwrap().clone()
    }

    pub fn with_metrics<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BrowserMetrics) -> R,
    {
        let mut guard = self.metrics.lock().unwrap();
        f(&mut guard)
    }
}

#[derive(Debug, Clone)]
pub enum BrowserEvent {
    PageOpened { url: String },
    ManifestCaptured { url: String },
}
