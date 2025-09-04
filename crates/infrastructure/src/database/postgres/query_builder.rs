// Database-specific query builder for PostgreSQL interval operations

/// PostgreSQL-specific query builder for handling database-specific syntax
pub struct PostgresQueryBuilder;

impl PostgresQueryBuilder {
    /// Build PostgreSQL interval syntax for date/time calculations
    /// 
    /// # Arguments
    /// * `days` - Number of days for the interval (must be positive and reasonable)
    /// 
    /// # Returns
    /// A properly formatted PostgreSQL INTERVAL string
    /// 
    /// # Panics
    /// Panics if days is negative or unreasonably large (> 36500, roughly 100 years)
    pub fn build_interval_days(days: i32) -> String {
        // 安全检查：防止恶意或无效的天数值
        if days < 0 {
            panic!("Days cannot be negative: {}", days);
        }
        if days > 36500 { // 约100年，防止过大的值
            panic!("Days value too large: {}", days);
        }
        format!("INTERVAL '{} days'", days)
    }

    /// Build query for task execution statistics with proper interval syntax
    /// 
    /// # Arguments
    /// * `days` - Number of days to look back for statistics
    /// 
    /// # Returns
    /// A tuple containing the SQL query and the interval string
    pub fn build_execution_stats_query(days: i32) -> String {
        format!(
            r#"
            SELECT 
                COUNT(*) as total_runs,
                COUNT(CASE WHEN status = 'COMPLETED' THEN 1 END) as successful_runs,
                COUNT(CASE WHEN status = 'FAILED' THEN 1 END) as failed_runs,
                COUNT(CASE WHEN status = 'TIMEOUT' THEN 1 END) as timeout_runs,
                AVG(CASE WHEN completed_at IS NOT NULL AND started_at IS NOT NULL 
                    THEN EXTRACT(EPOCH FROM (completed_at - started_at)) * 1000 END) as avg_execution_time_ms,
                MAX(created_at) as last_execution
            FROM task_runs 
            WHERE task_id = $1 AND created_at >= NOW() - {}
            "#,
            Self::build_interval_days(days)
        )
    }

    /// Build query for cleaning up old task runs with proper interval syntax
    /// 
    /// # Arguments
    /// * `days` - Number of days to keep (older records will be deleted)
    /// 
    /// # Returns
    /// A SQL DELETE query with proper PostgreSQL interval syntax
    pub fn build_cleanup_old_runs_query(days: i32) -> String {
        format!(
            "DELETE FROM task_runs WHERE created_at < NOW() - {}",
            Self::build_interval_days(days)
        )
    }

    /// Build query for finding timeout runs with proper time calculations
    /// 
    /// # Arguments
    /// * `timeout_seconds` - Timeout threshold in seconds
    /// 
    /// # Returns
    /// A SQL query for finding timed-out task runs
    pub fn build_timeout_runs_query() -> String {
        r#"
        SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, 
               scheduled_at, started_at, completed_at, result, error_message, created_at 
        FROM task_runs 
        WHERE status = $1 
        AND started_at IS NOT NULL 
        AND EXTRACT(EPOCH FROM (NOW() - started_at)) > $2
        ORDER BY started_at ASC
        "#.to_string()
    }

    /// Build query for getting recent task runs within a time window
    /// 
    /// # Arguments
    /// * `hours` - Number of hours to look back (must be positive and reasonable)
    /// 
    /// # Returns
    /// A SQL query with proper PostgreSQL interval syntax
    /// 
    /// # Panics
    /// Panics if hours is negative or unreasonably large (> 8760, roughly 1 year)
    pub fn build_recent_runs_query(hours: i32) -> String {
        // 安全检查：防止恶意或无效的小时值
        if hours < 0 {
            panic!("Hours cannot be negative: {}", hours);
        }
        if hours > 8760 { // 约1年，防止过大的值
            panic!("Hours value too large: {}", hours);
        }
        format!(
            r#"
            SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total,
                   scheduled_at, started_at, completed_at, result, error_message, created_at
            FROM task_runs 
            WHERE created_at >= NOW() - INTERVAL '{} hours'
            ORDER BY created_at DESC
            "#,
            hours
        )
    }

    /// Build query for task execution trends over time periods
    /// 
    /// # Arguments
    /// * `days` - Number of days to analyze
    /// 
    /// # Returns
    /// A SQL query for analyzing execution trends
    pub fn build_execution_trends_query(days: i32) -> String {
        format!(
            r#"
            SELECT 
                DATE_TRUNC('day', created_at) as execution_date,
                COUNT(*) as total_executions,
                COUNT(CASE WHEN status = 'COMPLETED' THEN 1 END) as successful_executions,
                COUNT(CASE WHEN status = 'FAILED' THEN 1 END) as failed_executions
            FROM task_runs 
            WHERE created_at >= NOW() - {}
            GROUP BY DATE_TRUNC('day', created_at)
            ORDER BY execution_date DESC
            "#,
            Self::build_interval_days(days)
        )
    }

    /// Build query for finding tasks that haven't run recently
    /// 
    /// # Arguments
    /// * `hours` - Number of hours to consider as "recent" (must be positive and reasonable)
    /// 
    /// # Returns
    /// A SQL query for finding stale tasks
    /// 
    /// # Panics
    /// Panics if hours is negative or unreasonably large (> 8760, roughly 1 year)
    pub fn build_stale_tasks_query(hours: i32) -> String {
        // 安全检查：防止恶意或无效的小时值
        if hours < 0 {
            panic!("Hours cannot be negative: {}", hours);
        }
        if hours > 8760 { // 约1年，防止过大的值
            panic!("Hours value too large: {}", hours);
        }
        format!(
            r#"
            SELECT t.id, t.name, t.schedule, MAX(tr.created_at) as last_run
            FROM tasks t
            LEFT JOIN task_runs tr ON t.id = tr.task_id
            WHERE t.status = 'ACTIVE'
            GROUP BY t.id, t.name, t.schedule
            HAVING MAX(tr.created_at) < NOW() - INTERVAL '{} hours' OR MAX(tr.created_at) IS NULL
            ORDER BY last_run ASC NULLS FIRST
            "#,
            hours
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_interval_days() {
        assert_eq!(PostgresQueryBuilder::build_interval_days(7), "INTERVAL '7 days'");
        assert_eq!(PostgresQueryBuilder::build_interval_days(1), "INTERVAL '1 days'");
        assert_eq!(PostgresQueryBuilder::build_interval_days(30), "INTERVAL '30 days'");
    }

    #[test]
    fn test_build_execution_stats_query() {
        let query = PostgresQueryBuilder::build_execution_stats_query(7);
        assert!(query.contains("INTERVAL '7 days'"));
        assert!(query.contains("COUNT(*) as total_runs"));
        assert!(query.contains("WHERE task_id = $1"));
        assert!(!query.contains("%d days")); // Ensure old format is not present
    }

    #[test]
    fn test_build_cleanup_old_runs_query() {
        let query = PostgresQueryBuilder::build_cleanup_old_runs_query(30);
        assert!(query.contains("INTERVAL '30 days'"));
        assert!(query.contains("DELETE FROM task_runs"));
        assert!(!query.contains("%d days")); // Ensure old format is not present
    }

    #[test]
    fn test_build_timeout_runs_query() {
        let query = PostgresQueryBuilder::build_timeout_runs_query();
        assert!(query.contains("EXTRACT(EPOCH FROM (NOW() - started_at)) > $2"));
        assert!(query.contains("WHERE status = $1"));
        assert!(query.contains("started_at IS NOT NULL"));
    }

    #[test]
    fn test_build_recent_runs_query() {
        let query = PostgresQueryBuilder::build_recent_runs_query(24);
        assert!(query.contains("INTERVAL '24 hours'"));
        assert!(query.contains("ORDER BY created_at DESC"));
    }

    #[test]
    fn test_build_execution_trends_query() {
        let query = PostgresQueryBuilder::build_execution_trends_query(7);
        assert!(query.contains("INTERVAL '7 days'"));
        assert!(query.contains("DATE_TRUNC('day', created_at)"));
        assert!(query.contains("GROUP BY DATE_TRUNC('day', created_at)"));
    }

    #[test]
    fn test_build_stale_tasks_query() {
        let query = PostgresQueryBuilder::build_stale_tasks_query(48);
        assert!(query.contains("INTERVAL '48 hours'"));
        assert!(query.contains("LEFT JOIN task_runs"));
        assert!(query.contains("HAVING MAX(tr.created_at)"));
    }

    #[test]
    #[should_panic(expected = "Days cannot be negative")]
    fn test_build_interval_days_negative() {
        PostgresQueryBuilder::build_interval_days(-1);
    }

    #[test]
    #[should_panic(expected = "Days value too large")]
    fn test_build_interval_days_too_large() {
        PostgresQueryBuilder::build_interval_days(50000);
    }

    #[test]
    #[should_panic(expected = "Hours cannot be negative")]
    fn test_build_recent_runs_query_negative() {
        PostgresQueryBuilder::build_recent_runs_query(-1);
    }

    #[test]
    #[should_panic(expected = "Hours value too large")]
    fn test_build_recent_runs_query_too_large() {
        PostgresQueryBuilder::build_recent_runs_query(10000);
    }

    #[test]
    #[should_panic(expected = "Hours cannot be negative")]
    fn test_build_stale_tasks_query_negative() {
        PostgresQueryBuilder::build_stale_tasks_query(-1);
    }

    #[test]
    #[should_panic(expected = "Hours value too large")]
    fn test_build_stale_tasks_query_too_large() {
        PostgresQueryBuilder::build_stale_tasks_query(10000);
    }
}