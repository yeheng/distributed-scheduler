//! Cache configuration for Redis-based caching

use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::net::{IpAddr, SocketAddr};
use url::Url;

/// Cache configuration validation report
#[derive(Debug, Clone, Default)]
pub struct CacheValidationReport {
    /// Whether the configuration is valid
    pub valid: bool,
    /// Validation score (0-100)
    pub score: u8,
    /// Critical errors that must be fixed
    pub errors: Vec<String>,
    /// Warnings about potential issues
    pub warnings: Vec<String>,
    /// Informational messages
    pub info: Vec<String>,
    /// Suggestions for improvement
    pub suggestions: Vec<String>,
    /// Validation timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl CacheValidationReport {
    /// Get summary of validation results
    pub fn get_summary(&self) -> String {
        format!(
            "Cache Configuration Validation - Score: {}/100 - {}",
            self.score,
            if self.valid { "VALID" } else { "INVALID" }
        )
    }

    /// Get severity assessment
    pub fn get_severity(&self) -> ValidationSeverity {
        if !self.valid {
            ValidationSeverity::Critical
        } else if self.score < 70 {
            ValidationSeverity::Warning
        } else if self.score < 90 {
            ValidationSeverity::Info
        } else {
            ValidationSeverity::Good
        }
    }

    /// Check if configuration is production-ready
    pub fn is_production_ready(&self) -> bool {
        self.valid && self.score >= 80 && self.errors.is_empty()
    }
}

/// Validation severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Critical,
    Warning,
    Info,
    Good,
}

impl std::fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationSeverity::Critical => write!(f, "Critical"),
            ValidationSeverity::Warning => write!(f, "Warning"),
            ValidationSeverity::Info => write!(f, "Info"),
            ValidationSeverity::Good => write!(f, "Good"),
        }
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

/// Redis cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Redis connection URL
    pub redis_url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Command timeout in seconds
    pub command_timeout_seconds: u64,
    /// Cache key prefix for this instance
    pub key_prefix: Option<String>,
    /// Default TTL for cache entries
    pub default_ttl_seconds: u64,
    /// Whether to use cluster mode
    pub use_cluster: bool,
    /// Cache-specific TTL configurations
    pub ttl: CacheTtlConfig,
}

/// Cache TTL configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTtlConfig {
    /// Task definitions cache TTL in seconds
    pub task_seconds: u64,
    /// Worker status cache TTL in seconds
    pub worker_seconds: u64,
    /// System stats cache TTL in seconds
    pub system_stats_seconds: u64,
    /// Task run cache TTL in seconds
    pub task_run_seconds: u64,
    /// Dependencies cache TTL in seconds
    pub dependencies_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            connection_timeout_seconds: 5,
            command_timeout_seconds: 3,
            key_prefix: Some("scheduler".to_string()),
            default_ttl_seconds: 300,
            use_cluster: false,
            ttl: CacheTtlConfig::default(),
        }
    }
}

impl Default for CacheTtlConfig {
    fn default() -> Self {
        Self {
            task_seconds: 300,         // 5 minutes
            worker_seconds: 30,        // 30 seconds
            system_stats_seconds: 60,  // 1 minute
            task_run_seconds: 600,     // 10 minutes
            dependencies_seconds: 120, // 2 minutes
        }
    }
}

impl CacheConfig {
    /// Validate cache configuration
    pub fn validate(&self) -> anyhow::Result<CacheValidationReport> {
        let mut report = CacheValidationReport {
            timestamp: chrono::Utc::now(),
            ..Default::default()
        };
        
        if !self.enabled {
            report.warnings.push("Cache is disabled".to_string());
            return Ok(report);
        }

        // Validate Redis URL
        if self.redis_url.is_empty() {
            report.errors.push("Redis URL cannot be empty".to_string());
        } else {
            self.validate_redis_url(&mut report)?;
        }

        // Validate connection settings
        self.validate_connection_settings(&mut report)?;

        // Validate timeout settings
        self.validate_timeout_settings(&mut report)?;

        // Validate TTL settings
        self.validate_ttl_settings(&mut report)?;

        // Validate performance settings
        self.validate_performance_settings(&mut report)?;

        // Validate security settings
        self.validate_security_settings(&mut report)?;

        // Calculate overall validation result
        report.valid = report.errors.is_empty();
        report.score = self.calculate_validation_score(&report);

        Ok(report)
    }

    /// Validate Redis URL format and connectivity
    fn validate_redis_url(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        // Check URL format
        if !self.redis_url.starts_with("redis://") && !self.redis_url.starts_with("rediss://") {
            report.errors.push("Redis URL must start with redis:// or rediss://".to_string());
            return Ok(());
        }

        // Parse URL
        let url = match Url::parse(&self.redis_url) {
            Ok(url) => url,
            Err(e) => {
                report.errors.push(format!("Invalid Redis URL format: {}", e));
                return Ok(());
            }
        };

        // Check host
        if url.host_str().is_none() {
            report.errors.push("Redis URL must include a host".to_string());
        }

        // Check port
        let port = url.port().unwrap_or(6379);
        if port < 1 || port > 65535 {
            report.errors.push(format!("Invalid Redis port: {}", port));
        }

        // Check for SSL
        if url.scheme() == "rediss" {
            report.info.push("Using SSL connection to Redis".to_string());
        }

        // Check for password in URL (security concern)
        if url.password().is_some() {
            report.warnings.push("Redis password is stored in URL (consider using environment variables)".to_string());
        }

        // Suggest connection pooling optimization
        if self.max_connections < 5 {
            report.suggestions.push("Consider increasing max_connections to at least 5 for better performance".to_string());
        }

        Ok(())
    }

    /// Validate connection settings
    fn validate_connection_settings(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        if self.max_connections == 0 {
            report.errors.push("Max connections must be greater than 0".to_string());
        } else if self.max_connections > 100 {
            report.warnings.push("High max_connections value (>100) may consume excessive resources".to_string());
        } else if self.max_connections < 3 {
            report.suggestions.push("Consider increasing max_connections to at least 3 for better concurrency".to_string());
        }

        // Check key prefix
        if let Some(ref prefix) = self.key_prefix {
            if prefix.is_empty() {
                report.warnings.push("Empty key prefix configured".to_string());
            } else if prefix.len() > 50 {
                report.warnings.push("Long key prefix (>50 chars) may increase memory usage".to_string());
            } else if prefix.contains(':') {
                report.errors.push("Key prefix should not contain ':' character".to_string());
            }
        }

        Ok(())
    }

    /// Validate timeout settings
    fn validate_timeout_settings(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        if self.connection_timeout_seconds == 0 {
            report.errors.push("Connection timeout must be greater than 0".to_string());
        } else if self.connection_timeout_seconds > 30 {
            report.warnings.push("High connection timeout (>30s) may cause slow failure detection".to_string());
        } else if self.connection_timeout_seconds < 3 {
            report.suggestions.push("Consider increasing connection timeout to at least 3s for reliable connections".to_string());
        }

        if self.command_timeout_seconds == 0 {
            report.errors.push("Command timeout must be greater than 0".to_string());
        } else if self.command_timeout_seconds > 10 {
            report.warnings.push("High command timeout (>10s) may cause slow operations".to_string());
        } else if self.command_timeout_seconds < 1 {
            report.suggestions.push("Consider increasing command timeout to at least 1s for reliable operations".to_string());
        }

        // Check timeout ratio
        if self.connection_timeout_seconds > self.command_timeout_seconds * 3 {
            report.warnings.push("Connection timeout is much higher than command timeout".to_string());
        }

        Ok(())
    }

    /// Validate TTL settings
    fn validate_ttl_settings(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        if self.default_ttl_seconds == 0 {
            report.errors.push("Default TTL must be greater than 0".to_string());
        } else if self.default_ttl_seconds > 86400 {
            report.warnings.push("High default TTL (>24h) may cause memory issues".to_string());
        } else if self.default_ttl_seconds < 60 {
            report.suggestions.push("Consider increasing default TTL to at least 60s for better cache effectiveness".to_string());
        }

        // Validate specific TTL configurations
        self.ttl.validate_detailed(report)?;

        // Check TTL consistency
        let ttl_config = &self.ttl;
        if ttl_config.worker_seconds > ttl_config.task_seconds {
            report.info.push("Worker TTL is longer than task TTL (may be intentional)".to_string());
        }

        if ttl_config.system_stats_seconds > 3600 {
            report.warnings.push("System stats TTL (>1h) may provide outdated information".to_string());
        }

        Ok(())
    }

    /// Validate performance settings
    fn validate_performance_settings(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        // Check for cluster mode appropriateness
        if self.use_cluster {
            if self.max_connections < 10 {
                report.suggestions.push("Consider increasing max_connections for cluster mode (recommended: >=10)".to_string());
            }
            report.info.push("Cluster mode enabled - ensure Redis cluster is properly configured".to_string());
        }

        // Check for connection pooling efficiency
        let expected_concurrency = std::env::var("RUST_MAX_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);

        if self.max_connections < expected_concurrency {
            report.suggestions.push(format!("Consider setting max_connections to at least {} for optimal concurrency", expected_concurrency));
        }

        Ok(())
    }

    /// Validate security settings
    fn validate_security_settings(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        // Check for SSL in production
        if !self.redis_url.starts_with("rediss://") {
            let is_production = std::env::var("ENVIRONMENT")
                .map(|env| env.to_lowercase() == "production")
                .unwrap_or(false);

            if is_production {
                report.warnings.push("Production environment should use SSL (rediss://)".to_string());
            }
        }

        // Check for sensitive information in logs
        if self.redis_url.contains("password") {
            report.warnings.push("Redis URL contains password - ensure proper logging configuration".to_string());
        }

        Ok(())
    }

    /// Calculate validation score (0-100)
    fn calculate_validation_score(&self, report: &CacheValidationReport) -> u8 {
        let mut score = 100;

        // Deduct for errors (major issues)
        score = score.saturating_sub(report.errors.len() as u8 * 20);

        // Deduct for warnings (minor issues)
        score = score.saturating_sub(report.warnings.len() as u8 * 5);

        // Add bonus for suggestions (optimization opportunities)
        score = (score + report.suggestions.len() as u8).min(100);

        score
    }

    /// Get configuration recommendations
    pub fn get_recommendations(&self) -> Vec<CacheConfigRecommendation> {
        let mut recommendations = Vec::new();

        // Performance recommendations
        if self.max_connections < 5 {
            recommendations.push(CacheConfigRecommendation {
                category: "Performance".to_string(),
                priority: RecommendationPriority::Medium,
                title: "Increase connection pool size".to_string(),
                description: "Consider increasing max_connections to 5-10 for better concurrency".to_string(),
                current_value: format!("{}", self.max_connections),
                suggested_value: "5-10".to_string(),
                impact: "Improved throughput and reduced connection latency".to_string(),
            });
        }

        // Security recommendations
        if !self.redis_url.starts_with("rediss://") {
            recommendations.push(CacheConfigRecommendation {
                category: "Security".to_string(),
                priority: RecommendationPriority::High,
                title: "Enable SSL encryption".to_string(),
                description: "Use rediss:// instead of redis:// for secure communication".to_string(),
                current_value: self.redis_url.clone(),
                suggested_value: self.redis_url.replace("redis://", "rediss://").to_string(),
                impact: "Encrypted communication between application and Redis".to_string(),
            });
        }

        // TTL optimization recommendations
        if self.default_ttl_seconds < 300 {
            recommendations.push(CacheConfigRecommendation {
                category: "Performance".to_string(),
                priority: RecommendationPriority::Low,
                title: "Optimize default TTL".to_string(),
                description: "Consider increasing default TTL for better cache hit rates".to_string(),
                current_value: format!("{}s", self.default_ttl_seconds),
                suggested_value: "300s".to_string(),
                impact: "Improved cache effectiveness and reduced database load".to_string(),
            });
        }

        recommendations
    }

    /// Quick validation for backwards compatibility
    pub fn validate_legacy(&self) -> anyhow::Result<()> {
        let report = self.validate()?;
        if !report.valid {
            return Err(anyhow::anyhow!("Cache configuration validation failed: {}", report.errors.join(", ")));
        }
        Ok(())
    }
}

impl CacheTtlConfig {
    /// Validate TTL configuration (legacy method)
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.task_seconds == 0 {
            return Err(anyhow::anyhow!("Task TTL must be greater than 0"));
        }

        if self.worker_seconds == 0 {
            return Err(anyhow::anyhow!("Worker TTL must be greater than 0"));
        }

        if self.system_stats_seconds == 0 {
            return Err(anyhow::anyhow!("System stats TTL must be greater than 0"));
        }

        if self.task_run_seconds == 0 {
            return Err(anyhow::anyhow!("Task run TTL must be greater than 0"));
        }

        if self.dependencies_seconds == 0 {
            return Err(anyhow::anyhow!("Dependencies TTL must be greater than 0"));
        }

        Ok(())
    }

    /// Detailed TTL validation with reporting
    pub fn validate_detailed(&self, report: &mut CacheValidationReport) -> anyhow::Result<()> {
        // Validate individual TTL values
        if self.task_seconds == 0 {
            report.errors.push("Task TTL must be greater than 0".to_string());
        } else if self.task_seconds > 7200 {
            report.warnings.push("Task TTL (>2h) may cause memory issues with stale data".to_string());
        } else if self.task_seconds < 60 {
            report.suggestions.push("Consider increasing task TTL to at least 60s for better cache effectiveness".to_string());
        }

        if self.worker_seconds == 0 {
            report.errors.push("Worker TTL must be greater than 0".to_string());
        } else if self.worker_seconds > 300 {
            report.warnings.push("Worker TTL (>5m) may provide outdated worker status".to_string());
        } else if self.worker_seconds < 15 {
            report.suggestions.push("Consider increasing worker TTL to at least 15s for better performance".to_string());
        }

        if self.system_stats_seconds == 0 {
            report.errors.push("System stats TTL must be greater than 0".to_string());
        } else if self.system_stats_seconds > 1800 {
            report.warnings.push("System stats TTL (>30m) may provide outdated metrics".to_string());
        } else if self.system_stats_seconds < 30 {
            report.suggestions.push("Consider increasing system stats TTL to at least 30s for better performance".to_string());
        }

        if self.task_run_seconds == 0 {
            report.errors.push("Task run TTL must be greater than 0".to_string());
        } else if self.task_run_seconds > 14400 {
            report.warnings.push("Task run TTL (>4h) may cause memory issues with stale run data".to_string());
        } else if self.task_run_seconds < 300 {
            report.suggestions.push("Consider increasing task run TTL to at least 300s for better cache effectiveness".to_string());
        }

        if self.dependencies_seconds == 0 {
            report.errors.push("Dependencies TTL must be greater than 0".to_string());
        } else if self.dependencies_seconds > 3600 {
            report.warnings.push("Dependencies TTL (>1h) may provide outdated dependency information".to_string());
        } else if self.dependencies_seconds < 120 {
            report.suggestions.push("Consider increasing dependencies TTL to at least 120s for better performance".to_string());
        }

        // Check TTL ratios and consistency
        if self.worker_seconds > self.task_seconds * 2 {
            report.warnings.push("Worker TTL is much longer than task TTL - consider adjusting for consistency".to_string());
        }

        if self.system_stats_seconds > self.task_seconds {
            report.info.push("System stats TTL is longer than task TTL - this may be intentional for metrics".to_string());
        }

        if self.dependencies_seconds > self.task_seconds * 3 {
            report.warnings.push("Dependencies TTL is much longer than task TTL - consider adjusting for consistency".to_string());
        }

        Ok(())
    }

    /// Convert to CacheTtl
    pub fn to_cache_ttl(&self) -> super::CacheTtl {
        super::CacheTtl {
            task: Duration::from_secs(self.task_seconds),
            worker: Duration::from_secs(self.worker_seconds),
            system_stats: Duration::from_secs(self.system_stats_seconds),
            task_run: Duration::from_secs(self.task_run_seconds),
            dependencies: Duration::from_secs(self.dependencies_seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.default_ttl_seconds, 300);
    }

    #[test]
    fn test_cache_config_validation() {
        let mut config = CacheConfig::default();
        config.enabled = true;

        // Valid configuration
        let report = config.validate().unwrap();
        assert!(report.valid);
        assert!(report.errors.is_empty());

        // Invalid Redis URL
        config.redis_url = "".to_string();
        let report = config.validate().unwrap();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|e| e.contains("Redis URL cannot be empty")));

        // Invalid max connections
        config.redis_url = "redis://localhost:6379".to_string();
        config.max_connections = 0;
        let report = config.validate().unwrap();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|e| e.contains("Max connections must be greater than 0")));
    }

    #[test]
    fn test_cache_config_validation_legacy() {
        let mut config = CacheConfig::default();
        config.enabled = true;

        // Valid configuration
        assert!(config.validate_legacy().is_ok());

        // Invalid configuration
        config.redis_url = "".to_string();
        assert!(config.validate_legacy().is_err());
    }

    #[test]
    fn test_cache_config_recommendations() {
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 2, // Low connection count
            ..Default::default()
        };

        let recommendations = config.get_recommendations();
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.title.contains("Increase connection pool size")));
    }

    #[test]
    fn test_validation_report() {
        let mut report = CacheValidationReport::default();
        report.valid = true;
        report.score = 85;
        report.timestamp = chrono::Utc::now();

        assert!(report.is_production_ready());
        assert_eq!(report.get_severity(), ValidationSeverity::Info);

        report.score = 75;
        assert_eq!(report.get_severity(), ValidationSeverity::Warning);

        report.valid = false;
        assert_eq!(report.get_severity(), ValidationSeverity::Critical);
        assert!(!report.is_production_ready());
    }

    #[test]
    fn test_ttl_config_conversion() {
        let ttl_config = CacheTtlConfig::default();
        let cache_ttl = ttl_config.to_cache_ttl();

        assert_eq!(cache_ttl.task, Duration::from_secs(300));
        assert_eq!(cache_ttl.worker, Duration::from_secs(30));
        assert_eq!(cache_ttl.system_stats, Duration::from_secs(60));
        assert_eq!(cache_ttl.task_run, Duration::from_secs(600));
        assert_eq!(cache_ttl.dependencies, Duration::from_secs(120));
    }
}
