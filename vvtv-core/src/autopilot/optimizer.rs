use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::engine::{
    AutopilotResult, ExpectedImpact, MetricsAnalysis, OptimizationOpportunity,
    ParameterChange,
};
use super::sliding_bounds::SlidingBounds;
use crate::business_logic::BusinessLogic;

/// Parameter optimizer that analyzes metrics and proposes parameter adjustments
#[derive(Debug)]
pub struct ParameterOptimizer {
    config: OptimizerConfig,
    current_parameters: HashMap<String, f64>,
    optimization_history: Vec<OptimizationRecord>,
}

/// Configuration for the parameter optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub max_changes_per_cycle: usize,
    pub min_confidence_threshold: f64,
    pub conservative_mode: bool,
    pub learning_rate: f64,
    pub momentum_factor: f64,
    pub exploration_factor: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            max_changes_per_cycle: 3,
            min_confidence_threshold: 0.6,
            conservative_mode: true,
            learning_rate: 0.1,
            momentum_factor: 0.2,
            exploration_factor: 0.05,
        }
    }
}

/// Record of optimization attempts for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecord {
    pub timestamp: DateTime<Utc>,
    pub parameter_name: String,
    pub old_value: f64,
    pub new_value: f64,
    pub predicted_impact: f64,
    pub actual_impact: Option<f64>,
    pub success: Option<bool>,
    pub rationale: String,
}

/// Optimization algorithm types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    GradientDescent,
    AdaptiveLearning,
    BayesianOptimization,
    ConservativeAdjustment,
}

/// Parameter change validation result
#[derive(Debug, Clone, Serialize)]
pub struct ChangeValidation {
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub bounds_check: bool,
    pub business_logic_check: bool,
    pub safety_check: bool,
    pub adjusted_value: Option<f64>,
}

impl ParameterOptimizer {
    /// Create a new parameter optimizer
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            config,
            current_parameters: HashMap::new(),
            optimization_history: Vec::new(),
        }
    }

    /// Load current parameter values from business logic
    pub fn load_current_parameters(&mut self, _business_logic: &BusinessLogic) -> AutopilotResult<()> {
        // Extract current parameter values from business logic
        // This would read from the actual business logic configuration
        
        // For now, use placeholder values that would be read from the actual config
        self.current_parameters.insert("selection_temperature".to_string(), 0.85);
        self.current_parameters.insert("selection_top_k".to_string(), 12.0);
        self.current_parameters.insert("plan_selection_bias".to_string(), 0.0);
        self.current_parameters.insert("curator_confidence_threshold".to_string(), 0.62);
        
        tracing::debug!(
            target: "autopilot_optimizer",
            parameters = ?self.current_parameters,
            "loaded current parameter values"
        );
        
        Ok(())
    }

    /// Analyze metrics and propose parameter changes
    pub fn propose_parameter_changes(
        &mut self,
        analysis: &MetricsAnalysis,
        bounds: &SlidingBounds,
    ) -> AutopilotResult<Vec<ParameterChange>> {
        tracing::info!(
            target: "autopilot_optimizer",
            confidence_score = analysis.confidence_score,
            opportunities_count = analysis.optimization_opportunities.len(),
            "starting parameter optimization"
        );

        // Filter opportunities by confidence threshold
        let viable_opportunities: Vec<&OptimizationOpportunity> = analysis
            .optimization_opportunities
            .iter()
            .filter(|op| op.confidence >= self.config.min_confidence_threshold)
            .take(self.config.max_changes_per_cycle)
            .collect();

        if viable_opportunities.is_empty() {
            tracing::info!(
                target: "autopilot_optimizer",
                min_confidence = self.config.min_confidence_threshold,
                "no viable optimization opportunities found"
            );
            return Ok(Vec::new());
        }

        let mut proposed_changes = Vec::new();

        for opportunity in viable_opportunities {
            match self.create_parameter_change(opportunity, analysis, bounds) {
                Ok(Some(change)) => {
                    tracing::debug!(
                        target: "autopilot_optimizer",
                        parameter = %change.parameter_name,
                        old_value = ?change.old_value,
                        new_value = ?change.new_value,
                        confidence = change.confidence,
                        "proposed parameter change"
                    );
                    proposed_changes.push(change);
                }
                Ok(None) => {
                    tracing::debug!(
                        target: "autopilot_optimizer",
                        parameter = %opportunity.parameter_name,
                        "opportunity filtered out during validation"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target: "autopilot_optimizer",
                        parameter = %opportunity.parameter_name,
                        error = %e,
                        "failed to create parameter change"
                    );
                }
            }
        }

        tracing::info!(
            target: "autopilot_optimizer",
            proposed_changes = proposed_changes.len(),
            "completed parameter optimization"
        );

        Ok(proposed_changes)
    }

    /// Create a parameter change from an optimization opportunity
    fn create_parameter_change(
        &self,
        opportunity: &OptimizationOpportunity,
        analysis: &MetricsAnalysis,
        bounds: &SlidingBounds,
    ) -> AutopilotResult<Option<ParameterChange>> {
        let current_value = self.current_parameters
            .get(&opportunity.parameter_name)
            .copied()
            .unwrap_or(opportunity.current_value);

        // Apply optimization algorithm to refine the suggested value
        let optimized_value = self.apply_optimization_algorithm(
            &opportunity.parameter_name,
            current_value,
            opportunity.suggested_value,
            opportunity.confidence,
            analysis,
        )?;

        // Validate the proposed change
        let validation = self.validate_parameter_change(
            &opportunity.parameter_name,
            current_value,
            optimized_value,
            bounds,
        )?;

        if !validation.is_valid {
            tracing::warn!(
                target: "autopilot_optimizer",
                parameter = %opportunity.parameter_name,
                errors = ?validation.validation_errors,
                "parameter change validation failed"
            );
            return Ok(None);
        }

        let final_value = validation.adjusted_value.unwrap_or(optimized_value);

        // Calculate expected impact with enhanced modeling
        let expected_impact = self.calculate_expected_impact(
            &opportunity.parameter_name,
            current_value,
            final_value,
            analysis,
        )?;

        // Determine change type based on the optimization context
        let change_type = self.determine_change_type(opportunity, analysis);

        // Create enhanced rationale
        let rationale = self.create_change_rationale(
            opportunity,
            current_value,
            final_value,
            &expected_impact,
            analysis,
        );

        let parameter_change = ParameterChange {
            parameter_name: opportunity.parameter_name.clone(),
            old_value: serde_json::Value::Number(
                serde_json::Number::from_f64(current_value).unwrap()
            ),
            new_value: serde_json::Value::Number(
                serde_json::Number::from_f64(final_value).unwrap()
            ),
            change_type,
            confidence: opportunity.confidence,
            expected_impact,
            rationale,
        };

        Ok(Some(parameter_change))
    }

    /// Apply optimization algorithm to refine parameter values
    fn apply_optimization_algorithm(
        &self,
        parameter_name: &str,
        current_value: f64,
        suggested_value: f64,
        confidence: f64,
        analysis: &MetricsAnalysis,
    ) -> AutopilotResult<f64> {
        let algorithm = self.select_optimization_algorithm(parameter_name, confidence, analysis);
        
        match algorithm {
            OptimizationAlgorithm::ConservativeAdjustment => {
                // Conservative approach: smaller steps with high confidence
                let max_change_pct = if self.config.conservative_mode { 0.05 } else { 0.10 };
                let change_direction = (suggested_value - current_value).signum();
                let max_change = current_value * max_change_pct * change_direction;
                let conservative_change = max_change * confidence;
                
                Ok(current_value + conservative_change)
            }
            OptimizationAlgorithm::GradientDescent => {
                // Gradient descent with learning rate
                let gradient = suggested_value - current_value;
                let step = gradient * self.config.learning_rate * confidence;
                
                Ok(current_value + step)
            }
            OptimizationAlgorithm::AdaptiveLearning => {
                // Adaptive learning based on historical performance
                let historical_success_rate = self.calculate_historical_success_rate(parameter_name);
                let adaptive_factor = historical_success_rate * self.config.momentum_factor;
                let change = (suggested_value - current_value) * adaptive_factor * confidence;
                
                Ok(current_value + change)
            }
            OptimizationAlgorithm::BayesianOptimization => {
                // Bayesian optimization with exploration/exploitation balance
                let exploration_noise = self.config.exploration_factor * 
                    (1.0 - confidence) * 
                    (rand::random::<f64>() - 0.5) * 2.0;
                
                let bayesian_value = suggested_value + exploration_noise;
                Ok(bayesian_value)
            }
        }
    }

    /// Select appropriate optimization algorithm
    fn select_optimization_algorithm(
        &self,
        parameter_name: &str,
        confidence: f64,
        analysis: &MetricsAnalysis,
    ) -> OptimizationAlgorithm {
        // Select algorithm based on parameter type, confidence, and data quality
        if self.config.conservative_mode || confidence < 0.7 {
            OptimizationAlgorithm::ConservativeAdjustment
        } else if analysis.data_quality.has_sufficient_data && confidence > 0.8 {
            match parameter_name {
                "selection_temperature" => OptimizationAlgorithm::BayesianOptimization,
                "selection_top_k" => OptimizationAlgorithm::GradientDescent,
                "curator_confidence_threshold" => OptimizationAlgorithm::AdaptiveLearning,
                _ => OptimizationAlgorithm::ConservativeAdjustment,
            }
        } else {
            OptimizationAlgorithm::GradientDescent
        }
    }

    /// Validate a proposed parameter change
    fn validate_parameter_change(
        &self,
        parameter_name: &str,
        current_value: f64,
        proposed_value: f64,
        bounds: &SlidingBounds,
    ) -> AutopilotResult<ChangeValidation> {
        let mut validation = ChangeValidation {
            is_valid: true,
            validation_errors: Vec::new(),
            bounds_check: false,
            business_logic_check: false,
            safety_check: false,
            adjusted_value: None,
        };

        // 1. Bounds validation
        match bounds.validate_change(parameter_name, proposed_value) {
            Ok(_) => {
                validation.bounds_check = true;
            }
            Err(e) => {
                validation.validation_errors.push(format!("Bounds violation: {}", e));
                validation.is_valid = false;
                
                // Try to adjust value to fit within bounds
                if let Some(param_bounds) = bounds.get_bounds(parameter_name) {
                    let adjusted = proposed_value.clamp(param_bounds.min_value, param_bounds.max_value);
                    validation.adjusted_value = Some(adjusted);
                    validation.bounds_check = true;
                    validation.is_valid = true;
                    
                    tracing::debug!(
                        target: "autopilot_optimizer",
                        parameter = parameter_name,
                        original = proposed_value,
                        adjusted = adjusted,
                        "adjusted parameter value to fit bounds"
                    );
                }
            }
        }

        // 2. Business logic constraints validation
        validation.business_logic_check = self.validate_business_logic_constraints(
            parameter_name,
            proposed_value,
            &mut validation.validation_errors,
        );

        // 3. Safety checks
        validation.safety_check = self.validate_safety_constraints(
            parameter_name,
            current_value,
            proposed_value,
            &mut validation.validation_errors,
        );

        validation.is_valid = validation.bounds_check && 
                            validation.business_logic_check && 
                            validation.safety_check;

        Ok(validation)
    }

    /// Validate business logic constraints
    fn validate_business_logic_constraints(
        &self,
        parameter_name: &str,
        value: f64,
        errors: &mut Vec<String>,
    ) -> bool {
        match parameter_name {
            "selection_temperature" => {
                if value <= 0.0 {
                    errors.push("Temperature must be positive".to_string());
                    false
                } else if value > 3.0 {
                    errors.push("Temperature too high (>3.0)".to_string());
                    false
                } else {
                    true
                }
            }
            "selection_top_k" => {
                if value < 1.0 {
                    errors.push("Top-k must be at least 1".to_string());
                    false
                } else if value > 100.0 {
                    errors.push("Top-k too high (>100)".to_string());
                    false
                } else if value.fract() != 0.0 {
                    errors.push("Top-k must be an integer".to_string());
                    false
                } else {
                    true
                }
            }
            "curator_confidence_threshold" => {
                if value < 0.0 || value > 1.0 {
                    errors.push("Confidence threshold must be between 0 and 1".to_string());
                    false
                } else {
                    true
                }
            }
            "plan_selection_bias" => {
                if value.abs() > 0.5 {
                    errors.push("Selection bias magnitude too high (>0.5)".to_string());
                    false
                } else {
                    true
                }
            }
            _ => {
                errors.push(format!("Unknown parameter: {}", parameter_name));
                false
            }
        }
    }

    /// Validate safety constraints
    fn validate_safety_constraints(
        &self,
        parameter_name: &str,
        current_value: f64,
        proposed_value: f64,
        errors: &mut Vec<String>,
    ) -> bool {
        let change_pct = if current_value != 0.0 {
            ((proposed_value - current_value) / current_value).abs()
        } else {
            proposed_value.abs()
        };

        // Maximum allowed change per cycle (safety limit)
        let max_change_pct = match parameter_name {
            "selection_temperature" => 0.15, // 15% max change
            "selection_top_k" => 0.25, // 25% max change
            "curator_confidence_threshold" => 0.10, // 10% max change
            "plan_selection_bias" => 1.0, // Allow larger relative changes for small values
            _ => 0.20, // 20% default
        };

        if change_pct > max_change_pct {
            errors.push(format!(
                "Change too large: {:.1}% exceeds {:.1}% limit",
                change_pct * 100.0,
                max_change_pct * 100.0
            ));
            false
        } else {
            true
        }
    }

    /// Calculate expected impact of parameter change
    fn calculate_expected_impact(
        &self,
        parameter_name: &str,
        current_value: f64,
        new_value: f64,
        analysis: &MetricsAnalysis,
    ) -> AutopilotResult<ExpectedImpact> {
        let change_magnitude = (new_value - current_value).abs() / current_value.max(0.1);
        
        // Base impact estimation based on parameter type and historical data
        let (entropy_delta, budget_delta, novelty_delta) = match parameter_name {
            "selection_temperature" => {
                let temp_change = new_value - current_value;
                let entropy_impact = temp_change * 0.3; // Temperature strongly affects entropy
                let budget_impact = -temp_change * 0.1; // Higher temp may reduce curation
                let novelty_impact = temp_change * 0.2; // Temperature affects novelty
                (Some(entropy_impact), Some(budget_impact), Some(novelty_impact))
            }
            "selection_top_k" => {
                let k_change = new_value - current_value;
                let entropy_impact = k_change * 0.02; // More candidates = more entropy
                let budget_impact = k_change * 0.01; // More candidates = more curation
                (Some(entropy_impact), Some(budget_impact), None)
            }
            "curator_confidence_threshold" => {
                let threshold_change = new_value - current_value;
                let budget_impact = -threshold_change * 2.0; // Higher threshold = less curation
                let entropy_impact = threshold_change * 0.1; // Affects selection diversity
                (Some(entropy_impact), Some(budget_impact), None)
            }
            "plan_selection_bias" => {
                let bias_change = new_value - current_value;
                let novelty_impact = bias_change * 1.0; // Bias directly affects novelty
                let entropy_impact = bias_change.abs() * 0.1; // Bias affects entropy
                (Some(entropy_impact), None, Some(novelty_impact))
            }
            _ => (None, None, None),
        };

        // Adjust impact based on current system state and confidence
        let confidence_factor = analysis.confidence_score;
        let data_quality_factor = if analysis.data_quality.has_sufficient_data { 1.0 } else { 0.5 };
        let adjustment_factor = confidence_factor * data_quality_factor;

        Ok(ExpectedImpact {
            selection_entropy_delta: entropy_delta.map(|d| d * adjustment_factor),
            curator_budget_delta: budget_delta.map(|d| d * adjustment_factor),
            novelty_kld_delta: novelty_delta.map(|d| d * adjustment_factor),
            overall_confidence: confidence_factor * change_magnitude.min(1.0),
        })
    }

    /// Determine the type of change being made
    fn determine_change_type(
        &self,
        opportunity: &OptimizationOpportunity,
        analysis: &MetricsAnalysis,
    ) -> super::engine::ChangeType {
        // Analyze the context to determine change type
        if opportunity.confidence > 0.8 && analysis.confidence_score > 0.7 {
            super::engine::ChangeType::Optimization
        } else if analysis.data_quality.has_sufficient_data {
            super::engine::ChangeType::Correction
        } else {
            super::engine::ChangeType::Exploration
        }
    }

    /// Create detailed rationale for the parameter change
    fn create_change_rationale(
        &self,
        opportunity: &OptimizationOpportunity,
        current_value: f64,
        new_value: f64,
        expected_impact: &ExpectedImpact,
        analysis: &MetricsAnalysis,
    ) -> String {
        let change_pct = if current_value != 0.0 {
            ((new_value - current_value) / current_value) * 100.0
        } else {
            new_value * 100.0
        };

        let direction = if change_pct > 0.0 { "increase" } else { "decrease" };
        
        let mut rationale = format!(
            "Proposing {:.1}% {} in {} (from {:.3} to {:.3}). ",
            change_pct.abs(),
            direction,
            opportunity.parameter_name,
            current_value,
            new_value
        );

        rationale.push_str(&opportunity.rationale);

        if let Some(entropy_delta) = expected_impact.selection_entropy_delta {
            rationale.push_str(&format!(
                " Expected entropy change: {:.3}.",
                entropy_delta
            ));
        }

        if let Some(budget_delta) = expected_impact.curator_budget_delta {
            rationale.push_str(&format!(
                " Expected budget impact: {:.1}%.",
                budget_delta * 100.0
            ));
        }

        rationale.push_str(&format!(
            " Analysis confidence: {:.1}%, data quality: {}.",
            analysis.confidence_score * 100.0,
            if analysis.data_quality.has_sufficient_data { "sufficient" } else { "limited" }
        ));

        rationale
    }

    /// Calculate historical success rate for a parameter
    fn calculate_historical_success_rate(&self, parameter_name: &str) -> f64 {
        let parameter_history: Vec<&OptimizationRecord> = self
            .optimization_history
            .iter()
            .filter(|record| record.parameter_name == parameter_name)
            .collect();

        if parameter_history.is_empty() {
            return 0.5; // Neutral success rate for new parameters
        }

        let successful_changes = parameter_history
            .iter()
            .filter(|record| record.success.unwrap_or(false))
            .count();

        successful_changes as f64 / parameter_history.len() as f64
    }

    /// Record optimization attempt for learning
    pub fn record_optimization_attempt(
        &mut self,
        parameter_name: String,
        old_value: f64,
        new_value: f64,
        predicted_impact: f64,
        rationale: String,
    ) {
        let record = OptimizationRecord {
            timestamp: Utc::now(),
            parameter_name,
            old_value,
            new_value,
            predicted_impact,
            actual_impact: None,
            success: None,
            rationale,
        };

        self.optimization_history.push(record);

        // Limit history size
        if self.optimization_history.len() > 1000 {
            self.optimization_history.remove(0);
        }
    }

    /// Update optimization record with actual results
    pub fn update_optimization_result(
        &mut self,
        parameter_name: &str,
        timestamp: DateTime<Utc>,
        actual_impact: f64,
        success: bool,
    ) {
        if let Some(record) = self
            .optimization_history
            .iter_mut()
            .find(|r| r.parameter_name == parameter_name && r.timestamp == timestamp)
        {
            record.actual_impact = Some(actual_impact);
            record.success = Some(success);
        }
    }

    /// Get optimization statistics
    pub fn get_optimization_stats(&self) -> OptimizationStats {
        let total_attempts = self.optimization_history.len();
        let successful_attempts = self
            .optimization_history
            .iter()
            .filter(|r| r.success.unwrap_or(false))
            .count();

        let success_rate = if total_attempts > 0 {
            successful_attempts as f64 / total_attempts as f64
        } else {
            0.0
        };

        let avg_prediction_accuracy = if !self.optimization_history.is_empty() {
            let accuracy_sum: f64 = self
                .optimization_history
                .iter()
                .filter_map(|r| {
                    if let Some(actual) = r.actual_impact {
                        let error = (r.predicted_impact - actual).abs();
                        let accuracy = 1.0 - (error / (actual.abs() + 0.1));
                        Some(accuracy.max(0.0))
                    } else {
                        None
                    }
                })
                .sum();
            
            let count = self
                .optimization_history
                .iter()
                .filter(|r| r.actual_impact.is_some())
                .count();
            
            if count > 0 {
                accuracy_sum / count as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        OptimizationStats {
            total_attempts,
            successful_attempts,
            success_rate,
            avg_prediction_accuracy,
        }
    }
}

/// Optimization statistics
#[derive(Debug, Clone, Serialize)]
pub struct OptimizationStats {
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub success_rate: f64,
    pub avg_prediction_accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot::sliding_bounds::SlidingBoundsConfig;

    #[test]
    fn test_optimizer_creation() {
        let config = OptimizerConfig::default();
        let optimizer = ParameterOptimizer::new(config);
        
        assert_eq!(optimizer.optimization_history.len(), 0);
        assert_eq!(optimizer.current_parameters.len(), 0);
    }

    #[test]
    fn test_parameter_validation() {
        let config = OptimizerConfig::default();
        let optimizer = ParameterOptimizer::new(config);
        let bounds = SlidingBounds::new(SlidingBoundsConfig::default());
        
        let validation = optimizer.validate_parameter_change(
            "selection_temperature",
            0.85,
            0.90,
            &bounds,
        ).unwrap();
        
        assert!(validation.is_valid);
        assert!(validation.bounds_check);
        assert!(validation.business_logic_check);
        assert!(validation.safety_check);
    }

    #[test]
    fn test_bounds_violation_adjustment() {
        let config = OptimizerConfig::default();
        let optimizer = ParameterOptimizer::new(config);
        let bounds = SlidingBounds::new(SlidingBoundsConfig::default());
        
        // Test value outside bounds
        let validation = optimizer.validate_parameter_change(
            "selection_temperature",
            0.85,
            3.0, // Outside bounds
            &bounds,
        ).unwrap();
        
        assert!(validation.is_valid); // Should be valid after adjustment
        assert!(validation.adjusted_value.is_some());
        assert!(validation.adjusted_value.unwrap() <= 2.0); // Should be clamped to max bound
    }
}