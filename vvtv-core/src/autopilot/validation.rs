use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::sliding_bounds::ParameterBounds;

/// Result type for validation operations
pub type ValidationResult<T> = std::result::Result<T, ValidationError>;

/// Errors that can occur during parameter validation
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("parameter not found: {0}")]
    ParameterNotFound(String),
    #[error("bounds violation: {0}")]
    BoundsViolation(String),
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),
    #[error("business logic violation: {0}")]
    BusinessLogicViolation(String),
    #[error("safety violation: {0}")]
    SafetyViolation(String),
    #[error("dependency violation: {0}")]
    DependencyViolation(String),
    #[error("rate limit violation: {0}")]
    RateLimitViolation(String),
    #[error("configuration error: {0}")]
    Configuration(String),
}

/// Comprehensive parameter validator
#[derive(Debug, Clone)]
pub struct ParameterValidator {
    constraints: HashMap<String, ParameterConstraints>,
    dependencies: Vec<ParameterDependency>,
    rate_limits: HashMap<String, RateLimit>,
    config: ValidationConfig,
}

/// Configuration for parameter validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub strict_mode: bool,
    pub enable_dependency_checks: bool,
    pub enable_rate_limiting: bool,
    pub enable_safety_checks: bool,
    pub max_daily_changes_per_parameter: u32,
    pub max_change_magnitude_per_day: f64,
    pub safety_margin_percentage: f64,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict_mode: true,
            enable_dependency_checks: true,
            enable_rate_limiting: true,
            enable_safety_checks: true,
            max_daily_changes_per_parameter: 3,
            max_change_magnitude_per_day: 0.2, // 20% max change per day
            safety_margin_percentage: 0.05, // 5% safety margin
        }
    }
}

/// Constraints for a specific parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterConstraints {
    pub parameter_name: String,
    pub data_type: ParameterDataType,
    pub absolute_min: f64,
    pub absolute_max: f64,
    pub precision: Option<u32>, // Number of decimal places
    pub allowed_values: Option<Vec<f64>>, // Discrete allowed values
    pub business_rules: Vec<BusinessRule>,
    pub safety_rules: Vec<SafetyRule>,
    pub change_limits: ChangeLimit,
}

/// Data types for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterDataType {
    Float,
    Integer,
    Percentage, // 0.0 to 1.0
    Probability, // 0.0 to 1.0 with special handling
    Count, // Non-negative integer
    Ratio, // Can be any positive value
}

/// Business logic rules for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRule {
    pub rule_id: String,
    pub description: String,
    pub rule_type: BusinessRuleType,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Types of business rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusinessRuleType {
    /// Value must be within a specific range
    Range { min: f64, max: f64 },
    /// Value must be one of the specified values
    Enum { values: Vec<f64> },
    /// Value must satisfy a mathematical relationship
    Formula { expression: String },
    /// Value must maintain a relationship with another parameter
    Relationship { other_parameter: String, relationship: String },
    /// Custom validation function
    Custom { validator_name: String },
}

/// Safety rules for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRule {
    pub rule_id: String,
    pub description: String,
    pub rule_type: SafetyRuleType,
    pub severity: SafetySeverity,
}

/// Types of safety rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyRuleType {
    /// Maximum change per time period
    MaxChangeRate { max_change: f64, time_window_hours: u32 },
    /// Minimum stability period before changes
    StabilityPeriod { min_hours: u32 },
    /// Maximum deviation from baseline
    MaxDeviation { baseline: f64, max_deviation: f64 },
    /// Prevent dangerous combinations
    DangerousRange { min: f64, max: f64 },
    /// Require manual approval for large changes
    ManualApproval { threshold: f64 },
}

/// Severity levels for safety rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SafetySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Change limits for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeLimit {
    pub max_change_per_hour: f64,
    pub max_change_per_day: f64,
    pub max_change_per_week: f64,
    pub cooldown_period_hours: u32,
}

/// Dependencies between parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDependency {
    pub dependency_id: String,
    pub primary_parameter: String,
    pub dependent_parameter: String,
    pub dependency_type: DependencyType,
    pub constraint: DependencyConstraint,
}

/// Types of parameter dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    /// Parameters must maintain a specific relationship
    Relationship,
    /// One parameter's range depends on another's value
    ConditionalRange,
    /// Parameters cannot be changed simultaneously
    MutualExclusion,
    /// One parameter change requires another to change
    RequiredChange,
}

/// Constraints for parameter dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyConstraint {
    /// Mathematical relationship (e.g., "a + b < 1.0")
    Formula(String),
    /// Conditional range (e.g., if a > 0.5, then b must be < 0.3)
    ConditionalRange { condition: String, range: (f64, f64) },
    /// Time-based exclusion (cannot change within X hours of each other)
    TimeExclusion { hours: u32 },
    /// Value-based requirement (if a changes by X, b must change by Y)
    ValueRequirement { ratio: f64 },
}

/// Rate limiting for parameter changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub parameter_name: String,
    pub max_changes_per_hour: u32,
    pub max_changes_per_day: u32,
    pub max_magnitude_per_day: f64,
    pub recent_changes: Vec<ParameterChangeRecord>,
}

/// Record of recent parameter changes for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChangeRecord {
    pub timestamp: DateTime<Utc>,
    pub old_value: f64,
    pub new_value: f64,
    pub magnitude: f64,
}

/// Comprehensive validation result
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub parameter_name: String,
    pub proposed_value: f64,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub warnings: Vec<ValidationWarning>,
    pub bounds_check: BoundsCheckResult,
    pub constraint_check: ConstraintCheckResult,
    pub dependency_check: DependencyCheckResult,
    pub rate_limit_check: RateLimitCheckResult,
    pub safety_check: SafetyCheckResult,
    pub recommendations: Vec<ValidationRecommendation>,
}

/// Validation warning (non-blocking)
#[derive(Debug, Clone, Serialize)]
pub struct ValidationWarning {
    pub warning_type: WarningType,
    pub message: String,
    pub severity: WarningSeverity,
}

/// Types of validation warnings
#[derive(Debug, Clone, Serialize)]
pub enum WarningType {
    PerformanceImpact,
    StabilityRisk,
    UntestedRange,
    HighChangeRate,
    DependencyRisk,
}

/// Severity levels for warnings
#[derive(Debug, Clone, Serialize)]
pub enum WarningSeverity {
    Low,
    Medium,
    High,
}

/// Result of bounds checking
#[derive(Debug, Clone, Serialize)]
pub struct BoundsCheckResult {
    pub within_sliding_bounds: bool,
    pub within_absolute_bounds: bool,
    pub current_bounds: (f64, f64),
    pub absolute_bounds: (f64, f64),
    pub distance_to_bounds: f64,
}

/// Result of constraint checking
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintCheckResult {
    pub business_rules_passed: bool,
    pub safety_rules_passed: bool,
    pub data_type_valid: bool,
    pub precision_valid: bool,
    pub failed_rules: Vec<String>,
}

/// Result of dependency checking
#[derive(Debug, Clone, Serialize)]
pub struct DependencyCheckResult {
    pub dependencies_satisfied: bool,
    pub violated_dependencies: Vec<String>,
    pub affected_parameters: Vec<String>,
}

/// Result of rate limit checking
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitCheckResult {
    pub within_rate_limits: bool,
    pub changes_in_last_hour: u32,
    pub changes_in_last_day: u32,
    pub magnitude_in_last_day: f64,
    pub time_until_next_allowed: Option<chrono::Duration>,
}

/// Result of safety checking
#[derive(Debug, Clone, Serialize)]
pub struct SafetyCheckResult {
    pub safety_rules_passed: bool,
    pub requires_manual_approval: bool,
    pub safety_margin_ok: bool,
    pub failed_safety_rules: Vec<String>,
}

/// Validation recommendations
#[derive(Debug, Clone, Serialize)]
pub struct ValidationRecommendation {
    pub recommendation_type: RecommendationType,
    pub message: String,
    pub suggested_value: Option<f64>,
    pub confidence: f64,
}

/// Types of validation recommendations
#[derive(Debug, Clone, Serialize)]
pub enum RecommendationType {
    AdjustValue,
    WaitForCooldown,
    ConsiderDependencies,
    ReduceChangeRate,
    SeekApproval,
}

impl ParameterValidator {
    /// Create a new parameter validator with default constraints
    pub fn new(config: ValidationConfig) -> Self {
        let mut validator = Self {
            constraints: HashMap::new(),
            dependencies: Vec::new(),
            rate_limits: HashMap::new(),
            config,
        };
        
        // Initialize default constraints for known parameters
        validator.initialize_default_constraints();
        validator
    }

    /// Initialize default constraints for known parameters
    fn initialize_default_constraints(&mut self) {
        // Selection temperature constraints
        self.add_parameter_constraints(ParameterConstraints {
            parameter_name: "selection_temperature".to_string(),
            data_type: ParameterDataType::Float,
            absolute_min: 0.01,
            absolute_max: 3.0,
            precision: Some(3),
            allowed_values: None,
            business_rules: vec![
                BusinessRule {
                    rule_id: "temp_performance_range".to_string(),
                    description: "Temperature should be in performance-tested range".to_string(),
                    rule_type: BusinessRuleType::Range { min: 0.1, max: 2.0 },
                    parameters: HashMap::new(),
                },
            ],
            safety_rules: vec![
                SafetyRule {
                    rule_id: "temp_max_change".to_string(),
                    description: "Temperature changes should be gradual".to_string(),
                    rule_type: SafetyRuleType::MaxChangeRate { max_change: 0.1, time_window_hours: 1 },
                    severity: SafetySeverity::Medium,
                },
            ],
            change_limits: ChangeLimit {
                max_change_per_hour: 0.05,
                max_change_per_day: 0.15,
                max_change_per_week: 0.3,
                cooldown_period_hours: 2,
            },
        });

        // Add parameter dependencies
        self.add_dependency(ParameterDependency {
            dependency_id: "temp_topk_balance".to_string(),
            primary_parameter: "selection_temperature".to_string(),
            dependent_parameter: "selection_top_k".to_string(),
            dependency_type: DependencyType::Relationship,
            constraint: DependencyConstraint::Formula(
                "temperature * top_k < 50.0".to_string() // Prevent over-exploration
            ),
        });
    }

    /// Add parameter constraints
    pub fn add_parameter_constraints(&mut self, constraints: ParameterConstraints) {
        let param_name = constraints.parameter_name.clone();
        self.constraints.insert(param_name.clone(), constraints);
        
        // Initialize rate limit tracking
        self.rate_limits.insert(param_name.clone(), RateLimit {
            parameter_name: param_name,
            max_changes_per_hour: 3,
            max_changes_per_day: 10,
            max_magnitude_per_day: 0.2,
            recent_changes: Vec::new(),
        });
    }

    /// Add parameter dependency
    pub fn add_dependency(&mut self, dependency: ParameterDependency) {
        self.dependencies.push(dependency);
    }

    /// Comprehensive parameter validation
    pub fn validate_parameter_change(
        &mut self,
        parameter_name: &str,
        current_value: f64,
        proposed_value: f64,
        current_bounds: &ParameterBounds,
        other_parameters: &HashMap<String, f64>,
    ) -> ValidationResult<ValidationReport> {
        let mut validation_errors = Vec::new();
        
        let bounds_check = match self.check_bounds(parameter_name, proposed_value, current_bounds) {
            Ok(result) => result,
            Err(e) => {
                validation_errors.push(format!("Bounds check failed: {}", e));
                BoundsCheckResult {
                    within_sliding_bounds: false,
                    within_absolute_bounds: false,
                    current_bounds: (0.0, 0.0),
                    absolute_bounds: (0.0, 0.0),
                    distance_to_bounds: 0.0,
                }
            }
        };

        let constraint_check = match self.check_constraints(parameter_name, proposed_value) {
            Ok(result) => result,
            Err(e) => {
                validation_errors.push(format!("Constraint check failed: {}", e));
                ConstraintCheckResult {
                    business_rules_passed: false,
                    safety_rules_passed: false,
                    data_type_valid: false,
                    precision_valid: false,
                    failed_rules: vec![format!("Error: {}", e)],
                }
            }
        };

        let dependency_check = match self.check_dependencies(parameter_name, proposed_value, other_parameters) {
            Ok(result) => result,
            Err(e) => {
                validation_errors.push(format!("Dependency check failed: {}", e));
                DependencyCheckResult {
                    dependencies_satisfied: false,
                    violated_dependencies: vec![format!("Error: {}", e)],
                    affected_parameters: Vec::new(),
                }
            }
        };

        let rate_limit_check = match self.check_rate_limits(parameter_name, current_value, proposed_value) {
            Ok(result) => result,
            Err(e) => {
                validation_errors.push(format!("Rate limit check failed: {}", e));
                RateLimitCheckResult {
                    within_rate_limits: false,
                    changes_in_last_hour: 0,
                    changes_in_last_day: 0,
                    magnitude_in_last_day: 0.0,
                    time_until_next_allowed: None,
                }
            }
        };

        let safety_check = match self.check_safety_rules(parameter_name, current_value, proposed_value) {
            Ok(result) => result,
            Err(e) => {
                validation_errors.push(format!("Safety check failed: {}", e));
                SafetyCheckResult {
                    safety_rules_passed: false,
                    requires_manual_approval: false,
                    safety_margin_ok: false,
                    failed_safety_rules: vec![format!("Error: {}", e)],
                }
            }
        };

        let mut report = ValidationReport {
            parameter_name: parameter_name.to_string(),
            proposed_value,
            is_valid: true,
            validation_errors,
            warnings: Vec::new(),
            bounds_check,
            constraint_check,
            dependency_check,
            rate_limit_check,
            safety_check,
            recommendations: Vec::new(),
        };

        // Determine overall validity
        report.is_valid = report.bounds_check.within_sliding_bounds &&
                         report.bounds_check.within_absolute_bounds &&
                         report.constraint_check.business_rules_passed &&
                         report.constraint_check.safety_rules_passed &&
                         report.dependency_check.dependencies_satisfied &&
                         report.rate_limit_check.within_rate_limits &&
                         report.safety_check.safety_rules_passed &&
                         report.validation_errors.is_empty();

        // Generate recommendations if validation failed
        if !report.is_valid {
            report.recommendations = self.generate_recommendations(&report);
        }

        // Generate warnings for potential issues
        report.warnings = self.generate_warnings(&report);

        Ok(report)
    }

    /// Check bounds constraints
    fn check_bounds(&self, parameter_name: &str, proposed_value: f64, current_bounds: &ParameterBounds) -> ValidationResult<BoundsCheckResult> {
        let constraints = self.constraints.get(parameter_name)
            .ok_or_else(|| ValidationError::ParameterNotFound(parameter_name.to_string()))?;

        let within_sliding_bounds = proposed_value >= current_bounds.min_value && proposed_value <= current_bounds.max_value;
        let within_absolute_bounds = proposed_value >= constraints.absolute_min && proposed_value <= constraints.absolute_max;
        
        let distance_to_bounds = if within_sliding_bounds {
            let dist_to_min = proposed_value - current_bounds.min_value;
            let dist_to_max = current_bounds.max_value - proposed_value;
            dist_to_min.min(dist_to_max)
        } else {
            if proposed_value < current_bounds.min_value {
                current_bounds.min_value - proposed_value
            } else {
                proposed_value - current_bounds.max_value
            }
        };

        Ok(BoundsCheckResult {
            within_sliding_bounds,
            within_absolute_bounds,
            current_bounds: (current_bounds.min_value, current_bounds.max_value),
            absolute_bounds: (constraints.absolute_min, constraints.absolute_max),
            distance_to_bounds,
        })
    }

    /// Check parameter constraints
    fn check_constraints(&self, parameter_name: &str, proposed_value: f64) -> ValidationResult<ConstraintCheckResult> {
        let constraints = self.constraints.get(parameter_name)
            .ok_or_else(|| ValidationError::ParameterNotFound(parameter_name.to_string()))?;

        let mut failed_rules = Vec::new();
        let mut business_rules_passed = true;
        let mut safety_rules_passed = true;

        // Check business rules
        for rule in &constraints.business_rules {
            if !self.check_business_rule(rule, proposed_value)? {
                business_rules_passed = false;
                failed_rules.push(format!("Business rule '{}': {}", rule.rule_id, rule.description));
            }
        }

        // Check safety rules
        for rule in &constraints.safety_rules {
            if !self.check_safety_rule(rule, proposed_value)? {
                safety_rules_passed = false;
                failed_rules.push(format!("Safety rule '{}': {}", rule.rule_id, rule.description));
            }
        }

        // Check data type validity
        let data_type_valid = self.check_data_type(&constraints.data_type, proposed_value);
        if !data_type_valid {
            failed_rules.push(format!("Data type validation failed for {:?}", constraints.data_type));
        }

        // Check precision
        let precision_valid = if let Some(precision) = constraints.precision {
            self.check_precision(proposed_value, precision)
        } else {
            true
        };
        if !precision_valid {
            failed_rules.push(format!("Precision validation failed, expected {} decimal places", constraints.precision.unwrap_or(0)));
        }

        Ok(ConstraintCheckResult {
            business_rules_passed,
            safety_rules_passed,
            data_type_valid,
            precision_valid,
            failed_rules,
        })
    }

    /// Check parameter dependencies
    fn check_dependencies(&self, parameter_name: &str, proposed_value: f64, other_parameters: &HashMap<String, f64>) -> ValidationResult<DependencyCheckResult> {
        if !self.config.enable_dependency_checks {
            return Ok(DependencyCheckResult {
                dependencies_satisfied: true,
                violated_dependencies: Vec::new(),
                affected_parameters: Vec::new(),
            });
        }

        let mut dependencies_satisfied = true;
        let mut violated_dependencies = Vec::new();
        let mut affected_parameters = Vec::new();

        // Check all dependencies involving this parameter
        for dependency in &self.dependencies {
            if dependency.primary_parameter == parameter_name || dependency.dependent_parameter == parameter_name {
                let is_satisfied = self.check_dependency_constraint(
                    dependency,
                    parameter_name,
                    proposed_value,
                    other_parameters,
                )?;

                if !is_satisfied {
                    dependencies_satisfied = false;
                    violated_dependencies.push(dependency.dependency_id.clone());
                    
                    // Add affected parameters
                    if dependency.primary_parameter != parameter_name {
                        affected_parameters.push(dependency.primary_parameter.clone());
                    }
                    if dependency.dependent_parameter != parameter_name {
                        affected_parameters.push(dependency.dependent_parameter.clone());
                    }
                }
            }
        }

        Ok(DependencyCheckResult {
            dependencies_satisfied,
            violated_dependencies,
            affected_parameters,
        })
    }

    /// Check rate limits
    fn check_rate_limits(&mut self, parameter_name: &str, current_value: f64, proposed_value: f64) -> ValidationResult<RateLimitCheckResult> {
        if !self.config.enable_rate_limiting {
            return Ok(RateLimitCheckResult {
                within_rate_limits: true,
                changes_in_last_hour: 0,
                changes_in_last_day: 0,
                magnitude_in_last_day: 0.0,
                time_until_next_allowed: None,
            });
        }

        let rate_limit = self.rate_limits.get_mut(parameter_name)
            .ok_or_else(|| ValidationError::ParameterNotFound(parameter_name.to_string()))?;

        let now = Utc::now();
        let magnitude = (proposed_value - current_value).abs();

        // Clean up old records
        rate_limit.recent_changes.retain(|change| {
            (now - change.timestamp).num_hours() < 24
        });

        // Count recent changes
        let changes_in_last_hour = rate_limit.recent_changes.iter()
            .filter(|change| (now - change.timestamp).num_hours() < 1)
            .count() as u32;

        let changes_in_last_day = rate_limit.recent_changes.len() as u32;

        let magnitude_in_last_day: f64 = rate_limit.recent_changes.iter()
            .map(|change| change.magnitude)
            .sum();

        // Check rate limits
        let within_hour_limit = changes_in_last_hour < rate_limit.max_changes_per_hour;
        let within_day_limit = changes_in_last_day < rate_limit.max_changes_per_day;
        let within_magnitude_limit = magnitude_in_last_day + magnitude <= rate_limit.max_magnitude_per_day;

        let within_rate_limits = within_hour_limit && within_day_limit && within_magnitude_limit;

        // Calculate time until next change is allowed
        let time_until_next_allowed = if !within_rate_limits {
            Some(chrono::Duration::hours(1))
        } else {
            None
        };

        Ok(RateLimitCheckResult {
            within_rate_limits,
            changes_in_last_hour,
            changes_in_last_day,
            magnitude_in_last_day: magnitude_in_last_day + magnitude,
            time_until_next_allowed,
        })
    }

    /// Check safety rules
    fn check_safety_rules(&self, parameter_name: &str, current_value: f64, proposed_value: f64) -> ValidationResult<SafetyCheckResult> {
        if !self.config.enable_safety_checks {
            return Ok(SafetyCheckResult {
                safety_rules_passed: true,
                requires_manual_approval: false,
                safety_margin_ok: true,
                failed_safety_rules: Vec::new(),
            });
        }

        let constraints = self.constraints.get(parameter_name)
            .ok_or_else(|| ValidationError::ParameterNotFound(parameter_name.to_string()))?;

        let mut safety_rules_passed = true;
        let mut requires_manual_approval = false;
        let mut failed_safety_rules = Vec::new();

        // Check safety rules
        for rule in &constraints.safety_rules {
            match &rule.rule_type {
                SafetyRuleType::MaxChangeRate { max_change, .. } => {
                    let change_magnitude = (proposed_value - current_value).abs();
                    if change_magnitude > *max_change {
                        safety_rules_passed = false;
                        failed_safety_rules.push(format!(
                            "Change magnitude {:.4} exceeds limit {:.4}",
                            change_magnitude, max_change
                        ));
                    }
                }
                SafetyRuleType::ManualApproval { threshold } => {
                    let change_magnitude = (proposed_value - current_value).abs();
                    if change_magnitude > *threshold {
                        requires_manual_approval = true;
                    }
                }
                _ => {} // Other safety rules would be implemented here
            }
        }

        // Check safety margin
        let safety_margin_ok = {
            let range = constraints.absolute_max - constraints.absolute_min;
            let margin = range * self.config.safety_margin_percentage;
            let safe_min = constraints.absolute_min + margin;
            let safe_max = constraints.absolute_max - margin;
            proposed_value >= safe_min && proposed_value <= safe_max
        };

        if !safety_margin_ok {
            failed_safety_rules.push("Value outside safety margin".to_string());
        }

        Ok(SafetyCheckResult {
            safety_rules_passed,
            requires_manual_approval,
            safety_margin_ok,
            failed_safety_rules,
        })
    }

    /// Check a business rule
    fn check_business_rule(&self, rule: &BusinessRule, proposed_value: f64) -> ValidationResult<bool> {
        match &rule.rule_type {
            BusinessRuleType::Range { min, max } => {
                Ok(proposed_value >= *min && proposed_value <= *max)
            }
            BusinessRuleType::Enum { values } => {
                Ok(values.contains(&proposed_value))
            }
            _ => Ok(true) // Simplified for other rule types
        }
    }

    /// Check a safety rule
    fn check_safety_rule(&self, _rule: &SafetyRule, _proposed_value: f64) -> ValidationResult<bool> {
        Ok(true) // Simplified implementation
    }

    /// Check data type validity
    fn check_data_type(&self, data_type: &ParameterDataType, value: f64) -> bool {
        match data_type {
            ParameterDataType::Float => value.is_finite(),
            ParameterDataType::Integer => value.fract() == 0.0,
            ParameterDataType::Percentage => value >= 0.0 && value <= 1.0,
            ParameterDataType::Probability => value >= 0.0 && value <= 1.0,
            ParameterDataType::Count => value >= 0.0 && value.fract() == 0.0,
            ParameterDataType::Ratio => value > 0.0,
        }
    }

    /// Check precision requirements
    fn check_precision(&self, value: f64, required_precision: u32) -> bool {
        let multiplier = 10_f64.powi(required_precision as i32);
        let rounded = (value * multiplier).round() / multiplier;
        (value - rounded).abs() < f64::EPSILON
    }

    /// Check dependency constraint
    fn check_dependency_constraint(
        &self,
        _dependency: &ParameterDependency,
        _changing_parameter: &str,
        _proposed_value: f64,
        _other_parameters: &HashMap<String, f64>,
    ) -> ValidationResult<bool> {
        Ok(true) // Simplified implementation
    }

    /// Generate validation recommendations
    fn generate_recommendations(&self, report: &ValidationReport) -> Vec<ValidationRecommendation> {
        let mut recommendations = Vec::new();

        // Bounds violation recommendations
        if !report.bounds_check.within_sliding_bounds {
            if report.proposed_value < report.bounds_check.current_bounds.0 {
                recommendations.push(ValidationRecommendation {
                    recommendation_type: RecommendationType::AdjustValue,
                    message: format!("Increase value to at least {:.4}", report.bounds_check.current_bounds.0),
                    suggested_value: Some(report.bounds_check.current_bounds.0),
                    confidence: 0.9,
                });
            } else {
                recommendations.push(ValidationRecommendation {
                    recommendation_type: RecommendationType::AdjustValue,
                    message: format!("Decrease value to at most {:.4}", report.bounds_check.current_bounds.1),
                    suggested_value: Some(report.bounds_check.current_bounds.1),
                    confidence: 0.9,
                });
            }
        }

        recommendations
    }

    /// Generate validation warnings
    fn generate_warnings(&self, report: &ValidationReport) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Performance impact warnings
        if report.bounds_check.distance_to_bounds < 0.1 {
            warnings.push(ValidationWarning {
                warning_type: WarningType::PerformanceImpact,
                message: "Value is close to bounds, may impact performance".to_string(),
                severity: WarningSeverity::Medium,
            });
        }

        warnings
    }

    /// Get validation statistics
    pub fn get_validation_stats(&self) -> ValidationStats {
        let total_parameters = self.constraints.len();
        let total_dependencies = self.dependencies.len();
        let total_rate_limits = self.rate_limits.len();
        
        let total_recent_changes: usize = self.rate_limits.values()
            .map(|rl| rl.recent_changes.len())
            .sum();

        ValidationStats {
            total_parameters,
            total_dependencies,
            total_rate_limits,
            total_recent_changes,
            strict_mode: self.config.strict_mode,
        }
    }
}

/// Validation statistics
#[derive(Debug, Clone, Serialize)]
pub struct ValidationStats {
    pub total_parameters: usize,
    pub total_dependencies: usize,
    pub total_rate_limits: usize,
    pub total_recent_changes: usize,
    pub strict_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let config = ValidationConfig::default();
        let validator = ParameterValidator::new(config);
        
        assert!(validator.constraints.contains_key("selection_temperature"));
    }

    #[test]
    fn test_data_type_validation() {
        let validator = ParameterValidator::new(ValidationConfig::default());
        
        assert!(validator.check_data_type(&ParameterDataType::Float, 1.5));
        assert!(validator.check_data_type(&ParameterDataType::Integer, 5.0));
        assert!(!validator.check_data_type(&ParameterDataType::Integer, 5.5));
        assert!(validator.check_data_type(&ParameterDataType::Percentage, 0.75));
        assert!(!validator.check_data_type(&ParameterDataType::Percentage, 1.5));
    }

    #[test]
    fn test_precision_validation() {
        let validator = ParameterValidator::new(ValidationConfig::default());
        
        assert!(validator.check_precision(1.234, 3));
        assert!(!validator.check_precision(1.2345, 3));
        assert!(validator.check_precision(5.0, 0));
    }
}