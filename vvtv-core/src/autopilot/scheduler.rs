use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{interval, sleep};

use super::engine::{AutopilotEngine, AutopilotError};

/// Daily scheduler for autopilot execution
#[derive(Debug)]
pub struct DailyScheduler {
    schedule_hour: u32,
    schedule_minute: u32,
    timeout_duration: Duration,
    last_execution: Option<DateTime<Utc>>,
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub enabled: bool,
    pub schedule_utc: String, // "03:00"
    pub timeout_minutes: u32,
    pub retry_on_failure: bool,
    pub max_retries: u32,
    pub retry_delay_minutes: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            schedule_utc: "03:00".to_string(),
            timeout_minutes: 10,
            retry_on_failure: true,
            max_retries: 3,
            retry_delay_minutes: 30,
        }
    }
}

/// Scheduler execution state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerState {
    pub last_execution: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub next_scheduled: Option<DateTime<Utc>>,
    pub is_paused: bool,
    pub pause_until: Option<DateTime<Utc>>,
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self {
            last_execution: None,
            last_success: None,
            consecutive_failures: 0,
            next_scheduled: None,
            is_paused: false,
            pause_until: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("scheduler timeout after {0} minutes")]
    Timeout(u32),
    #[error("scheduler is paused until {0}")]
    Paused(DateTime<Utc>),
    #[error("invalid schedule format: {0}")]
    InvalidSchedule(String),
    #[error("autopilot error: {0}")]
    Autopilot(#[from] AutopilotError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl DailyScheduler {
    /// Create a new daily scheduler
    pub fn new(schedule_utc: &str, timeout_minutes: u32) -> Result<Self, SchedulerError> {
        let (hour, minute) = Self::parse_schedule(schedule_utc)?;
        
        Ok(Self {
            schedule_hour: hour,
            schedule_minute: minute,
            timeout_duration: Duration::from_secs(timeout_minutes as u64 * 60),
            last_execution: None,
        })
    }

    /// Parse schedule string (HH:MM) into hour and minute
    fn parse_schedule(schedule: &str) -> Result<(u32, u32), SchedulerError> {
        let parts: Vec<&str> = schedule.split(':').collect();
        if parts.len() != 2 {
            return Err(SchedulerError::InvalidSchedule(
                "Schedule must be in HH:MM format".to_string()
            ));
        }

        let hour = parts[0].parse::<u32>()
            .map_err(|_| SchedulerError::InvalidSchedule("Invalid hour".to_string()))?;
        let minute = parts[1].parse::<u32>()
            .map_err(|_| SchedulerError::InvalidSchedule("Invalid minute".to_string()))?;

        if hour > 23 {
            return Err(SchedulerError::InvalidSchedule("Hour must be 0-23".to_string()));
        }
        if minute > 59 {
            return Err(SchedulerError::InvalidSchedule("Minute must be 0-59".to_string()));
        }

        Ok((hour, minute))
    }

    /// Calculate next scheduled execution time
    pub fn next_execution_time(&self) -> DateTime<Utc> {
        let now = Utc::now();
        let today_scheduled = now
            .date_naive()
            .and_hms_opt(self.schedule_hour, self.schedule_minute, 0)
            .unwrap()
            .and_utc();

        if now < today_scheduled {
            // Today's execution hasn't happened yet
            today_scheduled
        } else {
            // Schedule for tomorrow
            today_scheduled + chrono::Duration::days(1)
        }
    }

    /// Check if it's time to execute autopilot
    pub fn should_execute_now(&self) -> bool {
        let now = Utc::now();
        
        // Check if we're within the execution window (±5 minutes)
        let scheduled_time = self.next_execution_time();
        let window_start = scheduled_time - chrono::Duration::minutes(5);
        let window_end = scheduled_time + chrono::Duration::minutes(5);
        
        let in_window = now >= window_start && now <= window_end;
        
        // Check if we haven't executed today already
        let not_executed_today = self.last_execution
            .map(|last| last.date_naive() < now.date_naive())
            .unwrap_or(true);
        
        in_window && not_executed_today
    }

    /// Run the scheduler loop
    pub async fn run_scheduler_loop(
        &mut self,
        autopilot_engine: Arc<tokio::sync::Mutex<AutopilotEngine>>,
        config: SchedulerConfig,
    ) -> Result<(), SchedulerError> {
        let mut state = SchedulerState::default();
        let mut check_interval = interval(Duration::from_secs(60)); // Check every minute

        tracing::info!(
            target: "autopilot_scheduler",
            schedule = %config.schedule_utc,
            "starting autopilot scheduler loop"
        );

        loop {
            check_interval.tick().await;

            if !config.enabled {
                continue;
            }

            // Check if scheduler is paused
            if let Some(pause_until) = state.pause_until {
                if Utc::now() < pause_until {
                    continue;
                }
                // Resume from pause
                state.is_paused = false;
                state.pause_until = None;
                tracing::info!(target: "autopilot_scheduler", "resuming from pause");
            }

            if state.is_paused {
                continue;
            }

            // Check if it's time to execute
            if self.should_execute_now() {
                tracing::info!(
                    target: "autopilot_scheduler",
                    next_execution = %self.next_execution_time(),
                    "triggering scheduled autopilot execution"
                );

                match self.execute_with_timeout(autopilot_engine.clone()).await {
                    Ok(_) => {
                        state.last_execution = Some(Utc::now());
                        state.last_success = Some(Utc::now());
                        state.consecutive_failures = 0;
                        self.last_execution = Some(Utc::now());
                        
                        tracing::info!(target: "autopilot_scheduler", "scheduled execution completed successfully");
                    }
                    Err(e) => {
                        state.last_execution = Some(Utc::now());
                        state.consecutive_failures += 1;
                        self.last_execution = Some(Utc::now());
                        
                        tracing::error!(
                            target: "autopilot_scheduler",
                            error = %e,
                            consecutive_failures = state.consecutive_failures,
                            "scheduled execution failed"
                        );

                        // Handle retries and failure responses
                        if config.retry_on_failure && state.consecutive_failures <= config.max_retries {
                            tracing::info!(
                                target: "autopilot_scheduler",
                                retry_in_minutes = config.retry_delay_minutes,
                                attempt = state.consecutive_failures,
                                max_retries = config.max_retries,
                                "scheduling retry"
                            );
                            
                            // Wait before retry
                            sleep(Duration::from_secs(config.retry_delay_minutes as u64 * 60)).await;
                        } else if state.consecutive_failures > config.max_retries {
                            // Pause scheduler after too many failures
                            let pause_duration = Duration::from_secs(24 * 60 * 60); // 24 hours
                            state.is_paused = true;
                            state.pause_until = Some(Utc::now() + chrono::Duration::from_std(pause_duration).unwrap());
                            
                            tracing::error!(
                                target: "autopilot_scheduler",
                                pause_until = %state.pause_until.unwrap(),
                                "pausing scheduler due to repeated failures"
                            );
                        }
                    }
                }
            }

            // Update next scheduled time
            state.next_scheduled = Some(self.next_execution_time());
        }
    }

    /// Execute autopilot with timeout protection
    async fn execute_with_timeout(
        &self,
        autopilot_engine: Arc<tokio::sync::Mutex<AutopilotEngine>>,
    ) -> Result<(), SchedulerError> {
        let execution_future = async {
            let mut engine = autopilot_engine.lock().await;
            engine.run_daily_cycle().await
        };

        match tokio::time::timeout(self.timeout_duration, execution_future).await {
            Ok(Ok(_cycle)) => Ok(()),
            Ok(Err(e)) => Err(SchedulerError::Autopilot(e)),
            Err(_) => Err(SchedulerError::Timeout(self.timeout_duration.as_secs() as u32 / 60)),
        }
    }

    /// Manually trigger autopilot execution (for testing/emergency)
    pub async fn trigger_manual_execution(
        &mut self,
        autopilot_engine: Arc<tokio::sync::Mutex<AutopilotEngine>>,
    ) -> Result<(), SchedulerError> {
        tracing::info!(target: "autopilot_scheduler", "triggering manual autopilot execution");
        
        self.execute_with_timeout(autopilot_engine).await?;
        self.last_execution = Some(Utc::now());
        
        tracing::info!(target: "autopilot_scheduler", "manual execution completed");
        Ok(())
    }

    /// Pause scheduler for specified duration
    pub fn pause_scheduler(&self, duration: Duration) -> SchedulerState {
        let pause_until = Utc::now() + chrono::Duration::from_std(duration).unwrap();
        
        tracing::info!(
            target: "autopilot_scheduler",
            pause_until = %pause_until,
            "pausing scheduler"
        );

        SchedulerState {
            is_paused: true,
            pause_until: Some(pause_until),
            ..Default::default()
        }
    }

    /// Resume scheduler from pause
    pub fn resume_scheduler(&self) -> SchedulerState {
        tracing::info!(target: "autopilot_scheduler", "resuming scheduler");
        
        SchedulerState {
            is_paused: false,
            pause_until: None,
            ..Default::default()
        }
    }

    /// Get scheduler status
    pub fn get_status(&self) -> SchedulerStatus {
        SchedulerStatus {
            next_execution: self.next_execution_time(),
            last_execution: self.last_execution,
            schedule_hour: self.schedule_hour,
            schedule_minute: self.schedule_minute,
            timeout_minutes: self.timeout_duration.as_secs() / 60,
        }
    }
}

/// Scheduler status information
#[derive(Debug, Clone, Serialize)]
pub struct SchedulerStatus {
    pub next_execution: DateTime<Utc>,
    pub last_execution: Option<DateTime<Utc>>,
    pub schedule_hour: u32,
    pub schedule_minute: u32,
    pub timeout_minutes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_parsing() {
        assert!(DailyScheduler::parse_schedule("03:00").is_ok());
        assert!(DailyScheduler::parse_schedule("23:59").is_ok());
        assert!(DailyScheduler::parse_schedule("00:00").is_ok());
        
        assert!(DailyScheduler::parse_schedule("24:00").is_err());
        assert!(DailyScheduler::parse_schedule("12:60").is_err());
        assert!(DailyScheduler::parse_schedule("invalid").is_err());
        assert!(DailyScheduler::parse_schedule("12").is_err());
    }

    #[test]
    fn test_next_execution_time() {
        let scheduler = DailyScheduler::new("03:00", 10).unwrap();
        let next = scheduler.next_execution_time();
        
        assert_eq!(next.hour(), 3);
        assert_eq!(next.minute(), 0);
        assert_eq!(next.second(), 0);
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = DailyScheduler::new("15:30", 20).unwrap();
        assert_eq!(scheduler.schedule_hour, 15);
        assert_eq!(scheduler.schedule_minute, 30);
        assert_eq!(scheduler.timeout_duration, Duration::from_secs(20 * 60));
    }
}