use std::sync::Arc;
use tracing::{debug, info, warn};

use scheduler_domain::entities::Task;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
use scheduler_errors::SchedulerResult;

use crate::use_cases::dependency_checker::{DependencyCheckService, DependencyCheckServiceTrait};

/// 任务规划服务 - 负责检查依赖关系和并发限制，生成可执行的任务计划
pub struct TaskPlanningService {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    dependency_checker: DependencyCheckService,
}

/// 单个任务的执行计划
#[derive(Debug, Clone)]
pub struct TaskExecutionPlan {
    pub task: Task,
    pub can_execute: bool,
    pub block_reason: Option<String>,
    pub priority: i32,
}

/// 整体调度计划
#[derive(Debug, Clone)]
pub struct SchedulePlan {
    pub executable_tasks: Vec<TaskExecutionPlan>,
    pub blocked_tasks: Vec<TaskExecutionPlan>,
    pub total_evaluated: usize,
}

impl TaskPlanningService {
    /// 创建新的任务规划服务实例
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
    ) -> Self {
        let dependency_checker = DependencyCheckService::new(
            task_repo.clone(),
            task_run_repo.clone(),
        );

        Self {
            task_repo,
            task_run_repo,
            dependency_checker,
        }
    }

    /// 为给定的任务列表创建执行计划
    pub async fn create_execution_plan(&self, tasks: Vec<Task>) -> SchedulerResult<SchedulePlan> {
        let span = tracing::info_span!("create_execution_plan", task_count = tasks.len());
        let _guard = span.enter();

        info!("开始为 {} 个任务创建执行计划", tasks.len());

        let mut executable_tasks = Vec::new();
        let mut blocked_tasks = Vec::new();

        for task in tasks {
            let execution_plan = self.evaluate_task_execution(&task).await?;

            if execution_plan.can_execute {
                executable_tasks.push(execution_plan);
            } else {
                blocked_tasks.push(execution_plan);
            }
        }

        // 按优先级排序可执行任务
        executable_tasks.sort_by(|a, b| b.priority.cmp(&a.priority));

        let plan = SchedulePlan {
            total_evaluated: executable_tasks.len() + blocked_tasks.len(),
            executable_tasks,
            blocked_tasks,
        };

        info!(
            "执行计划创建完成: {} 个可执行，{} 个被阻塞",
            plan.executable_tasks.len(),
            plan.blocked_tasks.len()
        );

        Ok(plan)
    }

    /// 评估单个任务的可执行性
    async fn evaluate_task_execution(&self, task: &Task) -> SchedulerResult<TaskExecutionPlan> {
        let span = tracing::debug_span!("evaluate_task", task_name = %task.name);
        let _guard = span.enter();

        // 检查并发限制
        if let Some(reason) = self.check_concurrency_limit(task).await? {
            debug!("任务 {} 因并发限制被阻塞: {}", task.name, reason);
            return Ok(TaskExecutionPlan {
                task: task.clone(),
                can_execute: false,
                block_reason: Some(reason),
                priority: self.calculate_priority_score(task),
            });
        }

        // 检查依赖关系
        let dependency_result = self.dependency_checker.check_dependencies(task).await?;

        if !dependency_result.can_execute {
            let reason = format!(
                "依赖检查失败: {}",
                dependency_result.reason.unwrap_or_else(|| "未知原因".to_string())
            );
            debug!("任务 {} 因依赖关系被阻塞: {}", task.name, reason);

            return Ok(TaskExecutionPlan {
                task: task.clone(),
                can_execute: false,
                block_reason: Some(reason),
                priority: self.calculate_priority_score(task),
            });
        }

        // 任务可以执行
        debug!("任务 {} 已准备好执行", task.name);
        Ok(TaskExecutionPlan {
            task: task.clone(),
            can_execute: true,
            block_reason: None,
            priority: self.calculate_priority_score(task),
        })
    }

    /// 检查任务的并发限制
    async fn check_concurrency_limit(&self, task: &Task) -> SchedulerResult<Option<String>> {
        // 获取正在运行的任务实例数量
        let running_runs = self.task_run_repo.get_running_runs().await?;
        let running_count = running_runs
            .iter()
            .filter(|run| run.task_id == task.id)
            .count() as i32;

        // 使用默认并发限制（未来可以添加到Task结构体中）
        let max_concurrent_runs = task.max_retries.max(1);

        debug!(
            "任务 {} 并发检查: 运行中 {}/{}",
            task.name, running_count, max_concurrent_runs
        );

        if running_count >= max_concurrent_runs {
            return Ok(Some(format!(
                "已达到最大并发限制 ({}/{})",
                running_count, max_concurrent_runs
            )));
        }

        Ok(None)
    }

    /// 获取任务的优先级分数（可用于排序）
    pub fn calculate_priority_score(&self, task: &Task) -> i32 {
        // 基础优先级可以使用max_retries作为代理
        let score = task.max_retries;

        // 可以根据其他因素调整优先级
        // 例如：任务的等待时间、重试次数等
        
        score
    }

    /// 过滤出高优先级的可执行任务（限制数量以控制负载）
    pub fn limit_executable_tasks(&self, mut plan: SchedulePlan, max_concurrent: usize) -> SchedulePlan {
        if plan.executable_tasks.len() <= max_concurrent {
            return plan;
        }

        let remaining_tasks = plan.executable_tasks.split_off(max_concurrent);
        
        // 将超出限制的任务移到阻塞列表，并标注原因
        for mut task_plan in remaining_tasks {
            task_plan.can_execute = false;
            task_plan.block_reason = Some("系统负载限制".to_string());
            plan.blocked_tasks.push(task_plan);
        }

        warn!(
            "由于系统负载限制，{} 个任务被推迟执行",
            plan.blocked_tasks.len()
        );

        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{Task, TaskRun, TaskStatus, TaskRunStatus};
    use scheduler_testing_utils::mocks::{MockTaskRepository, MockTaskRunRepository};
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

    fn create_test_task_run(id: i64, task_id: i64, status: TaskRunStatus) -> TaskRun {
        let now = Utc::now();
        TaskRun {
            id,
            task_id,
            status,
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
    async fn test_create_execution_plan_all_executable() {
        let mock_task_repo = MockTaskRepository::new();
        let mock_run_repo = MockTaskRunRepository::new();

        let service = TaskPlanningService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
        );

        let tasks = vec![
            create_test_task(1, "task1"),
            create_test_task(2, "task2"),
        ];

        let plan = service.create_execution_plan(tasks).await.unwrap();

        assert_eq!(plan.total_evaluated, 2);
        assert_eq!(plan.executable_tasks.len(), 2);
        assert_eq!(plan.blocked_tasks.len(), 0);
        
        // 验证优先级排序 (使用max_retries作为priority)
        assert_eq!(plan.executable_tasks[0].priority, 3);
        assert_eq!(plan.executable_tasks[1].priority, 3);
    }

    #[tokio::test]
    async fn test_create_execution_plan_with_concurrency_limit() {
        let mock_task_repo = MockTaskRepository::new();
        
        // 模拟任务1已有3个运行中的实例（达到max_retries限制）
        let running_runs = vec![
            create_test_task_run(1, 1, TaskRunStatus::Running),
            create_test_task_run(2, 1, TaskRunStatus::Running),
            create_test_task_run(3, 1, TaskRunStatus::Running),
        ];
        
        let mock_run_repo = MockTaskRunRepository::with_task_runs(running_runs);

        let service = TaskPlanningService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
        );

        let tasks = vec![
            create_test_task(1, "task1"),
        ];

        let plan = service.create_execution_plan(tasks).await.unwrap();

        assert_eq!(plan.total_evaluated, 1);
        assert_eq!(plan.executable_tasks.len(), 0);
        assert_eq!(plan.blocked_tasks.len(), 1);
        assert!(plan.blocked_tasks[0].block_reason.as_ref().unwrap().contains("并发限制"));
    }

    #[tokio::test]
    async fn test_limit_executable_tasks() {
        let mock_task_repo = MockTaskRepository::new();
        let mock_run_repo = MockTaskRunRepository::new();
        
        let service = TaskPlanningService::new(
            Arc::new(mock_task_repo),
            Arc::new(mock_run_repo),
        );

        let executable_tasks = vec![
            TaskExecutionPlan {
                task: create_test_task(1, "task1"),
                can_execute: true,
                block_reason: None,
                priority: 3,
            },
            TaskExecutionPlan {
                task: create_test_task(2, "task2"),
                can_execute: true,
                block_reason: None,
                priority: 3,
            },
            TaskExecutionPlan {
                task: create_test_task(3, "task3"),
                can_execute: true,
                block_reason: None,
                priority: 3,
            },
        ];

        let plan = SchedulePlan {
            executable_tasks,
            blocked_tasks: vec![],
            total_evaluated: 3,
        };

        let limited_plan = service.limit_executable_tasks(plan, 2);

        assert_eq!(limited_plan.executable_tasks.len(), 2);
        assert_eq!(limited_plan.blocked_tasks.len(), 1);
        assert!(limited_plan.blocked_tasks[0].block_reason.as_ref().unwrap().contains("系统负载限制"));
    }
}