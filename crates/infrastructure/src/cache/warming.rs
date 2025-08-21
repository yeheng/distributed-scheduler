//! Cache warming procedures for the scheduler system
//!
//! This module provides cache warming strategies to proactively load
//! frequently accessed data into cache to improve performance.

use super::{CacheService, CacheServiceExt};
use crate::cache::{task_cache_key, task_name_cache_key, worker_cache_key};
use scheduler_domain::entities::*;
use scheduler_domain::repositories::*;
use scheduler_errors::SchedulerResult;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Cache warming strategy types
#[derive(Debug, Clone, PartialEq)]
pub enum WarmingStrategy {
    /// Full cache warming (load all data)
    Full,
    /// Selective cache warming (load only important data)
    Selective,
    /// Predictive cache warming (load based on access patterns)
    Predictive,
    /// On-demand cache warming (load when needed)
    OnDemand,
    /// Hybrid cache warming (combination of strategies)
    Hybrid,
}

/// Cache warming priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarmingPriority {
    /// Low priority (nice to have)
    Low,
    /// Medium priority (should be cached)
    Medium,
    /// High priority (must be cached)
    High,
    /// Critical priority (essential for performance)
    Critical,
}

/// Cache warming configuration
#[derive(Debug, Clone)]
pub struct WarmingConfig {
    /// Whether cache warming is enabled
    pub enabled: bool,
    /// Warming strategy to use
    pub strategy: WarmingStrategy,
    /// Warm-up interval (how often to warm cache)
    pub warm_up_interval: Duration,
    /// Maximum concurrent warming operations
    pub max_concurrent_warming: usize,
    /// Timeout for individual warming operations
    pub warming_timeout: Duration,
    /// Whether to warm cache on startup
    pub warm_on_startup: bool,
    /// Priority thresholds for different data types
    pub priority_thresholds: HashMap<String, WarmingPriority>,
}

impl Default for WarmingConfig {
    fn default() -> Self {
        let mut priority_thresholds = HashMap::new();
        priority_thresholds.insert("active_tasks".to_string(), WarmingPriority::High);
        priority_thresholds.insert("alive_workers".to_string(), WarmingPriority::High);
        priority_thresholds.insert("system_stats".to_string(), WarmingPriority::Medium);
        priority_thresholds.insert("schedulable_tasks".to_string(), WarmingPriority::Medium);
        priority_thresholds.insert("task_dependencies".to_string(), WarmingPriority::Low);

        Self {
            enabled: true,
            strategy: WarmingStrategy::Selective,
            warm_up_interval: Duration::from_secs(300), // 5 minutes
            max_concurrent_warming: 5,
            warming_timeout: Duration::from_secs(30),
            warm_on_startup: true,
            priority_thresholds,
        }
    }
}

/// Cache warming job definition
#[derive(Debug, Clone)]
pub struct WarmingJob {
    /// Job name
    pub name: String,
    /// Job description
    pub description: String,
    /// Priority level
    pub priority: WarmingPriority,
    /// Cache keys to warm
    pub cache_keys: Vec<String>,
    /// TTL for warmed cache entries
    pub ttl: Duration,
    /// Dependencies (other jobs that must complete first)
    pub dependencies: Vec<String>,
    /// Whether this job is enabled
    pub enabled: bool,
}

/// Cache warming result
#[derive(Debug, Clone)]
pub struct WarmingResult {
    /// Job name
    pub job_name: String,
    /// Number of keys warmed
    pub keys_warmed: usize,
    /// Time taken for warming
    pub duration_ms: u64,
    /// Any errors that occurred
    pub errors: Vec<String>,
    /// Whether the job was successful
    pub success: bool,
}

/// Cache warming statistics
#[derive(Debug, Clone, Default)]
pub struct WarmingStats {
    /// Total warming operations performed
    pub total_warming_operations: u64,
    /// Keys successfully warmed
    pub keys_warmed: u64,
    /// Failed warming operations
    pub failed_warming_operations: u64,
    /// Average warming time (milliseconds)
    pub avg_warming_time_ms: f64,
    /// Warming by priority
    pub by_priority: HashMap<WarmingPriority, u64>,
    /// Last warming time
    pub last_warming_time: Option<Instant>,
    /// Warming success rate
    pub success_rate: f64,
}

/// Cache warming manager
pub struct CacheWarmingManager {
    /// Cache service
    cache_service: Arc<dyn CacheService>,
    /// Repository interfaces
    task_repository: Arc<dyn TaskRepository>,
    worker_repository: Arc<dyn WorkerRepository>,
    task_run_repository: Arc<dyn TaskRunRepository>,
    /// Warming configuration
    config: WarmingConfig,
    /// Warming jobs
    jobs: Vec<WarmingJob>,
    /// Statistics
    stats: Arc<RwLock<WarmingStats>>,
    /// Currently running jobs
    running_jobs: Arc<RwLock<HashSet<String>>>,
}

impl CacheWarmingManager {
    /// Create new cache warming manager
    pub fn new(
        cache_service: Arc<dyn CacheService>,
        task_repository: Arc<dyn TaskRepository>,
        worker_repository: Arc<dyn WorkerRepository>,
        task_run_repository: Arc<dyn TaskRunRepository>,
        config: WarmingConfig,
    ) -> Self {
        let mut manager = Self {
            cache_service,
            task_repository,
            worker_repository,
            task_run_repository,
            config,
            jobs: Vec::new(),
            stats: Arc::new(RwLock::new(WarmingStats::default())),
            running_jobs: Arc::new(RwLock::new(HashSet::new())),
        };

        // Initialize default warming jobs
        manager.initialize_default_jobs();
        manager
    }

    /// Initialize default warming jobs
    fn initialize_default_jobs(&mut self) {
        // Active tasks warming job
        self.jobs.push(WarmingJob {
            name: "active_tasks".to_string(),
            description: "Warm cache for all active tasks".to_string(),
            priority: WarmingPriority::High,
            cache_keys: vec!["active_tasks".to_string()],
            ttl: Duration::from_secs(300),
            dependencies: vec![],
            enabled: true,
        });

        // Alive workers warming job
        self.jobs.push(WarmingJob {
            name: "alive_workers".to_string(),
            description: "Warm cache for all alive workers".to_string(),
            priority: WarmingPriority::High,
            cache_keys: vec!["alive_workers".to_string()],
            ttl: Duration::from_secs(60),
            dependencies: vec![],
            enabled: true,
        });

        // System stats warming job
        self.jobs.push(WarmingJob {
            name: "system_stats".to_string(),
            description: "Warm cache for system statistics".to_string(),
            priority: WarmingPriority::Medium,
            cache_keys: vec!["system_stats".to_string()],
            ttl: Duration::from_secs(120),
            dependencies: vec![],
            enabled: true,
        });

        // Schedulable tasks warming job
        self.jobs.push(WarmingJob {
            name: "schedulable_tasks".to_string(),
            description: "Warm cache for schedulable tasks".to_string(),
            priority: WarmingPriority::Medium,
            cache_keys: vec!["schedulable_tasks".to_string()],
            ttl: Duration::from_secs(60),
            dependencies: vec!["active_tasks".to_string()],
            enabled: true,
        });

        // High-priority tasks warming job
        self.jobs.push(WarmingJob {
            name: "high_priority_tasks".to_string(),
            description: "Warm cache for high-priority tasks".to_string(),
            priority: WarmingPriority::High,
            cache_keys: vec![],
            ttl: Duration::from_secs(600),
            dependencies: vec!["active_tasks".to_string()],
            enabled: true,
        });
    }

    /// Add custom warming job
    pub fn add_job(&mut self, job: WarmingJob) {
        let job_name = job.name.clone();
        self.jobs.push(job);
        info!("Added cache warming job: {}", job_name);
    }

    /// Remove warming job by name
    pub fn remove_job(&mut self, job_name: &str) -> bool {
        let initial_len = self.jobs.len();
        self.jobs.retain(|job| job.name != job_name);
        let removed = self.jobs.len() < initial_len;

        if removed {
            info!("Removed cache warming job: {}", job_name);
        }

        removed
    }

    /// Warm up cache on startup
    #[instrument(skip(self))]
    pub async fn warm_up_on_startup(&self) -> SchedulerResult<Vec<WarmingResult>> {
        if !self.config.enabled || !self.config.warm_on_startup {
            info!("Cache warming is disabled or not configured for startup");
            return Ok(Vec::new());
        }

        info!("Starting cache warm-up on startup");

        // Filter enabled jobs
        let enabled_jobs: Vec<&WarmingJob> = self.jobs.iter().filter(|job| job.enabled).collect();

        // Sort by priority (highest first)
        let mut sorted_jobs = enabled_jobs;
        sorted_jobs.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut results = Vec::new();
        let mut completed_jobs = HashSet::new();

        for job in sorted_jobs {
            // Check if dependencies are satisfied
            if self.are_dependencies_satisfied(job, &completed_jobs) {
                match self.execute_warming_job(job).await {
                    Ok(result) => {
                        results.push(result);
                        completed_jobs.insert(job.name.clone());
                    }
                    Err(e) => {
                        warn!("Failed to execute warming job '{}': {}", job.name, e);
                        // Continue with other jobs
                    }
                }
            } else {
                debug!(
                    "Skipping job '{}' due to unsatisfied dependencies",
                    job.name
                );
            }
        }

        info!("Cache warm-up completed: {} jobs executed", results.len());
        Ok(results)
    }

    /// Execute scheduled warming
    #[instrument(skip(self))]
    pub async fn execute_scheduled_warming(&self) -> SchedulerResult<Vec<WarmingResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        debug!("Executing scheduled cache warming");

        // Get jobs that meet priority threshold
        let threshold_jobs: Vec<&WarmingJob> = self
            .jobs
            .iter()
            .filter(|job| job.enabled && self.meets_priority_threshold(&job.name))
            .collect();

        let mut results = Vec::new();

        for job in threshold_jobs {
            // Check if job is not already running
            if !self.is_job_running(&job.name).await {
                match self.execute_warming_job(job).await {
                    Ok(result) => results.push(result),
                    Err(e) => warn!(
                        "Failed to execute scheduled warming job '{}': {}",
                        job.name, e
                    ),
                }
            }
        }

        Ok(results)
    }

    /// Execute single warming job
    async fn execute_warming_job(&self, job: &WarmingJob) -> SchedulerResult<WarmingResult> {
        let start_time = Instant::now();
        let job_name = job.name.clone();

        // Mark job as running
        self.mark_job_running(&job_name).await;

        debug!("Executing warming job: {}", job_name);

        let result = match job_name.as_str() {
            "active_tasks" => self.warm_active_tasks(job).await,
            "alive_workers" => self.warm_alive_workers(job).await,
            "system_stats" => self.warm_system_stats(job).await,
            "schedulable_tasks" => self.warm_schedulable_tasks(job).await,
            "high_priority_tasks" => self.warm_high_priority_tasks(job).await,
            _ => self.warm_custom_job(job).await,
        };

        // Mark job as not running
        self.mark_job_not_running(&job_name).await;

        match result {
            Ok(keys_warmed) => {
                let warming_result = WarmingResult {
                    job_name: job.name.clone(),
                    keys_warmed,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors: Vec::new(),
                    success: true,
                };

                self.update_stats(&warming_result).await;
                Ok(warming_result)
            }
            Err(e) => {
                let warming_result = WarmingResult {
                    job_name: job.name.clone(),
                    keys_warmed: 0,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors: vec![e.to_string()],
                    success: false,
                };

                self.update_stats(&warming_result).await;
                Ok(warming_result)
            }
        }
    }

    /// Warm active tasks cache
    async fn warm_active_tasks(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        let tasks = self.task_repository.get_active_tasks().await?;
        let mut keys_warmed = 0;

        for task in tasks {
            let cache_key = task_cache_key(task.id);
            self.cache_service
                .set_typed(&cache_key, &task, job.ttl)
                .await?;
            keys_warmed += 1;

            // Also cache by name if available
            if !task.name.is_empty() {
                let name_cache_key = task_name_cache_key(&task.name);
                self.cache_service
                    .set_typed(&name_cache_key, &task, job.ttl)
                    .await?;
                keys_warmed += 1;
            }
        }

        debug!("Warmed {} active tasks cache entries", keys_warmed);
        Ok(keys_warmed)
    }

    /// Warm alive workers cache
    async fn warm_alive_workers(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        let workers = self.worker_repository.get_alive_workers().await?;
        let mut keys_warmed = 0;

        for worker in workers {
            let cache_key = worker_cache_key(&worker.id);
            self.cache_service
                .set_typed(&cache_key, &worker, job.ttl)
                .await?;
            keys_warmed += 1;
        }

        debug!("Warmed {} alive workers cache entries", keys_warmed);
        Ok(keys_warmed)
    }

    /// Warm system stats cache
    async fn warm_system_stats(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        // For system stats, we'll create a basic stats object
        let stats = SystemStats {
            total_tasks: 0,
            active_tasks: 0,
            alive_workers: 0,
            total_workers: 0,
            timestamp: chrono::Utc::now(),
        };

        let cache_key = "system_stats:current";
        self.cache_service
            .set_typed(cache_key, &stats, job.ttl)
            .await?;

        debug!("Warmed system stats cache");
        Ok(1)
    }

    /// Warm schedulable tasks cache
    async fn warm_schedulable_tasks(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        let current_time = chrono::Utc::now();
        let tasks = self
            .task_repository
            .get_schedulable_tasks(current_time)
            .await?;
        let mut keys_warmed = 0;

        // Cache with timestamp-based key
        let cache_key = format!("schedulable:{}", current_time.timestamp());
        self.cache_service
            .set_typed(&cache_key, &tasks, job.ttl)
            .await?;
        keys_warmed += 1;

        debug!("Warmed {} schedulable tasks cache entries", tasks.len());
        Ok(keys_warmed)
    }

    /// Warm high-priority tasks cache
    async fn warm_high_priority_tasks(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            ..Default::default()
        };

        let tasks = self.task_repository.list(&filter).await?;
        let mut keys_warmed = 0;

        for task in tasks {
            let cache_key = task_cache_key(task.id);
            self.cache_service
                .set_typed(&cache_key, &task, job.ttl)
                .await?;
            keys_warmed += 1;

            if !task.name.is_empty() {
                let name_cache_key = task_name_cache_key(&task.name);
                self.cache_service
                    .set_typed(&name_cache_key, &task, job.ttl)
                    .await?;
                keys_warmed += 1;
            }
        }

        debug!("Warmed {} high-priority tasks cache entries", keys_warmed);
        Ok(keys_warmed)
    }

    /// Warm custom job
    async fn warm_custom_job(&self, job: &WarmingJob) -> SchedulerResult<usize> {
        let mut keys_warmed = 0;

        // For custom jobs, we just set empty values for the specified keys
        for cache_key in &job.cache_keys {
            let placeholder = "cache_warmed".to_string();
            self.cache_service
                .set_typed(cache_key, &placeholder, job.ttl)
                .await?;
            keys_warmed += 1;
        }

        debug!("Warmed {} custom cache entries", keys_warmed);
        Ok(keys_warmed)
    }

    /// Check if job dependencies are satisfied
    fn are_dependencies_satisfied(
        &self,
        job: &WarmingJob,
        completed_jobs: &HashSet<String>,
    ) -> bool {
        job.dependencies
            .iter()
            .all(|dep| completed_jobs.contains(dep))
    }

    /// Check if job meets priority threshold
    fn meets_priority_threshold(&self, job_name: &str) -> bool {
        self.config
            .priority_thresholds
            .get(job_name)
            .is_none_or(|threshold| *threshold >= WarmingPriority::Medium)
    }

    /// Check if job is currently running
    async fn is_job_running(&self, job_name: &str) -> bool {
        let running_jobs = self.running_jobs.read().await;
        running_jobs.contains(job_name)
    }

    /// Mark job as running
    async fn mark_job_running(&self, job_name: &str) {
        let mut running_jobs = self.running_jobs.write().await;
        running_jobs.insert(job_name.to_string());
    }

    /// Mark job as not running
    async fn mark_job_not_running(&self, job_name: &str) {
        let mut running_jobs = self.running_jobs.write().await;
        running_jobs.remove(job_name);
    }

    /// Update statistics
    async fn update_stats(&self, result: &WarmingResult) {
        let mut stats = self.stats.write().await;

        stats.total_warming_operations += 1;
        stats.keys_warmed += result.keys_warmed as u64;
        stats.last_warming_time = Some(Instant::now());

        if result.success {
            // Update average time
            let total_time_ms = result.duration_ms as f64;
            stats.avg_warming_time_ms = (stats.avg_warming_time_ms
                * (stats.total_warming_operations - 1) as f64
                + total_time_ms)
                / stats.total_warming_operations as f64;
        } else {
            stats.failed_warming_operations += 1;
        }

        // Update by priority (this would need to be enhanced to track job priorities)
        *stats
            .by_priority
            .entry(WarmingPriority::Medium)
            .or_insert(0) += 1;

        // Update success rate
        stats.success_rate = if stats.total_warming_operations > 0 {
            (stats.total_warming_operations - stats.failed_warming_operations) as f64
                / stats.total_warming_operations as f64
        } else {
            1.0
        };
    }

    /// Get warming statistics
    pub async fn get_stats(&self) -> WarmingStats {
        self.stats.read().await.clone()
    }

    /// Get all warming jobs
    pub fn get_jobs(&self) -> &Vec<WarmingJob> {
        &self.jobs
    }

    /// Get enabled warming jobs
    pub fn get_enabled_jobs(&self) -> Vec<&WarmingJob> {
        self.jobs.iter().filter(|job| job.enabled).collect()
    }

    /// Set job enabled state
    pub fn set_job_enabled(&mut self, job_name: &str, enabled: bool) -> bool {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.name == job_name) {
            job.enabled = enabled;
            info!(
                "Set warming job '{}' enabled state to: {}",
                job_name, enabled
            );
            true
        } else {
            false
        }
    }

    /// Get warming configuration
    pub fn get_config(&self) -> &WarmingConfig {
        &self.config
    }

    /// Update warming configuration
    pub fn update_config(&mut self, config: WarmingConfig) {
        self.config = config;
        info!("Updated cache warming configuration");
    }
}

/// System statistics structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemStats {
    pub total_tasks: u64,
    pub active_tasks: u64,
    pub alive_workers: u64,
    pub total_workers: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Convenience functions for common warming operations
pub mod convenience {
    use super::*;

    /// Warm task cache
    pub async fn warm_task_cache(
        manager: &CacheWarmingManager,
        task_id: i64,
        task: &Task,
        ttl: Duration,
    ) -> SchedulerResult<()> {
        let cache_key = task_cache_key(task_id);
        manager
            .cache_service
            .set_typed(&cache_key, task, ttl)
            .await?;

        if !task.name.is_empty() {
            let name_cache_key = task_name_cache_key(&task.name);
            manager
                .cache_service
                .set_typed(&name_cache_key, task, ttl)
                .await?;
        }

        Ok(())
    }

    /// Warm worker cache
    pub async fn warm_worker_cache(
        manager: &CacheWarmingManager,
        worker_id: &str,
        worker: &WorkerInfo,
        ttl: Duration,
    ) -> SchedulerResult<()> {
        let cache_key = worker_cache_key(worker_id);
        manager
            .cache_service
            .set_typed(&cache_key, worker, ttl)
            .await?;
        Ok(())
    }

    /// Warm multiple tasks cache
    pub async fn warm_tasks_cache(
        manager: &CacheWarmingManager,
        tasks: &[Task],
        ttl: Duration,
    ) -> SchedulerResult<usize> {
        let mut keys_warmed = 0;

        for task in tasks {
            let cache_key = task_cache_key(task.id);
            manager
                .cache_service
                .set_typed(&cache_key, task, ttl)
                .await?;
            keys_warmed += 1;

            if !task.name.is_empty() {
                let name_cache_key = task_name_cache_key(&task.name);
                manager
                    .cache_service
                    .set_typed(&name_cache_key, task, ttl)
                    .await?;
                keys_warmed += 1;
            }
        }

        Ok(keys_warmed)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use scheduler_errors::SchedulerError;

    use super::*;
    use crate::cache::config::CacheConfig;
    use crate::cache::manager::RedisCacheManager;
    use crate::cache::CacheStats;

    #[tokio::test]
    async fn test_warming_manager_creation() {
        let config = CacheConfig {
            enabled: false,
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

        // Create mock repositories
        struct MockRepository;

        #[async_trait]
        impl TaskRepository for MockRepository {
            async fn create(&self, _task: &Task) -> SchedulerResult<Task> {
                Err(SchedulerError::Internal("Mock error".to_string()))
            }

            async fn get_by_id(&self, _id: i64) -> SchedulerResult<Option<Task>> {
                Ok(None)
            }

            async fn get_by_name(&self, _name: &str) -> SchedulerResult<Option<Task>> {
                Ok(None)
            }

            async fn update(&self, _task: &Task) -> SchedulerResult<()> {
                Ok(())
            }

            async fn delete(&self, _id: i64) -> SchedulerResult<()> {
                Ok(())
            }

            async fn list(&self, _filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
                Ok(Vec::new())
            }

            async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
                Ok(Vec::new())
            }

            async fn get_schedulable_tasks(
                &self,
                _current_time: chrono::DateTime<chrono::Utc>,
            ) -> SchedulerResult<Vec<Task>> {
                Ok(Vec::new())
            }

            async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
                Ok(true)
            }

            async fn get_dependencies(&self, _task_id: i64) -> SchedulerResult<Vec<Task>> {
                Ok(Vec::new())
            }

            async fn batch_update_status(
                &self,
                _task_ids: &[i64],
                _status: TaskStatus,
            ) -> SchedulerResult<()> {
                Ok(())
            }
        }

        #[async_trait]
        impl WorkerRepository for MockRepository {
            async fn register(&self, _worker: &WorkerInfo) -> SchedulerResult<()> {
                Ok(())
            }

            async fn unregister(&self, _worker_id: &str) -> SchedulerResult<()> {
                Ok(())
            }

            async fn get_by_id(&self, _worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
                Ok(None)
            }

            async fn update(&self, _worker: &WorkerInfo) -> SchedulerResult<()> {
                Ok(())
            }

            async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
                Ok(Vec::new())
            }

            async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
                Ok(Vec::new())
            }

            async fn update_heartbeat(
                &self,
                _worker_id: &str,
                _heartbeat_time: chrono::DateTime<chrono::Utc>,
                _current_task_count: i32,
            ) -> SchedulerResult<()> {
                Ok(())
            }

            async fn batch_update_status(
                &self,
                _worker_ids: &[String],
                _status: WorkerStatus,
            ) -> SchedulerResult<()> {
                Ok(())
            }

            async fn get_workers_by_task_type(
                &self,
                _task_type: &str,
            ) -> SchedulerResult<Vec<WorkerInfo>> {
                Ok(Vec::new())
            }

            async fn update_status(
                &self,
                _worker_id: &str,
                _status: WorkerStatus,
            ) -> SchedulerResult<()> {
                Ok(())
            }

            async fn get_timeout_workers(
                &self,
                _timeout_seconds: i64,
            ) -> SchedulerResult<Vec<WorkerInfo>> {
                Ok(Vec::new())
            }

            async fn cleanup_offline_workers(&self, _timeout_seconds: i64) -> SchedulerResult<u64> {
                Ok(0)
            }

            async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
                Ok(Vec::new())
            }
        }

        #[async_trait]
        impl TaskRunRepository for MockRepository {
            async fn create(&self, _task_run: &TaskRun) -> SchedulerResult<TaskRun> {
                Err(SchedulerError::Internal("Mock error".to_string()))
            }

            async fn get_by_id(&self, _id: i64) -> SchedulerResult<Option<TaskRun>> {
                Ok(None)
            }

            async fn update(&self, _task_run: &TaskRun) -> SchedulerResult<()> {
                Ok(())
            }

            async fn delete(&self, _id: i64) -> SchedulerResult<()> {
                Ok(())
            }

            async fn get_by_task_id(&self, _task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_by_worker_id(&self, _worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_by_status(&self, _status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_pending_runs(&self, _limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_timeout_runs(
                &self,
                _timeout_seconds: i64,
            ) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn update_status(
                &self,
                _id: i64,
                _status: TaskRunStatus,
                _worker_id: Option<&str>,
            ) -> SchedulerResult<()> {
                Ok(())
            }

            async fn update_result(
                &self,
                _id: i64,
                _result: Option<&str>,
                _error_message: Option<&str>,
            ) -> SchedulerResult<()> {
                Ok(())
            }

            async fn get_recent_runs(
                &self,
                _task_id: i64,
                _limit: i64,
            ) -> SchedulerResult<Vec<TaskRun>> {
                Ok(Vec::new())
            }

            async fn get_execution_stats(
                &self,
                _task_id: i64,
                _days: i32,
            ) -> SchedulerResult<TaskExecutionStats> {
                Ok(TaskExecutionStats {
                    task_id: _task_id,
                    total_runs: 0,
                    successful_runs: 0,
                    failed_runs: 0,
                    timeout_runs: 0,
                    average_execution_time_ms: None,
                    success_rate: 0.0,
                    last_execution: None,
                })
            }

            async fn cleanup_old_runs(&self, _days: i32) -> SchedulerResult<u64> {
                Ok(0)
            }

            async fn batch_update_status(
                &self,
                _run_ids: &[i64],
                _status: TaskRunStatus,
            ) -> SchedulerResult<()> {
                Ok(())
            }
        }

        let task_repo = Arc::new(MockRepository) as Arc<dyn TaskRepository>;
        let worker_repo = Arc::new(MockRepository) as Arc<dyn WorkerRepository>;
        let task_run_repo = Arc::new(MockRepository) as Arc<dyn TaskRunRepository>;

        let config = WarmingConfig::default();
        let manager =
            CacheWarmingManager::new(cache_service, task_repo, worker_repo, task_run_repo, config);

        assert!(!manager.get_jobs().is_empty());
        assert!(!manager.get_enabled_jobs().is_empty());
    }

    #[test]
    fn test_warming_config_default() {
        let config = WarmingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.strategy, WarmingStrategy::Selective);
        assert!(config.warm_on_startup);
    }

    #[test]
    fn test_warming_priority_ordering() {
        assert!(WarmingPriority::Critical > WarmingPriority::High);
        assert!(WarmingPriority::High > WarmingPriority::Medium);
        assert!(WarmingPriority::Medium > WarmingPriority::Low);
    }
}
