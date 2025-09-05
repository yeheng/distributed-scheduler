use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, info};

use scheduler_domain::entities::Task;
use scheduler_domain::repositories::TaskRepository;
use scheduler_errors::SchedulerResult;

use crate::use_cases::cron_utils::CronScheduler;

/// 任务扫描服务 - 负责从数据库中查询符合调度条件的任务
pub struct TaskScannerService {
    task_repo: Arc<dyn TaskRepository>,
}

/// 扫描结果，包含可调度的任务列表
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub schedulable_tasks: Vec<Task>,
    pub skipped_count: usize,
    pub total_scanned: usize,
}

impl TaskScannerService {
    /// 创建新的任务扫描服务实例
    pub fn new(task_repo: Arc<dyn TaskRepository>) -> Self {
        Self { task_repo }
    }

    /// 扫描所有活跃任务，返回符合调度条件的任务列表
    pub async fn scan_schedulable_tasks(&self) -> SchedulerResult<ScanResult> {
        let span = tracing::info_span!("scan_schedulable_tasks");
        let _guard = span.enter();
        
        info!("开始扫描符合调度条件的任务");
        
        // 获取所有活跃任务
        let active_tasks = self.task_repo.get_active_tasks().await?;
        let total_scanned = active_tasks.len();
        
        debug!("发现 {} 个活跃任务，开始过滤符合调度条件的任务", total_scanned);
        
        let mut schedulable_tasks = Vec::new();
        let mut skipped_count = 0;
        
        for task in active_tasks {
            if self.should_schedule_now(&task).await? {
                schedulable_tasks.push(task);
            } else {
                skipped_count += 1;
            }
        }
        
        let result = ScanResult {
            schedulable_tasks: schedulable_tasks.clone(),
            skipped_count,
            total_scanned,
        };
        
        info!(
            "任务扫描完成: 总计 {} 个，可调度 {} 个，跳过 {} 个",
            total_scanned,
            schedulable_tasks.len(),
            skipped_count
        );
        
        Ok(result)
    }

    /// 检查单个任务是否符合调度条件（基于时间和状态）
    async fn should_schedule_now(&self, task: &Task) -> SchedulerResult<bool> {
        // 检查任务是否处于活跃状态
        if !task.is_active() {
            debug!("任务 {} 不处于活跃状态，跳过调度", task.name);
            return Ok(false);
        }

        // 检查时间调度条件
        let cron_scheduler = CronScheduler::new(&task.schedule)?;
        let now = Utc::now();

        if !cron_scheduler.should_run(now) {
            debug!(
                "任务 {} 当前时间不符合调度条件，跳过调度 (schedule: {})",
                task.name, task.schedule
            );
            return Ok(false);
        }

        debug!("任务 {} 符合调度条件", task.name);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{Task, TaskStatus};
    use scheduler_testing_utils::mocks::MockTaskRepository;
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

    #[tokio::test]
    async fn test_scan_schedulable_tasks_all_active() {
        // 准备测试数据
        let tasks = vec![
            create_test_task(1, "test_task_1"),
        ];
        
        let mock_repo = MockTaskRepository::with_tasks(tasks);
        let service = TaskScannerService::new(Arc::new(mock_repo));
        
        // 执行测试
        let result = service.scan_schedulable_tasks().await.unwrap();
        
        // 验证结果
        assert_eq!(result.total_scanned, 1);
        assert_eq!(result.schedulable_tasks.len(), 1);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.schedulable_tasks[0].name, "test_task_1");
    }

    #[tokio::test]
    async fn test_scan_schedulable_tasks_mixed_status() {
        let tasks = vec![
            create_test_task(1, "active_task"),
            {
                let mut task = create_test_task(2, "inactive_task");
                task.status = TaskStatus::Inactive;
                task
            },
        ];
        
        let mock_repo = MockTaskRepository::with_tasks(tasks);
        let service = TaskScannerService::new(Arc::new(mock_repo));
        let result = service.scan_schedulable_tasks().await.unwrap();
        
        // 只有活跃任务会被get_active_tasks()返回，因此total_scanned应该是1
        assert_eq!(result.total_scanned, 1);
        assert_eq!(result.schedulable_tasks.len(), 1);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.schedulable_tasks[0].name, "active_task");
    }

    #[tokio::test]
    async fn test_should_schedule_now_inactive_task() {
        let mock_repo = MockTaskRepository::new();
        let service = TaskScannerService::new(Arc::new(mock_repo));
        
        let mut inactive_task = create_test_task(1, "inactive_task");
        inactive_task.status = TaskStatus::Inactive;
        
        let result = service.should_schedule_now(&inactive_task).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_should_schedule_now_active_task() {
        let mock_repo = MockTaskRepository::new();
        let service = TaskScannerService::new(Arc::new(mock_repo));
        
        let active_task = create_test_task(1, "active_task");
        
        let result = service.should_schedule_now(&active_task).await.unwrap();
        assert!(result);
    }
}