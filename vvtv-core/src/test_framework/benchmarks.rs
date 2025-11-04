use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Performance Benchmark Suite for P7 validation requirements
#[derive(Debug)]
pub struct PerformanceBenchmarks {
    baseline_results: HashMap<String, BenchmarkResult>,
    baseline_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub p50_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub max_latency: Duration,
    pub min_latency: Duration,
    pub throughput: f64,
    pub memory_usage: usize,
    pub iterations: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkComparison {
    pub current: BenchmarkResult,
    pub baseline: BenchmarkResult,
    pub regression_detected: bool,
    pub performance_delta_pct: f64,
    pub latency_delta_pct: f64,
    pub throughput_delta_pct: f64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub regression_threshold_pct: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 1000,
            warmup_iterations: 100,
            regression_threshold_pct: 20.0,
        }
    }
}

impl PerformanceBenchmarks {
    pub fn new<P: AsRef<Path>>(baseline_file: P) -> Self {
        Self {
            baseline_results: HashMap::new(),
            baseline_file: baseline_file.as_ref().to_path_buf(),
        }
    }

    /// Load baseline results from file
    pub fn load_baseline(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.baseline_file.exists() {
            let content = fs::read_to_string(&self.baseline_file)?;
            self.baseline_results = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    /// Save current results as new baseline
    pub fn save_baseline(&self, results: &HashMap<String, BenchmarkResult>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.baseline_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(results)?;
        fs::write(&self.baseline_file, content)?;
        Ok(())
    }

    /// Benchmark Gumbel-Top-k selection performance
    pub fn benchmark_gumbel_topk(&self, config: &BenchmarkConfig) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let mut memory_usage = 0;

        // Warmup
        for _ in 0..config.warmup_iterations {
            let _ = self.run_gumbel_topk_iteration();
        }

        let start_time = Instant::now();

        // Actual benchmark
        for _ in 0..config.iterations {
            let iteration_start = Instant::now();
            let _result = self.run_gumbel_topk_iteration();
            let iteration_time = iteration_start.elapsed();
            latencies.push(iteration_time);

            // Estimate memory usage (simplified)
            memory_usage = std::cmp::max(memory_usage, 1024 * 100); // 100KB estimate
        }

        let total_time = start_time.elapsed();
        let throughput = config.iterations as f64 / total_time.as_secs_f64();

        latencies.sort();
        let result = BenchmarkResult {
            name: "gumbel_topk_selection".to_string(),
            p50_latency: Self::percentile(&latencies, 0.5),
            p95_latency: Self::percentile(&latencies, 0.95),
            p99_latency: Self::percentile(&latencies, 0.99),
            max_latency: latencies.last().copied().unwrap_or(Duration::ZERO),
            min_latency: latencies.first().copied().unwrap_or(Duration::ZERO),
            throughput,
            memory_usage,
            iterations: config.iterations,
            timestamp: chrono::Utc::now(),
        };

        Ok(result)
    }

    fn run_gumbel_topk_iteration(&self) -> Vec<usize> {
        // Simulate Gumbel-Top-k selection with 100 candidates
        let scores: Vec<f64> = (0..100).map(|i| rand::random::<f64>() + i as f64 / 100.0).collect();
        let temperature = 0.85;
        let top_k = 12;

        let scaled_scores: Vec<f64> = scores.iter().map(|s| s / temperature).collect();
        
        // Simplified Gumbel-Top-k implementation for benchmarking
        let mut rng = rand::thread_rng();
        crate::plan::selection::gumbel_topk_indices(&scaled_scores, top_k, &mut rng)
    }

    /// Benchmark Business Logic Engine YAML loading
    pub fn benchmark_business_logic_loading(&self, config: &BenchmarkConfig) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let yaml_content = r#"
policy_version: "1.0"
env: "benchmark"
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

        // Warmup
        for _ in 0..config.warmup_iterations {
            let _: crate::business_logic::BusinessLogic = serde_yaml::from_str(yaml_content)?;
        }

        let start_time = Instant::now();

        // Actual benchmark
        for _ in 0..config.iterations {
            let iteration_start = Instant::now();
            let mut business_logic: crate::business_logic::BusinessLogic = serde_yaml::from_str(yaml_content)?;
            let _ = business_logic.validate();
            let iteration_time = iteration_start.elapsed();
            latencies.push(iteration_time);
        }

        let total_time = start_time.elapsed();
        let throughput = config.iterations as f64 / total_time.as_secs_f64();

        latencies.sort();
        let result = BenchmarkResult {
            name: "business_logic_loading".to_string(),
            p50_latency: Self::percentile(&latencies, 0.5),
            p95_latency: Self::percentile(&latencies, 0.95),
            p99_latency: Self::percentile(&latencies, 0.99),
            max_latency: latencies.last().copied().unwrap_or(Duration::ZERO),
            min_latency: latencies.first().copied().unwrap_or(Duration::ZERO),
            throughput,
            memory_usage: 1024 * 50, // 50KB estimate
            iterations: config.iterations,
            timestamp: chrono::Utc::now(),
        };

        Ok(result)
    }

    /// Benchmark Metrics Store write operations
    pub fn benchmark_metrics_store_writes(&self, config: &BenchmarkConfig) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("benchmark_metrics.db");
        
        let store = crate::monitor::MetricsStore::new(&db_path)?;
        store.initialize()?;

        let mut latencies = Vec::new();

        // Warmup
        for i in 0..config.warmup_iterations {
            let metric = crate::monitor::BusinessMetric::new(
                crate::monitor::BusinessMetricType::SelectionEntropy,
                rand::random::<f64>()
            ).with_context(serde_json::json!({"iteration": i}));
            let _ = store.record_business_metric(&metric);
        }

        let start_time = Instant::now();

        // Actual benchmark
        for i in 0..config.iterations {
            let iteration_start = Instant::now();
            let metric = crate::monitor::BusinessMetric::new(
                crate::monitor::BusinessMetricType::SelectionEntropy,
                rand::random::<f64>()
            ).with_context(serde_json::json!({"iteration": i}));
            let _ = store.record_business_metric(&metric)?;
            let iteration_time = iteration_start.elapsed();
            latencies.push(iteration_time);
        }

        let total_time = start_time.elapsed();
        let throughput = config.iterations as f64 / total_time.as_secs_f64();

        latencies.sort();
        let result = BenchmarkResult {
            name: "metrics_store_writes".to_string(),
            p50_latency: Self::percentile(&latencies, 0.5),
            p95_latency: Self::percentile(&latencies, 0.95),
            p99_latency: Self::percentile(&latencies, 0.99),
            max_latency: latencies.last().copied().unwrap_or(Duration::ZERO),
            min_latency: latencies.first().copied().unwrap_or(Duration::ZERO),
            throughput,
            memory_usage: 1024 * 200, // 200KB estimate
            iterations: config.iterations,
            timestamp: chrono::Utc::now(),
        };

        Ok(result)
    }

    /// Benchmark Curator Vigilante decision processing
    pub fn benchmark_curator_decisions(&self, config: &BenchmarkConfig) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let curator_config = crate::curation::CuratorVigilanteConfig::with_log_dir(temp_dir.path());
        let curator = crate::curation::CuratorVigilante::new(curator_config)?;

        let mut latencies = Vec::new();

        // Create test candidates
        let candidates = self.create_test_candidates(12);

        // Warmup
        for _ in 0..config.warmup_iterations {
            let _ = curator.review(chrono::Utc::now(), &candidates, None);
        }

        let start_time = Instant::now();

        // Actual benchmark
        for _ in 0..config.iterations {
            let iteration_start = Instant::now();
            let _review = curator.review(chrono::Utc::now(), &candidates, None);
            let iteration_time = iteration_start.elapsed();
            latencies.push(iteration_time);
        }

        let total_time = start_time.elapsed();
        let throughput = config.iterations as f64 / total_time.as_secs_f64();

        latencies.sort();
        let result = BenchmarkResult {
            name: "curator_decisions".to_string(),
            p50_latency: Self::percentile(&latencies, 0.5),
            p95_latency: Self::percentile(&latencies, 0.95),
            p99_latency: Self::percentile(&latencies, 0.99),
            max_latency: latencies.last().copied().unwrap_or(Duration::ZERO),
            min_latency: latencies.first().copied().unwrap_or(Duration::ZERO),
            throughput,
            memory_usage: 1024 * 150, // 150KB estimate
            iterations: config.iterations,
            timestamp: chrono::Utc::now(),
        };

        Ok(result)
    }

    fn create_test_candidates(&self, count: usize) -> Vec<crate::curation::CuratorCandidate> {
        (0..count).map(|i| {
            let mut plan = crate::plan::Plan::new(&format!("test_plan_{}", i), "music");
            plan.curation_score = rand::random::<f64>();
            plan.trending_score = rand::random::<f64>();
            plan.engagement_score = rand::random::<f64>();
            plan.tags = vec![format!("tag_{}", i % 5)];
            
            crate::curation::CuratorCandidate {
                plan,
                score: rand::random::<f64>(),
                rationale: format!("test rationale {}", i),
            }
        }).collect()
    }

    /// Compare current results against baseline and detect regressions
    pub fn compare_with_baseline(&self, current: &BenchmarkResult, threshold_pct: f64) -> Option<BenchmarkComparison> {
        if let Some(baseline) = self.baseline_results.get(&current.name) {
            let latency_delta_pct = ((current.p95_latency.as_nanos() as f64 - baseline.p95_latency.as_nanos() as f64) 
                / baseline.p95_latency.as_nanos() as f64) * 100.0;
            
            let throughput_delta_pct = ((current.throughput - baseline.throughput) / baseline.throughput) * 100.0;
            
            let performance_delta_pct = latency_delta_pct; // Use latency as primary performance indicator
            let regression_detected = performance_delta_pct > threshold_pct;

            Some(BenchmarkComparison {
                current: current.clone(),
                baseline: baseline.clone(),
                regression_detected,
                performance_delta_pct,
                latency_delta_pct,
                throughput_delta_pct,
            })
        } else {
            None
        }
    }

    /// Run all core algorithm benchmarks
    pub fn run_all_benchmarks(&self, config: &BenchmarkConfig) -> Result<HashMap<String, BenchmarkResult>, Box<dyn std::error::Error>> {
        let mut results = HashMap::new();

        println!("Running Gumbel-Top-k benchmark...");
        let gumbel_result = self.benchmark_gumbel_topk(config)?;
        results.insert(gumbel_result.name.clone(), gumbel_result);

        println!("Running Business Logic loading benchmark...");
        let bl_result = self.benchmark_business_logic_loading(config)?;
        results.insert(bl_result.name.clone(), bl_result);

        println!("Running Metrics Store writes benchmark...");
        let metrics_result = self.benchmark_metrics_store_writes(config)?;
        results.insert(metrics_result.name.clone(), metrics_result);

        println!("Running Curator decisions benchmark...");
        let curator_result = self.benchmark_curator_decisions(config)?;
        results.insert(curator_result.name.clone(), curator_result);

        Ok(results)
    }

    /// Generate performance report with regression detection
    pub fn generate_report(&self, results: &HashMap<String, BenchmarkResult>, threshold_pct: f64) -> String {
        let mut report = String::new();
        report.push_str("Performance Benchmark Report\n");
        report.push_str("============================\n\n");

        let mut regressions_detected = 0;

        for (name, result) in results {
            report.push_str(&format!("Benchmark: {}\n", name));
            report.push_str(&format!("  Iterations: {}\n", result.iterations));
            report.push_str(&format!("  P50 Latency: {:?}\n", result.p50_latency));
            report.push_str(&format!("  P95 Latency: {:?}\n", result.p95_latency));
            report.push_str(&format!("  P99 Latency: {:?}\n", result.p99_latency));
            report.push_str(&format!("  Max Latency: {:?}\n", result.max_latency));
            report.push_str(&format!("  Throughput: {:.1} ops/sec\n", result.throughput));
            report.push_str(&format!("  Memory Usage: {} KB\n", result.memory_usage / 1024));

            if let Some(comparison) = self.compare_with_baseline(result, threshold_pct) {
                report.push_str(&format!("  Baseline Comparison:\n"));
                report.push_str(&format!("    Latency Change: {:+.1}%\n", comparison.latency_delta_pct));
                report.push_str(&format!("    Throughput Change: {:+.1}%\n", comparison.throughput_delta_pct));
                
                if comparison.regression_detected {
                    report.push_str(&format!("    ❌ REGRESSION DETECTED (>{:.1}% threshold)\n", threshold_pct));
                    regressions_detected += 1;
                } else {
                    report.push_str(&format!("    ✅ Performance within acceptable range\n"));
                }
            } else {
                report.push_str("  No baseline available for comparison\n");
            }
            
            report.push_str("\n");
        }

        if regressions_detected > 0 {
            report.push_str(&format!("⚠️  {} performance regressions detected!\n", regressions_detected));
        } else {
            report.push_str("✅ All benchmarks passed performance requirements\n");
        }

        report
    }

    fn percentile(sorted_durations: &[Duration], percentile: f64) -> Duration {
        if sorted_durations.is_empty() {
            return Duration::ZERO;
        }

        let index = ((sorted_durations.len() as f64 - 1.0) * percentile) as usize;
        sorted_durations[index.min(sorted_durations.len() - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_benchmark_result_serialization() {
        let result = BenchmarkResult {
            name: "test_benchmark".to_string(),
            p50_latency: Duration::from_millis(10),
            p95_latency: Duration::from_millis(50),
            p99_latency: Duration::from_millis(100),
            max_latency: Duration::from_millis(200),
            min_latency: Duration::from_millis(5),
            throughput: 1000.0,
            memory_usage: 1024 * 100,
            iterations: 1000,
            timestamp: chrono::Utc::now(),
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: BenchmarkResult = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.name, "test_benchmark");
        assert_eq!(deserialized.throughput, 1000.0);
        assert_eq!(deserialized.iterations, 1000);
    }

    #[test]
    fn test_percentile_calculation() {
        let durations = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
            Duration::from_millis(40),
            Duration::from_millis(50),
        ];
        
        let p50 = PerformanceBenchmarks::percentile(&durations, 0.5);
        let p95 = PerformanceBenchmarks::percentile(&durations, 0.95);
        
        assert_eq!(p50, Duration::from_millis(30));
        // For 5 elements, 95th percentile should be at index 3.8, which rounds to index 3 (40ms)
        assert_eq!(p95, Duration::from_millis(40));
    }

    #[test]
    fn test_regression_detection() {
        let temp_dir = tempdir().unwrap();
        let baseline_file = temp_dir.path().join("baseline.json");
        
        let mut benchmarks = PerformanceBenchmarks::new(&baseline_file);
        
        let baseline = BenchmarkResult {
            name: "test".to_string(),
            p50_latency: Duration::from_millis(10),
            p95_latency: Duration::from_millis(50),
            p99_latency: Duration::from_millis(100),
            max_latency: Duration::from_millis(200),
            min_latency: Duration::from_millis(5),
            throughput: 1000.0,
            memory_usage: 1024,
            iterations: 1000,
            timestamp: chrono::Utc::now(),
        };
        
        benchmarks.baseline_results.insert("test".to_string(), baseline);
        
        let current = BenchmarkResult {
            name: "test".to_string(),
            p50_latency: Duration::from_millis(15),
            p95_latency: Duration::from_millis(75), // 50% increase
            p99_latency: Duration::from_millis(150),
            max_latency: Duration::from_millis(300),
            min_latency: Duration::from_millis(8),
            throughput: 800.0, // 20% decrease
            memory_usage: 1024,
            iterations: 1000,
            timestamp: chrono::Utc::now(),
        };
        
        let comparison = benchmarks.compare_with_baseline(&current, 20.0).unwrap();
        assert!(comparison.regression_detected); // Should detect regression due to 50% latency increase
    }
}