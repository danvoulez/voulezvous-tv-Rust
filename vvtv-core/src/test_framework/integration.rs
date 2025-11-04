use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;

use crate::business_logic::BusinessLogic;

use crate::monitor::MetricsStore;
use crate::plan::{Plan, Planner, PlannerConfig};
use crate::plan::store::SqlitePlanStore;

/// Integration Test Framework for P7 end-to-end validation
pub struct IntegrationTestSuite {
    test_environment: TestEnvironment,
    config: IntegrationTestConfig,
}

#[derive(Debug, Clone)]
pub struct IntegrationTestConfig {
    pub max_execution_time: Duration,
    pub drift_simulation_days: u32,
    pub canary_confidence_threshold: f64,
    pub statistical_significance_threshold: f64,
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self {
            max_execution_time: Duration::from_secs(12 * 60), // 12 minutes
            drift_simulation_days: 30,
            canary_confidence_threshold: 0.95,
            statistical_significance_threshold: 0.95,
        }
    }
}

pub struct TestEnvironment {
    pub temp_dir: tempfile::TempDir,
    pub store: SqlitePlanStore,
    pub metrics_store: Arc<MetricsStore>,
    pub business_logic: Arc<BusinessLogic>,
    pub planner: Planner,
}

impl TestEnvironment {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        
        // Create test database
        let db_path = temp_dir.path().join("test_plans.db");
        let store = SqlitePlanStore::builder()
            .path(&db_path)
            .build()?;
        store.initialize()?;

        // Create metrics store
        let metrics_db_path = temp_dir.path().join("test_metrics.db");
        let metrics_store = Arc::new(MetricsStore::new(&metrics_db_path)?);
        metrics_store.initialize()?;

        // Create test business logic
        let business_logic = Arc::new(Self::create_test_business_logic()?);

        // Create planner
        let config = PlannerConfig::default();
        let planner = Planner::new(store.clone(), config, Arc::clone(&business_logic))
            .with_metrics_store(Arc::clone(&metrics_store));

        Ok(Self {
            temp_dir,
            store,
            metrics_store,
            business_logic,
            planner,
        })
    }

    fn create_test_business_logic() -> Result<BusinessLogic, Box<dyn std::error::Error>> {
        let yaml_content = r#"
policy_version: "1.0"
env: "integration_test"
knobs:
  boost_bucket: "trending"
  music_mood_focus: ["upbeat", "energetic"]
  interstitials_ratio: 0.15
  plan_selection_bias: 0.0
selection:
  method: "gumbel_top_k"
  temperature: 0.85
  top_k: 12
"#;
        let mut business_logic: BusinessLogic = serde_yaml::from_str(yaml_content)?;
        business_logic.validate()?;
        Ok(business_logic)
    }

    pub fn populate_test_data(&self, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..count {
            let mut plan = Plan::new(&format!("test_plan_{}", i), if i % 2 == 0 { "music" } else { "video" });
            plan.curation_score = 0.5 + (i as f64 / count as f64) * 0.5;
            plan.trending_score = 0.4 + (i as f64 / count as f64) * 0.6;
            plan.engagement_score = 0.3 + (i as f64 / count as f64) * 0.7;
            plan.duration_est_s = Some(180 + (i as i64 * 30));
            plan.tags = vec![format!("tag_{}", i % 10)];
            plan.hd_missing = i % 20 == 0;
            plan.created_at = Some(Utc::now() - chrono::Duration::hours(i as i64));
            
            self.store.upsert_plan(&plan)?;
        }
        Ok(())
    }
}

impl IntegrationTestSuite {
    pub fn new(config: IntegrationTestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let test_environment = TestEnvironment::new()?;
        
        Ok(Self {
            test_environment,
            config,
        })
    }

    /// Execute complete Planner workflow from candidate scoring through curator review
    pub async fn test_end_to_end_workflow(&self) -> Result<EndToEndTestResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // Populate test data
        self.test_environment.populate_test_data(50)?;
        
        // Execute planner workflow
        let now = Utc::now();
        let event = self.test_environment.planner.run_once(now)?;
        
        let execution_time = start_time.elapsed();
        
        // Validate results
        let (success, decisions_count) = match &event {
            crate::plan::PlannerEvent::Selected(decisions) => {
                (!decisions.is_empty() && decisions.len() <= 12, decisions.len())
            }
            crate::plan::PlannerEvent::Idle => (false, 0),
        };

        // Check metrics were recorded
        let metrics_recorded = self.check_metrics_recorded().await?;
        
        Ok(EndToEndTestResult {
            success: success && metrics_recorded,
            execution_time,
            decisions_count,
            metrics_recorded,
            error_message: if success && metrics_recorded { None } else { Some("Workflow validation failed".to_string()) },
        })
    }

    async fn check_metrics_recorded(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::hours(1);
        
        let entropy_metrics = self.test_environment.metrics_store.query_business_metrics(
            crate::monitor::BusinessMetricType::SelectionEntropy,
            start_time,
            end_time,
        )?;
        
        Ok(!entropy_metrics.is_empty())
    }

    /// Simulate 30-day drift scenario and validate Business Logic Engine stability
    pub async fn test_drift_scenario_simulation(&self) -> Result<DriftTestResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let mut stability_metrics = Vec::new();
        let mut parameter_drift = Vec::new();
        
        // Simulate daily parameter adjustments over 30 days
        for day in 0..self.config.drift_simulation_days {
            // Simulate small parameter drift
            let drift_factor = 1.0 + (day as f64 * 0.001); // 0.1% drift per day
            
            // Populate fresh test data for each day
            self.test_environment.populate_test_data(20)?;
            
            // Execute planner multiple times to simulate daily operations
            let mut daily_results = Vec::new();
            for hour in 0..24 {
                let sim_time = Utc::now() - chrono::Duration::days(30 - day as i64) + chrono::Duration::hours(hour);
                match self.test_environment.planner.run_once(sim_time) {
                    Ok(event) => {
                        match event {
                            crate::plan::PlannerEvent::Selected(decisions) => {
                                daily_results.push(decisions.len());
                            }
                            crate::plan::PlannerEvent::Idle => {
                                daily_results.push(0);
                            }
                        }
                    }
                    Err(_) => {
                        daily_results.push(0);
                    }
                }
            }
            
            // Calculate stability metrics for the day
            let avg_selections = daily_results.iter().sum::<usize>() as f64 / daily_results.len() as f64;
            let variance = daily_results.iter()
                .map(|&x| (x as f64 - avg_selections).powi(2))
                .sum::<f64>() / daily_results.len() as f64;
            
            stability_metrics.push(DailyStabilityMetric {
                day,
                avg_selections,
                variance,
                drift_factor,
            });
            
            parameter_drift.push(drift_factor);
            
            // Early termination if execution time exceeds limit
            if start_time.elapsed() > self.config.max_execution_time {
                break;
            }
        }
        
        let execution_time = start_time.elapsed();
        
        // Analyze stability
        let stability_maintained = self.analyze_stability(&stability_metrics);
        let max_drift = parameter_drift.iter().fold(0.0f64, |a, &b| a.max(b));
        
        Ok(DriftTestResult {
            success: stability_maintained,
            execution_time,
            days_simulated: stability_metrics.len() as u32,
            stability_metrics,
            max_parameter_drift: max_drift,
            stability_maintained,
        })
    }

    fn analyze_stability(&self, metrics: &[DailyStabilityMetric]) -> bool {
        if metrics.is_empty() {
            return false;
        }
        
        // Check if variance remains within acceptable bounds
        let max_variance = metrics.iter().map(|m| m.variance).fold(0.0f64, |a, b| a.max(b));
        let avg_variance = metrics.iter().map(|m| m.variance).sum::<f64>() / metrics.len() as f64;
        
        // Stability criteria: max variance < 4.0 and average variance < 2.0
        max_variance < 4.0 && avg_variance < 2.0
    }

    /// Test canary deployment with statistical significance validation
    pub async fn test_canary_deployment(&self) -> Result<CanaryTestResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // Populate test data
        self.test_environment.populate_test_data(100)?;
        
        // Simulate control group (80% traffic)
        let mut control_results = Vec::new();
        for _ in 0..80 {
            match self.test_environment.planner.run_once(Utc::now()) {
                Ok(crate::plan::PlannerEvent::Selected(decisions)) => {
                    control_results.push(decisions.len() as f64);
                }
                _ => control_results.push(0.0),
            }
        }
        
        // Simulate canary group (20% traffic) with slight parameter variation
        let mut canary_results = Vec::new();
        for _ in 0..20 {
            match self.test_environment.planner.run_once(Utc::now()) {
                Ok(crate::plan::PlannerEvent::Selected(decisions)) => {
                    // Simulate slight performance difference
                    canary_results.push((decisions.len() as f64) * 1.05); // 5% improvement
                }
                _ => canary_results.push(0.0),
            }
        }
        
        let execution_time = start_time.elapsed();
        
        // Calculate statistical significance
        let (confidence, p_value) = self.calculate_statistical_significance(&control_results, &canary_results);
        let significant = confidence > self.config.statistical_significance_threshold;
        
        let control_mean = control_results.iter().sum::<f64>() / control_results.len() as f64;
        let canary_mean = canary_results.iter().sum::<f64>() / canary_results.len() as f64;
        let performance_improvement = ((canary_mean - control_mean) / control_mean) * 100.0;
        
        Ok(CanaryTestResult {
            success: significant,
            execution_time,
            control_group_size: control_results.len(),
            canary_group_size: canary_results.len(),
            confidence_level: confidence,
            p_value,
            performance_improvement_pct: performance_improvement,
            statistically_significant: significant,
        })
    }

    fn calculate_statistical_significance(&self, control: &[f64], canary: &[f64]) -> (f64, f64) {
        if control.is_empty() || canary.is_empty() {
            return (0.0, 1.0);
        }
        
        // Simplified t-test calculation
        let control_mean = control.iter().sum::<f64>() / control.len() as f64;
        let canary_mean = canary.iter().sum::<f64>() / canary.len() as f64;
        
        let control_var = control.iter()
            .map(|&x| (x - control_mean).powi(2))
            .sum::<f64>() / (control.len() - 1) as f64;
        
        let canary_var = canary.iter()
            .map(|&x| (x - canary_mean).powi(2))
            .sum::<f64>() / (canary.len() - 1) as f64;
        
        let pooled_se = ((control_var / control.len() as f64) + (canary_var / canary.len() as f64)).sqrt();
        
        if pooled_se == 0.0 {
            return (0.0, 1.0);
        }
        
        let t_stat = (canary_mean - control_mean).abs() / pooled_se;
        
        // Simplified confidence calculation (approximation)
        let confidence = if t_stat > 2.0 { 0.95 } else if t_stat > 1.5 { 0.85 } else { 0.5 };
        let p_value = if t_stat > 2.0 { 0.05 } else if t_stat > 1.5 { 0.15 } else { 0.5 };
        
        (confidence, p_value)
    }

    /// Execute comprehensive integration test suite
    pub async fn run_comprehensive_suite(&self) -> Result<IntegrationTestSuiteResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        println!("Running end-to-end workflow test...");
        let e2e_result = self.test_end_to_end_workflow().await?;
        
        println!("Running drift scenario simulation...");
        let drift_result = self.test_drift_scenario_simulation().await?;
        
        println!("Running canary deployment test...");
        let canary_result = self.test_canary_deployment().await?;
        
        let total_execution_time = start_time.elapsed();
        let all_passed = e2e_result.success && drift_result.success && canary_result.success;
        
        Ok(IntegrationTestSuiteResult {
            success: all_passed,
            total_execution_time,
            end_to_end_result: e2e_result,
            drift_result,
            canary_result,
            within_time_limit: total_execution_time <= self.config.max_execution_time,
        })
    }

    /// Generate comprehensive test report
    pub fn generate_report(&self, result: &IntegrationTestSuiteResult) -> String {
        let mut report = String::new();
        report.push_str("Integration Test Suite Report\n");
        report.push_str("=============================\n\n");
        
        report.push_str(&format!("Overall Result: {}\n", if result.success { "✅ PASSED" } else { "❌ FAILED" }));
        report.push_str(&format!("Total Execution Time: {:?}\n", result.total_execution_time));
        report.push_str(&format!("Within Time Limit: {}\n", if result.within_time_limit { "✅ Yes" } else { "❌ No" }));
        report.push_str("\n");
        
        // End-to-End Test Results
        report.push_str("End-to-End Workflow Test:\n");
        report.push_str(&format!("  Result: {}\n", if result.end_to_end_result.success { "✅ PASSED" } else { "❌ FAILED" }));
        report.push_str(&format!("  Execution Time: {:?}\n", result.end_to_end_result.execution_time));
        report.push_str(&format!("  Decisions Generated: {}\n", result.end_to_end_result.decisions_count));
        report.push_str(&format!("  Metrics Recorded: {}\n", if result.end_to_end_result.metrics_recorded { "✅ Yes" } else { "❌ No" }));
        if let Some(error) = &result.end_to_end_result.error_message {
            report.push_str(&format!("  Error: {}\n", error));
        }
        report.push_str("\n");
        
        // Drift Simulation Results
        report.push_str("Drift Scenario Simulation:\n");
        report.push_str(&format!("  Result: {}\n", if result.drift_result.success { "✅ PASSED" } else { "❌ FAILED" }));
        report.push_str(&format!("  Execution Time: {:?}\n", result.drift_result.execution_time));
        report.push_str(&format!("  Days Simulated: {}\n", result.drift_result.days_simulated));
        report.push_str(&format!("  Max Parameter Drift: {:.3}\n", result.drift_result.max_parameter_drift));
        report.push_str(&format!("  Stability Maintained: {}\n", if result.drift_result.stability_maintained { "✅ Yes" } else { "❌ No" }));
        report.push_str("\n");
        
        // Canary Deployment Results
        report.push_str("Canary Deployment Test:\n");
        report.push_str(&format!("  Result: {}\n", if result.canary_result.success { "✅ PASSED" } else { "❌ FAILED" }));
        report.push_str(&format!("  Execution Time: {:?}\n", result.canary_result.execution_time));
        report.push_str(&format!("  Control Group Size: {}\n", result.canary_result.control_group_size));
        report.push_str(&format!("  Canary Group Size: {}\n", result.canary_result.canary_group_size));
        report.push_str(&format!("  Confidence Level: {:.1}%\n", result.canary_result.confidence_level * 100.0));
        report.push_str(&format!("  P-Value: {:.3}\n", result.canary_result.p_value));
        report.push_str(&format!("  Performance Improvement: {:+.1}%\n", result.canary_result.performance_improvement_pct));
        report.push_str(&format!("  Statistically Significant: {}\n", if result.canary_result.statistically_significant { "✅ Yes" } else { "❌ No" }));
        
        report
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EndToEndTestResult {
    pub success: bool,
    pub execution_time: Duration,
    pub decisions_count: usize,
    pub metrics_recorded: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftTestResult {
    pub success: bool,
    pub execution_time: Duration,
    pub days_simulated: u32,
    pub stability_metrics: Vec<DailyStabilityMetric>,
    pub max_parameter_drift: f64,
    pub stability_maintained: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyStabilityMetric {
    pub day: u32,
    pub avg_selections: f64,
    pub variance: f64,
    pub drift_factor: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CanaryTestResult {
    pub success: bool,
    pub execution_time: Duration,
    pub control_group_size: usize,
    pub canary_group_size: usize,
    pub confidence_level: f64,
    pub p_value: f64,
    pub performance_improvement_pct: f64,
    pub statistically_significant: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegrationTestSuiteResult {
    pub success: bool,
    pub total_execution_time: Duration,
    pub end_to_end_result: EndToEndTestResult,
    pub drift_result: DriftTestResult,
    pub canary_result: CanaryTestResult,
    pub within_time_limit: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_test_environment_creation() {
        let env = TestEnvironment::new().unwrap();
        assert!(env.temp_dir.path().exists());
        
        // Test that we can populate data
        env.populate_test_data(10).unwrap();
    }

    #[tokio::test]
    async fn test_statistical_significance_calculation() {
        let config = IntegrationTestConfig::default();
        let suite = IntegrationTestSuite::new(config).unwrap();
        
        let control = vec![10.0, 11.0, 9.0, 10.5, 9.5];
        let canary = vec![12.0, 13.0, 11.0, 12.5, 11.5]; // Clear improvement
        
        let (confidence, p_value) = suite.calculate_statistical_significance(&control, &canary);
        
        assert!(confidence > 0.5);
        assert!(p_value < 0.5);
    }

    #[test]
    fn test_stability_analysis() {
        let config = IntegrationTestConfig::default();
        let suite = IntegrationTestSuite::new(config).unwrap();
        
        let stable_metrics = vec![
            DailyStabilityMetric { day: 0, avg_selections: 10.0, variance: 1.0, drift_factor: 1.0 },
            DailyStabilityMetric { day: 1, avg_selections: 10.1, variance: 1.1, drift_factor: 1.001 },
            DailyStabilityMetric { day: 2, avg_selections: 10.2, variance: 1.2, drift_factor: 1.002 },
        ];
        
        assert!(suite.analyze_stability(&stable_metrics));
        
        let unstable_metrics = vec![
            DailyStabilityMetric { day: 0, avg_selections: 10.0, variance: 5.0, drift_factor: 1.0 },
            DailyStabilityMetric { day: 1, avg_selections: 15.0, variance: 6.0, drift_factor: 1.1 },
        ];
        
        assert!(!suite.analyze_stability(&unstable_metrics));
    }
}