use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for drift monitoring operations
pub type DriftResult<T> = std::result::Result<T, DriftError>;

/// Errors that can occur during drift monitoring
#[derive(Debug, Error)]
pub enum DriftError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("insufficient data: {0}")]
    InsufficientData(String),
    #[error("drift detected: {0}")]
    DriftDetected(String),
    #[error("system paused: {0}")]
    SystemPaused(String),
    #[error("io error: {0}")]
    IoError(String),
}

/// Anti-drift monitor for detecting and preventing autopilot drift
#[derive(Debug)]
pub struct AntiDriftMonitor {
    config: DriftConfig,
    state: DriftMonitorState,
    prediction_errors: VecDeque<PredictionErrorRecord>,
    rollback_history: VecDeque<RollbackRecord>,
    drift_events: Vec<DriftEvent>,
    pause_state: Option<PauseState>,
    state_file_path: Option<PathBuf>,
}

/// Configuration for drift monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftConfig {
    /// Target prediction accuracy (p50) - default 80%
    pub target_prediction_accuracy_p50: f64,
    /// Maximum allowed prediction error - default 30%
    pub max_prediction_error_threshold: f64,
    /// Target rollback rate - default 5%
    pub target_rollback_rate: f64,
    /// Maximum allowed rollback rate - default 10%
    pub max_rollback_rate_threshold: f64,
    /// Analysis window in days - default 14
    pub analysis_window_days: u32,
    /// Minimum samples required for analysis - default 10
    pub min_samples_for_analysis: usize,
    /// Consecutive failure threshold for drift detection - default 3
    pub consecutive_failure_threshold: u32,
    /// Initial pause duration in hours - default 48
    pub initial_pause_duration_hours: u32,
    /// Maximum pause duration in hours - default 168 (7 days)
    pub max_pause_duration_hours: u32,
    /// Exponential backoff multiplier - default 2.0
    pub pause_backoff_multiplier: f64,
    /// Enable automatic pause on drift detection
    pub enable_automatic_pause: bool,
    /// Enable prediction error tracking
    pub enable_prediction_tracking: bool,
    /// Enable rollback rate monitoring
    pub enable_rollback_monitoring: bool,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            target_prediction_accuracy_p50: 0.8,
            max_prediction_error_threshold: 0.3,
            target_rollback_rate: 0.05,
            max_rollback_rate_threshold: 0.1,
            analysis_window_days: 14,
            min_samples_for_analysis: 10,
            consecutive_failure_threshold: 3,
            initial_pause_duration_hours: 48,
            max_pause_duration_hours: 168,
            pause_backoff_multiplier: 2.0,
            enable_automatic_pause: true,
            enable_prediction_tracking: true,
            enable_rollback_monitoring: true,
        }
    }
}///
 Current state of the drift monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftMonitorState {
    pub is_paused: bool,
    pub last_analysis_time: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub total_predictions: u64,
    pub total_rollbacks: u64,
    pub current_prediction_accuracy: f64,
    pub current_rollback_rate: f64,
    pub drift_risk_level: DriftRiskLevel,
    pub last_drift_detection: Option<DateTime<Utc>>,
}

impl Default for DriftMonitorState {
    fn default() -> Self {
        Self {
            is_paused: false,
            last_analysis_time: None,
            consecutive_failures: 0,
            total_predictions: 0,
            total_rollbacks: 0,
            current_prediction_accuracy: 1.0,
            current_rollback_rate: 0.0,
            drift_risk_level: DriftRiskLevel::Low,
            last_drift_detection: None,
        }
    }
}

/// Drift risk levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DriftRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Record of prediction accuracy for drift analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionErrorRecord {
    pub timestamp: DateTime<Utc>,
    pub parameter_name: String,
    pub predicted_impact: f64,
    pub actual_impact: f64,
    pub prediction_error: f64,
    pub deployment_id: String,
    pub was_rollback: bool,
}

/// Record of rollback events for drift analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub timestamp: DateTime<Utc>,
    pub deployment_id: String,
    pub parameter_changes: HashMap<String, f64>,
    pub rollback_reason: String,
    pub time_to_rollback_minutes: u32,
    pub kpi_impacts: HashMap<String, f64>,
}

/// Drift detection event
#[derive(Debug, Clone, Serialize)]
pub struct DriftEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub drift_type: DriftType,
    pub severity: DriftSeverity,
    pub description: String,
    pub evidence: Vec<String>,
    pub action_taken: DriftAction,
    pub confidence: f64,
}

/// Types of drift that can be detected
#[derive(Debug, Clone, Serialize)]
pub enum DriftType {
    PredictionAccuracyDrift,
    RollbackRateIncrease,
    ConsecutiveFailures,
    SystemInstability,
    ParameterSensitivityChange,
}

/// Severity levels for drift events
#[derive(Debug, Clone, Serialize)]
pub enum DriftSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Actions taken in response to drift detection
#[derive(Debug, Clone, Serialize)]
pub enum DriftAction {
    NoAction,
    WarningIssued,
    SystemPaused,
    BoundsContracted,
    ManualReviewRequested,
    EmergencyStop,
}

/// Current pause state of the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseState {
    pub paused_at: DateTime<Utc>,
    pub pause_duration_hours: u32,
    pub resume_at: DateTime<Utc>,
    pub pause_reason: String,
    pub pause_count: u32,
    pub can_resume_automatically: bool,
}i
mpl AntiDriftMonitor {
    /// Create new anti-drift monitor
    pub fn new(config: DriftConfig) -> Self {
        Self {
            config,
            state: DriftMonitorState::default(),
            prediction_errors: VecDeque::new(),
            rollback_history: VecDeque::new(),
            drift_events: Vec::new(),
            pause_state: None,
            state_file_path: None,
        }
    }

    /// Create monitor with persistent state file
    pub fn with_state_file(config: DriftConfig, state_file_path: PathBuf) -> DriftResult<Self> {
        let mut monitor = Self::new(config);
        monitor.state_file_path = Some(state_file_path.clone());
        
        // Try to load existing state
        if state_file_path.exists() {
            monitor.load_state()?;
        }
        
        Ok(monitor)
    }

    /// Load state from file
    fn load_state(&mut self) -> DriftResult<()> {
        if let Some(path) = &self.state_file_path {
            let content = std::fs::read_to_string(path)
                .map_err(|e| DriftError::IoError(format!("Failed to read state file: {}", e)))?;
            
            let saved_state: DriftMonitorState = serde_json::from_str(&content)
                .map_err(|e| DriftError::Configuration(format!("Failed to parse state file: {}", e)))?;
            
            self.state = saved_state;
        }
        Ok(())
    }

    /// Save state to file
    fn save_state(&self) -> DriftResult<()> {
        if let Some(path) = &self.state_file_path {
            let content = serde_json::to_string_pretty(&self.state)
                .map_err(|e| DriftError::Configuration(format!("Failed to serialize state: {}", e)))?;
            
            std::fs::write(path, content)
                .map_err(|e| DriftError::IoError(format!("Failed to write state file: {}", e)))?;
        }
        Ok(())
    }

    /// Check if system is currently paused
    pub fn is_paused(&self) -> bool {
        if let Some(pause_state) = &self.pause_state {
            Utc::now() < pause_state.resume_at
        } else {
            false
        }
    }

    /// Get current drift risk level
    pub fn get_drift_risk_level(&self) -> DriftRiskLevel {
        self.state.drift_risk_level.clone()
    }

    /// Get current monitoring statistics
    pub fn get_monitoring_stats(&self) -> DriftMonitoringStats {
        DriftMonitoringStats {
            total_predictions: self.state.total_predictions,
            total_rollbacks: self.state.total_rollbacks,
            current_prediction_accuracy: self.state.current_prediction_accuracy,
            current_rollback_rate: self.state.current_rollback_rate,
            consecutive_failures: self.state.consecutive_failures,
            drift_risk_level: self.state.drift_risk_level.clone(),
            is_paused: self.is_paused(),
            pause_state: self.pause_state.clone(),
            recent_drift_events: self.drift_events.iter().rev().take(5).cloned().collect(),
        }
    }
}

/// Statistics for drift monitoring
#[derive(Debug, Clone, Serialize)]
pub struct DriftMonitoringStats {
    pub total_predictions: u64,
    pub total_rollbacks: u64,
    pub current_prediction_accuracy: f64,
    pub current_rollback_rate: f64,
    pub consecutive_failures: u32,
    pub drift_risk_level: DriftRiskLevel,
    pub is_paused: bool,
    pub pause_state: Option<PauseState>,
    pub recent_drift_events: Vec<DriftEvent>,
} 
   /// Record a prediction error for drift analysis
    pub fn record_prediction_error(
        &mut self,
        parameter_name: String,
        predicted_impact: f64,
        actual_impact: f64,
        deployment_id: String,
        was_rollback: bool,
    ) -> DriftResult<()> {
        if !self.config.enable_prediction_tracking {
            return Ok(());
        }

        let prediction_error = ((predicted_impact - actual_impact) / predicted_impact.max(0.001)).abs();
        
        let record = PredictionErrorRecord {
            timestamp: Utc::now(),
            parameter_name,
            predicted_impact,
            actual_impact,
            prediction_error,
            deployment_id,
            was_rollback,
        };

        self.prediction_errors.push_back(record);
        self.state.total_predictions += 1;

        // Maintain rolling window
        self.cleanup_old_prediction_records();

        // Update current accuracy
        self.update_prediction_accuracy()?;

        // Check for drift
        self.check_prediction_drift()?;

        self.save_state()?;
        Ok(())
    }

    /// Analyze prediction accuracy over the analysis window
    pub fn analyze_prediction_accuracy(&self) -> DriftResult<PredictionAccuracyAnalysis> {
        let cutoff_time = Utc::now() - chrono::Duration::days(self.config.analysis_window_days as i64);
        
        let recent_errors: Vec<&PredictionErrorRecord> = self.prediction_errors.iter()
            .filter(|record| record.timestamp >= cutoff_time)
            .collect();

        if recent_errors.len() < self.config.min_samples_for_analysis {
            return Err(DriftError::InsufficientData(
                format!("Need {} samples, have {}", self.config.min_samples_for_analysis, recent_errors.len())
            ));
        }

        // Calculate accuracy metrics
        let mut errors: Vec<f64> = recent_errors.iter().map(|r| r.prediction_error).collect();
        errors.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let p50_error = errors[errors.len() / 2];
        let p95_error = errors[(errors.len() as f64 * 0.95) as usize];
        let mean_error = errors.iter().sum::<f64>() / errors.len() as f64;

        let accuracy_p50 = 1.0 - p50_error;
        let accuracy_p95 = 1.0 - p95_error;
        let mean_accuracy = 1.0 - mean_error;

        // Analyze by parameter
        let mut parameter_accuracy = HashMap::new();
        for param in ["selection_temperature", "selection_top_k", "plan_selection_bias"] {
            let param_errors: Vec<f64> = recent_errors.iter()
                .filter(|r| r.parameter_name == param)
                .map(|r| r.prediction_error)
                .collect();
            
            if !param_errors.is_empty() {
                let param_mean_error = param_errors.iter().sum::<f64>() / param_errors.len() as f64;
                parameter_accuracy.insert(param.to_string(), 1.0 - param_mean_error);
            }
        }

        // Trend analysis
        let trend = self.analyze_accuracy_trend(&recent_errors)?;

        Ok(PredictionAccuracyAnalysis {
            analysis_period: (cutoff_time, Utc::now()),
            sample_count: recent_errors.len(),
            accuracy_p50,
            accuracy_p95,
            mean_accuracy,
            parameter_accuracy,
            trend,
            meets_target: accuracy_p50 >= self.config.target_prediction_accuracy_p50,
            exceeds_threshold: p50_error > self.config.max_prediction_error_threshold,
        })
    }

    /// Update current prediction accuracy
    fn update_prediction_accuracy(&mut self) -> DriftResult<()> {
        match self.analyze_prediction_accuracy() {
            Ok(analysis) => {
                self.state.current_prediction_accuracy = analysis.accuracy_p50;
                Ok(())
            }
            Err(DriftError::InsufficientData(_)) => {
                // Not enough data yet, keep current accuracy
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Check for prediction accuracy drift
    fn check_prediction_drift(&mut self) -> DriftResult<()> {
        if !self.config.enable_prediction_tracking {
            return Ok(());
        }

        let analysis = match self.analyze_prediction_accuracy() {
            Ok(analysis) => analysis,
            Err(DriftError::InsufficientData(_)) => return Ok(()), // Not enough data yet
            Err(e) => return Err(e),
        };

        // Check if accuracy is below threshold
        if analysis.exceeds_threshold {
            self.state.consecutive_failures += 1;
            
            let drift_event = DriftEvent {
                event_id: format!("drift_{}", Utc::now().timestamp()),
                timestamp: Utc::now(),
                drift_type: DriftType::PredictionAccuracyDrift,
                severity: if analysis.accuracy_p50 < 0.5 { DriftSeverity::Critical } else { DriftSeverity::High },
                description: format!("Prediction accuracy dropped to {:.1}%", analysis.accuracy_p50 * 100.0),
                evidence: vec![
                    format!("P50 accuracy: {:.3}", analysis.accuracy_p50),
                    format!("Target: {:.3}", self.config.target_prediction_accuracy_p50),
                    format!("Consecutive failures: {}", self.state.consecutive_failures),
                ],
                action_taken: if self.state.consecutive_failures >= self.config.consecutive_failure_threshold {
                    DriftAction::SystemPaused
                } else {
                    DriftAction::WarningIssued
                },
                confidence: 0.9,
            };

            self.drift_events.push(drift_event);
            self.state.last_drift_detection = Some(Utc::now());

            // Trigger pause if threshold reached
            if self.state.consecutive_failures >= self.config.consecutive_failure_threshold && self.config.enable_automatic_pause {
                self.pause_system("Consecutive prediction accuracy failures detected".to_string())?;
            }
        } else if analysis.meets_target {
            // Reset consecutive failures on success
            self.state.consecutive_failures = 0;
        }

        // Update risk level
        self.update_drift_risk_level(&analysis);

        Ok(())
    }

    /// Analyze prediction accuracy trend
    fn analyze_accuracy_trend(&self, records: &[&PredictionErrorRecord]) -> DriftResult<AccuracyTrend> {
        if records.len() < 5 {
            return Ok(AccuracyTrend::Insufficient);
        }

        // Split into two halves for trend analysis
        let mid_point = records.len() / 2;
        let first_half = &records[..mid_point];
        let second_half = &records[mid_point..];

        let first_half_accuracy = 1.0 - (first_half.iter().map(|r| r.prediction_error).sum::<f64>() / first_half.len() as f64);
        let second_half_accuracy = 1.0 - (second_half.iter().map(|r| r.prediction_error).sum::<f64>() / second_half.len() as f64);

        let accuracy_change = second_half_accuracy - first_half_accuracy;

        if accuracy_change > 0.05 {
            Ok(AccuracyTrend::Improving)
        } else if accuracy_change < -0.05 {
            Ok(AccuracyTrend::Degrading)
        } else {
            Ok(AccuracyTrend::Stable)
        }
    }

    /// Update drift risk level based on current analysis
    fn update_drift_risk_level(&mut self, analysis: &PredictionAccuracyAnalysis) {
        let new_risk_level = if analysis.accuracy_p50 < 0.5 {
            DriftRiskLevel::Critical
        } else if analysis.accuracy_p50 < 0.6 || self.state.consecutive_failures >= 2 {
            DriftRiskLevel::High
        } else if analysis.accuracy_p50 < 0.7 || !analysis.meets_target {
            DriftRiskLevel::Medium
        } else {
            DriftRiskLevel::Low
        };

        if new_risk_level != self.state.drift_risk_level {
            self.state.drift_risk_level = new_risk_level;
        }
    }

    /// Clean up old prediction records
    fn cleanup_old_prediction_records(&mut self) {
        let cutoff_time = Utc::now() - chrono::Duration::days(self.config.analysis_window_days as i64);
        
        while let Some(front) = self.prediction_errors.front() {
            if front.timestamp < cutoff_time {
                self.prediction_errors.pop_front();
            } else {
                break;
            }
        }
    }

    /// Pause the autopilot system
    fn pause_system(&mut self, reason: String) -> DriftResult<()> {
        let now = Utc::now();
        let pause_count = self.pause_state.as_ref().map(|p| p.pause_count).unwrap_or(0) + 1;
        
        // Calculate pause duration with exponential backoff
        let base_duration = self.config.initial_pause_duration_hours as f64;
        let backoff_multiplier = self.config.pause_backoff_multiplier.powi((pause_count - 1) as i32);
        let pause_duration = (base_duration * backoff_multiplier).min(self.config.max_pause_duration_hours as f64) as u32;

        let resume_at = now + chrono::Duration::hours(pause_duration as i64);

        self.pause_state = Some(PauseState {
            paused_at: now,
            pause_duration_hours: pause_duration,
            resume_at,
            pause_reason: reason,
            pause_count,
            can_resume_automatically: true,
        });

        self.state.is_paused = true;
        self.save_state()?;

        Ok(())
    }
}

/// Analysis result for prediction accuracy
#[derive(Debug, Clone, Serialize)]
pub struct PredictionAccuracyAnalysis {
    pub analysis_period: (DateTime<Utc>, DateTime<Utc>),
    pub sample_count: usize,
    pub accuracy_p50: f64,
    pub accuracy_p95: f64,
    pub mean_accuracy: f64,
    pub parameter_accuracy: HashMap<String, f64>,
    pub trend: AccuracyTrend,
    pub meets_target: bool,
    pub exceeds_threshold: bool,
}

/// Trend analysis for prediction accuracy
#[derive(Debug, Clone, Serialize)]
pub enum AccuracyTrend {
    Improving,
    Stable,
    Degrading,
    Insufficient,
}