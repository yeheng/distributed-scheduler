
use scheduler_domain::entities::TaskFilter;
use scheduler_domain::entities::TaskStatus;

pub struct TaskQueryBuilder;

impl TaskQueryBuilder {
    pub fn build_select_query(filter: &TaskFilter) -> (String, Vec<TaskQueryParam>) {
        let mut query = "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE 1=1".to_string();
        let mut params = Vec::new();
        if let Some(status) = filter.status {
            query.push_str(" AND status = $");
            query.push_str(&(params.len() + 1).to_string());
            params.push(TaskQueryParam::Status(status));
        }
        if let Some(task_type) = &filter.task_type {
            query.push_str(" AND task_type = $");
            query.push_str(&(params.len() + 1).to_string());
            params.push(TaskQueryParam::String(task_type.clone()));
        }
        if let Some(name_pattern) = &filter.name_pattern {
            query.push_str(" AND name ILIKE $");
            query.push_str(&(params.len() + 1).to_string());
            params.push(TaskQueryParam::String(format!("%{name_pattern}%")));
        }
        query.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            query.push_str(" LIMIT $");
            query.push_str(&(params.len() + 1).to_string());
            params.push(TaskQueryParam::Int64(limit));
        }
        if let Some(offset) = filter.offset {
            query.push_str(" OFFSET $");
            query.push_str(&(params.len() + 1).to_string());
            params.push(TaskQueryParam::Int64(offset));
        }

        (query, params)
    }
    pub fn build_batch_update_query() -> String {
        "UPDATE tasks SET status = $1, updated_at = NOW() WHERE id = ANY($2)".to_string()
    }
    pub fn build_dependency_check_query() -> String {
        "SELECT status FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1"
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub enum TaskQueryParam {
    String(String),
    Status(TaskStatus),
    Int64(i64),
    Int32(i32),
}

impl TaskQueryParam {
    pub fn type_name(&self) -> &'static str {
        match self {
            TaskQueryParam::String(_) => "TEXT",
            TaskQueryParam::Status(_) => "TEXT",
            TaskQueryParam::Int64(_) => "BIGINT",
            TaskQueryParam::Int32(_) => "INTEGER",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_select_query_no_filter() {
        let filter = TaskFilter::default();
        let (query, params) = TaskQueryBuilder::build_select_query(&filter);

        assert!(query.contains("SELECT id, name, task_type, schedule"));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_build_select_query_with_status() {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            ..Default::default()
        };

        let (query, params) = TaskQueryBuilder::build_select_query(&filter);

        assert!(query.contains("AND status = $1"));
        assert_eq!(params.len(), 1);
        matches!(params[0], TaskQueryParam::Status(TaskStatus::Active));
    }

    #[test]
    fn test_build_select_query_with_multiple_filters() {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            task_type: Some("test_type".to_string()),
            limit: Some(10),
            ..Default::default()
        };

        let (query, params) = TaskQueryBuilder::build_select_query(&filter);

        assert!(query.contains("AND status = $1"));
        assert!(query.contains("AND task_type = $2"));
        assert!(query.contains("LIMIT $3"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_build_batch_update_query() {
        let query = TaskQueryBuilder::build_batch_update_query();
        assert!(query.contains("UPDATE tasks SET status = $1"));
        assert!(query.contains("WHERE id = ANY($2)"));
    }

    #[test]
    fn test_build_dependency_check_query() {
        let query = TaskQueryBuilder::build_dependency_check_query();
        assert!(query.contains("SELECT status FROM task_runs"));
        assert!(query.contains("WHERE task_id = $1"));
        assert!(query.contains("ORDER BY created_at DESC LIMIT 1"));
    }
}
