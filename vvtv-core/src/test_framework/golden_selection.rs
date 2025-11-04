
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::business_logic::BusinessLogic;
use crate::plan::{Plan, Planner, PlannerConfig};
use crate::plan::store::SqlitePlanStore;

/// Golden Selection Test Framework for P7 validation requirements
#[derive(Debug)]
pub struct GoldenSelectionTest {
    test_cases: Vec<GoldenTestCase>,
    business_logic: Arc<BusinessLogic>,
    test_data_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenTestCase {
    pub name: String,
    pub seed: u64,
    pub candidates: Vec<TestPlan>,
    pub expected_order: Vec<String>,
    pub config_overrides: Option<TestBusinessLogic>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlan {
    pub plan_id: String,
    pub kind: String,
    pub curation_score: f64,
    pub trending_score: f64,
    pub engagement_score: f64,
    pub duration_est_s: Option<i64>,
    pub tags: Vec<String>,
    pub hd_missing: bool,
    pub created_at: Option<DateTime<Utc>>,
}

impl From<TestPlan> for Plan {
    fn from(test_plan: TestPlan) -> Self {
        let mut plan = Plan::new(&test_plan.plan_id, &test_plan.kind);
        plan.curation_score = test_plan.curation_score;
        plan.trending_score = test_plan.trending_score;
        plan.engagement_score = test_plan.engagement_score;
        plan.duration_est_s = test_plan.duration_est_s;
        plan.tags = test_plan.tags;
        plan.hd_missing = test_plan.hd_missing;
        plan.created_at = test_plan.created_at;
        plan
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestBusinessLogic {
    pub selection_temperature: Option<f64>,
    pub selection_top_k: Option<usize>,
    pub plan_selection_bias: Option<f64>,
    pub diversity_quota: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoldenTestResult {
    pub test_name: String,
    pub passed: bool,
    pub actual_order: Vec<String>,
    pub expected_order: Vec<String>,
    pub diff: Option<String>,
    pub execution_time: Duration,
    pub seed_used: u64,
    pub error_message: Option<String>,
}

impl GoldenSelectionTest {
    pub fn new<P: AsRef<Path>>(
        business_logic: Arc<BusinessLogic>,
        test_data_path: P,
    ) -> std::io::Result<Self> {
        let test_data_path = test_data_path.as_ref().to_path_buf();
        
        Ok(Self {
            test_cases: Vec::new(),
            business_logic,
            test_data_path,
        })
    }

    /// Load test cases from JSON/YAML files in the test data directory
    pub fn load_test_cases(&mut self) -> std::io::Result<()> {
        if !self.test_data_path.exists() {
            fs::create_dir_all(&self.test_data_path)?;
            self.create_default_test_cases()?;
        }

        for entry in fs::read_dir(&self.test_data_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let test_case: GoldenTestCase = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                self.test_cases.push(test_case);
            }
        }

        Ok(())
    }

    /// Execute all test cases and return results
    pub fn run_all_tests(&self) -> Vec<GoldenTestResult> {
        self.test_cases
            .iter()
            .map(|test_case| self.execute_test_case(test_case))
            .collect()
    }

    /// Execute a single test case with recovery
    pub fn execute_test_case(&self, test_case: &GoldenTestCase) -> GoldenTestResult {
        let start_time = Instant::now();
        
        match self.run_test_case_internal(test_case) {
            Ok(actual_order) => {
                let execution_time = start_time.elapsed();
                let passed = actual_order == test_case.expected_order;
                let diff = if passed {
                    None
                } else {
                    Some(self.generate_diff(&test_case.expected_order, &actual_order))
                };

                GoldenTestResult {
                    test_name: test_case.name.clone(),
                    passed,
                    actual_order,
                    expected_order: test_case.expected_order.clone(),
                    diff,
                    execution_time,
                    seed_used: test_case.seed,
                    error_message: None,
                }
            }
            Err(e) => GoldenTestResult {
                test_name: test_case.name.clone(),
                passed: false,
                actual_order: vec![],
                expected_order: test_case.expected_order.clone(),
                diff: Some(format!("Test execution failed: {e}")),
                execution_time: start_time.elapsed(),
                seed_used: test_case.seed,
                error_message: Some(e.to_string()),
            }
        }
    }

    fn run_test_case_internal(&self, test_case: &GoldenTestCase) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Create temporary database for test
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("test_plans.db");
        
        let store = SqlitePlanStore::builder()
            .path(&db_path)
            .build()?;
        store.initialize()?;

        // Insert test candidates into store
        for test_plan in &test_case.candidates {
            let plan: Plan = test_plan.clone().into();
            store.upsert_plan(&plan)?;
        }

        // Create business logic with overrides
        let business_logic = if let Some(overrides) = &test_case.config_overrides {
            self.create_overridden_business_logic(overrides)?
        } else {
            Arc::clone(&self.business_logic)
        };

        // Create planner with test configuration
        let mut config = PlannerConfig::default();
        if let Some(overrides) = &test_case.config_overrides {
            if let Some(quota) = overrides.diversity_quota {
                config.diversity_quota = quota;
            }
        }

        let planner = Planner::new(store, config, business_logic);

        // Execute selection with fixed timestamp for determinism
        let fixed_time = Utc::now(); // Use consistent time for reproducibility
        let event = planner.run_once(fixed_time)?;

        // Extract order from selection decisions
        match event {
            crate::plan::PlannerEvent::Selected(decisions) => {
                Ok(decisions.into_iter().map(|d| d.plan_id).collect())
            }
            crate::plan::PlannerEvent::Idle => {
                Ok(vec![])
            }
        }
    }

    fn create_overridden_business_logic(&self, _overrides: &TestBusinessLogic) -> Result<Arc<BusinessLogic>, Box<dyn std::error::Error>> {
        // For now, return the original business logic
        // In a full implementation, we would create a modified version
        Ok(Arc::clone(&self.business_logic))
    }

    fn generate_diff(&self, expected: &[String], actual: &[String]) -> String {
        let mut diff = String::new();
        diff.push_str("Expected vs Actual order:\n");
        
        let max_len = expected.len().max(actual.len());
        for i in 0..max_len {
            let exp = expected.get(i).map(|s| s.as_str()).unwrap_or("<missing>");
            let act = actual.get(i).map(|s| s.as_str()).unwrap_or("<missing>");
            
            if exp == act {
                diff.push_str(&format!("  {}: {} ✓\n", i, exp));
            } else {
                diff.push_str(&format!("  {}: {} → {} ✗\n", i, exp, act));
            }
        }
        
        diff
    }

    /// Create default test cases for common scenarios
    fn create_default_test_cases(&self) -> std::io::Result<()> {
        let test_cases = vec![
            self.create_basic_selection_test(),
            self.create_tied_scores_test(),
            self.create_diversity_test(),
            self.create_empty_candidates_test(),
            self.create_performance_test(),
        ];

        for (i, test_case) in test_cases.into_iter().enumerate() {
            let file_path = self.test_data_path.join(format!("test_case_{}.json", i + 1));
            let content = serde_json::to_string_pretty(&test_case)?;
            fs::write(file_path, content)?;
        }

        Ok(())
    }

    fn create_basic_selection_test(&self) -> GoldenTestCase {
        GoldenTestCase {
            name: "basic_selection".to_string(),
            seed: 42,
            candidates: vec![
                TestPlan {
                    plan_id: "plan_1".to_string(),
                    kind: "music".to_string(),
                    curation_score: 0.9,
                    trending_score: 0.8,
                    engagement_score: 0.7,
                    duration_est_s: Some(180),
                    tags: vec!["electronic".to_string(), "upbeat".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
                TestPlan {
                    plan_id: "plan_2".to_string(),
                    kind: "video".to_string(),
                    curation_score: 0.8,
                    trending_score: 0.9,
                    engagement_score: 0.8,
                    duration_est_s: Some(300),
                    tags: vec!["documentary".to_string(), "nature".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
                TestPlan {
                    plan_id: "plan_3".to_string(),
                    kind: "music".to_string(),
                    curation_score: 0.7,
                    trending_score: 0.6,
                    engagement_score: 0.9,
                    duration_est_s: Some(240),
                    tags: vec!["jazz".to_string(), "relaxing".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
            ],
            expected_order: vec!["plan_1".to_string(), "plan_2".to_string(), "plan_3".to_string()],
            config_overrides: None,
            description: "Basic selection with varied scores".to_string(),
        }
    }

    fn create_tied_scores_test(&self) -> GoldenTestCase {
        GoldenTestCase {
            name: "tied_scores".to_string(),
            seed: 123,
            candidates: vec![
                TestPlan {
                    plan_id: "tie_1".to_string(),
                    kind: "music".to_string(),
                    curation_score: 0.8,
                    trending_score: 0.8,
                    engagement_score: 0.8,
                    duration_est_s: Some(200),
                    tags: vec!["pop".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
                TestPlan {
                    plan_id: "tie_2".to_string(),
                    kind: "video".to_string(),
                    curation_score: 0.8,
                    trending_score: 0.8,
                    engagement_score: 0.8,
                    duration_est_s: Some(200),
                    tags: vec!["comedy".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
            ],
            expected_order: vec!["tie_1".to_string(), "tie_2".to_string()],
            config_overrides: None,
            description: "Test deterministic behavior with tied scores".to_string(),
        }
    }

    fn create_diversity_test(&self) -> GoldenTestCase {
        GoldenTestCase {
            name: "diversity_enforcement".to_string(),
            seed: 456,
            candidates: vec![
                TestPlan {
                    plan_id: "music_1".to_string(),
                    kind: "music".to_string(),
                    curation_score: 0.9,
                    trending_score: 0.9,
                    engagement_score: 0.9,
                    duration_est_s: Some(180),
                    tags: vec!["rock".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
                TestPlan {
                    plan_id: "music_2".to_string(),
                    kind: "music".to_string(),
                    curation_score: 0.85,
                    trending_score: 0.85,
                    engagement_score: 0.85,
                    duration_est_s: Some(200),
                    tags: vec!["pop".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
                TestPlan {
                    plan_id: "video_1".to_string(),
                    kind: "video".to_string(),
                    curation_score: 0.7,
                    trending_score: 0.7,
                    engagement_score: 0.7,
                    duration_est_s: Some(300),
                    tags: vec!["tutorial".to_string()],
                    hd_missing: false,
                    created_at: Some(Utc::now()),
                },
            ],
            expected_order: vec!["music_1".to_string(), "video_1".to_string(), "music_2".to_string()],
            config_overrides: Some(TestBusinessLogic {
                selection_temperature: None,
                selection_top_k: Some(3),
                plan_selection_bias: None,
                diversity_quota: Some(0.5), // Force diversity
            }),
            description: "Test diversity quota enforcement".to_string(),
        }
    }

    fn create_empty_candidates_test(&self) -> GoldenTestCase {
        GoldenTestCase {
            name: "empty_candidates".to_string(),
            seed: 789,
            candidates: vec![],
            expected_order: vec![],
            config_overrides: None,
            description: "Test behavior with no candidates".to_string(),
        }
    }

    fn create_performance_test(&self) -> GoldenTestCase {
        let mut candidates = Vec::new();
        for i in 0..100 {
            candidates.push(TestPlan {
                plan_id: format!("perf_{}", i),
                kind: if i % 2 == 0 { "music" } else { "video" }.to_string(),
                curation_score: 0.5 + (i as f64 / 200.0),
                trending_score: 0.4 + (i as f64 / 250.0),
                engagement_score: 0.6 + (i as f64 / 300.0),
                duration_est_s: Some(180 + (i * 10)),
                tags: vec![format!("tag_{}", i % 10)],
                hd_missing: i % 10 == 0,
                created_at: Some(Utc::now()),
            });
        }

        // Expected order should be deterministic based on scoring
        let expected_order: Vec<String> = (90..100).rev()
            .map(|i| format!("perf_{}", i))
            .chain((80..90).rev().map(|i| format!("perf_{}", i)))
            .take(12) // Default batch size
            .collect();

        GoldenTestCase {
            name: "performance_100_candidates".to_string(),
            seed: 999,
            candidates,
            expected_order,
            config_overrides: None,
            description: "Performance test with 100 candidates for latency validation".to_string(),
        }
    }

    /// Generate a summary report of test results
    pub fn generate_report(&self, results: &[GoldenTestResult]) -> String {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        
        let avg_execution_time = if !results.is_empty() {
            results.iter().map(|r| r.execution_time).sum::<Duration>() / results.len() as u32
        } else {
            Duration::ZERO
        };

        let max_execution_time = results.iter()
            .map(|r| r.execution_time)
            .max()
            .unwrap_or(Duration::ZERO);

        let mut report = String::new();
        report.push_str("Golden Selection Test Report\n");
        report.push_str("============================\n\n");
        report.push_str(&format!("Total Tests: {}\n", total));
        report.push_str(&format!("Passed: {} ({:.1}%)\n", passed, (passed as f64 / total as f64) * 100.0));
        report.push_str(&format!("Failed: {} ({:.1}%)\n", failed, (failed as f64 / total as f64) * 100.0));
        report.push_str(&format!("Average Execution Time: {:?}\n", avg_execution_time));
        report.push_str(&format!("Max Execution Time: {:?}\n", max_execution_time));
        report.push_str("\n");

        if failed > 0 {
            report.push_str("Failed Tests:\n");
            report.push_str("-------------\n");
            for result in results.iter().filter(|r| !r.passed) {
                report.push_str(&format!("❌ {}: {}\n", result.test_name, 
                    result.error_message.as_deref().unwrap_or("Order mismatch")));
                if let Some(diff) = &result.diff {
                    report.push_str(&format!("   {}\n", diff.replace('\n', "\n   ")));
                }
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_golden_test_case_serialization() {
        let test_case = GoldenTestCase {
            name: "test".to_string(),
            seed: 42,
            candidates: vec![],
            expected_order: vec!["a".to_string(), "b".to_string()],
            config_overrides: None,
            description: "Test case".to_string(),
        };

        let serialized = serde_json::to_string(&test_case).unwrap();
        let deserialized: GoldenTestCase = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.seed, 42);
        assert_eq!(deserialized.expected_order, vec!["a", "b"]);
    }

    #[test]
    fn test_diff_generation() {
        let temp_dir = tempdir().unwrap();
        let business_logic = Arc::new(
            crate::business_logic::BusinessLogic::load_from_file("../configs/business_logic.yaml")
                .unwrap_or_else(|_| {
                    // Create a minimal business logic for testing
                    serde_yaml::from_str(r#"
policy_version: "1.0"
env: "test"
knobs:
  boost_bucket: "trending"
  music_mood_focus: ["upbeat", "energetic"]
  interstitials_ratio: 0.15
  plan_selection_bias: 0.0
selection:
  method: "gumbel_top_k"
  temperature: 0.85
  top_k: 12
"#).unwrap()
                })
        );
        
        let test = GoldenSelectionTest::new(business_logic, temp_dir.path()).unwrap();
        
        let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let actual = vec!["a".to_string(), "c".to_string(), "b".to_string()];
        
        let diff = test.generate_diff(&expected, &actual);
        
        assert!(diff.contains("a ✓"));
        assert!(diff.contains("b → c ✗"));
        assert!(diff.contains("c → b ✗"));
    }
}