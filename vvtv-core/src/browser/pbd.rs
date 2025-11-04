use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use chromiumoxide::element::Element;
use chromiumoxide::page::{Page, ScreenshotParams};

use crate::config::BrowserConfig;

use super::automation::{BrowserAutomation, BrowserContext};
use super::error::{BrowserError, BrowserResult};
use super::human::HumanMotionController;
use super::metadata::{ContentMetadata, MetadataExtractor};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BrowserCaptureKind {
    HlsMaster,
    HlsMediaPlaylist,
    DashManifest,
    Progressive,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserCapture {
    pub url: String,
    pub kind: BrowserCaptureKind,
    pub quality_label: Option<String>,
    pub associated_requests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackValidation {
    pub video_width: u32,
    pub video_height: u32,
    pub duration_seconds: Option<f64>,
    pub current_time: f64,
    pub buffer_ahead: Option<f64>,
    pub ready_state: u32,
    pub hd_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PbdOutcome {
    pub capture: BrowserCapture,
    pub validation: PlaybackValidation,
    pub metadata: ContentMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct CollectOptions {
    pub capture_screenshot: bool,
}

#[derive(Debug, Clone)]
pub struct PbdArtifacts {
    pub outcome: PbdOutcome,
    pub screenshot: Option<Vec<u8>>,
}

pub struct PlayBeforeDownload {
    config: Arc<BrowserConfig>,
    metadata: MetadataExtractor,
}

impl PlayBeforeDownload {
    pub fn new(config: Arc<BrowserConfig>) -> Self {
        let metadata = MetadataExtractor::new(config.selectors.clone());
        Self { config, metadata }
    }

    pub async fn collect(
        &self,
        automation: &BrowserAutomation,
        url: &str,
    ) -> BrowserResult<PbdOutcome> {
        let artifacts = self
            .collect_with_options(automation, url, CollectOptions::default())
            .await?;
        Ok(artifacts.outcome)
    }

    pub async fn collect_with_options(
        &self,
        automation: &BrowserAutomation,
        url: &str,
        options: CollectOptions,
    ) -> BrowserResult<PbdArtifacts> {
        let mut human = HumanMotionController::new(self.config.human_simulation.clone());
        let context = automation.new_context().await?;
        context.goto(url).await?;
        human.random_idle().await?;
        self.dismiss_consent(&context, &mut human).await?;
        let video = self.locate_video_element(&context).await?;
        human.click_element(context.page(), &video).await?;
        let validation = self.wait_for_playback(&context).await?;
        let hd_label = if self.config.pbd.force_hd {
            let result = self.force_hd(&context, &mut human).await?;
            // Record HD detection attempt for P6 observability
            context.with_metrics(|metrics| {
                metrics.record_hd_attempt(result.is_some());
            });
            result
        } else {
            None
        };
        let mut validation = validation;
        if validation.hd_label.is_none() {
            validation.hd_label = hd_label.clone();
        }
        let capture = self
            .capture_media(&context, validation.hd_label.clone())
            .await?;
        context.with_metrics(|metrics| {
            metrics.record_video_playback();
            if capture.kind != BrowserCaptureKind::Unknown {
                metrics.record_manifest();
            }
        });
        let metadata = self.metadata.extract(context.page()).await?;
        let screenshot = if options.capture_screenshot {
            let params = ScreenshotParams::builder().build();
            match context.page().screenshot(params).await {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    warn!(error = %err, "failed to capture screenshot during PBD");
                    None
                }
            }
        } else {
            None
        };
        Ok(PbdArtifacts {
            outcome: PbdOutcome {
                capture,
                validation,
                metadata,
            },
            screenshot,
        })
    }

    async fn dismiss_consent(
        &self,
        context: &BrowserContext,
        human: &mut HumanMotionController,
    ) -> BrowserResult<()> {
        for selector in &self.config.selectors.consent_buttons {
            if let Ok(element) = context.page().find_element(selector.clone()).await {
                human.click_element(context.page(), &element).await?;
                sleep(Duration::from_millis(250)).await;
                break;
            }
        }
        Ok(())
    }

    async fn locate_video_element(&self, context: &BrowserContext) -> BrowserResult<Element> {
        let selector = &self.config.selectors.video_element;
        context
            .page()
            .find_element(selector.clone())
            .await
            .map_err(|err| {
                BrowserError::Network(format!("video element not found ({selector}): {err}"))
            })
    }

    async fn wait_for_playback(
        &self,
        context: &BrowserContext,
    ) -> BrowserResult<PlaybackValidation> {
        let mut attempts = 0usize;
        let wait_range = self.config.pbd.playback_wait_seconds;
        let target_wait = rand::thread_rng().gen_range(wait_range[0]..=wait_range[1]) as f64;
        loop {
            attempts += 1;
            let state: VideoState = context
                .page()
                .evaluate(PLAYBACK_STATE_SCRIPT)
                .await
                .map_err(|err| {
                    BrowserError::Unexpected(format!("failed to inspect playback state: {err}"))
                })?
                .into_value()
                .map_err(|err| {
                    BrowserError::Unexpected(format!("failed to parse playback state: {err}"))
                })?;
            if state.ready_state >= 3 && state.current_time >= target_wait && !state.paused {
                return Ok(PlaybackValidation {
                    video_width: state.video_width,
                    video_height: state.video_height,
                    duration_seconds: state.duration,
                    current_time: state.current_time,
                    buffer_ahead: state.buffer_ahead,
                    ready_state: state.ready_state,
                    hd_label: None,
                });
            }
            if attempts > 30 {
                return Err(BrowserError::Timeout("video playback readiness".into()));
            }
            sleep(Duration::from_millis(
                self.config.pbd.validation_buffer_seconds as u64 * 100,
            ))
            .await;
        }
    }

    async fn force_hd(
        &self,
        context: &BrowserContext,
        human: &mut HumanMotionController,
    ) -> BrowserResult<Option<String>> {
        let mut last_label = None;
        for menu_selector in &self.config.selectors.quality_menu {
            if let Ok(menu) = context.page().find_element(menu_selector.clone()).await {
                human.click_element(context.page(), &menu).await?;
                sleep(Duration::from_millis(350)).await;
                if let Some((element, label)) = self.select_quality_option(context.page()).await? {
                    human.click_element(context.page(), &element).await?;
                    last_label = Some(label);
                    break;
                }
            }
        }
        Ok(last_label)
    }

    async fn select_quality_option(&self, page: &Page) -> BrowserResult<Option<(Element, String)>> {
        let script = format!(
            "(() => {{
        const results = [];
        const prefer = {priority};
        const selectors = ['button', 'li', 'a', '[role\"=menuitem]'];
        let idx = 0;
        selectors.forEach(sel => {{
            document.querySelectorAll(sel).forEach(node => {{
                const text = (node.innerText || node.textContent || '').trim();
                if (!text) return;
                const lower = text.toLowerCase();
                if (prefer.some(label => lower.includes(label.toLowerCase()))) {{
                    node.setAttribute('data-vvtv-quality-option', String(idx));
                    results.push({{ index: idx, text }});
                    idx += 1;
                }}
            }});
        }});
        return results;
    }})()",
            priority = serde_json::to_string(&self.config.pbd.hd_priority).unwrap()
        );
        let options: Vec<QualityOption> = page
            .evaluate(script.as_str())
            .await
            .map_err(|err| {
                BrowserError::Unexpected(format!("failed to list quality options: {err}"))
            })?
            .into_value()
            .map_err(|err| {
                BrowserError::Unexpected(format!("failed to deserialize quality options: {err}"))
            })?;
        for preferred in &self.config.pbd.hd_priority {
            if let Some(option) = options.iter().find(|candidate| {
                candidate
                    .text
                    .to_lowercase()
                    .contains(&preferred.to_lowercase())
            }) {
                let selector = format!("[data-vvtv-quality-option='{}']", option.index);
                if let Ok(element) = page.find_element(selector.as_str()).await {
                    return Ok(Some((element, option.text.clone())));
                }
            }
        }
        Ok(None)
    }

    async fn capture_media(
        &self,
        context: &BrowserContext,
        hd_label: Option<String>,
    ) -> BrowserResult<BrowserCapture> {
        let payload: CapturePayload = context
            .page()
            .evaluate(CAPTURE_SCRIPT)
            .await
            .map_err(|err| {
                BrowserError::Network(format!("failed to capture network payload: {err}"))
            })?
            .into_value()
            .map_err(|err| {
                BrowserError::Network(format!("failed to decode capture payload: {err}"))
            })?;

        let mut associated = Vec::new();
        let mut candidate_url = payload.current.or_else(|| {
            payload
                .sources
                .iter()
                .find(|source| !source.url.is_empty())
                .map(|source| source.url.clone())
        });
        if let Some(request) = payload
            .captured
            .iter()
            .find(|request| request.url.contains(".m3u8") || request.url.contains(".mpd"))
        {
            candidate_url = Some(request.url.clone());
        }
        associated.extend(payload.captured.into_iter().map(|entry| entry.url));
        let url = candidate_url
            .ok_or_else(|| BrowserError::Network("unable to determine manifest url".into()))?;
        let kind = infer_capture_kind(&url);
        Ok(BrowserCapture {
            url,
            kind,
            quality_label: hd_label,
            associated_requests: associated,
        })
    }
}

#[derive(Debug, Deserialize)]
struct QualityOption {
    index: u32,
    text: String,
}

#[derive(Debug, Deserialize)]
struct VideoState {
    ready_state: u32,
    current_time: f64,
    duration: Option<f64>,
    video_width: u32,
    video_height: u32,
    paused: bool,
    buffer_ahead: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CapturePayload {
    current: Option<String>,
    captured: Vec<CapturedRequest>,
    sources: Vec<VideoSource>,
}

#[derive(Debug, Deserialize)]
struct CapturedRequest {
    url: String,
}

#[derive(Debug, Deserialize)]
struct VideoSource {
    url: String,
}

const PLAYBACK_STATE_SCRIPT: &str = r#"
(() => {
    const video = document.querySelector('video');
    if (!video) {
        return { ready_state: 0, current_time: 0, duration: null, video_width: 0, video_height: 0, paused: true, buffer_ahead: null };
    }
    let bufferAhead = null;
    if (video.buffered && video.buffered.length > 0) {
        try {
            bufferAhead = video.buffered.end(video.buffered.length - 1) - video.currentTime;
        } catch (err) {
            bufferAhead = null;
        }
    }
    return {
        ready_state: video.readyState || 0,
        current_time: video.currentTime || 0,
        duration: isFinite(video.duration) ? video.duration : null,
        video_width: video.videoWidth || 0,
        video_height: video.videoHeight || 0,
        paused: video.paused,
        buffer_ahead: bufferAhead,
    };
})()
"#;

const CAPTURE_SCRIPT: &str = r#"
(() => {
    const video = document.querySelector('video');
    const captured = Array.from(window.__vvtvCapturedRequests || []);
    const sources = [];
    if (video) {
        if (video.currentSrc) {
            sources.push({ url: video.currentSrc });
        }
        video.querySelectorAll('source').forEach(src => {
            const srcUrl = src.src || (src.dataset ? src.dataset.src : '');
            if (srcUrl) {
                sources.push({ url: srcUrl });
            }
        });
    }
    return { current: video ? (video.currentSrc || null) : null, captured, sources };
})()
"#;

fn infer_capture_kind(url: &str) -> BrowserCaptureKind {
    if url.contains(".m3u8") {
        if url.contains("master") {
            BrowserCaptureKind::HlsMaster
        } else {
            BrowserCaptureKind::HlsMediaPlaylist
        }
    } else if url.contains(".mpd") {
        BrowserCaptureKind::DashManifest
    } else if url.ends_with(".mp4") || url.ends_with(".webm") {
        BrowserCaptureKind::Progressive
    } else {
        BrowserCaptureKind::Unknown
    }
}
