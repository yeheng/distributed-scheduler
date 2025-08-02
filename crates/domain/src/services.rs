//! 领域服务
//! 
//! 核心业务逻辑服务

use async_trait::async_trait;
use crate::{entities::{Task, TaskRun, Worker}, repositories::{TaskRepository, TaskRunRepository, WorkerRepository}};
use scheduler_core::{SchedulerError, SchedulerResult};

/// 任务调度服务
#[async_trait]
pub trait TaskSchedulingService: Send + Sync {
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun>;
    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
}

/// Worker选择服务
#[async_trait]
pub trait WorkerSelectionService: Send + Sync {
    async fn select_worker_for_task(&self, task: &Task) -> SchedulerResult<Option<Worker>>;
    async fn release_worker(&self, worker_id: &str) -> SchedulerResult<()>;
}

/// 默认任务调度服务实现
pub struct DefaultTaskSchedulingService<TR, TRR, WR> 
where
    TR: TaskRepository,
    TRR: TaskRunRepository,
    WR: WorkerRepository,
{
    task_repo: TR,
    task_run_repo: TRR,
    worker_repo: WR,
}

impl<TR, TRR, WR> DefaultTaskSchedulingService<TR, TRR, WR>
where
    TR: TaskRepository,
    TRR: TaskRunRepository,
    WR: WorkerRepository,
{
    pub fn new(task_repo: TR, task_run_repo: TRR, worker_repo: WR) -> Self {
        Self {
            task_repo,
            task_run_repo,
            worker_repo,
        }
    }
}

#[async_trait]
impl<TR, TRR, WR> TaskSchedulingService for DefaultTaskSchedulingService<TR, TRR, WR>
where
    TR: TaskRepository + 'static,
    TRR: TaskRunRepository + 'static,  
    WR: WorkerRepository + 'static,
{
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun> {
        // 基础实现：创建任务执行实例
        let task_run = TaskRun {
            id: 0, // 数据库生成
            task_id: task.id,
            worker_id: "".to_string(), // 稍后分配
            status: crate::entities::TaskRunStatus::Pending,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error_message: None,
            result: None,
            retry_count: 0,
        };
        
        self.task_run_repo.create(&task_run).await
    }

    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        if let Some(mut task_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            task_run.status = crate::entities::TaskRunStatus::Cancelled;
            task_run.completed_at = Some(chrono::Utc::now());
            self.task_run_repo.update(&task_run).await?;
        }
        Ok(())
    }

    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        if let Some(failed_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            let retry_run = TaskRun {
                id: 0,
                task_id: failed_run.task_id,
                worker_id: "".to_string(),
                status: crate::entities::TaskRunStatus::Pending,
                started_at: chrono::Utc::now(),
                completed_at: None,
                error_message: None,
                result: None,
                retry_count: failed_run.retry_count + 1,
            };
            
            self.task_run_repo.create(&retry_run).await
        } else {
            Err(SchedulerError::TaskRunNotFound { id: task_run_id })
        }
    }
}