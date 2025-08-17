//! Cache integration module
//!
//! This module provides integration utilities for combining all cache components
//! including validation, warming, invalidation, and monitoring.

use crate::cache::config::CacheValidationReport;
use crate::cache::{
    config::CacheConfig,
    invalidation::CacheInvalidationManager,
    manager::RedisCacheManager,
    metrics::{CacheMetrics, CachePerformanceMonitor},
    validation::CacheConfigValidator,
    warming::{CacheWarmingManager, WarmingConfig},
    CacheService, CacheStats,
};
use scheduler_domain::repositories::*;
use scheduler_foundation::SchedulerResult;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};

/// Integrated cache management system
pub struct IntegratedCacheManager {
    /// Cache service instance
    cache_service: Arc<dyn CacheService>,
    /// Cache invalidation manager
    invalidation_manager: Arc<CacheInvalidationManager>,
    /// Cache warming manager
    warming_manager: Option<Arc<CacheWarmingManager>>,
    /// Cache performance monitor
    performance_monitor: Arc<CachePerformanceMonitor>,
    /// Cache configuration validator
    validator: CacheConfigValidator,
    /// Cache configuration
    config: CacheConfig,
    /// Current cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Environment name
    environment: String,
}

/// Cache management configuration
#[derive(Debug, Clone)]
pub struct CacheManagementConfig {
    /// Whether to enable cache warming
    pub enable_warming: bool,
    /// Whether to enable performance monitoring
    pub enable_monitoring: bool,
    /// Whether to enable cache invalidation
    pub enable_invalidation: bool,
    /// Performance monitoring interval
    pub monitoring_interval_seconds: u64,
    /// Cache warming interval
    pub warming_interval_seconds: u64,
    /// Environment name
    pub environment: String,
}

impl Default for CacheManagementConfig {
    fn default() -> Self {
        Self {
            enable_warming: true,
            enable_monitoring: true,
            enable_invalidation: true,
            monitoring_interval_seconds: 60,
            warming_interval_seconds: 300,
            environment: "development".to_string(),
        }
    }
}

impl IntegratedCacheManager {
    /// Create new integrated cache manager
    #[instrument(skip_all)]
    pub async fn new(
        cache_config: CacheConfig,
        management_config: CacheManagementConfig,
        task_repository: Option<Arc<dyn TaskRepository>>,
        worker_repository: Option<Arc<dyn WorkerRepository>>,
        task_run_repository: Option<Arc<dyn TaskRunRepository>>,
    ) -> SchedulerResult<Self> {
        info!(
            "Initializing integrated cache manager for {} environment",
            management_config.environment
        );

        // Validate cache configuration
        let validator = CacheConfigValidator::new();
        let validation_report =
            validator.validate_with_environment(&cache_config, &management_config.environment)?;

        if !validation_report.valid {
            warn!(
                "Cache configuration validation failed: {:?}",
                validation_report.errors
            );
            return Err(scheduler_errors::SchedulerError::CacheError(format!(
                "Cache configuration validation failed: {}",
                validation_report.errors.join(", ")
            )));
        }

        info!(
            "Cache configuration validated successfully (score: {}/100)",
            validation_report.score
        );

        // Initialize cache service
        let cache_manager = RedisCacheManager::new(cache_config.clone()).await?;
        let cache_service = Arc::new(cache_manager) as Arc<dyn CacheService>;

        // Initialize invalidation manager
        let invalidation_manager = Arc::new(CacheInvalidationManager::new(cache_service.clone()));

        // Initialize warming manager if enabled and repositories are provided
        let warming_manager = if management_config.enable_warming
            && task_repository.is_some()
            && worker_repository.is_some()
            && task_run_repository.is_some()
        {
            let warming_config = WarmingConfig::default();
            let manager = Arc::new(CacheWarmingManager::new(
                cache_service.clone(),
                task_repository.clone().unwrap(),
                worker_repository.clone().unwrap(),
                task_run_repository.clone().unwrap(),
                warming_config,
            ));

            // Perform initial cache warming
            match manager.warm_up_on_startup().await {
                Ok(results) => {
                    let total_warmed: usize = results.iter().map(|r| r.keys_warmed).sum();
                    info!("Cache warm-up completed: {} keys warmed", total_warmed);
                }
                Err(e) => {
                    warn!("Cache warm-up failed: {}", e);
                }
            }

            Some(manager)
        } else {
            if management_config.enable_warming {
                warn!("Cache warming enabled but required repositories not provided");
            }
            None
        };

        // Initialize performance monitor
        let registry = prometheus::Registry::new();
        let cache_metrics = CacheMetrics::new(&registry);
        let performance_monitor = Arc::new(CachePerformanceMonitor::new(cache_metrics));

        let has_warming = warming_manager.is_some();

        // Create integrated manager
        let manager = Self {
            cache_service: cache_service.clone(),
            invalidation_manager,
            warming_manager,
            performance_monitor,
            validator,
            config: cache_config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            environment: management_config.environment,
        };

        // Start background tasks if enabled
        if management_config.enable_monitoring {
            manager.start_performance_monitoring(management_config.monitoring_interval_seconds);
        }

        if management_config.enable_warming && has_warming {
            manager.start_periodic_warming(management_config.warming_interval_seconds);
        }

        info!("Integrated cache manager initialized successfully");
        Ok(manager)
    }

    /// Get cache service reference
    pub fn cache_service(&self) -> Arc<dyn CacheService> {
        self.cache_service.clone()
    }

    /// Get invalidation manager reference
    pub fn invalidation_manager(&self) -> Arc<CacheInvalidationManager> {
        self.invalidation_manager.clone()
    }

    /// Get warming manager reference (if available)
    pub fn warming_manager(&self) -> Option<Arc<CacheWarmingManager>> {
        self.warming_manager.clone()
    }

    /// Get performance monitor reference
    pub fn performance_monitor(&self) -> Arc<CachePerformanceMonitor> {
        self.performance_monitor.clone()
    }

    /// Get current cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Get comprehensive cache health report
    #[instrument(skip(self))]
    pub async fn get_health_report(&self) -> SchedulerResult<CacheHealthReport> {
        let stats = self.get_stats().await;
        let mut alerts = Vec::new();

        // Check performance alerts
        let perf_alerts = self.performance_monitor.monitor(&stats).await;
        alerts.extend(perf_alerts);

        // Generate performance report
        let performance_report = self
            .performance_monitor
            .get_performance_report(&stats)
            .await;

        // Validate configuration
        let config_validation = self
            .validator
            .validate_with_environment(&self.config, &self.environment)?;

        // Get invalidation stats
        let invalidation_stats = self.invalidation_manager.get_stats().await;

        // Get warming stats if available
        let warming_stats = if let Some(ref warming_manager) = self.warming_manager {
            Some(warming_manager.get_stats().await)
        } else {
            None
        };

        // Calculate overall health status
        let health_status = self.calculate_overall_health(
            &performance_report,
            &config_validation,
            &invalidation_stats,
            &warming_stats,
        );

        Ok(CacheHealthReport {
            overall_health: health_status,
            performance_report,
            config_validation,
            invalidation_stats,
            warming_stats,
            alerts,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Update cache statistics
    pub async fn update_stats(&self, new_stats: CacheStats) {
        // Update performance metrics first
        self.performance_monitor
            .metrics()
            .update_stats(&new_stats)
            .await;

        // Then update internal stats
        let mut stats = self.stats.write().await;
        *stats = new_stats;
    }

    /// Invalidate cache based on event
    pub async fn invalidate_on_event(
        &self,
        event: crate::cache::invalidation::InvalidationEvent,
    ) -> SchedulerResult<Vec<crate::cache::invalidation::InvalidationResult>> {
        self.invalidation_manager.invalidate_on_event(event).await
    }

    /// Perform cache warming
    pub async fn perform_warming(
        &self,
    ) -> SchedulerResult<Vec<crate::cache::warming::WarmingResult>> {
        if let Some(ref warming_manager) = self.warming_manager {
            warming_manager.execute_scheduled_warming().await
        } else {
            Ok(Vec::new())
        }
    }

    /// Validate cache configuration
    pub fn validate_config(&self) -> SchedulerResult<CacheValidationReport> {
        self.validator
            .validate_with_environment(&self.config, &self.environment)
    }

    /// Get cache configuration recommendations
    pub fn get_recommendations(&self) -> Vec<crate::cache::validation::CacheConfigRecommendation> {
        self.validator
            .get_recommendations(&self.config, &self.environment)
    }

    /// Start performance monitoring in background
    fn start_performance_monitoring(&self, interval_seconds: u64) {
        let monitor = self.performance_monitor.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;
                let current_stats = stats.read().await.clone();

                // Monitor performance and log alerts
                let alerts = monitor.monitor(&current_stats).await;
                if !alerts.is_empty() {
                    warn!("Cache performance alerts: {:?}", alerts);
                }
            }
        });

        info!(
            "Started performance monitoring with {}s interval",
            interval_seconds
        );
    }

    /// Start periodic cache warming
    fn start_periodic_warming(&self, interval_seconds: u64) {
        if let Some(ref warming_manager) = self.warming_manager {
            let warming_manager = warming_manager.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

                loop {
                    interval.tick().await;

                    match warming_manager.execute_scheduled_warming().await {
                        Ok(results) => {
                            let total_warmed: usize = results.iter().map(|r| r.keys_warmed).sum();
                            if total_warmed > 0 {
                                info!(
                                    "Periodic cache warming completed: {} keys warmed",
                                    total_warmed
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Periodic cache warming failed: {}", e);
                        }
                    }
                }
            });

            info!(
                "Started periodic cache warming with {}s interval",
                interval_seconds
            );
        }
    }

    /// Calculate overall cache health status
    fn calculate_overall_health(
        &self,
        performance_report: &crate::cache::metrics::CachePerformanceReport,
        config_validation: &CacheValidationReport,
        invalidation_stats: &crate::cache::invalidation::InvalidationStats,
        warming_stats: &Option<crate::cache::warming::WarmingStats>,
    ) -> CacheHealthStatus {
        // Performance health (40% weight)
        let performance_health = if performance_report.metrics.efficiency_score >= 0.8 {
            100
        } else if performance_report.metrics.efficiency_score >= 0.6 {
            80
        } else if performance_report.metrics.efficiency_score >= 0.4 {
            60
        } else {
            40
        };

        // Configuration health (30% weight)
        let config_health = config_validation.score as u32;

        // Invalidation health (20% weight)
        let invalidation_health = if invalidation_stats.failed_invalidations == 0 {
            100
        } else {
            ((invalidation_stats.total_invalidations - invalidation_stats.failed_invalidations)
                * 100
                / invalidation_stats.total_invalidations.max(1)) as u32
        };

        // Warming health (10% weight)
        let warming_health = if let Some(ref warming_stats) = warming_stats {
            if warming_stats.success_rate >= 0.9 {
                100
            } else if warming_stats.success_rate >= 0.7 {
                80
            } else {
                60
            }
        } else {
            100 // No warming is not a health issue
        };

        // Calculate weighted health score
        let health_score = (performance_health * 40
            + config_health * 30
            + invalidation_health * 20
            + warming_health * 10)
            / 100;

        // Determine health status
        if health_score >= 90 {
            CacheHealthStatus::Excellent
        } else if health_score >= 75 {
            CacheHealthStatus::Good
        } else if health_score >= 60 {
            CacheHealthStatus::Fair
        } else {
            CacheHealthStatus::Poor
        }
    }
}

/// Comprehensive cache health report
#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    /// Overall health status
    pub overall_health: CacheHealthStatus,
    /// Performance report
    pub performance_report: crate::cache::metrics::CachePerformanceReport,
    /// Configuration validation report
    pub config_validation: CacheValidationReport,
    /// Invalidation statistics
    pub invalidation_stats: crate::cache::invalidation::InvalidationStats,
    /// Warming statistics (if available)
    pub warming_stats: Option<crate::cache::warming::WarmingStats>,
    /// Active alerts
    pub alerts: Vec<String>,
    /// Report timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Cache health status
#[derive(Debug, Clone, PartialEq)]
pub enum CacheHealthStatus {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl std::fmt::Display for CacheHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheHealthStatus::Excellent => write!(f, "Excellent"),
            CacheHealthStatus::Good => write!(f, "Good"),
            CacheHealthStatus::Fair => write!(f, "Fair"),
            CacheHealthStatus::Poor => write!(f, "Poor"),
        }
    }
}

impl CacheHealthReport {
    /// Get summary of the health report
    pub fn get_summary(&self) -> String {
        format!(
            "Cache Health Report - {}: {} (Performance: {:.1}%, Config: {}/100)",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.overall_health,
            self.performance_report.metrics.efficiency_score * 100.0,
            self.config_validation.score
        )
    }

    /// Check if cache health is acceptable
    pub fn is_health_acceptable(&self) -> bool {
        self.overall_health != CacheHealthStatus::Poor
    }

    /// Get critical issues (if any)
    pub fn get_critical_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.overall_health == CacheHealthStatus::Poor {
            issues.push("Overall cache health is poor".to_string());
        }

        if !self.config_validation.valid {
            issues.extend(self.config_validation.errors.clone());
        }

        if self.performance_report.metrics.error_rate > 0.05 {
            issues.push(format!(
                "High error rate: {:.2}%",
                self.performance_report.metrics.error_rate * 100.0
            ));
        }

        if self.performance_report.metrics.hit_rate < 0.3 {
            issues.push(format!(
                "Low hit rate: {:.2}%",
                self.performance_report.metrics.hit_rate * 100.0
            ));
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::config::CacheConfig;

    #[tokio::test]
    async fn test_integrated_cache_manager_creation() {
        let cache_config = CacheConfig {
            enabled: false, // Disabled for testing
            ..Default::default()
        };

        let management_config = CacheManagementConfig::default();

        // Test without repositories (should work but without warming)
        let result =
            IntegratedCacheManager::new(cache_config, management_config, None, None, None).await;

        assert!(result.is_ok());
        let manager = result.unwrap();

        assert!(manager.cache_service().health_check().await.unwrap());
        assert!(manager.warming_manager().is_none());
    }

    #[test]
    fn test_cache_management_config_default() {
        let config = CacheManagementConfig::default();
        assert!(config.enable_warming);
        assert!(config.enable_monitoring);
        assert!(config.enable_invalidation);
        assert_eq!(config.environment, "development");
    }

    #[test]
    fn test_cache_health_status_display() {
        assert_eq!(format!("{}", CacheHealthStatus::Excellent), "Excellent");
        assert_eq!(format!("{}", CacheHealthStatus::Good), "Good");
        assert_eq!(format!("{}", CacheHealthStatus::Fair), "Fair");
        assert_eq!(format!("{}", CacheHealthStatus::Poor), "Poor");
    }
}
