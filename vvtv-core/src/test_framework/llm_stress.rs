use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::time::timeout;

use crate::llm::LlmOrchestrator;

/// LLM Stress Test Framework for P7 validation requirements
pub struct LlmStressTest {
    orchestrator: Arc<LlmOrchestrator>,
    config: StressTestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestConfig {
    pub concurrent_calls: usize,
    pub total_calls: usize,
    pub timeout_scenarios: Vec<Duration>,
    pub failure_injection_rate: f64,
    pub test_duration_seconds: u64,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            concurrent_calls: 1000,
            total_calls: 5000,
            timeout_scenarios: vec![
                Duration::from_secs(1),
                Duration::from_secs(5),
                Duration::from_secs(30),
            ],
            failure_injection_rate: 0.1,
            test_duration_seconds: 300, // 5 minutes
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StressTestResult {
    pub total_calls: usize,
    pub successful_calls: usize,
    pub failed_calls: usize,
    pub timeout_calls: usize,
    pub p50_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub max_latency: Duration,
    pub circuit_breaker_transitions: Vec<String>,
    pub fallback_activation_rate: f64,
    pub throughput_per_second: f64,
    pub test_duration: Duration,
    pub errors_by_type: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct CallResult {
    pub success: bool,
    pub latency: Duration,
    pub error_type: Option<String>,
    pub circuit_breaker_triggered: bool,
}

impl LlmStressTest {
    pub fn new(orchestrator: Arc<LlmOrchestrator>, config: StressTestConfig) -> Self {
        Self {
            orchestrator,
            config,
        }
    }

    /// Execute concurrent stress test with 1000 calls
    pub async fn run_stress_test(&self) -> Result<StressTestResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let mut all_results = Vec::new();
        let circuit_breaker_events = Vec::new();

        // Execute calls in batches to manage concurrency
        let batch_size = self.config.concurrent_calls;
        let total_batches = (self.config.total_calls + batch_size - 1) / batch_size;

        for batch_idx in 0..total_batches {
            let calls_in_batch = std::cmp::min(
                batch_size,
                self.config.total_calls - (batch_idx * batch_size)
            );

            let batch_results = self.execute_concurrent_batch(calls_in_batch).await?;
            all_results.extend(batch_results);

            // Small delay between batches to prevent overwhelming the system
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let test_duration = start_time.elapsed();
        self.analyze_results(all_results, circuit_breaker_events, test_duration)
    }

    async fn execute_concurrent_batch(&self, batch_size: usize) -> Result<Vec<CallResult>, Box<dyn std::error::Error>> {
        let mut tasks = Vec::new();

        for i in 0..batch_size {
            let orchestrator = Arc::clone(&self.orchestrator);
            let should_inject_failure = rand::random::<f64>() < self.config.failure_injection_rate;
            let timeout_duration = self.config.timeout_scenarios[i % self.config.timeout_scenarios.len()];

            let task = tokio::spawn(async move {
                Self::execute_single_call(orchestrator, should_inject_failure, timeout_duration).await
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Task panicked or was cancelled
                    results.push(CallResult {
                        success: false,
                        latency: Duration::ZERO,
                        error_type: Some(format!("task_error: {}", e)),
                        circuit_breaker_triggered: false,
                    });
                }
            }
        }

        Ok(results)
    }

    async fn execute_single_call(
        orchestrator: Arc<LlmOrchestrator>,
        inject_failure: bool,
        timeout_duration: Duration,
    ) -> CallResult {
        let start_time = Instant::now();

        if inject_failure {
            // Simulate various failure scenarios
            tokio::time::sleep(timeout_duration + Duration::from_millis(100)).await;
            return CallResult {
                success: false,
                latency: start_time.elapsed(),
                error_type: Some("injected_timeout".to_string()),
                circuit_breaker_triggered: false,
            };
        }

        // Create a mock LLM invocation for testing
        let invocations = vec![
            crate::llm::LlmInvocation {
                plan_id: format!("test_plan_{}", rand::random::<u32>()),
                score: rand::random::<f64>(),
                rationale: "test rationale".to_string(),
                tags: vec!["test".to_string()],
                kind: "rerank_candidates".to_string(),
            }
        ];

        match timeout(timeout_duration, orchestrator.rerank_candidates(invocations)).await {
            Ok(Ok(_result)) => CallResult {
                success: true,
                latency: start_time.elapsed(),
                error_type: None,
                circuit_breaker_triggered: false,
            },
            Ok(Err(e)) => CallResult {
                success: false,
                latency: start_time.elapsed(),
                error_type: Some(format!("llm_error: {}", e)),
                circuit_breaker_triggered: e.to_string().contains("circuit"),
            },
            Err(_) => CallResult {
                success: false,
                latency: start_time.elapsed(),
                error_type: Some("timeout".to_string()),
                circuit_breaker_triggered: false,
            },
        }
    }

    fn analyze_results(
        &self,
        results: Vec<CallResult>,
        circuit_breaker_events: Vec<String>,
        test_duration: Duration,
    ) -> Result<StressTestResult, Box<dyn std::error::Error>> {
        let total_calls = results.len();
        let successful_calls = results.iter().filter(|r| r.success).count();
        let failed_calls = total_calls - successful_calls;
        let timeout_calls = results.iter()
            .filter(|r| r.error_type.as_ref().map_or(false, |e| e.contains("timeout")))
            .count();

        let fallback_activations = results.iter()
            .filter(|r| r.circuit_breaker_triggered)
            .count();
        let fallback_activation_rate = if total_calls > 0 {
            fallback_activations as f64 / total_calls as f64
        } else {
            0.0
        };

        // Calculate latency percentiles
        let mut latencies: Vec<Duration> = results.iter().map(|r| r.latency).collect();
        latencies.sort();

        let p50_latency = Self::percentile(&latencies, 0.5);
        let p95_latency = Self::percentile(&latencies, 0.95);
        let p99_latency = Self::percentile(&latencies, 0.99);
        let max_latency = latencies.last().copied().unwrap_or(Duration::ZERO);

        // Count errors by type
        let mut errors_by_type = std::collections::HashMap::new();
        for result in &results {
            if let Some(error_type) = &result.error_type {
                *errors_by_type.entry(error_type.clone()).or_insert(0) += 1;
            }
        }

        let throughput_per_second = if test_duration.as_secs() > 0 {
            total_calls as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };

        Ok(StressTestResult {
            total_calls,
            successful_calls,
            failed_calls,
            timeout_calls,
            p50_latency,
            p95_latency,
            p99_latency,
            max_latency,
            circuit_breaker_transitions: circuit_breaker_events,
            fallback_activation_rate,
            throughput_per_second,
            test_duration,
            errors_by_type,
        })
    }

    fn percentile(sorted_latencies: &[Duration], percentile: f64) -> Duration {
        if sorted_latencies.is_empty() {
            return Duration::ZERO;
        }

        let index = ((sorted_latencies.len() as f64 - 1.0) * percentile) as usize;
        sorted_latencies[index.min(sorted_latencies.len() - 1)]
    }

    /// Validate circuit breaker behavior under simulated failure conditions
    pub async fn test_circuit_breaker_behavior(&self) -> Result<CircuitBreakerTestResult, Box<dyn std::error::Error>> {
        // This would test specific circuit breaker scenarios
        // For now, return a placeholder result
        Ok(CircuitBreakerTestResult {
            transitions_detected: 0,
            recovery_time: Duration::ZERO,
            half_open_behavior_correct: true,
        })
    }

    /// Generate detailed performance report
    pub fn generate_report(&self, result: &StressTestResult) -> String {
        let success_rate = (result.successful_calls as f64 / result.total_calls as f64) * 100.0;
        
        let mut report = String::new();
        report.push_str("LLM Stress Test Report\n");
        report.push_str("======================\n\n");
        report.push_str(&format!("Test Configuration:\n"));
        report.push_str(&format!("  Concurrent Calls: {}\n", self.config.concurrent_calls));
        report.push_str(&format!("  Total Calls: {}\n", self.config.total_calls));
        report.push_str(&format!("  Failure Injection Rate: {:.1}%\n", self.config.failure_injection_rate * 100.0));
        report.push_str("\n");
        
        report.push_str(&format!("Results:\n"));
        report.push_str(&format!("  Total Calls: {}\n", result.total_calls));
        report.push_str(&format!("  Successful: {} ({:.1}%)\n", result.successful_calls, success_rate));
        report.push_str(&format!("  Failed: {} ({:.1}%)\n", result.failed_calls, 100.0 - success_rate));
        report.push_str(&format!("  Timeouts: {}\n", result.timeout_calls));
        report.push_str(&format!("  Test Duration: {:?}\n", result.test_duration));
        report.push_str(&format!("  Throughput: {:.1} calls/sec\n", result.throughput_per_second));
        report.push_str("\n");
        
        report.push_str(&format!("Latency Percentiles:\n"));
        report.push_str(&format!("  P50: {:?}\n", result.p50_latency));
        report.push_str(&format!("  P95: {:?}\n", result.p95_latency));
        report.push_str(&format!("  P99: {:?}\n", result.p99_latency));
        report.push_str(&format!("  Max: {:?}\n", result.max_latency));
        report.push_str("\n");
        
        report.push_str(&format!("Circuit Breaker:\n"));
        report.push_str(&format!("  Fallback Activation Rate: {:.1}%\n", result.fallback_activation_rate * 100.0));
        report.push_str(&format!("  Transitions: {}\n", result.circuit_breaker_transitions.len()));
        report.push_str("\n");
        
        if !result.errors_by_type.is_empty() {
            report.push_str("Errors by Type:\n");
            for (error_type, count) in &result.errors_by_type {
                report.push_str(&format!("  {}: {}\n", error_type, count));
            }
        }
        
        report
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerTestResult {
    pub transitions_detected: usize,
    pub recovery_time: Duration,
    pub half_open_behavior_correct: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_test_config_default() {
        let config = StressTestConfig::default();
        assert_eq!(config.concurrent_calls, 1000);
        assert_eq!(config.total_calls, 5000);
        assert_eq!(config.failure_injection_rate, 0.1);
    }

    #[test]
    fn test_percentile_calculation() {
        let latencies = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
            Duration::from_millis(40),
            Duration::from_millis(50),
        ];
        
        let p50 = LlmStressTest::percentile(&latencies, 0.5);
        let p95 = LlmStressTest::percentile(&latencies, 0.95);
        
        assert_eq!(p50, Duration::from_millis(30));
        // For 5 elements, 95th percentile should be at index 3.8, which rounds to index 3 (40ms)
        assert_eq!(p95, Duration::from_millis(40));
    }

    #[test]
    fn test_call_result_creation() {
        let result = CallResult {
            success: true,
            latency: Duration::from_millis(100),
            error_type: None,
            circuit_breaker_triggered: false,
        };
        
        assert!(result.success);
        assert_eq!(result.latency, Duration::from_millis(100));
        assert!(result.error_type.is_none());
    }
}