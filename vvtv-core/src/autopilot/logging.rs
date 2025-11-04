use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::engine::{AutopilotCycle, AutopilotResult, ParameterChange};
use crate::monitor::{BusinessMetric, BusinessMetricType, MetricsStore, MonitorError};

/// Autopilot logging and monitoring integration
#[derive(Debug)]
pub struct AutopilotLogger {
    metrics_store: Arc<MetricsStore>,
    config: LoggingConfig,
}

/// Configuration for autopilot logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub structured_logging_enabled: bool,
    pub metrics_recording_enabled: bool,
    pub performance_tracking_enabled: bool,
    pub alert_integration_enabled: bool,
    pub log_level: LogLevel,
    pub retention_days: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            structured_logging_enabled: true,
            metrics_recording_enabled: true,
            performance_tracking_enabled: true,
            alert_integration_enabled: true,
            log_level: LogLevel::Info,
            retention_days: 90,
        }
    }
}

/// Log levels for autopilot operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Structured log entry for autopilot operations
#[derive(Debug, Clone, Serialize)]
pub struct AutopilotLogEntry {
    pub timestamp: DateTime<Utc>,
    pub cycle_id: String,
    pub event_type: AutopilotEventType,
    pub level: LogLevel,
    pub message: String,
    pub context: HashMap<String, serde_json::Value>,
    pub metrics: Option<AutopilotMetrics>,
}

/// Types of autopilot events for structured logging
#[derive(Debug, Clone, Serialize)]
pub enum AutopilotEventType {
    CycleStarted,
    CycleCompleted,
    CycleFailed,
    MetricsAnalysisStarted,
    MetricsAnalysisCompleted,
    ParameterChangeProposed,
    ParameterChangeValidated,
    ParameterChangeDeployed,
    ParameterChangeRolledBack,
    BoundsAdjusted,
    AlertTriggered,
    EmergencyPause,
    ManualOverride,
}

/// Autopilot performance metrics
#[derive(Debug, Clone, Serialize)]
pub struct AutopilotMetrics {
    pub cycle_duration_ms: u64,
    pub analysis_duration_ms: u64,
    pub validation_duration_ms: u64,
    pub deployment_duration_ms: u64,
    pub changes_proposed: usize,
    pub changes_deployed: usize,
    pub confidence_score: f64,
    pub prediction_accuracy: Option<f64>,
}

/// Autopilot prediction vs reality tracking
#[derive(Debug, Clone, Serialize)]
pub struct PredictionAccuracyRecord {
    pub timestamp: DateTime<Utc>,
    pub cycle_id: String,
    pub parameter_name: String,
    pub predicted_impact: f64,
    pub actual_impact: f64,
    pub accuracy_score: f64,
    pub measurement_window_hours: u32,
}

impl AutopilotLogger {
    /// Create a new autopilot logger
    pub fn new(metrics_store: Arc<MetricsStore>, config: LoggingConfig) -> Self {
        Self {
            metrics_store,
            config,
        }
    }

    /// Log autopilot cycle start
    pub async fn log_cycle_start(&self, cycle: &AutopilotCycle) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(cycle.cycle_id.clone()));
        context.insert("started_at".to_string(), serde_json::Value::String(cycle.started_at.to_rfc3339()));

        self.log_event(
            &cycle.cycle_id,
            AutopilotEventType::CycleStarted,
            LogLevel::Info,
            "Autopilot cycle started",
            context,
            None,
        ).await?;

        // Record cycle start metric
        if self.config.metrics_recording_enabled {
            self.record_business_metric(
                BusinessMetricType::AutopilotPredVsRealError,
                0.0, // Placeholder, will be updated when cycle completes
                serde_json::json!({
                    "cycle_id": cycle.cycle_id,
                    "event": "cycle_started"
                }),
            ).await?;
        }

        Ok(())
    }

    /// Log autopilot cycle completion
    pub async fn log_cycle_completion(&self, cycle: &AutopilotCycle) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(cycle.cycle_id.clone()));
        context.insert("status".to_string(), serde_json::Value::String(format!("{:?}", cycle.status)));
        context.insert("changes_proposed".to_string(), serde_json::Value::Number(serde_json::Number::from(cycle.proposed_changes.len())));
        
        if let Some(duration) = cycle.duration() {
            context.insert("duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(duration.as_millis() as u64)));
        }

        let metrics = self.calculate_cycle_metrics(cycle);

        self.log_event(
            &cycle.cycle_id,
            AutopilotEventType::CycleCompleted,
            LogLevel::Info,
            "Autopilot cycle completed successfully",
            context,
            Some(metrics.clone()),
        ).await?;

        // Record performance metrics
        if self.config.performance_tracking_enabled {
            self.record_performance_metrics(&cycle.cycle_id, &metrics).await?;
        }

        Ok(())
    }

    /// Log autopilot cycle failure
    pub async fn log_cycle_failure(&self, cycle: &AutopilotCycle, error: &str) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(cycle.cycle_id.clone()));
        context.insert("error".to_string(), serde_json::Value::String(error.to_string()));
        context.insert("status".to_string(), serde_json::Value::String(format!("{:?}", cycle.status)));

        if let Some(duration) = cycle.duration() {
            context.insert("duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(duration.as_millis() as u64)));
        }

        self.log_event(
            &cycle.cycle_id,
            AutopilotEventType::CycleFailed,
            LogLevel::Error,
            &format!("Autopilot cycle failed: {}", error),
            context,
            None,
        ).await?;

        // Trigger alert for cycle failure
        if self.config.alert_integration_enabled {
            self.trigger_cycle_failure_alert(&cycle.cycle_id, error).await?;
        }

        Ok(())
    }

    /// Log parameter change proposal
    pub async fn log_parameter_change_proposal(
        &self,
        cycle_id: &str,
        changes: &[ParameterChange],
    ) -> AutopilotResult<()> {
        for change in changes {
            let mut context = HashMap::new();
            context.insert("cycle_id".to_string(), serde_json::Value::String(cycle_id.to_string()));
            context.insert("parameter_name".to_string(), serde_json::Value::String(change.parameter_name.clone()));
            context.insert("old_value".to_string(), change.old_value.clone());
            context.insert("new_value".to_string(), change.new_value.clone());
            context.insert("confidence".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(change.confidence).unwrap()));
            context.insert("change_type".to_string(), serde_json::Value::String(format!("{:?}", change.change_type)));
            context.insert("rationale".to_string(), serde_json::Value::String(change.rationale.clone()));

            self.log_event(
                cycle_id,
                AutopilotEventType::ParameterChangeProposed,
                LogLevel::Info,
                &format!("Parameter change proposed for {}", change.parameter_name),
                context,
                None,
            ).await?;
        }

        Ok(())
    }

    /// Log parameter change deployment
    pub async fn log_parameter_change_deployment(
        &self,
        cycle_id: &str,
        changes: &[ParameterChange],
        success: bool,
        deployment_time_ms: u64,
    ) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(cycle_id.to_string()));
        context.insert("changes_count".to_string(), serde_json::Value::Number(serde_json::Number::from(changes.len())));
        context.insert("success".to_string(), serde_json::Value::Bool(success));
        context.insert("deployment_time_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(deployment_time_ms)));

        let event_type = if success {
            AutopilotEventType::ParameterChangeDeployed
        } else {
            AutopilotEventType::ParameterChangeRolledBack
        };

        let level = if success { LogLevel::Info } else { LogLevel::Warn };
        let message = if success {
            format!("Successfully deployed {} parameter changes", changes.len())
        } else {
            format!("Failed to deploy {} parameter changes", changes.len())
        };

        self.log_event(cycle_id, event_type, level, &message, context, None).await?;

        // Record deployment success rate metric
        if self.config.metrics_recording_enabled {
            self.record_business_metric(
                BusinessMetricType::AutopilotPredVsRealError,
                if success { 0.0 } else { 1.0 }, // 0 for success, 1 for failure
                serde_json::json!({
                    "cycle_id": cycle_id,
                    "event": "deployment",
                    "success": success,
                    "changes_count": changes.len()
                }),
            ).await?;
        }

        Ok(())
    }

    /// Log bounds adjustment events
    pub async fn log_bounds_adjustment(
        &self,
        cycle_id: &str,
        parameter_name: &str,
        adjustment_type: &str,
        old_bounds: (f64, f64),
        new_bounds: (f64, f64),
        reason: &str,
    ) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(cycle_id.to_string()));
        context.insert("parameter_name".to_string(), serde_json::Value::String(parameter_name.to_string()));
        context.insert("adjustment_type".to_string(), serde_json::Value::String(adjustment_type.to_string()));
        context.insert("old_min".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(old_bounds.0).unwrap()));
        context.insert("old_max".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(old_bounds.1).unwrap()));
        context.insert("new_min".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(new_bounds.0).unwrap()));
        context.insert("new_max".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(new_bounds.1).unwrap()));
        context.insert("reason".to_string(), serde_json::Value::String(reason.to_string()));

        self.log_event(
            cycle_id,
            AutopilotEventType::BoundsAdjusted,
            LogLevel::Info,
            &format!("Adjusted bounds for {} ({})", parameter_name, adjustment_type),
            context,
            None,
        ).await?;

        Ok(())
    }

    /// Log emergency pause events
    pub async fn log_emergency_pause(&self, reason: &str, duration_hours: u32) -> AutopilotResult<()> {
        let mut context = HashMap::new();
        context.insert("reason".to_string(), serde_json::Value::String(reason.to_string()));
        context.insert("duration_hours".to_string(), serde_json::Value::Number(serde_json::Number::from(duration_hours)));
        context.insert("timestamp".to_string(), serde_json::Value::String(Utc::now().to_rfc3339()));

        self.log_event(
            "emergency",
            AutopilotEventType::EmergencyPause,
            LogLevel::Warn,
            &format!("Autopilot emergency pause activated: {}", reason),
            context,
            None,
        ).await?;

        // Trigger critical alert
        if self.config.alert_integration_enabled {
            self.trigger_emergency_pause_alert(reason, duration_hours).await?;
        }

        Ok(())
    }

    /// Record prediction accuracy after measuring actual impact
    pub async fn record_prediction_accuracy(
        &self,
        record: &PredictionAccuracyRecord,
    ) -> AutopilotResult<()> {
        // Record as business metric for P6 integration
        self.record_business_metric(
            BusinessMetricType::AutopilotPredVsRealError,
            1.0 - record.accuracy_score, // Convert accuracy to error rate
            serde_json::json!({
                "cycle_id": record.cycle_id,
                "parameter_name": record.parameter_name,
                "predicted_impact": record.predicted_impact,
                "actual_impact": record.actual_impact,
                "accuracy_score": record.accuracy_score,
                "measurement_window_hours": record.measurement_window_hours
            }),
        ).await?;

        // Log the accuracy measurement
        let mut context = HashMap::new();
        context.insert("cycle_id".to_string(), serde_json::Value::String(record.cycle_id.clone()));
        context.insert("parameter_name".to_string(), serde_json::Value::String(record.parameter_name.clone()));
        context.insert("predicted_impact".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(record.predicted_impact).unwrap()));
        context.insert("actual_impact".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(record.actual_impact).unwrap()));
        context.insert("accuracy_score".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(record.accuracy_score).unwrap()));

        let level = if record.accuracy_score > 0.8 {
            LogLevel::Info
        } else if record.accuracy_score > 0.6 {
            LogLevel::Warn
        } else {
            LogLevel::Error
        };

        self.log_event(
            &record.cycle_id,
            AutopilotEventType::ParameterChangeValidated,
            level,
            &format!("Prediction accuracy for {}: {:.1}%", record.parameter_name, record.accuracy_score * 100.0),
            context,
            None,
        ).await?;

        Ok(())
    }

    /// Internal method to log structured events
    async fn log_event(
        &self,
        cycle_id: &str,
        event_type: AutopilotEventType,
        level: LogLevel,
        message: &str,
        context: HashMap<String, serde_json::Value>,
        metrics: Option<AutopilotMetrics>,
    ) -> AutopilotResult<()> {
        if !self.config.structured_logging_enabled {
            return Ok(());
        }

        let log_entry = AutopilotLogEntry {
            timestamp: Utc::now(),
            cycle_id: cycle_id.to_string(),
            event_type,
            level: level.clone(),
            message: message.to_string(),
            context,
            metrics,
        };

        // Use tracing for structured logging
        match level {
            LogLevel::Debug => {
                tracing::debug!(
                    target: "autopilot_structured",
                    cycle_id = %log_entry.cycle_id,
                    event_type = ?log_entry.event_type,
                    context = ?log_entry.context,
                    metrics = ?log_entry.metrics,
                    "{}",
                    log_entry.message
                );
            }
            LogLevel::Info => {
                tracing::info!(
                    target: "autopilot_structured",
                    cycle_id = %log_entry.cycle_id,
                    event_type = ?log_entry.event_type,
                    context = ?log_entry.context,
                    metrics = ?log_entry.metrics,
                    "{}",
                    log_entry.message
                );
            }
            LogLevel::Warn => {
                tracing::warn!(
                    target: "autopilot_structured",
                    cycle_id = %log_entry.cycle_id,
                    event_type = ?log_entry.event_type,
                    context = ?log_entry.context,
                    metrics = ?log_entry.metrics,
                    "{}",
                    log_entry.message
                );
            }
            LogLevel::Error => {
                tracing::error!(
                    target: "autopilot_structured",
                    cycle_id = %log_entry.cycle_id,
                    event_type = ?log_entry.event_type,
                    context = ?log_entry.context,
                    metrics = ?log_entry.metrics,
                    "{}",
                    log_entry.message
                );
            }
        }

        Ok(())
    }

    /// Calculate performance metrics for a cycle
    fn calculate_cycle_metrics(&self, cycle: &AutopilotCycle) -> AutopilotMetrics {
        let cycle_duration_ms = cycle.duration()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let analysis_duration_ms = cycle.metrics_analysis.as_ref()
            .map(|_| 1000) // Placeholder - would track actual analysis time
            .unwrap_or(0);

        let validation_duration_ms = cycle.validation_results.as_ref()
            .map(|v| v.validation_time.as_millis() as u64)
            .unwrap_or(0);

        let deployment_duration_ms = cycle.deployment_result.as_ref()
            .map(|d| d.deployment_time.as_millis() as u64)
            .unwrap_or(0);

        let confidence_score = cycle.metrics_analysis.as_ref()
            .map(|a| a.confidence_score)
            .unwrap_or(0.0);

        AutopilotMetrics {
            cycle_duration_ms,
            analysis_duration_ms,
            validation_duration_ms,
            deployment_duration_ms,
            changes_proposed: cycle.proposed_changes.len(),
            changes_deployed: if cycle.deployment_result.as_ref().map(|d| d.success).unwrap_or(false) {
                cycle.proposed_changes.len()
            } else {
                0
            },
            confidence_score,
            prediction_accuracy: None, // Will be updated later when actual impact is measured
        }
    }

    /// Record performance metrics to P6 MetricsStore
    async fn record_performance_metrics(
        &self,
        cycle_id: &str,
        metrics: &AutopilotMetrics,
    ) -> AutopilotResult<()> {
        if !self.config.metrics_recording_enabled {
            return Ok(());
        }

        // Record cycle duration
        self.record_business_metric(
            BusinessMetricType::AutopilotPredVsRealError, // Reusing this type for now
            metrics.cycle_duration_ms as f64,
            serde_json::json!({
                "cycle_id": cycle_id,
                "metric_type": "cycle_duration_ms",
                "changes_proposed": metrics.changes_proposed,
                "changes_deployed": metrics.changes_deployed,
                "confidence_score": metrics.confidence_score
            }),
        ).await?;

        Ok(())
    }

    /// Record business metric to P6 MetricsStore
    async fn record_business_metric(
        &self,
        metric_type: BusinessMetricType,
        value: f64,
        context: serde_json::Value,
    ) -> Result<(), MonitorError> {
        let metric = BusinessMetric {
            timestamp: Utc::now(),
            metric_type,
            value,
            context,
        };

        self.metrics_store.record_business_metric(&metric)
    }

    /// Trigger cycle failure alert
    async fn trigger_cycle_failure_alert(&self, cycle_id: &str, error: &str) -> AutopilotResult<()> {
        tracing::error!(
            target: "autopilot_alerts",
            cycle_id = %cycle_id,
            error = %error,
            alert_type = "cycle_failure",
            severity = "high",
            "Autopilot cycle failure alert"
        );

        // This would integrate with P6 AlertEngine in a full implementation
        Ok(())
    }

    /// Trigger emergency pause alert
    async fn trigger_emergency_pause_alert(&self, reason: &str, duration_hours: u32) -> AutopilotResult<()> {
        tracing::error!(
            target: "autopilot_alerts",
            reason = %reason,
            duration_hours = duration_hours,
            alert_type = "emergency_pause",
            severity = "critical",
            "Autopilot emergency pause alert"
        );

        // This would integrate with P6 AlertEngine in a full implementation
        Ok(())
    }

    /// Get logging statistics
    pub fn get_logging_stats(&self) -> LoggingStats {
        LoggingStats {
            structured_logging_enabled: self.config.structured_logging_enabled,
            metrics_recording_enabled: self.config.metrics_recording_enabled,
            performance_tracking_enabled: self.config.performance_tracking_enabled,
            alert_integration_enabled: self.config.alert_integration_enabled,
            retention_days: self.config.retention_days,
        }
    }
}

/// Logging statistics
#[derive(Debug, Clone, Serialize)]
pub struct LoggingStats {
    pub structured_logging_enabled: bool,
    pub metrics_recording_enabled: bool,
    pub performance_tracking_enabled: bool,
    pub alert_integration_enabled: bool,
    pub retention_days: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot::engine::CycleStatus;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert!(config.structured_logging_enabled);
        assert!(config.metrics_recording_enabled);
        assert!(config.performance_tracking_enabled);
        assert!(config.alert_integration_enabled);
        assert_eq!(config.retention_days, 90);
    }

    #[test]
    fn test_cycle_metrics_calculation() {
        // This would test the metrics calculation logic
        // Placeholder for now since we need a full cycle structure
    }
}