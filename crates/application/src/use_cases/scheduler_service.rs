use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::ports::service_interfaces::{task_services::TaskSchedulerService, SchedulerStats};
use scheduler_domain::entities::{Task, TaskRun};
use scheduler_domain::messaging::MessageQueue;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
use scheduler_errors::{SchedulerError, SchedulerResult};

use crate::use_cases::task_scanner_service::{TaskScannerService, ScanResult};
use crate::use_cases::task_planning_service::{TaskPlanningService, SchedulePlan};
use crate::use_cases::task_dispatch_service::{TaskDispatchService, DispatchResult};
use crate::use_cases::dependency_checker::{DependencyCheckService, DependencyCheckServiceTrait};

/// 协调者模式的调度器服务 - 协调三个专业服务完成调度工作
pub struct SchedulerService {
    // 保留原有接口兼容性的必要字段
    pub task_repo: Arc<dyn TaskRepository>,
    pub task_run_repo: Arc<dyn TaskRunRepository>,
    pub message_queue: Arc<dyn MessageQueue>,
    pub task_queue_name: String,
    pub is_running: std::sync::atomic::AtomicBool,
    
    // 新的专业化服务
    scanner_service: TaskScannerService,
    planning_service: TaskPlanningService,
    dispatch_service: TaskDispatchService,
    
    // 保持向后兼容的依赖检查器
    dependency_checker: DependencyCheckService,
    
    // 调度配置
    max_concurrent_dispatches: usize,
}

impl SchedulerService {
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        task_queue_name: String,
    ) -> Self {
        // 创建专业化服务
        let scanner_service = TaskScannerService::new(task_repo.clone());
        let planning_service = TaskPlanningService::new(
            task_repo.clone(),
            task_run_repo.clone(),
        );
        let dispatch_service = TaskDispatchService::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue.clone(),
            task_queue_name.clone(),
        );
        
        // 保持向后兼容的依赖检查器
        let dependency_checker = DependencyCheckService::new(
            task_repo.clone(),
            task_run_repo.clone(),
        );

        Self {
            task_repo,
            task_run_repo,
            message_queue,
            task_queue_name,
            is_running: std::sync::atomic::AtomicBool::new(false),
            scanner_service,
            planning_service,
            dispatch_service,
            dependency_checker,
            max_concurrent_dispatches: 10, // 默认并发限制
        }
    }
    
    /// 设置最大并发分发数量
    pub fn with_max_concurrent_dispatches(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent_dispatches = max_concurrent;
        self
    }

    /// 协调者模式的核心调度方法 - 通过三个专业服务完成调度
    async fn orchestrate_scheduling(&self) -> SchedulerResult<Vec<TaskRun>> {
        let span = tracing::info_span!("orchestrate_scheduling");
        let _guard = span.enter();
        let start_time = std::time::Instant::now();
        
        info!("开始协调调度流程");
        
        // 步骤 1: 扫描符合条件的任务
        let scan_result: ScanResult = self.scanner_service.scan_schedulable_tasks().await?;
        
        if scan_result.schedulable_tasks.is_empty() {
            info!("没有发现符合调度条件的任务，本次调度结束");
            return Ok(vec![]);
        }
        
        info!(
            "扫描完成: 发现 {} 个可调度任务",
            scan_result.schedulable_tasks.len()
        );
        
        // 步骤 2: 创建执行计划（依赖检查、并发控制等）
        let mut schedule_plan: SchedulePlan = self
            .planning_service
            .create_execution_plan(scan_result.schedulable_tasks)
            .await?;
            
        // 步骤 2.1: 应用系统并发限制
        schedule_plan = self
            .planning_service
            .limit_executable_tasks(schedule_plan, self.max_concurrent_dispatches);
        
        if schedule_plan.executable_tasks.is_empty() {
            warn!(
                "所有任务都被阻塞，无法执行: {} 个任务因各种原因被阻塞",
                schedule_plan.blocked_tasks.len()
            );
            return Ok(vec![]);
        }
        
        info!(
            "规划完成: {} 个任务可执行，{} 个任务被阻塞",
            schedule_plan.executable_tasks.len(),
            schedule_plan.blocked_tasks.len()
        );
        
        // 步骤 3: 分发可执行的任务
        let dispatch_result: DispatchResult = self
            .dispatch_service
            .dispatch_tasks_concurrent(
                schedule_plan.executable_tasks,
                self.max_concurrent_dispatches,
            )
            .await?;
        
        // 记录调度结果
        let duration = start_time.elapsed().as_secs_f64();
        if !dispatch_result.failed_dispatches.is_empty() {
            for failure in &dispatch_result.failed_dispatches {
                error!(
                    "任务分发失败: {} (ID: {}) - {}",
                    failure.task_name, failure.task_id, failure.error_message
                );
            }
        }
        
        info!(
            "调度协调完成: 成功 {}/{} 个任务，耗时 {:.3} 秒",
            dispatch_result.successful_dispatches.len(),
            dispatch_result.total_attempted,
            duration
        );
        
        Ok(dispatch_result.successful_dispatches)
    }
}

#[async_trait::async_trait]
impl TaskSchedulerService for SchedulerService {
    /// 主调度入口 - 使用协调者模式
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>> {
        self.orchestrate_scheduling().await
    }

    /// 兼容性方法 - 委托给依赖检查器
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool> {
        let span = tracing::info_span!("check_dependencies");
        let _guard = span.enter();
        let check_result = self.dependency_checker.check_dependencies(task).await?;

        tracing::debug!(
            "依赖检查结果: can_execute={}, reason={:?}",
            check_result.can_execute, check_result.reason
        );

        Ok(check_result.can_execute)
    }

    /// 兼容性方法 - 委托给分发服务
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun> {
        // 创建单个任务的执行计划
        let plan = self.planning_service.create_execution_plan(vec![task.clone()]).await?;
        
        if plan.executable_tasks.is_empty() {
            return Err(SchedulerError::ValidationError(
                format!("任务 {} 不满足执行条件", task.name)
            ));
        }
        
        // 分发单个任务
        let dispatch_result = self.dispatch_service.dispatch_tasks(plan.executable_tasks).await?;
        
        if dispatch_result.successful_dispatches.is_empty() {
            return Err(SchedulerError::TaskExecution(
                format!("任务 {} 分发失败", task.name)
            ));
        }
        
        Ok(dispatch_result.successful_dispatches.into_iter().next().unwrap())
    }

    /// 兼容性方法 - 委托给分发服务
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        // 获取任务信息
        let task = self
            .task_repo
            .get_by_id(task_run.task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound {
                id: task_run.task_id,
            })?;

        // 使用分发服务发送到队列
        self.dispatch_service.send_to_queue(&task, task_run).await
    }

    async fn start(&self) -> SchedulerResult<()> {
        info!("启动调度器服务");
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> SchedulerResult<()> {
        info!("停止调度器服务");
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// 兼容性方法 - 使用新的协调流程
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()> {
        let result = self.orchestrate_scheduling().await?;
        
        // 检查指定任务是否被成功调度
        let task_scheduled = result.iter().any(|run| run.task_id == task.id);
        
        if task_scheduled {
            info!("任务 {} 已调度", task.name);
        } else {
            info!("任务 {} 未被调度（可能不满足调度条件）", task.name);
        }
        
        Ok(())
    }

    /// 批量调度任务 - 使用协调者模式
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()> {
        // 将任务转换为向量
        let task_list = tasks.to_vec();
        
        // 使用规划服务创建执行计划
        let schedule_plan = self.planning_service.create_execution_plan(task_list).await?;
        
        // 限制并发
        let limited_plan = self.planning_service.limit_executable_tasks(
            schedule_plan, 
            self.max_concurrent_dispatches
        );
        
        // 分发任务
        let dispatch_result = self.dispatch_service.dispatch_tasks_concurrent(
            limited_plan.executable_tasks,
            self.max_concurrent_dispatches,
        ).await?;
        
        info!(
            "批量调度完成: {} 成功，{} 失败",
            dispatch_result.successful_dispatches.len(),
            dispatch_result.failed_dispatches.len()
        );
        
        Ok(())
    }

    async fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn get_stats(&self) -> SchedulerResult<SchedulerStats> {
        // 获取所有任务数量（使用现有的API）
        let active_tasks = self.task_repo.get_active_tasks().await?;
        let total_tasks = active_tasks.len() as i64;
        let active_tasks_count = active_tasks.len() as i64;

        // 获取任务运行统计
        let running_runs = self.task_run_repo.get_running_runs().await?;
        let running_task_runs = running_runs.len() as i64;

        let pending_runs = self.task_run_repo.get_pending_runs(None).await?;
        let pending_task_runs = pending_runs.len() as i64;

        // 计算运行时间（使用运行状态作为代理）
        let uptime_seconds = if self.is_running().await {
            // 简单计算，实际中可以存储启动时间
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        } else {
            0
        };

        Ok(SchedulerStats {
            total_tasks,
            active_tasks: active_tasks_count,
            running_task_runs,
            pending_task_runs,
            uptime_seconds,
            last_schedule_time: Some(Utc::now()), // 可以存储实际的最后调度时间
        })
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        info!("重新加载调度器配置");

        // 从配置文件重新加载配置
        match scheduler_config::AppConfig::load(None) {
            Ok(new_config) => {
                info!("配置重新加载成功");

                // 检查并更新相关配置
                // TODO: 实际项目中可以将配置传递给各个组件
                tracing::debug!(
                    "新配置加载: 调度间隔={}s, 最大并发={}",
                    new_config.dispatcher.schedule_interval_seconds,
                    new_config.dispatcher.max_concurrent_dispatches
                );

                Ok(())
            }
            Err(e) => {
                error!("配置重新加载失败: {}", e);
                Err(SchedulerError::Configuration(format!(
                    "配置重新加载失败: {}",
                    e
                )))
            }
        }
    }
}