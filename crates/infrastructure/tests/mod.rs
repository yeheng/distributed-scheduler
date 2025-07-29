// 集成测试模块
pub mod database_integration_tests;
pub mod dispatcher_worker_integration_tests;
pub mod end_to_end_tests;
pub mod migration_tests;

// 测试辅助函数和共享设置
use std::sync::Once;
use tracing_subscriber;

static INIT: Once = Once::new();

/// 初始化测试环境的日志记录
pub fn init_test_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .init();
    });
}

/// 测试配置常量
pub mod test_config {
    pub const TEST_DB_NAME: &str = "scheduler_integration_test";
    pub const TEST_DB_USER: &str = "test_user";
    pub const TEST_DB_PASSWORD: &str = "test_password";
    pub const TEST_WORKER_ID: &str = "integration-test-worker";
    pub const TEST_TASK_TIMEOUT: u64 = 300;
    pub const TEST_MAX_RETRIES: i32 = 3;
}
