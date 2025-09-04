use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_domain::{
    entities::{Task, TaskFilter, TaskStatus},
    repositories::TaskRepository,
};
use scheduler_errors::SchedulerResult;
use sqlx::{Row, SqlitePool};
use tracing::{debug, instrument};

use crate::{
    database::mapping::MappingHelpers,
    error_handling::{RepositoryErrorHelpers, RepositoryOperation},
    task_context,
};

pub struct SqliteTaskRepository {
    pool: SqlitePool,
}

impl SqliteTaskRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 创建嵌入式SQLite任务仓库，自动初始化数据库
    pub async fn new_embedded(database_path: &str) -> SchedulerResult<Self> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        
        debug!("Creating embedded SQLite task repository at: {}", database_path);
        
        // 创建连接选项，启用外键约束和WAL模式
        let connect_options = SqliteConnectOptions::from_str(database_path)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        
        // 创建连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect_with(connect_options)
            .await?;
        
        // 运行数据库迁移
        Self::run_migrations(&pool).await?;
        
        debug!("Successfully created embedded SQLite task repository");
        Ok(Self { pool })
    }

    /// 运行数据库迁移
    async fn run_migrations(pool: &SqlitePool) -> SchedulerResult<()> {
        debug!("Running SQLite database migrations");
        
        // 创建任务表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                task_type TEXT NOT NULL,
                schedule TEXT NOT NULL,
                parameters TEXT NOT NULL DEFAULT '{}',
                timeout_seconds INTEGER NOT NULL DEFAULT 300,
                max_retries INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'ACTIVE',
                dependencies TEXT DEFAULT '[]',
                shard_config TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        // 创建任务执行记录表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'PENDING',
                worker_id TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                shard_index INTEGER,
                shard_total INTEGER,
                scheduled_at DATETIME NOT NULL,
                started_at DATETIME,
                completed_at DATETIME,
                result TEXT,
                error_message TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await?;

        // 创建Worker信息表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workers (
                id TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                supported_task_types TEXT NOT NULL DEFAULT '[]',
                max_concurrent_tasks INTEGER NOT NULL DEFAULT 5,
                current_task_count INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'ALIVE',
                last_heartbeat DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                registered_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        // 创建系统状态表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        // 创建索引
        let indexes = vec![
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            "CREATE INDEX IF NOT EXISTS idx_tasks_name ON tasks(name)",
            "CREATE INDEX IF NOT EXISTS idx_tasks_task_type ON tasks(task_type)",
            "CREATE INDEX IF NOT EXISTS idx_task_runs_task_id ON task_runs(task_id)",
            "CREATE INDEX IF NOT EXISTS idx_task_runs_status ON task_runs(status)",
            "CREATE INDEX IF NOT EXISTS idx_task_runs_scheduled_at ON task_runs(scheduled_at)",
            "CREATE INDEX IF NOT EXISTS idx_task_runs_worker_id ON task_runs(worker_id)",
            "CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status)",
            "CREATE INDEX IF NOT EXISTS idx_workers_last_heartbeat ON workers(last_heartbeat)",
        ];

        for index_sql in indexes {
            sqlx::query(index_sql)
                .execute(pool)
                .await?;
        }

        debug!("Successfully completed SQLite database migrations");
        Ok(())
    }

    fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> SchedulerResult<Task> {
        use sqlx::Row;

        let dependencies = MappingHelpers::parse_dependencies_sqlite(row, "dependencies");

        let parameters = MappingHelpers::parse_parameters_sqlite(row, "parameters")?;

        let shard_config = MappingHelpers::parse_shard_config_sqlite(row, "shard_config")?;

        Ok(Task {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            task_type: row.try_get("task_type")?,
            schedule: row.try_get("schedule")?,
            parameters,
            timeout_seconds: row.try_get("timeout_seconds")?,
            max_retries: row.try_get("max_retries")?,
            status: row.try_get("status")?,
            dependencies,
            shard_config,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    /// Get latest run status for multiple tasks in batch (internal helper)
    async fn get_dependency_run_statuses_batch(
        &self,
        task_ids: &[i64],
    ) -> SchedulerResult<Vec<(i64, Option<String>)>> {
        if task_ids.is_empty() {
            return Ok(vec![]);
        }

        let context = task_context!(RepositoryOperation::BatchRead)
            .with_additional_info(format!("批量查询{}个任务的依赖状态", task_ids.len()));

        // Create placeholders for the IN clause
        let placeholders: Vec<String> = (1..=task_ids.len()).map(|i| format!("?{i}")).collect();

        let sql = format!(
            "SELECT 
                tr1.task_id,
                tr1.status
             FROM task_runs tr1
             INNER JOIN (
                 SELECT task_id, MAX(created_at) as max_created_at
                 FROM task_runs 
                 WHERE task_id IN ({})
                 GROUP BY task_id
             ) tr2 ON tr1.task_id = tr2.task_id AND tr1.created_at = tr2.max_created_at
             ORDER BY tr1.task_id",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for &task_id in task_ids {
            query = query.bind(task_id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::database_error(context.clone(), e))?;

        let mut statuses = Vec::new();
        let mut found_task_ids = std::collections::HashSet::new();

        for row in rows {
            let task_id: i64 = row.try_get("task_id")?;
            let status: String = row.try_get("status")?;
            statuses.push((task_id, Some(status)));
            found_task_ids.insert(task_id);
        }

        // Add None status for tasks that have never been executed
        for &task_id in task_ids {
            if !found_task_ids.contains(&task_id) {
                statuses.push((task_id, None));
            }
        }

        statuses.sort_by_key(|(task_id, _)| *task_id);
        Ok(statuses)
    }

    /// Get multiple tasks by their IDs using a single batch query (internal helper)
    async fn get_tasks_by_ids_batch(&self, task_ids: &[i64]) -> SchedulerResult<Vec<Task>> {
        if task_ids.is_empty() {
            return Ok(vec![]);
        }

        let context = task_context!(RepositoryOperation::BatchRead)
            .with_additional_info(format!("批量查询{}个任务详情", task_ids.len()));

        // Create placeholders for the IN clause
        let placeholders: Vec<String> = (1..=task_ids.len()).map(|i| format!("?{i}")).collect();

        let sql = format!(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at 
             FROM tasks 
             WHERE id IN ({}) 
             ORDER BY id",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for &task_id in task_ids {
            query = query.bind(task_id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::database_error(context.clone(), e))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::row_to_task(&row)?);
        }

        Ok(tasks)
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository {
    #[instrument(skip(self, task), fields(
        task_id = %task.id,
        task_name = %task.name,
        task_type = %task.task_type,
    ))]
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let context = task_context!(
            RepositoryOperation::Create,
            task_id = task.id,
            task_name = &task.name
        )
        .with_task_type(task.task_type.clone());

        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let dependencies_json = serde_json::to_string(&task.dependencies)
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let parameters_json = serde_json::to_string(&task.parameters)
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let row = sqlx::query(
            r#"
            INSERT INTO tasks (name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at
            "#,
        )
        .bind(&task.name)
        .bind(&task.task_type)
        .bind(&task.schedule)
        .bind(parameters_json)
        .bind(task.timeout_seconds)
        .bind(task.max_retries)
        .bind(task.status)
        .bind(dependencies_json)
        .bind(shard_config_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        let created_task = Self::row_to_task(&row)?;
        RepositoryErrorHelpers::log_operation_success(
            context,
            &created_task.entity_description(),
            Some(&format!(
                "ID: {}, 类型: {}",
                created_task.id, created_task.task_type
            )),
        );
        Ok(created_task)
    }
    #[instrument(skip(self), fields(task_id = %id))]
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let context = task_context!(RepositoryOperation::Read, task_id = id);

        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        match row {
            Some(row) => {
                let task = Self::row_to_task(&row)?;
                debug!("查询任务成功: ID {}, 名称: {}", task.id, task.name);
                Ok(Some(task))
            }
            None => {
                debug!("查询任务不存在: ID {}", id);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self), fields(task_name = %name))]
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let context = task_context!(RepositoryOperation::Read, task_name = name);

        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        match row {
            Some(row) => {
                let task = Self::row_to_task(&row)?;
                debug!("按名称查询任务成功: {} (ID: {})", task.name, task.id);
                Ok(Some(task))
            }
            None => {
                debug!("按名称查询任务不存在: {}", name);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self, task), fields(
        task_id = %task.id,
        task_name = %task.name,
        task_type = %task.task_type,
    ))]
    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let context = task_context!(
            RepositoryOperation::Update,
            task_id = task.id,
            task_name = &task.name
        )
        .with_task_type(task.task_type.clone());

        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let dependencies_json = serde_json::to_string(&task.dependencies)
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let parameters_json = serde_json::to_string(&task.parameters)
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let result = sqlx::query(
            r#"
            UPDATE tasks 
            SET name = $2, task_type = $3, schedule = $4, parameters = $5, 
                timeout_seconds = $6, max_retries = $7, status = $8, 
                dependencies = $9, shard_config = $10, updated_at = datetime('now')
            WHERE id = $1
            "#,
        )
        .bind(task.id)
        .bind(&task.name)
        .bind(&task.task_type)
        .bind(&task.schedule)
        .bind(parameters_json)
        .bind(task.timeout_seconds)
        .bind(task.max_retries)
        .bind(task.status)
        .bind(dependencies_json)
        .bind(shard_config_json)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success(
            context,
            &task.entity_description(),
            Some(&format!(
                "状态: {:?}, 类型: {}",
                task.status, task.task_type
            )),
        );
        Ok(())
    }
    #[instrument(skip(self), fields(task_id = %id))]
    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let context = task_context!(RepositoryOperation::Delete, task_id = id);

        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success(context, &format!("任务 (ID: {id})"), None);
        Ok(())
    }
    #[instrument(skip(self, filter), fields(
        task_status = ?filter.status,
        task_type = ?filter.task_type,
        name_pattern = ?filter.name_pattern,
        limit = ?filter.limit,
    ))]
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        let context = task_context!(RepositoryOperation::Query)
            .with_additional_info(format!("过滤器: {filter:?}"));

        let mut query = "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE 1=1".to_string();
        let mut bind_count = 0;
        if filter.status.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND status = ${bind_count}"));
        }

        if filter.task_type.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND task_type = ${bind_count}"));
        }

        if filter.name_pattern.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND name LIKE ${bind_count}"));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(_limit) = filter.limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ${bind_count}"));
        }

        if let Some(_offset) = filter.offset {
            bind_count += 1;
            query.push_str(&format!(" OFFSET ${bind_count}"));
        }

        let mut sqlx_query = sqlx::query(&query);
        if let Some(status) = filter.status {
            sqlx_query = sqlx_query.bind(status);
        }

        if let Some(task_type) = &filter.task_type {
            sqlx_query = sqlx_query.bind(task_type);
        }

        if let Some(name_pattern) = &filter.name_pattern {
            sqlx_query = sqlx_query.bind(format!("%{name_pattern}%"));
        }

        if let Some(limit) = filter.limit {
            sqlx_query = sqlx_query.bind(limit);
        }

        if let Some(offset) = filter.offset {
            sqlx_query = sqlx_query.bind(offset);
        }

        let rows = sqlx_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        let tasks: SchedulerResult<Vec<Task>> = rows.iter().map(Self::row_to_task).collect();
        let result = tasks?;
        debug!("查询任务列表成功，返回 {} 个任务", result.len());
        Ok(result)
    }
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            ..Default::default()
        };
        self.list(&filter).await
    }
    async fn get_schedulable_tasks(
        &self,
        _current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>> {
        self.get_active_tasks().await
    }
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        let context = task_context!(RepositoryOperation::Query, task_id = task_id);

        let task = self.get_by_id(task_id).await?;

        if let Some(task) = task {
            if task.dependencies.is_empty() {
                return Ok(true);
            }

            // Optimized batch query for checking dependency statuses
            let dependency_statuses = self
                .get_dependency_run_statuses_batch(&task.dependencies)
                .await?;

            // Check if all dependencies are completed
            for (_, status) in dependency_statuses {
                match status {
                    Some(status_str) if status_str == "COMPLETED" => continue,
                    _ => return Ok(false), // Dependency not completed or never executed
                }
            }

            Ok(true)
        } else {
            Err(RepositoryErrorHelpers::task_not_found(context))
        }
    }
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let context = task_context!(RepositoryOperation::Query, task_id = task_id);

        let task = self.get_by_id(task_id).await?;

        if let Some(task) = task {
            if task.dependencies.is_empty() {
                return Ok(vec![]);
            }

            // Use batch query to get all dependency tasks
            self.get_tasks_by_ids_batch(&task.dependencies).await
        } else {
            Err(RepositoryErrorHelpers::task_not_found(context))
        }
    }
    #[instrument(skip(self, task_ids), fields(
        task_count = %task_ids.len(),
        target_status = ?status,
    ))]
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: TaskStatus,
    ) -> SchedulerResult<()> {
        if task_ids.is_empty() {
            debug!("批量更新任务状态: 任务ID列表为空，跳过操作");
            return Ok(());
        }

        let context = task_context!(RepositoryOperation::BatchUpdate).with_additional_info(
            format!("批量更新 {} 个任务状态为 {:?}", task_ids.len(), status),
        );

        let status_str = match status {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };
        let placeholders: Vec<String> =
            (0..task_ids.len()).map(|i| format!("${}", i + 2)).collect();
        let query = format!(
            "UPDATE tasks SET status = $1, updated_at = datetime('now') WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut sqlx_query = sqlx::query(&query).bind(status_str);
        for &task_id in task_ids {
            sqlx_query = sqlx_query.bind(task_id);
        }

        let result = sqlx_query
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success(
            context,
            "批量任务状态更新",
            Some(&format!(
                "更新了 {} 个任务的状态为 {}",
                result.rows_affected(),
                status_str
            )),
        );
        Ok(())
    }
}
