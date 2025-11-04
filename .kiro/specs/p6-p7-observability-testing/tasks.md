# Implementation Plan

- [ ] 1. Enhanced Metrics Infrastructure
  - Extend existing MetricsStore with business metrics support
  - Create BusinessMetric types and collection interfaces
  - Implement retention policies and automated cleanup
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 1.1 Create BusinessMetric data structures and enums
  - Define BusinessMetric struct with timestamp, type, value, and context fields
  - Implement BusinessMetricType enum covering selection_entropy, curator_apply_budget_used_pct, novelty_temporal_kld, hd_detection_slow_rate, autopilot_pred_vs_real_error
  - Add serde serialization support for JSON context storage
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 1.2 Extend MetricsStore with business metrics capabilities
  - Add business_metrics table schema to existing SQLite database
  - Implement record_business_metric method with error handling
  - Create query_metrics method with time range filtering and metric type selection
  - Add cleanup_expired_metrics method respecting retention_days configuration
  - _Requirements: 1.5_

- [x] 1.3 Integrate metrics collection into core components
  - Add metrics recording to Planner selection algorithm (selection_entropy)
  - Integrate curator budget tracking in CuratorVigilante (curator_apply_budget_used_pct)
  - Add novelty measurement collection in content processing pipeline
  - Implement HD detection rate tracking in browser automation
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 1.4 Write unit tests for metrics infrastructure
  - Test BusinessMetric serialization and deserialization
  - Validate metrics store CRUD operations and concurrent access
  - Test retention policy enforcement and cleanup mechanisms
  - Verify time range queries and metric type filtering
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 2. Dashboard Generation System
  - Build DashboardGenerator with template engine support
  - Implement HTML and Grafana JSON export formats
  - Create business logic overview and autopilot health dashboards
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [x] 2.1 Implement core DashboardGenerator structure
  - Create DashboardGenerator struct with metrics store integration
  - Define Dashboard, DashboardConfig, and DashboardFormat types
  - Implement template engine for HTML rendering with time series data
  - Add Grafana JSON export functionality for dashboard definitions
  - _Requirements: 2.3, 2.4_

- [x] 2.2 Build Business Logic Overview dashboard
  - Create dashboard showing selection entropy trends over time
  - Add curator apply budget usage visualization with threshold indicators
  - Include LLM circuit breaker status and failure rate charts
  - Implement drill-down capabilities for detailed metric analysis
  - _Requirements: 2.1, 2.5_

- [x] 2.3 Build Autopilot Health dashboard
  - Create prediction accuracy tracking charts (autopilot_pred_vs_real_error)
  - Add rollback rate monitoring and drift detection visualizations
  - Include novelty temporal KLD trending for content diversity analysis
  - Implement alert status indicators and recent incident timeline
  - _Requirements: 2.2, 2.5_

- [x] 2.4 Add dashboard export and rendering capabilities
  - Implement HTML export with embedded CSS and JavaScript for interactivity
  - Create Grafana JSON export compatible with Grafana import API
  - Add time range selectors and auto-refresh functionality
  - Ensure dashboard rendering completes within 30 seconds for all data ranges
  - _Requirements: 2.3, 2.4_

- [x] 2.5 Write dashboard generation tests
  - Test HTML template rendering with sample metrics data
  - Validate Grafana JSON format compliance and import compatibility
  - Test dashboard generation performance with large datasets
  - Verify time range filtering and drill-down functionality
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] 3. Alert Engine Implementation
  - Create AlertEngine with configurable rules and channels
  - Implement alert conditions and severity levels
  - Add cooldown periods and state tracking
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 3.1 Build AlertEngine core infrastructure
  - Create AlertEngine struct with metrics store integration and rule management
  - Define AlertRule, AlertCondition, and AlertSeverity types
  - Implement alert state tracking with SQLite persistence
  - Add AlertChannel trait for extensible notification delivery
  - _Requirements: 3.5_

- [x] 3.2 Implement critical alert rules
  - Create diversity_loss alert for selection_entropy below 0.3 threshold
  - Add curator_budget_exhausted alert for budget usage above 90%
  - Implement llm_degraded alert for circuit breaker open state duration
  - Create quality_degradation alert for HD detection slow rate above 25%
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 3.3 Add alert evaluation and delivery logic
  - Implement periodic alert rule evaluation against current metrics
  - Add cooldown period enforcement to prevent alert spam
  - Create alert message formatting with severity levels and recommended actions
  - Implement alert delivery via configured channels (email, webhook, etc.)
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 3.4 Write alert engine tests
  - Test alert rule evaluation logic with simulated metric data
  - Validate cooldown period enforcement and state transitions
  - Test alert delivery mechanisms and failure handling
  - Verify alert message formatting and severity classification
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 4. Golden Selection Test Framework
  - Create deterministic test suite for selection algorithms
  - Implement test case management and execution
  - Add regression detection and reporting
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [x] 4.1 Build GoldenSelectionTest framework
  - Create GoldenSelectionTest struct with test case management
  - Define GoldenTestCase and GoldenTestResult data structures
  - Implement test case loading from JSON/YAML test data files
  - Add deterministic test execution with fixed seed values
  - _Requirements: 4.1, 4.2_

- [x] 4.2 Create comprehensive golden test dataset
  - Generate test cases covering normal selection scenarios with varied candidate scores
  - Add edge cases for tied scores, boundary conditions, and empty candidate sets
  - Create configuration override scenarios testing different business logic parameters
  - Include performance test cases with 100+ candidates for latency validation
  - _Requirements: 4.3, 4.5_

- [x] 4.3 Implement test execution and validation
  - Add test runner with parallel execution support for performance
  - Implement detailed diff generation for failed test cases showing expected vs actual
  - Create test result reporting with execution time tracking
  - Add regression detection comparing current results against baseline
  - _Requirements: 4.1, 4.2, 4.4_

- [x] 4.4 Integrate with CI pipeline
  - Create make test target for golden selection test execution
  - Add test result parsing and CI failure reporting
  - Implement test timeout enforcement (5 seconds maximum)
  - Create test coverage reporting for selection algorithm code paths
  - _Requirements: 4.3, 4.4_

- [x] 4.5 Write golden test framework tests
  - Test golden test case loading and validation
  - Verify deterministic behavior across multiple test runs
  - Test diff generation accuracy for failed cases
  - Validate performance requirements and timeout handling
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [ ] 5. LLM Stress Testing Framework
  - Build concurrent LLM testing infrastructure
  - Implement circuit breaker behavior validation
  - Add performance measurement and reporting
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [x] 5.1 Create LlmStressTest framework
  - Build LlmStressTest struct with configurable concurrent call management
  - Define StressTestConfig for test parameters (concurrent calls, timeouts, failure rates)
  - Implement StressTestResult with detailed performance and reliability metrics
  - Add mock LLM handler for controlled testing scenarios
  - _Requirements: 5.1, 5.4_

- [x] 5.2 Implement concurrent stress testing
  - Create 1000 concurrent LLM call execution with tokio task management
  - Add timeout scenario simulation with configurable delay injection
  - Implement failure injection for circuit breaker behavior testing
  - Measure p95 response times and throughput under load
  - _Requirements: 5.1, 5.4_

- [x] 5.3 Add circuit breaker validation
  - Test circuit breaker state transitions (closed → open → half-open)
  - Validate failure threshold detection and recovery behavior
  - Measure fallback activation rates during circuit breaker open state
  - Test half-open state limited traffic and recovery validation
  - _Requirements: 5.2, 5.3_

- [x] 5.4 Create stress test reporting
  - Generate detailed performance reports with latency percentiles
  - Add circuit breaker event timeline and state transition analysis
  - Create failure analysis with categorization (timeout, error, circuit breaker)
  - Implement comparison against baseline performance metrics
  - _Requirements: 5.4, 5.5_

- [x] 5.5 Write LLM stress test validation
  - Test concurrent execution framework and task management
  - Validate circuit breaker behavior under simulated failure conditions
  - Test performance measurement accuracy and reporting
  - Verify stress test timeout and resource cleanup
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 6. Performance Benchmark Suite
  - Create performance measurement infrastructure
  - Implement baseline comparison and regression detection
  - Add automated performance reporting
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 6.1 Build PerformanceBenchmarks framework
  - Create PerformanceBenchmarks struct with baseline management
  - Define BenchmarkResult and BenchmarkComparison data structures
  - Implement criterion-based micro-benchmarking for critical code paths
  - Add memory usage profiling and throughput measurement capabilities
  - _Requirements: 7.1, 7.2, 7.3_

- [x] 6.2 Implement core algorithm benchmarks
  - Benchmark Gumbel-Top-k selection with p95 latency measurement for 100 candidates
  - Add Business Logic Engine YAML loading performance testing
  - Benchmark Metrics Store write operations for throughput validation (1000+ records/sec)
  - Create Curator Vigilante decision processing performance tests
  - _Requirements: 7.1, 7.2, 7.3_

- [x] 6.3 Add regression detection and reporting
  - Implement baseline comparison with 20% regression threshold
  - Create performance delta calculation and trend analysis
  - Add automated CI pipeline failure on performance regression
  - Generate detailed performance reports with historical comparison
  - _Requirements: 7.4, 7.5_

- [x] 6.4 Write performance benchmark tests
  - Test benchmark execution accuracy and repeatability
  - Validate regression detection logic with simulated performance changes
  - Test baseline management and historical data persistence
  - Verify CI integration and failure reporting mechanisms
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 7. Integration Testing Suite
  - Create end-to-end workflow testing
  - Implement drift scenario simulation
  - Add canary deployment testing
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [x] 7.1 Build end-to-end integration test framework
  - Create complete Planner workflow testing from candidate scoring through curator review
  - Implement test environment setup with isolated databases and configurations
  - Add integration test orchestration with proper setup and teardown
  - Create test data factories for realistic candidate and configuration scenarios
  - _Requirements: 8.1_

- [x] 7.2 Implement drift scenario simulation
  - Create 30-day simulation framework with accelerated time progression
  - Add Business Logic Engine stability testing under parameter drift
  - Implement statistical validation of system behavior over extended periods
  - Test autopilot-style parameter adjustment within safety bounds
  - _Requirements: 8.2_

- [x] 7.3 Add canary deployment testing
  - Implement A/B testing framework with statistical significance validation
  - Create canary traffic splitting and performance comparison
  - Add confidence interval calculation (>95% confidence requirement)
  - Test rollback mechanisms and safety threshold enforcement
  - _Requirements: 8.3_

- [x] 7.4 Create comprehensive test execution
  - Implement test suite orchestration with 12-minute maximum execution time
  - Add parallel test execution for performance optimization
  - Create detailed test reporting with coverage metrics and failure analysis
  - Integrate with CI pipeline for automated execution and reporting
  - _Requirements: 8.4, 8.5_

- [x] 7.5 Write integration test validation
  - Test end-to-end workflow execution and result validation
  - Verify drift scenario simulation accuracy and statistical methods
  - Test canary deployment framework and significance testing
  - Validate test execution performance and timeout handling
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [ ] 8. CLI Integration and Documentation
  - Add vvtvctl commands for observability and testing
  - Create operational documentation and runbooks
  - Implement CI/CD integration scripts
  - _Requirements: All requirements (supporting infrastructure)_

- [ ] 8.1 Extend vvtvctl with observability commands
  - Add `vvtvctl metrics query` for ad-hoc metrics retrieval
  - Implement `vvtvctl dashboard generate` for on-demand dashboard creation
  - Create `vvtvctl alerts status` for alert state monitoring
  - Add `vvtvctl metrics cleanup` for manual retention policy execution
  - _Requirements: 1.5, 2.3, 3.5_

- [ ] 8.2 Add testing framework CLI commands
  - Implement `vvtvctl test golden` for golden selection test execution
  - Create `vvtvctl test stress-llm` for LLM stress testing
  - Add `vvtvctl test benchmark` for performance benchmark execution
  - Implement `vvtvctl test integration` for end-to-end testing
  - _Requirements: 4.4, 5.4, 7.4, 8.4_

- [ ] 8.3 Create operational documentation
  - Write observability runbook with dashboard interpretation and troubleshooting
  - Create testing guide with test execution procedures and result analysis
  - Add performance tuning guide with benchmark interpretation
  - Document alert response procedures and escalation paths
  - _Requirements: All requirements (operational support)_

- [ ] 8.4 Implement CI/CD integration
  - Create GitHub Actions workflow for automated testing
  - Add performance regression detection in CI pipeline
  - Implement test result reporting and artifact collection
  - Create deployment validation with observability checks
  - _Requirements: 4.4, 7.4, 8.4, 8.5_