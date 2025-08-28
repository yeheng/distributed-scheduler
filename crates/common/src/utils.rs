//! # 通用工具函数
//!
//! 包含系统中常用的辅助函数

use chrono::{DateTime, Utc};
use scheduler_errors::{SchedulerError, SchedulerResult};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::types::{Environment, Timestamp};

/// 时间工具函数
pub mod time {
    use super::*;

    /// 获取当前UTC时间戳
    pub fn now_utc() -> Timestamp {
        Utc::now()
    }

    /// 将系统时间转换为UTC时间
    pub fn system_time_to_utc(system_time: SystemTime) -> SchedulerResult<Timestamp> {
        let duration = system_time
            .duration_since(UNIX_EPOCH)
            .map_err(|e| SchedulerError::Internal(format!("Invalid system time: {}", e)))?;

        let timestamp = duration.as_secs() as i64;
        DateTime::from_timestamp(timestamp, 0)
            .ok_or_else(|| SchedulerError::Internal("Failed to convert timestamp".to_string()))
    }

    /// 格式化时间为可读字符串
    pub fn format_timestamp(timestamp: &Timestamp) -> String {
        timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    /// 解析时间字符串
    pub fn parse_timestamp(s: &str) -> SchedulerResult<Timestamp> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                SchedulerError::ValidationError(format!("Invalid timestamp format: {}", e))
            })
    }

    /// 计算两个时间之间的持续时间（秒）
    pub fn duration_seconds(start: &Timestamp, end: &Timestamp) -> i64 {
        (end.timestamp() - start.timestamp()).abs()
    }

    /// 检查时间是否在指定范围内
    pub fn is_within_range(time: &Timestamp, start: &Timestamp, end: &Timestamp) -> bool {
        time >= start && time <= end
    }
}

/// 字符串工具函数
pub mod string {
    use super::*;

    /// 生成随机字符串
    pub fn generate_random_string(length: usize) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789";
        let mut rng = rand::rng();

        (0..length)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// 生成UUID字符串
    pub fn generate_uuid() -> String {
        Uuid::new_v4().to_string()
    }

    /// 验证字符串是否为有效的UUID
    pub fn is_valid_uuid(s: &str) -> bool {
        Uuid::parse_str(s).is_ok()
    }

    /// 清理字符串（移除多余空白字符）
    pub fn clean_string(s: &str) -> String {
        s.trim().replace('\n', " ").replace('\t', " ")
    }

    /// 截断字符串到指定长度
    pub fn truncate_string(s: &str, max_length: usize) -> String {
        if s.len() <= max_length {
            s.to_string()
        } else {
            format!("{}...", &s[..max_length.saturating_sub(3)])
        }
    }

    /// 转换为snake_case
    pub fn to_snake_case(s: &str) -> String {
        let mut result = String::new();
        let mut prev_char_was_uppercase = false;

        for (i, c) in s.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 && !prev_char_was_uppercase {
                    result.push('_');
                }
                result.push(c.to_lowercase().next().unwrap());
                prev_char_was_uppercase = true;
            } else {
                result.push(c);
                prev_char_was_uppercase = false;
            }
        }

        result
    }
}

/// JSON工具函数
pub mod json {
    use super::*;

    /// 安全地序列化为JSON字符串
    pub fn safe_serialize<T: Serialize>(value: &T) -> SchedulerResult<String> {
        serde_json::to_string(value)
            .map_err(|e| SchedulerError::Internal(format!("JSON serialization failed: {}", e)))
    }

    /// 美化的JSON序列化
    pub fn pretty_serialize<T: Serialize>(value: &T) -> SchedulerResult<String> {
        serde_json::to_string_pretty(value)
            .map_err(|e| SchedulerError::Internal(format!("JSON serialization failed: {}", e)))
    }

    /// 安全地反序列化JSON字符串
    pub fn safe_deserialize<T: for<'de> Deserialize<'de>>(s: &str) -> SchedulerResult<T> {
        serde_json::from_str(s).map_err(|e| {
            SchedulerError::ValidationError(format!("JSON deserialization failed: {}", e))
        })
    }

    /// 合并两个JSON对象
    pub fn merge_json_objects(
        base: &serde_json::Value,
        overlay: &serde_json::Value,
    ) -> serde_json::Value {
        match (base, overlay) {
            (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
                let mut result = base_map.clone();
                for (key, value) in overlay_map {
                    result.insert(key.clone(), value.clone());
                }
                serde_json::Value::Object(result)
            }
            _ => overlay.clone(),
        }
    }
}

/// 环境变量工具函数
pub mod env {
    use super::*;

    /// 获取环境变量，支持默认值
    pub fn get_env_var(key: &str, default: Option<&str>) -> String {
        std::env::var(key).unwrap_or_else(|_| default.unwrap_or_default().to_string())
    }

    /// 获取环境变量并解析为指定类型
    pub fn get_env_var_as<T: std::str::FromStr>(key: &str, default: T) -> T
    where
        T::Err: std::fmt::Debug,
    {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    /// 检测当前运行环境
    pub fn detect_environment() -> Environment {
        let env_str = get_env_var("SCHEDULER_ENV", Some("development"));
        env_str.parse().unwrap_or(Environment::Development)
    }

    /// 检查是否为开发环境
    pub fn is_development() -> bool {
        matches!(detect_environment(), Environment::Development)
    }

    /// 检查是否为生产环境
    pub fn is_production() -> bool {
        matches!(detect_environment(), Environment::Production)
    }
}

/// 网络工具函数
pub mod network {
    use super::*;

    /// 检查端口是否可用
    pub fn is_port_available(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// 获取本机IP地址
    pub fn get_local_ip() -> SchedulerResult<String> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| SchedulerError::Internal(format!("Failed to create socket: {}", e)))?;

        socket
            .connect("8.8.8.8:80")
            .map_err(|e| SchedulerError::Internal(format!("Failed to connect: {}", e)))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| SchedulerError::Internal(format!("Failed to get local address: {}", e)))?;

        Ok(local_addr.ip().to_string())
    }

    /// 获取主机名
    pub fn get_hostname() -> SchedulerResult<String> {
        hostname::get()
            .map_err(|e| SchedulerError::Internal(format!("Failed to get hostname: {}", e)))?
            .to_string_lossy()
            .into_owned()
            .pipe(Ok)
    }
}

/// 验证工具函数
pub mod validation {
    use super::*;

    /// 验证任务名称
    pub fn validate_task_name(name: &str) -> SchedulerResult<()> {
        if name.is_empty() {
            return Err(SchedulerError::ValidationError(
                "Task name cannot be empty".to_string(),
            ));
        }

        if name.len() > crate::constants::MAX_TASK_NAME_LENGTH {
            return Err(SchedulerError::ValidationError(format!(
                "Task name too long: {} > {}",
                name.len(),
                crate::constants::MAX_TASK_NAME_LENGTH
            )));
        }

        // 检查是否包含有效字符
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(SchedulerError::ValidationError(
                "Task name can only contain alphanumeric characters, underscore, hyphen, and dot"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// 验证Worker ID
    pub fn validate_worker_id(worker_id: &str) -> SchedulerResult<()> {
        if worker_id.is_empty() {
            return Err(SchedulerError::ValidationError(
                "Worker ID cannot be empty".to_string(),
            ));
        }

        if worker_id.len() > crate::constants::MAX_WORKER_ID_LENGTH {
            return Err(SchedulerError::ValidationError(format!(
                "Worker ID too long: {} > {}",
                worker_id.len(),
                crate::constants::MAX_WORKER_ID_LENGTH
            )));
        }

        Ok(())
    }

    /// 验证Cron表达式格式
    pub fn validate_cron_expression(expr: &str) -> SchedulerResult<()> {
        let fields: Vec<&str> = expr.split_whitespace().collect();

        if fields.len() != crate::constants::CRON_FIELDS_COUNT {
            return Err(SchedulerError::ValidationError(format!(
                "Cron expression must have {} fields, got {}",
                crate::constants::CRON_FIELDS_COUNT,
                fields.len()
            )));
        }

        // 这里可以添加更详细的Cron表达式验证
        // 目前只做基本的字段数量检查

        Ok(())
    }
}

/// 扩展trait，为类型添加pipeline操作
pub trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_utils() {
        let now = time::now_utc();
        let formatted = time::format_timestamp(&now);
        assert!(!formatted.is_empty());

        let parsed = time::parse_timestamp(&now.to_rfc3339()).unwrap();
        assert_eq!(now.timestamp(), parsed.timestamp());
    }

    #[test]
    fn test_string_utils() {
        let random = string::generate_random_string(10);
        assert_eq!(random.len(), 10);

        let uuid = string::generate_uuid();
        assert!(string::is_valid_uuid(&uuid));

        assert_eq!(string::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(string::truncate_string("hello world", 5), "he...");
    }

    #[test]
    fn test_json_utils() {
        let value = serde_json::json!({"key": "value"});
        let serialized = json::safe_serialize(&value).unwrap();
        let deserialized: serde_json::Value = json::safe_deserialize(&serialized).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_env_utils() {
        let env = env::detect_environment();
        assert!(matches!(
            env,
            Environment::Development
                | Environment::Testing
                | Environment::Staging
                | Environment::Production
        ));
    }

    #[test]
    fn test_validation_utils() {
        assert!(validation::validate_task_name("valid_task").is_ok());
        assert!(validation::validate_task_name("").is_err());
        assert!(validation::validate_worker_id("worker-1").is_ok());
        assert!(validation::validate_worker_id("").is_err());
    }

    #[test]
    fn test_pipe_trait() {
        let result = "hello"
            .pipe(|s| s.to_uppercase())
            .pipe(|s| format!("{}_world", s));
        assert_eq!(result, "HELLO_world");
    }
}
