//! Cache configuration validation system
//!
//! This module provides comprehensive validation for cache configurations
//! including security, performance, and operational checks.

use crate::cache::config::{CacheConfig, CacheValidationReport};
use crate::cache::warming::{WarmingConfig, WarmingStrategy};
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::time::Duration;

/// Cache configuration validator
pub struct CacheConfigValidator {
    /// Environment-specific validation rules
    environment_rules: HashMap<String, EnvironmentRules>,
    /// Performance thresholds
    performance_thresholds: PerformanceThresholds,
    /// Security requirements
    security_requirements: SecurityRequirements,
}

/// Environment-specific validation rules
#[derive(Debug, Clone)]
pub struct EnvironmentRules {
    /// Whether cache is required in this environment
    pub cache_required: bool,
    /// Minimum connection pool size
    pub min_connections: u32,
    /// Maximum connection pool size
    pub max_connections: u32,
    /// Whether SSL is required
    pub ssl_required: bool,
    /// Minimum TTL for cache entries
    pub min_ttl_seconds: u64,
    /// Maximum TTL for cache entries
    pub max_ttl_seconds: u64,
    /// Whether cluster mode is allowed
    pub allow_cluster: bool,
    /// Performance monitoring required
    pub performance_monitoring_required: bool,
}

/// Performance thresholds for cache validation
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// Maximum acceptable operation latency (milliseconds)
    pub max_operation_latency_ms: u64,
    /// Minimum acceptable hit rate (0.0 to 1.0)
    pub min_hit_rate: f64,
    /// Maximum acceptable error rate (0.0 to 1.0)
    pub max_error_rate: f64,
    /// Minimum connection pool efficiency (0.0 to 1.0)
    pub min_pool_efficiency: f64,
    /// Maximum memory usage (MB)
    pub max_memory_usage_mb: u64,
}

/// Security requirements for cache validation
#[derive(Debug, Clone)]
pub struct SecurityRequirements {
    /// Whether SSL encryption is required
    pub ssl_required: bool,
    /// Whether password authentication is required
    pub password_required: bool,
    /// Whether connection encryption is required
    pub connection_encryption_required: bool,
    /// Allowed cipher suites (empty means all allowed)
    pub allowed_cipher_suites: Vec<String>,
    /// Whether TLS version checking is required
    pub tls_version_check_required: bool,
    /// Minimum TLS version
    pub min_tls_version: String,
}

impl Default for CacheConfigValidator {
    fn default() -> Self {
        let mut environment_rules = HashMap::new();

        // Development environment rules
        environment_rules.insert(
            "development".to_string(),
            EnvironmentRules {
                cache_required: false,
                min_connections: 1,
                max_connections: 10,
                ssl_required: false,
                min_ttl_seconds: 30,
                max_ttl_seconds: 3600,
                allow_cluster: false,
                performance_monitoring_required: false,
            },
        );

        // Testing environment rules
        environment_rules.insert(
            "testing".to_string(),
            EnvironmentRules {
                cache_required: true,
                min_connections: 3,
                max_connections: 20,
                ssl_required: false,
                min_ttl_seconds: 60,
                max_ttl_seconds: 1800,
                allow_cluster: true,
                performance_monitoring_required: true,
            },
        );

        // Staging environment rules
        environment_rules.insert(
            "staging".to_string(),
            EnvironmentRules {
                cache_required: true,
                min_connections: 5,
                max_connections: 50,
                ssl_required: true,
                min_ttl_seconds: 120,
                max_ttl_seconds: 7200,
                allow_cluster: true,
                performance_monitoring_required: true,
            },
        );

        // Production environment rules
        environment_rules.insert(
            "production".to_string(),
            EnvironmentRules {
                cache_required: true,
                min_connections: 10,
                max_connections: 100,
                ssl_required: true,
                min_ttl_seconds: 300,
                max_ttl_seconds: 14400,
                allow_cluster: true,
                performance_monitoring_required: true,
            },
        );

        Self {
            environment_rules,
            performance_thresholds: PerformanceThresholds {
                max_operation_latency_ms: 100,
                min_hit_rate: 0.6,
                max_error_rate: 0.01,
                min_pool_efficiency: 0.7,
                max_memory_usage_mb: 1024,
            },
            security_requirements: SecurityRequirements {
                ssl_required: true,
                password_required: true,
                connection_encryption_required: true,
                allowed_cipher_suites: vec![
                    "TLS_AES_256_GCM_SHA384".to_string(),
                    "TLS_AES_128_GCM_SHA256".to_string(),
                    "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                ],
                tls_version_check_required: true,
                min_tls_version: "1.3".to_string(),
            },
        }
    }
}

impl CacheConfigValidator {
    /// Create new cache configuration validator
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate cache configuration with environment context
    pub fn validate_with_environment(
        &self,
        config: &CacheConfig,
        environment: &str,
    ) -> SchedulerResult<CacheValidationReport> {
        let mut report = CacheValidationReport {
            timestamp: chrono::Utc::now(),
            ..Default::default()
        };

        // Get environment rules
        let env_rules =
            self.environment_rules
                .get(environment)
                .cloned()
                .unwrap_or(EnvironmentRules {
                    cache_required: true,
                    min_connections: 5,
                    max_connections: 50,
                    ssl_required: true,
                    min_ttl_seconds: 300,
                    max_ttl_seconds: 7200,
                    allow_cluster: true,
                    performance_monitoring_required: true,
                });

        // Basic validation first
        let basic_report = config.validate()?;
        report.errors.extend(basic_report.errors);
        report.warnings.extend(basic_report.warnings);
        report.info.extend(basic_report.info);
        report.suggestions.extend(basic_report.suggestions);

        // Environment-specific validation
        self.validate_environment_requirements(config, &env_rules, environment, &mut report);

        // Performance validation
        self.validate_performance_requirements(config, &mut report);

        // Security validation
        self.validate_security_requirements(config, &env_rules, &mut report);

        // Operational validation
        self.validate_operational_requirements(config, &mut report);

        // Calculate final score and validity
        report.valid = report.errors.is_empty();
        report.score = self.calculate_comprehensive_score(&report, &env_rules);

        Ok(report)
    }

    /// Validate environment-specific requirements
    fn validate_environment_requirements(
        &self,
        config: &CacheConfig,
        env_rules: &EnvironmentRules,
        environment: &str,
        report: &mut CacheValidationReport,
    ) {
        // Check if cache is required
        if env_rules.cache_required && !config.enabled {
            report.errors.push(format!(
                "Cache is required in {environment} environment but currently disabled"
            ));
        }

        // Validate connection pool size
        if config.enabled {
            if config.max_connections < env_rules.min_connections {
                report.errors.push(format!(
                    "Connection pool size ({}) is below minimum required ({}) for {} environment",
                    config.max_connections, env_rules.min_connections, environment
                ));
            }

            if config.max_connections > env_rules.max_connections {
                report.warnings.push(format!(
                    "Connection pool size ({}) exceeds maximum recommended ({}) for {} environment",
                    config.max_connections, env_rules.max_connections, environment
                ));
            }

            // Calculate pool efficiency
            let expected_threads = std::env::var("RUST_MAX_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4);

            let pool_efficiency = if expected_threads > 0 {
                config.max_connections as f64 / expected_threads as f64
            } else {
                1.0
            };

            if pool_efficiency < self.performance_thresholds.min_pool_efficiency {
                report.suggestions.push(format!(
                    "Consider increasing max_connections to {} for better thread utilization (current efficiency: {:.2})",
                    (expected_threads as f64 * self.performance_thresholds.min_pool_efficiency).ceil() as u32,
                    pool_efficiency
                ));
            }
        }

        // Validate SSL requirement
        if env_rules.ssl_required && !config.redis_url.starts_with("rediss://") {
            report.errors.push(format!(
                "SSL is required in {environment} environment but Redis URL does not use rediss://"
            ));
        }

        // Validate TTL ranges
        if config.enabled {
            if config.default_ttl_seconds < env_rules.min_ttl_seconds {
                report.errors.push(format!(
                    "Default TTL ({}) is below minimum required ({}) for {} environment",
                    config.default_ttl_seconds, env_rules.min_ttl_seconds, environment
                ));
            }

            if config.default_ttl_seconds > env_rules.max_ttl_seconds {
                report.errors.push(format!(
                    "Default TTL ({}) exceeds maximum allowed ({}) for {} environment",
                    config.default_ttl_seconds, env_rules.max_ttl_seconds, environment
                ));
            }

            // Validate specific TTLs
            self.validate_ttl_ranges(config, env_rules, report);
        }

        // Validate cluster mode
        if config.use_cluster && !env_rules.allow_cluster {
            report.errors.push(format!(
                "Cluster mode is not allowed in {environment} environment"
            ));
        }

        // Validate performance monitoring requirement
        if env_rules.performance_monitoring_required {
            report
                .info
                .push("Performance monitoring is required for this environment".to_string());
        }
    }

    /// Validate TTL ranges against environment rules
    fn validate_ttl_ranges(
        &self,
        config: &CacheConfig,
        env_rules: &EnvironmentRules,
        report: &mut CacheValidationReport,
    ) {
        let ttl_config = &config.ttl;

        // Task TTL
        if ttl_config.task_seconds < env_rules.min_ttl_seconds {
            report.errors.push(format!(
                "Task TTL ({}) is below minimum required ({})",
                ttl_config.task_seconds, env_rules.min_ttl_seconds
            ));
        }

        if ttl_config.task_seconds > env_rules.max_ttl_seconds {
            report.errors.push(format!(
                "Task TTL ({}) exceeds maximum allowed ({})",
                ttl_config.task_seconds, env_rules.max_ttl_seconds
            ));
        }

        // Worker TTL
        if ttl_config.worker_seconds < env_rules.min_ttl_seconds {
            report.errors.push(format!(
                "Worker TTL ({}) is below minimum required ({})",
                ttl_config.worker_seconds, env_rules.min_ttl_seconds
            ));
        }

        if ttl_config.worker_seconds > env_rules.max_ttl_seconds {
            report.warnings.push(format!(
                "Worker TTL ({}) may be too long for optimal performance",
                ttl_config.worker_seconds
            ));
        }

        // System stats TTL
        if ttl_config.system_stats_seconds < env_rules.min_ttl_seconds {
            report.errors.push(format!(
                "System stats TTL ({}) is below minimum required ({})",
                ttl_config.system_stats_seconds, env_rules.min_ttl_seconds
            ));
        }

        // Task run TTL
        if ttl_config.task_run_seconds < env_rules.min_ttl_seconds {
            report.errors.push(format!(
                "Task run TTL ({}) is below minimum required ({})",
                ttl_config.task_run_seconds, env_rules.min_ttl_seconds
            ));
        }

        // Dependencies TTL
        if ttl_config.dependencies_seconds < env_rules.min_ttl_seconds {
            report.errors.push(format!(
                "Dependencies TTL ({}) is below minimum required ({})",
                ttl_config.dependencies_seconds, env_rules.min_ttl_seconds
            ));
        }
    }

    /// Validate performance requirements
    fn validate_performance_requirements(
        &self,
        config: &CacheConfig,
        report: &mut CacheValidationReport,
    ) {
        if !config.enabled {
            return;
        }

        // Validate timeout settings for performance
        if config.connection_timeout_seconds > 10 {
            report
                .warnings
                .push("High connection timeout may impact performance".to_string());
        }

        if config.command_timeout_seconds > 5 {
            report
                .warnings
                .push("High command timeout may impact performance".to_string());
        }

        // Validate connection pool settings
        if config.max_connections < 5 {
            report
                .suggestions
                .push("Consider increasing max_connections for better performance".to_string());
        }

        // Validate TTL settings for performance
        if config.default_ttl_seconds < 60 {
            report
                .suggestions
                .push("Consider increasing default TTL for better cache effectiveness".to_string());
        }

        // Calculate expected memory usage
        let expected_memory_mb = self.estimate_memory_usage(config);
        if expected_memory_mb > self.performance_thresholds.max_memory_usage_mb {
            report.warnings.push(format!(
                "Expected memory usage ({} MB) may exceed recommended maximum ({} MB)",
                expected_memory_mb, self.performance_thresholds.max_memory_usage_mb
            ));
        }

        // Performance optimization suggestions
        if config.use_cluster && config.max_connections < 10 {
            report.suggestions.push(
                "Consider increasing max_connections for cluster mode performance".to_string(),
            );
        }

        if config.key_prefix.is_none() {
            report
                .suggestions
                .push("Consider setting a key prefix to avoid cache collisions".to_string());
        }
    }

    /// Validate security requirements
    fn validate_security_requirements(
        &self,
        config: &CacheConfig,
        env_rules: &EnvironmentRules,
        report: &mut CacheValidationReport,
    ) {
        if !config.enabled {
            return;
        }

        // Check SSL requirement
        if (self.security_requirements.ssl_required && env_rules.ssl_required)
            && !config.redis_url.starts_with("rediss://")
        {
            report
                .errors
                .push("SSL encryption is required for secure cache communication".to_string());
        }

        // Check for password in URL (security risk)
        if config.redis_url.contains("password") {
            report.warnings.push(
                "Password found in Redis URL - consider using environment variables".to_string(),
            );
        }

        // Validate TLS version if SSL is used
        if config.redis_url.starts_with("rediss://")
            && self.security_requirements.tls_version_check_required
        {
            report
                .info
                .push("SSL enabled - ensure TLS 1.3 is supported by Redis server".to_string());
        }

        // Check for secure key prefix
        if let Some(ref prefix) = config.key_prefix {
            if prefix.contains("password") || prefix.contains("secret") || prefix.contains("key") {
                report.warnings.push(
                    "Key prefix contains sensitive terms - consider using a generic prefix"
                        .to_string(),
                );
            }
        }

        // Security suggestions
        if !config.redis_url.starts_with("rediss://") {
            let is_production = std::env::var("ENVIRONMENT")
                .map(|env| env.to_lowercase() == "production")
                .unwrap_or(false);

            if is_production {
                report.suggestions.push(
                    "Consider using SSL (rediss://) for production environment security"
                        .to_string(),
                );
            }
        }
    }

    /// Validate operational requirements
    fn validate_operational_requirements(
        &self,
        config: &CacheConfig,
        report: &mut CacheValidationReport,
    ) {
        if !config.enabled {
            return;
        }

        // Validate operational readiness
        if config.max_connections == 0 {
            report
                .errors
                .push("Max connections cannot be zero for operational cache".to_string());
        }

        // Validate timeout operational bounds
        if config.connection_timeout_seconds == 0 {
            report
                .errors
                .push("Connection timeout cannot be zero for operational cache".to_string());
        }

        if config.command_timeout_seconds == 0 {
            report
                .errors
                .push("Command timeout cannot be zero for operational cache".to_string());
        }

        // Check for operational efficiency
        let connection_to_command_ratio =
            config.connection_timeout_seconds as f64 / config.command_timeout_seconds as f64;
        if connection_to_command_ratio > 5.0 {
            report.warnings.push(
                "Connection timeout is much higher than command timeout - consider adjusting for better efficiency".to_string()
            );
        }

        // Operational suggestions
        if config.default_ttl_seconds < 300 {
            report.suggestions.push(
                "Consider increasing default TTL for better operational efficiency".to_string(),
            );
        }

        if config.key_prefix.is_none() {
            report
                .suggestions
                .push("Consider setting a key prefix for better operational isolation".to_string());
        }

        // Cluster operational considerations
        if config.use_cluster {
            report.info.push(
                "Cluster mode enabled - ensure proper cluster configuration and monitoring"
                    .to_string(),
            );
        }
    }

    /// Estimate memory usage based on configuration
    fn estimate_memory_usage(&self, config: &CacheConfig) -> u64 {
        // Basic estimation based on connection pool and TTL settings
        let base_memory_mb = 10; // Base memory for Redis client
        let connection_memory_mb = config.max_connections as u64 * 2; // 2MB per connection
        let ttl_factor = config.default_ttl_seconds as f64 / 300.0; // Normalized to 5 minutes

        ((base_memory_mb + connection_memory_mb) as f64 * ttl_factor) as u64
    }

    /// Calculate comprehensive validation score
    fn calculate_comprehensive_score(
        &self,
        report: &CacheValidationReport,
        env_rules: &EnvironmentRules,
    ) -> u8 {
        let mut score: u8 = 100;

        // Deduct for errors (critical issues)
        score = score.saturating_sub(report.errors.len() as u8 * 25);

        // Deduct for warnings (important issues)
        score = score.saturating_sub(report.warnings.len() as u8 * 10);

        // Deduct for missing suggestions (optimization opportunities)
        score = score.saturating_sub(report.suggestions.len() as u8 * 2);

        // Add bonus for info messages (good practices)
        score = (score.saturating_add(report.info.len() as u8)).min(100);

        // Apply environment-specific scoring
        if env_rules.cache_required && !report.errors.iter().any(|e| e.contains("disabled")) {
            score = (score + 5).min(100);
        }

        score
    }

    /// Get cache configuration recommendations
    pub fn get_recommendations(
        &self,
        config: &CacheConfig,
        environment: &str,
    ) -> Vec<CacheConfigRecommendation> {
        let mut recommendations = Vec::new();

        // Performance recommendations
        if config.max_connections < 5 {
            recommendations.push(CacheConfigRecommendation {
                category: "Performance".to_string(),
                priority: RecommendationPriority::High,
                title: "Increase connection pool size".to_string(),
                description: "Consider increasing max_connections to 5-10 for better concurrency"
                    .to_string(),
                current_value: format!("{}", config.max_connections),
                suggested_value: "5-10".to_string(),
                impact: "Improved throughput and reduced connection latency".to_string(),
            });
        }

        // Security recommendations
        if !config.redis_url.starts_with("rediss://") {
            recommendations.push(CacheConfigRecommendation {
                category: "Security".to_string(),
                priority: RecommendationPriority::High,
                title: "Enable SSL encryption".to_string(),
                description: "Use rediss:// instead of redis:// for secure communication"
                    .to_string(),
                current_value: config.redis_url.clone(),
                suggested_value: config
                    .redis_url
                    .replace("redis://", "rediss://")
                    .to_string(),
                impact: "Encrypted communication between application and Redis".to_string(),
            });
        }

        // TTL optimization recommendations
        if config.default_ttl_seconds < 300 {
            recommendations.push(CacheConfigRecommendation {
                category: "Performance".to_string(),
                priority: RecommendationPriority::Medium,
                title: "Optimize default TTL".to_string(),
                description: "Consider increasing default TTL for better cache hit rates"
                    .to_string(),
                current_value: format!("{}s", config.default_ttl_seconds),
                suggested_value: "300s".to_string(),
                impact: "Improved cache effectiveness and reduced database load".to_string(),
            });
        }

        // Environment-specific recommendations
        if environment == "production" && config.max_connections < 10 {
            recommendations.push(CacheConfigRecommendation {
                category: "Production Readiness".to_string(),
                priority: RecommendationPriority::Critical,
                title: "Increase production connection pool".to_string(),
                description:
                    "Production environment requires larger connection pool for scalability"
                        .to_string(),
                current_value: format!("{}", config.max_connections),
                suggested_value: "10-20".to_string(),
                impact: "Better scalability and performance under load".to_string(),
            });
        }

        recommendations
    }

    /// Validate warming configuration
    pub fn validate_warming_config(
        &self,
        warming_config: &WarmingConfig,
    ) -> SchedulerResult<CacheValidationReport> {
        let mut report = CacheValidationReport {
            timestamp: chrono::Utc::now(),
            ..Default::default()
        };

        // Validate warming enabled state
        if warming_config.enabled {
            report.info.push("Cache warming is enabled".to_string());
        } else {
            report.warnings.push(
                "Cache warming is disabled - consider enabling for better performance".to_string(),
            );
        }

        // Validate warming strategy
        match warming_config.strategy {
            WarmingStrategy::Full => {
                report
                    .info
                    .push("Full cache warming strategy selected".to_string());
                if warming_config.max_concurrent_warming > 10 {
                    report
                        .warnings
                        .push("High concurrent warming may impact system performance".to_string());
                }
            }
            WarmingStrategy::Selective => {
                report
                    .info
                    .push("Selective cache warming strategy selected".to_string());
            }
            WarmingStrategy::Predictive => {
                report
                    .info
                    .push("Predictive cache warming strategy selected".to_string());
                report
                    .suggestions
                    .push("Ensure sufficient historical data for accurate predictions".to_string());
            }
            WarmingStrategy::OnDemand => {
                report
                    .info
                    .push("On-demand cache warming strategy selected".to_string());
            }
            WarmingStrategy::Hybrid => {
                report
                    .info
                    .push("Hybrid cache warming strategy selected".to_string());
            }
        }

        // Validate warming interval
        if warming_config.warm_up_interval < Duration::from_secs(60) {
            report
                .warnings
                .push("Warm-up interval is very short - may cause excessive load".to_string());
        }

        if warming_config.warm_up_interval > Duration::from_secs(1800) {
            report.suggestions.push(
                "Consider reducing warm-up interval for more frequent cache updates".to_string(),
            );
        }

        // Validate concurrent warming
        if warming_config.max_concurrent_warming == 0 {
            report
                .errors
                .push("Max concurrent warming must be greater than 0".to_string());
        }

        if warming_config.max_concurrent_warming > 20 {
            report
                .warnings
                .push("High concurrent warming may impact system performance".to_string());
        }

        // Validate warming timeout
        if warming_config.warming_timeout < Duration::from_secs(10) {
            report
                .warnings
                .push("Warming timeout is very short - may cause incomplete warming".to_string());
        }

        if warming_config.warming_timeout > Duration::from_secs(300) {
            report
                .suggestions
                .push("Consider reducing warming timeout for better responsiveness".to_string());
        }

        // Validate startup warming
        if warming_config.warm_on_startup {
            report
                .info
                .push("Cache warming on startup is enabled".to_string());
        } else {
            report.suggestions.push(
                "Consider enabling warm on startup for better initial performance".to_string(),
            );
        }

        report.valid = report.errors.is_empty();
        report.score = self.calculate_warming_score(&report);

        Ok(report)
    }

    /// Calculate warming configuration score
    fn calculate_warming_score(&self, report: &CacheValidationReport) -> u8 {
        let mut score: u8 = 100;

        // Deduct for errors
        score = score.saturating_sub(report.errors.len() as u8 * 30);

        // Deduct for warnings
        score = score.saturating_sub(report.warnings.len() as u8 * 15);

        // Deduct for missing suggestions
        score = score.saturating_sub(report.suggestions.len() as u8 * 5);

        // Add bonus for info messages
        score = (score.saturating_add(report.info.len() as u8 * 2)).min(100);

        score
    }
}

/// Cache configuration recommendation
#[derive(Debug, Clone)]
pub struct CacheConfigRecommendation {
    /// Category of the recommendation
    pub category: String,
    /// Priority level
    pub priority: RecommendationPriority,
    /// Short title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Current value
    pub current_value: String,
    /// Suggested value
    pub suggested_value: String,
    /// Expected impact
    pub impact: String,
}

/// Recommendation priority levels
#[derive(Debug, Clone, PartialEq)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RecommendationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecommendationPriority::Low => write!(f, "Low"),
            RecommendationPriority::Medium => write!(f, "Medium"),
            RecommendationPriority::High => write!(f, "High"),
            RecommendationPriority::Critical => write!(f, "Critical"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::config::CacheConfig;

    #[test]
    fn test_cache_config_validator_creation() {
        let validator = CacheConfigValidator::new();
        assert!(!validator.environment_rules.is_empty());
        assert_eq!(
            validator.performance_thresholds.max_operation_latency_ms,
            100
        );
    }

    #[test]
    fn test_environment_rules() {
        let validator = CacheConfigValidator::new();

        let dev_rules = &validator.environment_rules["development"];
        assert!(!dev_rules.cache_required);
        assert!(!dev_rules.ssl_required);

        let prod_rules = &validator.environment_rules["production"];
        assert!(prod_rules.cache_required);
        assert!(prod_rules.ssl_required);
    }

    #[test]
    fn test_validate_with_environment() {
        let validator = CacheConfigValidator::new();
        let config = CacheConfig::default();

        // Test development environment (cache disabled is allowed)
        let report = validator
            .validate_with_environment(&config, "development")
            .unwrap();
        assert!(report.valid || report.errors.iter().any(|e| e.contains("disabled")));

        // Test production environment (cache disabled should cause error)
        let report = validator
            .validate_with_environment(&config, "production")
            .unwrap();
        assert!(report.errors.iter().any(|e| e.contains("required")));
    }

    #[test]
    fn test_warming_config_validation() {
        let validator = CacheConfigValidator::new();
        let warming_config = WarmingConfig::default();

        let report = validator.validate_warming_config(&warming_config).unwrap();
        assert!(report.valid);
        assert!(report.score > 80);
    }

    #[test]
    fn test_recommendations() {
        let validator = CacheConfigValidator::new();
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 2, // Low for recommendations
            ..Default::default()
        };

        let recommendations = validator.get_recommendations(&config, "production");
        assert!(!recommendations.is_empty());

        // Should recommend increasing connection pool
        assert!(recommendations
            .iter()
            .any(|r| r.title.contains("Increase connection pool")));

        // Should recommend SSL for production
        assert!(recommendations
            .iter()
            .any(|r| r.title.contains("Enable SSL")));
    }
}
