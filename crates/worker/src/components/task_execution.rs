use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use scheduler_core::{SchedulerError, SchedulerResult, traits::{ExecutorRegistry, TaskExecutionContext, ResourceLimits}};
use scheduler_domain::entities::{TaskExecutionMessage, TaskRun, TaskRunStatus};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub struct TaskExecutionManager {
    worker_id: String,
    executor_registry: Arc<dyn ExecutorRegistry>,
    max_concurrent_tasks: usize,
    running_tasks: Arc<RwLock<HashMap<i64, TaskRun>>>,
}

impl TaskExecutionManager {
    pub fn new(
        worker_id: String,
        executor_registry: Arc<dyn ExecutorRegistry>,
        max_concurrent_tasks: usize,
    ) -> Self {
        Self {
            worker_id,
            executor_registry,
            max_concurrent_tasks,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn get_supported_task_types(&self) -> Vec<String> {
        self.executor_registry.list_executors().await
    }
    pub async fn get_current_task_count(&self) -> i32 {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.len() as i32
    }
    pub async fn can_accept_task(&self, task_type: &str) -> bool {
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks as i32 {
            return false;
        }
        self.executor_registry.contains(task_type).await
    }
    pub async fn handle_task_execution<F, Fut>(
        &self,
        message: TaskExecutionMessage,
        status_callback: F,
    ) -> SchedulerResult<()>
    where
        F: Fn(i64, TaskRunStatus, Option<String>, Option<String>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchedulerResult<()>> + Send + 'static,
    {
        let task_run_id = message.task_run_id;
        let task_type = &message.task_type;

        info!(
            "Processing task execution: task_run_id={}, task_type={}, timeout={}s",
            task_run_id, task_type, message.timeout_seconds
        );
        let executor = match self.executor_registry.get(task_type).await {
            Some(executor) => executor,
            None => {
                error!("No executor found for task type '{}'", task_type);
                status_callback(
                    task_run_id,
                    TaskRunStatus::Failed,
                    None,
                    Some(format!("Unsupported task type: {task_type}")),
                )
                .await?;
                return Ok(());
            }
        };
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks as i32 {
            warn!(
                "Max concurrency limit {} reached, rejecting task {}",
                self.max_concurrent_tasks, task_run_id
            );
            status_callback(
                task_run_id,
                TaskRunStatus::Failed,
                None,
                Some("Worker reached max concurrency limit".to_string()),
            )
            .await?;
            return Ok(());
        }
        let task_run = TaskRun {
            id: task_run_id,
            task_id: message.task_id,
            status: TaskRunStatus::Running,
            worker_id: Some(self.worker_id.clone()),
            retry_count: message.retry_count,
            shard_index: message.shard_index,
            shard_total: message.shard_total,
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            result: None,
            error_message: None,
            created_at: Utc::now(),
        };
        let parameters =
            serde_json::from_value::<HashMap<String, serde_json::Value>>(message.parameters)
                .unwrap_or_default();

        let context = TaskExecutionContext {
            task_run: task_run.clone(),
            task_type: task_type.clone(),
            parameters,
            timeout_seconds: message.timeout_seconds as u64,
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: ResourceLimits::default(),
        };
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.insert(task_run_id, task_run);
        }
        status_callback(task_run_id, TaskRunStatus::Running, None, None).await?;
        let executor_clone = Arc::clone(&executor);
        let running_tasks_clone = Arc::clone(&self.running_tasks);

        tokio::spawn(async move {
            let execution_start = std::time::Instant::now();
            let result = executor_clone.execute_task(&context).await;
            let execution_duration = execution_start.elapsed();
            {
                let mut running_tasks = running_tasks_clone.write().await;
                running_tasks.remove(&task_run_id);
            }
            match result {
                Ok(task_result) => {
                    info!(
                        "Task {} completed successfully in {:?}",
                        task_run_id, execution_duration
                    );
                    let _ = status_callback(
                        task_run_id,
                        TaskRunStatus::Completed,
                        task_result.output,
                        task_result.error_message,
                    )
                    .await;
                }
                Err(e) => {
                    error!("Task {} failed: {}", task_run_id, e);
                    let _ = status_callback(
                        task_run_id,
                        TaskRunStatus::Failed,
                        None,
                        Some(e.to_string()),
                    )
                    .await;
                }
            }
        });

        Ok(())
    }
    pub async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        let mut running_tasks = self.running_tasks.write().await;
        if let Some(_task) = running_tasks.remove(&task_run_id) {
            info!("Task {} cancelled", task_run_id);
            Ok(())
        } else {
            Err(SchedulerError::Internal(format!(
                "Task {task_run_id} not found in running tasks"
            )))
        }
    }
}
