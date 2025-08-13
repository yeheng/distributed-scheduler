use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use scheduler_foundation::{SchedulerError, SchedulerResult};
use scheduler_domain::entities::Task;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};

pub struct DependencyCheckService {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
}

#[derive(Debug, Clone)]
pub struct DependencyCheckResult {
    pub can_execute: bool,
    pub blocking_dependencies: Vec<i64>,
    pub reason: Option<String>,
}

#[async_trait]
pub trait DependencyCheckServiceTrait: Send + Sync {
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<DependencyCheckResult>;
    async fn validate_dependencies(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<()>;
}

impl DependencyCheckService {
    pub fn new(task_repo: Arc<dyn TaskRepository>, task_run_repo: Arc<dyn TaskRunRepository>) -> Self {
        Self {
            task_repo,
            task_run_repo,
        }
    }
}

#[async_trait]
impl DependencyCheckServiceTrait for DependencyCheckService {
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<DependencyCheckResult> {
        if task.dependencies.is_empty() {
            return Ok(DependencyCheckResult {
                can_execute: true,
                blocking_dependencies: Vec::new(),
                reason: None,
            });
        }

        let mut blocking_dependencies = Vec::new();
        let mut dependency_status = HashMap::new();

        for dep_id in &task.dependencies {
            match self.check_single_dependency(*dep_id, task.id).await {
                Ok(status) => {
                    dependency_status.insert(*dep_id, status.clone());
                    if !status.completed {
                        blocking_dependencies.push(*dep_id);
                    }
                }
                Err(e) => {
                    warn!("检查依赖任务 {} 失败: {}", dep_id, e);
                    blocking_dependencies.push(*dep_id);
                }
            }
        }

        let can_execute = blocking_dependencies.is_empty();
        let reason = if !can_execute {
            Some(format!(
                "等待依赖任务完成: {:?}",
                blocking_dependencies
            ))
        } else {
            None
        };

        Ok(DependencyCheckResult {
            can_execute,
            blocking_dependencies,
            reason,
        })
    }

    async fn validate_dependencies(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<()> {
        if dependencies.is_empty() {
            return Ok(());
        }

        let existing_ids: HashSet<i64> = dependencies.iter().cloned().collect();
        let mut invalid_deps = Vec::new();

        for dep_id in dependencies {
            match self.task_repo.get_by_id(*dep_id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    invalid_deps.push(*dep_id);
                }
                Err(e) => {
                    return Err(SchedulerError::InvalidDependency {
                        task_id,
                        dependency_id: *dep_id,
                        reason: format!("验证依赖时出错: {}", e),
                    });
                }
            }
        }

        if !invalid_deps.is_empty() {
            return Err(SchedulerError::InvalidDependency {
                task_id,
                dependency_id: invalid_deps[0],
                reason: format!("无效的依赖任务: {:?}", invalid_deps),
            });
        }

        if existing_ids.contains(&task_id) {
            return Err(SchedulerError::InvalidDependency {
                task_id,
                dependency_id: task_id,
                reason: "任务不能依赖自身".to_string(),
            });
        }

        self.check_circular_dependencies(task_id, dependencies).await?;

        Ok(())
    }
}

impl DependencyCheckService {
    async fn check_single_dependency(
        &self,
        dependency_id: i64,
        _task_id: i64,
    ) -> SchedulerResult<DependencyStatus> {
        let dependency_task = self
            .task_repo
            .get_by_id(dependency_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: dependency_id })?;

        if !dependency_task.is_active() {
            return Ok(DependencyStatus {
                completed: true,
                status: "inactive".to_string(),
            });
        }

        let recent_runs = self
            .task_run_repo
            .get_recent_runs(dependency_id, 1)
            .await?;
        let latest_run = recent_runs.first();

        match latest_run {
            Some(run) => {
                let status = if run.status == scheduler_domain::entities::TaskRunStatus::Completed {
                    "completed".to_string()
                } else {
                    "running".to_string()
                };

                Ok(DependencyStatus {
                    completed: run.status == scheduler_domain::entities::TaskRunStatus::Completed,
                    status,
                })
            }
            None => Ok(DependencyStatus {
                completed: false,
                status: "pending".to_string(),
            }),
        }
    }

    async fn check_circular_dependencies(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<()> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        
        for &dep_id in dependencies {
            queue.push_back(dep_id);
        }

        while let Some(current_id) = queue.pop_front() {
            if current_id == task_id {
                return Err(SchedulerError::CircularDependency);
            }

            if visited.contains(&current_id) {
                continue;
            }

            visited.insert(current_id);

            if let Ok(Some(task)) = self.task_repo.get_by_id(current_id).await {
                for &dep_id in &task.dependencies {
                    queue.push_back(dep_id);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub completed: bool,
    pub status: String,
}