use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrowserMetrics {
    pub pages_opened: u64,
    pub videos_played: u64,
    pub hd_attempts: u64,
    pub hd_success: u64,
    pub playback_failures: u64,
    pub manifests_collected: u64,
    pub network_requests: u64,
    pub proxy_rotations: u64,
    pub bot_detections: u64,
}

impl BrowserMetrics {
    pub fn record_page_open(&mut self) {
        self.pages_opened = self.pages_opened.saturating_add(1);
    }

    pub fn record_video_playback(&mut self) {
        self.videos_played = self.videos_played.saturating_add(1);
    }

    pub fn record_hd_attempt(&mut self, success: bool) {
        self.hd_attempts = self.hd_attempts.saturating_add(1);
        if success {
            self.hd_success = self.hd_success.saturating_add(1);
        }
    }

    pub fn record_playback_failure(&mut self) {
        self.playback_failures = self.playback_failures.saturating_add(1);
    }

    pub fn record_manifest(&mut self) {
        self.manifests_collected = self.manifests_collected.saturating_add(1);
    }

    pub fn record_network_events(&mut self, count: u64) {
        self.network_requests = self.network_requests.saturating_add(count);
    }

    pub fn record_proxy_rotation(&mut self) {
        self.proxy_rotations = self.proxy_rotations.saturating_add(1);
    }

    pub fn record_bot_detection(&mut self) {
        self.bot_detections = self.bot_detections.saturating_add(1);
    }

    pub fn pbd_success_rate(&self) -> f64 {
        if self.hd_attempts == 0 {
            0.0
        } else {
            (self.hd_success as f64 / self.hd_attempts as f64) * 100.0
        }
    }
}
