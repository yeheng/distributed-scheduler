//! Cache invalidation strategies for the scheduler system
//!
//! This module provides various cache invalidation strategies including
//! time-based, event-based, and dependency-based invalidation.

use super::CacheService;
use crate::cache::{task_cache_key, task_run_cache_key, worker_cache_key};
use scheduler_foundation::SchedulerResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Cache invalidation strategy types
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationStrategy {
    /// Time-based invalidation (TTL)
    TimeBased,
    /// Event-based invalidation (on data changes)
    EventBased,
    /// Dependency-based invalidation (cascade invalidation)
    DependencyBased,
    /// Manual invalidation (explicit invalidation)
    Manual,
    /// Hybrid invalidation (combination of strategies)
    Hybrid,
}

/// Cache invalidation event types
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationEvent {
    /// Task created or updated
    TaskChanged { task_id: i64, task_name: String },
    /// Task deleted
    TaskDeleted { task_id: i64, task_name: String },
    /// Worker registered or updated
    WorkerChanged { worker_id: String },
    /// Worker unregistered
    WorkerDeleted { worker_id: String },
    /// Task run created or updated
    TaskRunChanged { task_run_id: i64, task_id: i64 },
    /// Task run deleted
    TaskRunDeleted { task_run_id: i64, task_id: i64 },
    /// System configuration changed
    SystemConfigChanged,
    /// Dependencies changed
    DependenciesChanged { task_id: i64 },
    /// Batch operation completed
    BatchOperationCompleted { operation_type: String },
    /// Custom event
    Custom {
        event_type: String,
        data: HashMap<String, String>,
    },
}

/// Cache invalidation rule
#[derive(Debug, Clone)]
pub struct InvalidationRule {
    /// Rule name
    pub name: String,
    /// Strategy type
    pub strategy: InvalidationStrategy,
    /// Event types that trigger this rule
    pub trigger_events: Vec<InvalidationEvent>,
    /// Cache keys/prefixes to invalidate
    pub targets: Vec<InvalidationTarget>,
    /// Whether this rule is active
    pub active: bool,
    /// Priority (higher numbers = higher priority)
    pub priority: u32,
    /// Custom conditions (if any)
    pub conditions: Option<String>,
}

/// Cache invalidation target
#[derive(Debug, Clone)]
pub enum InvalidationTarget {
    /// Single cache key
    Key(String),
    /// Cache prefix (all keys starting with this prefix)
    Prefix(String),
    /// Pattern-based key matching
    Pattern(String),
    /// Task-related cache
    Task { task_id: i64 },
    /// Worker-related cache
    Worker { worker_id: String },
    /// Task run-related cache
    TaskRun { task_run_id: i64, task_id: i64 },
    /// System-wide cache
    System,
}

/// Cache invalidation result
#[derive(Debug, Clone)]
pub struct InvalidationResult {
    /// Rule that was applied
    pub rule_name: String,
    /// Event that triggered invalidation
    pub event: InvalidationEvent,
    /// Number of keys invalidated
    pub keys_invalidated: usize,
    /// Time taken for invalidation
    pub duration_ms: u64,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Cache invalidation manager
pub struct CacheInvalidationManager {
    /// Cache service
    cache_service: Arc<dyn CacheService>,
    /// Invalidation rules
    rules: Vec<InvalidationRule>,
    /// Event history for debugging
    event_history: Arc<RwLock<Vec<InvalidationEvent>>>,
    /// Statistics
    stats: Arc<RwLock<InvalidationStats>>,
    /// Maximum event history size
    max_history_size: usize,
}

/// Cache invalidation statistics
#[derive(Debug, Clone, Default)]
pub struct InvalidationStats {
    /// Total invalidations performed
    pub total_invalidations: u64,
    /// Keys invalidated
    pub keys_invalidated: u64,
    /// Failed invalidations
    pub failed_invalidations: u64,
    /// Average invalidation time (milliseconds)
    pub avg_invalidation_time_ms: f64,
    /// Invalidation by event type
    pub by_event_type: HashMap<String, u64>,
    /// Last invalidation time
    pub last_invalidation: Option<Instant>,
}

impl CacheInvalidationManager {
    /// Create new cache invalidation manager
    pub fn new(cache_service: Arc<dyn CacheService>) -> Self {
        let mut manager = Self {
            cache_service,
            rules: Vec::new(),
            event_history: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(InvalidationStats::default())),
            max_history_size: 1000,
        };

        // Initialize with default rules
        manager.initialize_default_rules();
        manager
    }

    /// Initialize default invalidation rules
    fn initialize_default_rules(&mut self) {
        // Task-related invalidation rules
        self.rules.push(InvalidationRule {
            name: "task_updated".to_string(),
            strategy: InvalidationStrategy::EventBased,
            trigger_events: vec![InvalidationEvent::TaskChanged {
                task_id: 0,
                task_name: "".to_string(),
            }],
            targets: vec![
                InvalidationTarget::Task { task_id: 0 }, // Will be replaced with actual task_id
                InvalidationTarget::Prefix("task".to_string()),
                InvalidationTarget::Prefix("dependencies".to_string()),
            ],
            active: true,
            priority: 10,
            conditions: None,
        });

        // Worker-related invalidation rules
        self.rules.push(InvalidationRule {
            name: "worker_updated".to_string(),
            strategy: InvalidationStrategy::EventBased,
            trigger_events: vec![InvalidationEvent::WorkerChanged {
                worker_id: "".to_string(),
            }],
            targets: vec![
                InvalidationTarget::Worker {
                    worker_id: "".to_string(),
                }, // Will be replaced
                InvalidationTarget::Prefix("worker".to_string()),
            ],
            active: true,
            priority: 10,
            conditions: None,
        });

        // Task run-related invalidation rules
        self.rules.push(InvalidationRule {
            name: "task_run_updated".to_string(),
            strategy: InvalidationStrategy::EventBased,
            trigger_events: vec![InvalidationEvent::TaskRunChanged {
                task_run_id: 0,
                task_id: 0,
            }],
            targets: vec![
                InvalidationTarget::TaskRun {
                    task_run_id: 0,
                    task_id: 0,
                },
                InvalidationTarget::Prefix("task_run".to_string()),
            ],
            active: true,
            priority: 10,
            conditions: None,
        });

        // System-wide invalidation rule
        self.rules.push(InvalidationRule {
            name: "system_config_changed".to_string(),
            strategy: InvalidationStrategy::EventBased,
            trigger_events: vec![InvalidationEvent::SystemConfigChanged],
            targets: vec![
                InvalidationTarget::Prefix("system_stats".to_string()),
                InvalidationTarget::Prefix("config".to_string()),
            ],
            active: true,
            priority: 20,
            conditions: None,
        });

        // Dependency invalidation rule
        self.rules.push(InvalidationRule {
            name: "dependencies_changed".to_string(),
            strategy: InvalidationStrategy::DependencyBased,
            trigger_events: vec![InvalidationEvent::DependenciesChanged { task_id: 0 }],
            targets: vec![
                InvalidationTarget::Task { task_id: 0 },
                InvalidationTarget::Prefix("dependencies".to_string()),
            ],
            active: true,
            priority: 15,
            conditions: None,
        });
    }

    /// Add custom invalidation rule
    pub fn add_rule(&mut self, rule: InvalidationRule) {
        let rule_name = rule.name.clone();
        self.rules.push(rule);
        info!("Added cache invalidation rule: {}", rule_name);
    }

    /// Remove invalidation rule by name
    pub fn remove_rule(&mut self, rule_name: &str) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|rule| rule.name != rule_name);
        let removed = self.rules.len() < initial_len;

        if removed {
            info!("Removed cache invalidation rule: {}", rule_name);
        }

        removed
    }

    /// Invalidate cache based on event
    #[instrument(skip(self))]
    pub async fn invalidate_on_event(
        &self,
        event: InvalidationEvent,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let start_time = Instant::now();

        // Record event in history
        self.record_event(event.clone()).await;

        // Find matching rules
        let matching_rules: Vec<&InvalidationRule> = self
            .rules
            .iter()
            .filter(|rule| rule.active && self.event_matches_rule(&event, rule))
            .collect();

        if matching_rules.is_empty() {
            debug!("No invalidation rules found for event: {:?}", event);
            return Ok(Vec::new());
        }

        // Sort by priority (highest first)
        let mut sorted_rules = matching_rules;
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut results = Vec::new();

        for rule in sorted_rules {
            match self.apply_rule(rule, &event).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Failed to apply invalidation rule '{}': {}", rule.name, e);
                    // Continue with other rules
                }
            }
        }

        // Update statistics
        self.update_stats(&results, start_time.elapsed()).await;

        Ok(results)
    }

    /// Check if event matches rule
    fn event_matches_rule(&self, event: &InvalidationEvent, rule: &InvalidationRule) -> bool {
        rule.trigger_events
            .iter()
            .any(|trigger_event| self.events_match(event, trigger_event))
    }

    /// Check if two events match (pattern matching)
    fn events_match(&self, event: &InvalidationEvent, pattern: &InvalidationEvent) -> bool {
        match (event, pattern) {
            (
                InvalidationEvent::TaskChanged {
                    task_id: _,
                    task_name: _,
                },
                InvalidationEvent::TaskChanged {
                    task_id: _,
                    task_name: _,
                },
            ) => true, // Any task change matches the pattern
            (
                InvalidationEvent::WorkerChanged { worker_id: _ },
                InvalidationEvent::WorkerChanged { worker_id: _ },
            ) => true, // Any worker change matches the pattern
            (
                InvalidationEvent::TaskRunChanged {
                    task_run_id: _,
                    task_id: _,
                },
                InvalidationEvent::TaskRunChanged {
                    task_run_id: _,
                    task_id: _,
                },
            ) => true, // Any task run change matches the pattern
            (InvalidationEvent::SystemConfigChanged, InvalidationEvent::SystemConfigChanged) => {
                true
            }
            (
                InvalidationEvent::DependenciesChanged { task_id: _ },
                InvalidationEvent::DependenciesChanged { task_id: _ },
            ) => true,
            (a, b) => a == b,
        }
    }

    /// Apply invalidation rule
    async fn apply_rule(
        &self,
        rule: &InvalidationRule,
        event: &InvalidationEvent,
    ) -> SchedulerResult<InvalidationResult> {
        let start_time = Instant::now();
        let mut keys_invalidated = 0;
        let mut errors = Vec::new();

        for target in &rule.targets {
            let resolved_targets = self.resolve_target(target, event);

            for resolved_target in resolved_targets {
                match self.invalidate_target(&resolved_target).await {
                    Ok(count) => keys_invalidated += count,
                    Err(e) => errors.push(format!(
                        "Failed to invalidate target '{resolved_target}': {e}"
                    )),
                }
            }
        }

        Ok(InvalidationResult {
            rule_name: rule.name.clone(),
            event: event.clone(),
            keys_invalidated,
            duration_ms: start_time.elapsed().as_millis() as u64,
            errors,
        })
    }

    /// Resolve invalidation target with event data
    fn resolve_target(
        &self,
        target: &InvalidationTarget,
        event: &InvalidationEvent,
    ) -> Vec<String> {
        match target {
            InvalidationTarget::Key(key) => vec![key.clone()],
            InvalidationTarget::Prefix(prefix) => vec![prefix.clone()],
            InvalidationTarget::Pattern(pattern) => vec![pattern.clone()],
            InvalidationTarget::Task { task_id } => {
                if let InvalidationEvent::TaskChanged {
                    task_id: event_task_id,
                    ..
                } = event
                {
                    vec![task_cache_key(*event_task_id)]
                } else {
                    vec![task_cache_key(*task_id)]
                }
            }
            InvalidationTarget::Worker { worker_id } => {
                if let InvalidationEvent::WorkerChanged {
                    worker_id: event_worker_id,
                } = event
                {
                    vec![worker_cache_key(event_worker_id)]
                } else {
                    vec![worker_cache_key(worker_id)]
                }
            }
            InvalidationTarget::TaskRun {
                task_run_id,
                task_id,
            } => {
                if let InvalidationEvent::TaskRunChanged {
                    task_run_id: event_task_run_id,
                    ..
                } = event
                {
                    vec![
                        task_run_cache_key(*event_task_run_id),
                        format!("task_runs:task:{}", event_task_run_id),
                    ]
                } else {
                    vec![
                        task_run_cache_key(*task_run_id),
                        format!("task_runs:task:{}", task_id),
                    ]
                }
            }
            InvalidationTarget::System => vec![
                "system_stats".to_string(),
                "config".to_string(),
                "workers:alive".to_string(),
                "workers:list".to_string(),
            ],
        }
    }

    /// Invalidate specific target
    async fn invalidate_target(&self, target: &str) -> SchedulerResult<usize> {
        if target.contains('*') || target.contains('?') {
            // Pattern-based invalidation (treat as prefix)
            let prefix = target.split('*').next().unwrap_or(target);
            self.cache_service.clear_prefix(prefix).await
        } else if target.ends_with(':') {
            // Prefix-based invalidation
            self.cache_service.clear_prefix(target).await
        } else {
            // Single key invalidation
            let deleted = self.cache_service.delete(target).await?;
            Ok(if deleted { 1 } else { 0 })
        }
    }

    /// Record event in history
    async fn record_event(&self, event: InvalidationEvent) {
        let mut history = self.event_history.write().await;
        history.push(event);

        // Trim history if needed
        if history.len() > self.max_history_size {
            history.remove(0);
        }
    }

    /// Update statistics
    async fn update_stats(&self, results: &[InvalidationResult], duration: Duration) {
        let mut stats = self.stats.write().await;

        stats.total_invalidations += results.len() as u64;
        stats.keys_invalidated += results.iter().map(|r| r.keys_invalidated).sum::<usize>() as u64;
        stats.failed_invalidations +=
            results.iter().filter(|r| !r.errors.is_empty()).count() as u64;

        let total_time_ms = duration.as_millis() as f64;
        let avg_time = if results.is_empty() {
            0.0
        } else {
            total_time_ms / results.len() as f64
        };
        stats.avg_invalidation_time_ms = (stats.avg_invalidation_time_ms
            * (stats.total_invalidations - results.len() as u64) as f64
            + avg_time * results.len() as f64)
            / stats.total_invalidations as f64;

        stats.last_invalidation = Some(Instant::now());

        // Update by event type
        for result in results {
            let event_type = self.get_event_type_name(&result.event);
            *stats.by_event_type.entry(event_type).or_insert(0) += 1;
        }
    }

    /// Get event type name for statistics
    fn get_event_type_name(&self, event: &InvalidationEvent) -> String {
        match event {
            InvalidationEvent::TaskChanged { .. } => "task_changed".to_string(),
            InvalidationEvent::TaskDeleted { .. } => "task_deleted".to_string(),
            InvalidationEvent::WorkerChanged { .. } => "worker_changed".to_string(),
            InvalidationEvent::WorkerDeleted { .. } => "worker_deleted".to_string(),
            InvalidationEvent::TaskRunChanged { .. } => "task_run_changed".to_string(),
            InvalidationEvent::TaskRunDeleted { .. } => "task_run_deleted".to_string(),
            InvalidationEvent::SystemConfigChanged => "system_config_changed".to_string(),
            InvalidationEvent::DependenciesChanged { .. } => "dependencies_changed".to_string(),
            InvalidationEvent::BatchOperationCompleted { .. } => {
                "batch_operation_completed".to_string()
            }
            InvalidationEvent::Custom { event_type, .. } => event_type.clone(),
        }
    }

    /// Get invalidation statistics
    pub async fn get_stats(&self) -> InvalidationStats {
        self.stats.read().await.clone()
    }

    /// Get event history
    pub async fn get_event_history(&self, limit: Option<usize>) -> Vec<InvalidationEvent> {
        let history = self.event_history.read().await;
        match limit {
            Some(limit) => history.iter().rev().take(limit).cloned().collect(),
            None => history.clone(),
        }
    }

    /// Manual cache invalidation by prefix
    pub async fn invalidate_prefix(&self, prefix: &str) -> SchedulerResult<usize> {
        self.cache_service.clear_prefix(prefix).await
    }

    /// Manual cache invalidation by key
    pub async fn invalidate_key(&self, key: &str) -> SchedulerResult<bool> {
        self.cache_service.delete(key).await
    }

    /// Clear all cache
    pub async fn invalidate_all(&self) -> SchedulerResult<usize> {
        let mut total_invalidated = 0;
        let prefixes = ["task", "worker", "system_stats", "task_run", "dependencies"];

        for prefix in &prefixes {
            match self.invalidate_prefix(prefix).await {
                Ok(count) => total_invalidated += count,
                Err(e) => warn!("Failed to invalidate prefix '{}': {}", prefix, e),
            }
        }

        Ok(total_invalidated)
    }

    /// Get active rules
    pub fn get_active_rules(&self) -> Vec<&InvalidationRule> {
        self.rules.iter().filter(|rule| rule.active).collect()
    }

    /// Set rule active state
    pub fn set_rule_active(&mut self, rule_name: &str, active: bool) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|rule| rule.name == rule_name) {
            rule.active = active;
            info!(
                "Set invalidation rule '{}' active state to: {}",
                rule_name, active
            );
            true
        } else {
            false
        }
    }
}

/// Convenience functions for common invalidation scenarios
pub mod convenience {
    use super::*;

    /// Invalidate task-related cache
    pub async fn invalidate_task_cache(
        manager: &CacheInvalidationManager,
        task_id: i64,
        task_name: &str,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let event = InvalidationEvent::TaskChanged {
            task_id,
            task_name: task_name.to_string(),
        };
        manager.invalidate_on_event(event).await
    }

    /// Invalidate worker-related cache
    pub async fn invalidate_worker_cache(
        manager: &CacheInvalidationManager,
        worker_id: &str,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let event = InvalidationEvent::WorkerChanged {
            worker_id: worker_id.to_string(),
        };
        manager.invalidate_on_event(event).await
    }

    /// Invalidate task run-related cache
    pub async fn invalidate_task_run_cache(
        manager: &CacheInvalidationManager,
        task_run_id: i64,
        task_id: i64,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let event = InvalidationEvent::TaskRunChanged {
            task_run_id,
            task_id,
        };
        manager.invalidate_on_event(event).await
    }

    /// Invalidate system-wide cache
    pub async fn invalidate_system_cache(
        manager: &CacheInvalidationManager,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let event = InvalidationEvent::SystemConfigChanged;
        manager.invalidate_on_event(event).await
    }

    /// Invalidate dependencies cache
    pub async fn invalidate_dependencies_cache(
        manager: &CacheInvalidationManager,
        task_id: i64,
    ) -> SchedulerResult<Vec<InvalidationResult>> {
        let event = InvalidationEvent::DependenciesChanged { task_id };
        manager.invalidate_on_event(event).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::cache::config::CacheConfig;
    use crate::cache::manager::RedisCacheManager;
    use crate::cache::CacheStats;

    #[tokio::test]
    async fn test_invalidation_manager_creation() {
        // Create a mock cache service for testing
        let config = CacheConfig {
            enabled: false, // Disabled for testing
            ..Default::default()
        };

        let cache_manager = RedisCacheManager::new(config).await;
        let cache_service = if let Ok(manager) = cache_manager {
            Arc::new(manager) as Arc<dyn CacheService>
        } else {
            // Create a mock cache service for testing
            use tokio::sync::Notify;

            struct MockCacheService {
                notify: Arc<Notify>,
            }

            #[async_trait]
            impl CacheService for MockCacheService {
                async fn get(&self, _key: &str) -> SchedulerResult<Option<Vec<u8>>> {
                    Ok(None)
                }

                async fn set(
                    &self,
                    _key: &str,
                    _value: &[u8],
                    _ttl: Duration,
                ) -> SchedulerResult<()> {
                    Ok(())
                }

                async fn delete(&self, _key: &str) -> SchedulerResult<bool> {
                    Ok(false)
                }

                async fn exists(&self, _key: &str) -> SchedulerResult<bool> {
                    Ok(false)
                }

                async fn get_stats(&self) -> CacheStats {
                    CacheStats::default()
                }

                async fn clear_prefix(&self, _prefix: &str) -> SchedulerResult<usize> {
                    Ok(0)
                }

                async fn health_check(&self) -> SchedulerResult<bool> {
                    Ok(true)
                }
            }

            Arc::new(MockCacheService {
                notify: Arc::new(Notify::new()),
            }) as Arc<dyn CacheService>
        };

        let manager = CacheInvalidationManager::new(cache_service);
        assert!(!manager.get_active_rules().is_empty());
    }

    #[test]
    fn test_event_matching() {
        // DEBUG: 验证假设1 - 检查是否使用了正确的CacheService实现
        // 这里使用MockCacheService而不是空的Mutex，因为Mutex<()>不实现CacheService trait
        debug!("Creating CacheInvalidationManager for test_event_matching");
        
        // 创建一个合适的mock服务而不是空的Mutex
        use tokio::sync::Notify;
        
        struct MockCacheService {
            notify: Arc<Notify>,
        }
        
        #[async_trait]
        impl CacheService for MockCacheService {
            async fn get(&self, _key: &str) -> SchedulerResult<Option<Vec<u8>>> {
                debug!("MockCacheService::get called");
                Ok(None)
            }
            
            async fn set(&self, _key: &str, _value: &[u8], _ttl: Duration) -> SchedulerResult<()> {
                debug!("MockCacheService::set called");
                Ok(())
            }
            
            async fn delete(&self, _key: &str) -> SchedulerResult<bool> {
                debug!("MockCacheService::delete called");
                Ok(false)
            }
            
            async fn exists(&self, _key: &str) -> SchedulerResult<bool> {
                debug!("MockCacheService::exists called");
                Ok(false)
            }
            
            async fn get_stats(&self) -> CacheStats {
                debug!("MockCacheService::get_stats called");
                CacheStats::default()
            }
            
            async fn clear_prefix(&self, _prefix: &str) -> SchedulerResult<usize> {
                debug!("MockCacheService::clear_prefix called");
                Ok(0)
            }
            
            async fn health_check(&self) -> SchedulerResult<bool> {
                debug!("MockCacheService::health_check called");
                Ok(true)
            }
        }
        
        let cache_service = Arc::new(MockCacheService {
            notify: Arc::new(Notify::new()),
        });
        
        debug!("Successfully created CacheInvalidationManager with proper CacheService implementation");
        let manager = CacheInvalidationManager::new(cache_service);

        let event1 = InvalidationEvent::TaskChanged {
            task_id: 123,
            task_name: "test_task".to_string(),
        };

        let pattern = InvalidationEvent::TaskChanged {
            task_id: 0,
            task_name: "".to_string(),
        };

        assert!(manager.events_match(&event1, &pattern));
    }

    #[test]
    fn test_target_resolution() {
        // DEBUG: 验证假设1 - 检查是否使用了正确的CacheService实现
        debug!("Creating CacheInvalidationManager for test_target_resolution");
        
        // 使用与test_event_matching相同的MockCacheService实现
        use tokio::sync::Notify;
        
        struct MockCacheService {
            notify: Arc<Notify>,
        }
        
        #[async_trait]
        impl CacheService for MockCacheService {
            async fn get(&self, _key: &str) -> SchedulerResult<Option<Vec<u8>>> {
                debug!("MockCacheService::get called");
                Ok(None)
            }
            
            async fn set(&self, _key: &str, _value: &[u8], _ttl: Duration) -> SchedulerResult<()> {
                debug!("MockCacheService::set called");
                Ok(())
            }
            
            async fn delete(&self, _key: &str) -> SchedulerResult<bool> {
                debug!("MockCacheService::delete called");
                Ok(false)
            }
            
            async fn exists(&self, _key: &str) -> SchedulerResult<bool> {
                debug!("MockCacheService::exists called");
                Ok(false)
            }
            
            async fn get_stats(&self) -> CacheStats {
                debug!("MockCacheService::get_stats called");
                CacheStats::default()
            }
            
            async fn clear_prefix(&self, _prefix: &str) -> SchedulerResult<usize> {
                debug!("MockCacheService::clear_prefix called");
                Ok(0)
            }
            
            async fn health_check(&self) -> SchedulerResult<bool> {
                debug!("MockCacheService::health_check called");
                Ok(true)
            }
        }
        
        let cache_service = Arc::new(MockCacheService {
            notify: Arc::new(Notify::new()),
        });
        
        debug!("Successfully created CacheInvalidationManager with proper CacheService implementation");
        let manager = CacheInvalidationManager::new(cache_service);

        let event = InvalidationEvent::TaskChanged {
            task_id: 123,
            task_name: "test_task".to_string(),
        };

        let target = InvalidationTarget::Task { task_id: 0 };
        let resolved = manager.resolve_target(&target, &event);

        assert_eq!(resolved, vec!["task:123"]);
    }
}
