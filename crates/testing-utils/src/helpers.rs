//! Test helper utilities and common testing patterns
//!
//! This module provides utilities for setting up test environments,
//! managing test data, and implementing common testing patterns.

use chrono::{DateTime, Utc};
use std::time::Duration;
use tokio::time::sleep;

/// Test environment setup utilities
pub struct TestEnv;

impl TestEnv {
    /// Wait for a condition to be true with timeout
    /// 
    /// This is useful for integration tests where you need to wait for
    /// asynchronous operations to complete.
    pub async fn wait_for<F, Fut>(mut condition: F, timeout: Duration) -> bool
    where
        F: FnMut() -> Fut, // Changed from Fn to FnMut
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            if condition().await {
                return true;
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        false
    }

    /// Wait for a condition with a custom poll interval
    pub async fn wait_for_with_interval<F, Fut>(
        condition: F, 
        timeout: Duration, 
        poll_interval: Duration
    ) -> bool
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            if condition().await {
                return true;
            }
            sleep(poll_interval).await;
        }
        
        false
    }

    /// Generate unique test names based on timestamp
    pub fn unique_name(prefix: &str) -> String {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        format!("{}_{}", prefix, timestamp)
    }

    /// Generate test timestamps with offsets
    pub fn timestamp_with_offset(offset_seconds: i64) -> DateTime<Utc> {
        Utc::now() + chrono::Duration::seconds(offset_seconds)
    }
}

/// Utilities for managing test data and state
pub struct TestData;

impl TestData {
    /// Create a vector of sequential IDs
    pub fn sequential_ids(start: i64, count: usize) -> Vec<i64> {
        (start..start + count as i64).collect()
    }

    /// Create test JSON parameters
    pub fn json_params(params: &[(&str, serde_json::Value)]) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (key, value) in params {
            map.insert(key.to_string(), value.clone());
        }
        serde_json::Value::Object(map)
    }

    /// Create simple string parameters
    pub fn string_params(params: &[(&str, &str)]) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (key, value) in params {
            map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        }
        serde_json::Value::Object(map)
    }
}

/// Assertion helpers for common testing patterns
pub struct TestAssertions;

impl TestAssertions {
    /// Assert that a collection contains exactly the expected items (order independent)
    pub fn assert_contains_exactly<T: PartialEq + std::fmt::Debug>(
        actual: &[T], 
        expected: &[T]
    ) {
        assert_eq!(
            actual.len(), 
            expected.len(),
            "Collections have different lengths. Actual: {:?}, Expected: {:?}",
            actual,
            expected
        );

        for expected_item in expected {
            assert!(
                actual.contains(expected_item),
                "Expected item {:?} not found in actual collection {:?}",
                expected_item,
                actual
            );
        }
    }

    /// Assert that all items in a collection satisfy a predicate
    pub fn assert_all<T, P>(items: &[T], predicate: P, message: &str)
    where
        P: Fn(&T) -> bool,
        T: std::fmt::Debug,
    {
        for item in items {
            assert!(
                predicate(item),
                "{}: Failed for item {:?}",
                message,
                item
            );
        }
    }

    /// Assert that at least one item in a collection satisfies a predicate
    pub fn assert_any<T, P>(items: &[T], predicate: P, message: &str)
    where
        P: Fn(&T) -> bool,
        T: std::fmt::Debug,
    {
        assert!(
            items.iter().any(predicate),
            "{}: No item satisfied the predicate in {:?}",
            message,
            items
        );
    }

    /// Assert that no items in a collection satisfy a predicate
    pub fn assert_none<T, P>(items: &[T], predicate: P, message: &str)
    where
        P: Fn(&T) -> bool,
        T: std::fmt::Debug,
    {
        assert!(
            !items.iter().any(predicate),
            "{}: At least one item satisfied the predicate in {:?}",
            message,
            items
        );
    }
}

/// Integration test setup helpers
pub struct IntegrationTestSetup;

impl IntegrationTestSetup {
    /// Set up logging for tests (call once per test binary)
    pub fn init_logging() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("debug")
            .try_init();
    }

    /// Create a test configuration for services
    pub fn create_test_config() -> serde_json::Value {
        serde_json::json!({
            "database": {
                "max_connections": 5,
                "min_connections": 1,
                "connection_timeout_seconds": 10,
                "idle_timeout_seconds": 300
            },
            "message_queue": {
                "queue_name": "test_tasks",
                "prefetch_count": 10
            },
            "scheduler": {
                "poll_interval_seconds": 1,
                "max_concurrent_tasks": 10
            },
            "worker": {
                "heartbeat_interval_seconds": 5,
                "max_concurrent_tasks": 3,
                "supported_task_types": ["shell", "http"]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_wait_for_success() {
        let mut counter = 0;
        let condition = || {
            counter += 1;
            async move { counter >= 3 }
        };

        let result = TestEnv::wait_for(condition, Duration::from_millis(500)).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_wait_for_timeout() {
        let condition = || async { false };
        let result = TestEnv::wait_for(condition, Duration::from_millis(100)).await;
        assert!(!result);
    }

    #[test]
    fn test_unique_name() {
        let name1 = TestEnv::unique_name("test");
        let name2 = TestEnv::unique_name("test");
        
        assert!(name1.starts_with("test_"));
        assert!(name2.starts_with("test_"));
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_sequential_ids() {
        let ids = TestData::sequential_ids(5, 3);
        assert_eq!(ids, vec![5, 6, 7]);
    }

    #[test]
    fn test_json_params() {
        let params = TestData::json_params(&[
            ("key1", serde_json::Value::String("value1".to_string())),
            ("key2", serde_json::Value::Number(serde_json::Number::from(42))),
        ]);

        assert_eq!(params["key1"], "value1");
        assert_eq!(params["key2"], 42);
    }

    #[test]
    fn test_string_params() {
        let params = TestData::string_params(&[
            ("name", "test"),
            ("type", "shell"),
        ]);

        assert_eq!(params["name"], "test");
        assert_eq!(params["type"], "shell");
    }

    #[test]
    fn test_assert_contains_exactly() {
        let actual = vec![1, 2, 3];
        let expected = vec![3, 1, 2];
        
        TestAssertions::assert_contains_exactly(&actual, &expected);
    }

    #[test]
    fn test_assert_all() {
        let numbers = vec![2, 4, 6, 8];
        TestAssertions::assert_all(&numbers, |&n| n % 2 == 0, "All numbers should be even");
    }

    #[test]
    fn test_assert_any() {
        let numbers = vec![1, 3, 4, 7];
        TestAssertions::assert_any(&numbers, |&n| n % 2 == 0, "At least one number should be even");
    }

    #[test]
    fn test_assert_none() {
        let numbers = vec![1, 3, 5, 7];
        TestAssertions::assert_none(&numbers, |&n| n % 2 == 0, "No numbers should be even");
    }
}