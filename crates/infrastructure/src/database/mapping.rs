//! Shared database mapping utilities to reduce code duplication
//! 
//! This module provides helper functions for parsing complex JSON fields
//! from database rows, handling the differences between PostgreSQL and SQLite.

use scheduler_foundation::SchedulerResult;
use scheduler_domain::entities::ShardConfig;
use scheduler_errors::SchedulerError;

/// Helper functions for parsing database fields across different database types
pub struct MappingHelpers;

impl MappingHelpers {
    /// Parse dependencies field from either Vec<i64> (PostgreSQL) or JSON string (SQLite)
    pub fn parse_dependencies_postgres(row: &sqlx::postgres::PgRow, field_name: &str) -> Vec<i64> {
        use sqlx::Row;
        row.try_get::<Vec<i64>, _>(field_name).unwrap_or_default()
    }

    pub fn parse_dependencies_sqlite(row: &sqlx::sqlite::SqliteRow, field_name: &str) -> Vec<i64> {
        use sqlx::Row;
        if let Ok(Some(json_str)) = row.try_get::<Option<String>, _>(field_name) {
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Parse parameters field from either serde_json::Value (PostgreSQL) or JSON string (SQLite)
    pub fn parse_parameters_postgres(
        row: &sqlx::postgres::PgRow, 
        field_name: &str
    ) -> SchedulerResult<serde_json::Value> {
        use sqlx::Row;
        Ok(row.try_get(field_name)?)
    }

    pub fn parse_parameters_sqlite(
        row: &sqlx::sqlite::SqliteRow, 
        field_name: &str
    ) -> SchedulerResult<serde_json::Value> {
        use sqlx::Row;
        let json_str: String = row.try_get(field_name)?;
        json_str.parse()
            .map_err(|e| SchedulerError::Serialization(format!("解析参数失败: {e}")))
    }

    /// Parse shard_config field from either serde_json::Value (PostgreSQL) or JSON string (SQLite)
    pub fn parse_shard_config_postgres(
        row: &sqlx::postgres::PgRow, 
        field_name: &str
    ) -> SchedulerResult<Option<ShardConfig>> {
        use sqlx::Row;
        let config_value = row.try_get::<Option<serde_json::Value>, _>(field_name)
            .ok()
            .flatten();
        
        match config_value {
            Some(value) => serde_json::from_value(value)
                .map_err(|e| SchedulerError::Serialization(format!("解析分片配置失败: {e}")))
                .map(Some),
            None => Ok(None),
        }
    }

    pub fn parse_shard_config_sqlite(
        row: &sqlx::sqlite::SqliteRow, 
        field_name: &str
    ) -> SchedulerResult<Option<ShardConfig>> {
        use sqlx::Row;
        if let Ok(Some(json_str)) = row.try_get::<Option<String>, _>(field_name) {
            serde_json::from_str(&json_str)
                .map_err(|e| SchedulerError::Serialization(format!("解析分片配置失败: {e}")))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse supported_task_types field from either Vec<String> (PostgreSQL) or JSON string (SQLite)
    pub fn parse_supported_task_types_postgres(
        row: &sqlx::postgres::PgRow, 
        field_name: &str
    ) -> Vec<String> {
        use sqlx::Row;
        row.try_get::<Vec<String>, _>(field_name).unwrap_or_default()
    }

    pub fn parse_supported_task_types_sqlite(
        row: &sqlx::sqlite::SqliteRow, 
        field_name: &str
    ) -> SchedulerResult<Vec<String>> {
        use sqlx::Row;
        if let Ok(Some(json_str)) = row.try_get::<Option<String>, _>(field_name) {
            serde_json::from_str(&json_str)
                .map_err(|e| SchedulerError::Serialization(format!("解析支持的任务类型失败: {e}")))
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_dependencies_sqlite() {
        // Test would require actual SQLite row, so we keep this as integration test
    }
}