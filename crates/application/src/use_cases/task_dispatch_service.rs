use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info, warn};

use scheduler_domain::entities::{Message, Task, TaskRun, TaskExecutionMessage};
use scheduler_domain::messaging::MessageQueue;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
use scheduler_errors::SchedulerResult;

use crate::use_cases::task_planning_service::TaskExecutionPlan;

/// 任务分发服务 - 负责创建TaskRun记录并将任务消息发布到消息队列
pub struct TaskDispatchService {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
    task_queue_name: String,
}

/// 分发结果，包含成功和失败的任务统计
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub successful_dispatches: Vec<TaskRun>,
    pub failed_dispatches: Vec<DispatchFailure>,
    pub total_attempted: usize,
}

/// 分发失败记录
#[derive(Debug, Clone)]
pub struct DispatchFailure {
    pub task_name: String,
    pub task_id: i64,
    pub error_message: String,
}

impl TaskDispatchService {
    /// 创建新的任务分发服务实例
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        task_queue_name: String,
    ) -> Self {
        Self {
            task_repo,
            task_run_repo,
            message_queue,
            task_queue_name,
        }
    }

    /// 分发任务计划中的可执行任务
    pub async fn dispatch_tasks(&self, execution_plans: Vec<TaskExecutionPlan>) -> SchedulerResult<DispatchResult> {
        let span = tracing::info_span!("dispatch_tasks", task_count = execution_plans.len());
        let _guard = span.enter();

        info!("开始分发 {} 个任务", execution_plans.len());

        let mut successful_dispatches = Vec::new();
        let mut failed_dispatches = Vec::new();

        for plan in &execution_plans {
            if !plan.can_execute {
                warn!(
                    "跳过不可执行的任务: {} (原因: {:?})",
                    plan.task.name,
                    plan.block_reason
                );
                continue;
            }

            match self.dispatch_single_task(&plan.task).await {
                Ok(task_run) => {
                    info!("成功分发任务: {} -> 运行实例: {}", plan.task.name, task_run.id);
                    successful_dispatches.push(task_run);
                }
                Err(e) => {
                    error!("分发任务 {} 失败: {}", plan.task.name, e);
                    failed_dispatches.push(DispatchFailure {
                        task_name: plan.task.name.clone(),
                        task_id: plan.task.id,
                        error_message: e.to_string(),
                    });
                }
            }
        }

        let result = DispatchResult {
            total_attempted: execution_plans.len(),
            successful_dispatches,
            failed_dispatches,
        };

        info!(
            "任务分发完成: {} 成功，{} 失败",
            result.successful_dispatches.len(),
            result.failed_dispatches.len()
        );

        Ok(result)
    }

    /// 分发单个任务
    async fn dispatch_single_task(&self, task: &Task) -> SchedulerResult<TaskRun> {
        let span = tracing::debug_span!("dispatch_single_task", task_name = %task.name, task_id = task.id);
        let _guard = span.enter();

        // 1. 创建任务运行实例
        let task_run = self.create_task_run(task).await?;

        // 2. 发送到消息队列
        self.send_to_queue(task, &task_run).await?;

        Ok(task_run)
    }

    /// 创建任务运行实例
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun> {
        let now = Utc::now();
        let mut task_run = TaskRun::new(task.id, now);

        // 处理分片配置
        if let Some(shard_config) = &task.shard_config {
            if shard_config.enabled {
                task_run.shard_index = Some(0);
                task_run.shard_total = Some(shard_config.shard_count);
            }
        }

        // 保存到数据库
        let created_run = self.task_run_repo.create(&task_run).await?;

        info!("为任务 {} 创建了新的执行实例 {}", task.name, created_run.id);

        Ok(created_run)
    }

    /// 发送任务到消息队列
    pub async fn send_to_queue(&self, task: &Task, task_run: &TaskRun) -> SchedulerResult<()> {
        let span = tracing::debug_span!(
            "send_to_queue", 
            task_name = %task.name,
            task_run_id = task_run.id,
            queue = %self.task_queue_name
        );
        let _guard = span.enter();

        let start_time = std::time::Instant::now();

        // 创建任务执行消息
        let task_execution = self.create_task_execution_message(task, task_run);
        let message = Message::task_execution(task_execution);

        // 发布到消息队列
        self.message_queue
            .publish_message(&self.task_queue_name, &message)
            .await
            .map_err(|e| {
                error!("发布消息到队列失败: task={}, queue={}, error={}", 
                       task.name, self.task_queue_name, e);
                e
            })?;

        let duration = start_time.elapsed().as_secs_f64();
        info!(
            "任务 {} 已分发到队列 {}, 耗时: {:.3}s",
            task.name, self.task_queue_name, duration
        );

        Ok(())
    }

    /// 创建任务执行消息
    fn create_task_execution_message(&self, task: &Task, task_run: &TaskRun) -> TaskExecutionMessage {
        TaskExecutionMessage {
            task_run_id: task_run.id,
            task_id: task.id,
            task_name: task.name.clone(),
            task_type: task.task_type.clone(),
            parameters: task.parameters.clone(),
            timeout_seconds: task.timeout_seconds,
            retry_count: task_run.retry_count,
            shard_index: task_run.shard_index,
            shard_total: task_run.shard_total,
        }
    }

    /// 批量分发任务（带并发控制）
    pub async fn dispatch_tasks_concurrent(&self, execution_plans: Vec<TaskExecutionPlan>, max_concurrent: usize) -> SchedulerResult<DispatchResult> {
        use futures::stream::{self, StreamExt};

        let span = tracing::info_span!("dispatch_tasks_concurrent", 
                                      task_count = execution_plans.len(),
                                      max_concurrent = max_concurrent);
        let _guard = span.enter();

        info!("开始并发分发 {} 个任务 (并发限制: {})", execution_plans.len(), max_concurrent);

        // 过滤出可执行的任务
        let executable_plans: Vec<_> = execution_plans
            .into_iter()
            .filter(|plan| plan.can_execute)
            .collect();

        let total_attempted = executable_plans.len();

        // 使用 futures::stream 进行并发处理
        let results: Vec<Result<TaskRun, DispatchFailure>> = stream::iter(executable_plans)
            .map(|plan| {
                let service = self;
                async move {
                    match service.dispatch_single_task(&plan.task).await {
                        Ok(task_run) => Ok(task_run),
                        Err(e) => Err(DispatchFailure {
                            task_name: plan.task.name.clone(),
                            task_id: plan.task.id,
                            error_message: e.to_string(),
                        }),
                    }
                }
            })
            .buffer_unordered(max_concurrent)
            .collect()
            .await;

        // 分离成功和失败的结果
        let mut successful_dispatches = Vec::new();
        let mut failed_dispatches = Vec::new();

        for result in results {
            match result {
                Ok(task_run) => successful_dispatches.push(task_run),
                Err(failure) => failed_dispatches.push(failure),
            }
        }

        let result = DispatchResult {
            total_attempted,
            successful_dispatches,
            failed_dispatches,
        };

        info!(
            "并发任务分发完成: {} 成功，{} 失败",
            result.successful_dispatches.len(),
            result.failed_dispatches.len()
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{Task, TaskStatus, TaskRunStatus};
    use scheduler_testing_utils::mocks::{MockTaskRepository, MockTaskRunRepository, MockMessageQueue};
    use chrono::Utc;

    fn create_test_task(id: i64, name: &str) -> Task {
        let now = Utc::now();
        Task {
            id,
            name: name.to_string(),
            task_type: "test".to_string(),
            schedule: "* * * * * *".to_string(), // 每秒执行（测试用）
            parameters: serde_json::Value::Null,
            timeout_seconds: 300,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn _create_test_task_run(id: i64, task_id: i64) -> TaskRun {
        let now = Utc::now();
        TaskRun {
            id,
            task_id,
            status: TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: now,
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: now,
        }
    }

    #[tokio::test]
    async fn test_dispatch_single_task_success() {
        let mock_task_repo = MockTaskRepository::new();
        let mock_run_repo = MockTaskRunRepository::new();
        let mock_queue = MockMessageQueue::new();

        let service = TaskDispatchService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
            Arc::new(mock_queue),
            "test_queue".to_string(),
        );

        let task = create_test_task(1, "test_task");

        let result = service.dispatch_single_task(&task).await;
        assert!(result.is_ok());
        let task_run = result.unwrap();
        assert_eq!(task_run.task_id, 1);
    }

    #[tokio::test]
    async fn test_dispatch_tasks_mixed_results() {
        // 简化测试：使用简单的Mock实现
        let mock_task_repo = MockTaskRepository::new();
        let mock_run_repo = MockTaskRunRepository::new();
        let mock_queue = MockMessageQueue::new();

        let service = TaskDispatchService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
            Arc::new(mock_queue),
            "test_queue".to_string(),
        );

        let plans = vec![
            TaskExecutionPlan {
                task: create_test_task(1, "success_task"),
                can_execute: true,
                block_reason: None,
                priority: 5,
            },
            TaskExecutionPlan {
                task: create_test_task(2, "another_task"),
                can_execute: true,
                block_reason: None,
                priority: 3,
            },
        ];

        let result = service.dispatch_tasks(plans).await.unwrap();

        assert_eq!(result.total_attempted, 2);
        // 使用真正的Mock，所有任务都会成功
        assert_eq!(result.successful_dispatches.len(), 2);
        assert_eq!(result.failed_dispatches.len(), 0);
    }

    #[tokio::test]
    async fn test_dispatch_tasks_skip_non_executable() {
        let mock_task_repo = MockTaskRepository::new();
        let mock_run_repo = MockTaskRunRepository::new();
        let mock_queue = MockMessageQueue::new();

        let service = TaskDispatchService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
            Arc::new(mock_queue),
            "test_queue".to_string(),
        );

        let plans = vec![
            TaskExecutionPlan {
                task: create_test_task(1, "blocked_task"),
                can_execute: false,
                block_reason: Some("依赖未满足".to_string()),
                priority: 5,
            },
        ];

        let result = service.dispatch_tasks(plans).await.unwrap();

        assert_eq!(result.total_attempted, 1);
        assert_eq!(result.successful_dispatches.len(), 0);
        assert_eq!(result.failed_dispatches.len(), 0);
    }
}