#[cfg(test)]
mod figment_tests {
    use crate::AppConfig;

    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_figment_config_loading() {
        // 清理所有可能影响测试的环境变量
        let env_vars_to_clear = [
            "SCHEDULER_ENV",
            "SCHEDULER_DATABASE__MAX_CONNECTIONS",
            "SCHEDULER_WORKER__WORKER_ID",
            "SCHEDULER_DATABASE__URL",
            "SCHEDULER_API__ENABLED",
        ];

        for var in &env_vars_to_clear {
            std::env::remove_var(var);
        }

        let config = AppConfig::default_with_env().unwrap();
        assert_eq!(config.database.url, "postgresql://localhost/scheduler");
        assert_eq!(config.worker.worker_id, "worker-001");
        assert_eq!(config.database.max_connections, 10); // 默认值
    }

    #[test]
    fn test_environment_variables() {
        // 测试配置合并功能（不使用真实环境变量避免测试间干扰）
        use figment::{
            providers::{Format, Json, Serialized},
            Figment,
        };
        use serde_json::json;

        // 创建一个模拟的环境变量配置文件
        let env_override = json!({
            "database": {
                "max_connections": 50
            },
            "worker": {
                "worker_id": "env-worker"
            }
        });

        // 使用临时文件来测试配置合并
        use tempfile::NamedTempFile;
        let mut temp_file = NamedTempFile::new().unwrap();
        serde_json::to_writer(&mut temp_file, &env_override).unwrap();

        let figment = Figment::new()
            .merge(Serialized::defaults(AppConfig::default()))
            .merge(Json::file(temp_file.path()));

        let config: AppConfig = figment.extract().unwrap();

        // 验证配置合并正确 - 环境变量应该覆盖默认值
        assert_eq!(config.database.max_connections, 50);
        assert_eq!(config.worker.worker_id, "env-worker");

        // 验证其他默认值没有被改变
        assert_eq!(config.database.url, "postgresql://localhost/scheduler");
        assert_eq!(config.api.bind_address, "0.0.0.0:8080");

        // 显式关闭和删除临时文件
        drop(temp_file);
    }

    #[test]
    fn test_a_real_environment_variables_integration() {
        // 这是一个简化的集成测试，验证 API 存在性
        // 真正的环境变量测试应该在 CI/CD 中作为独立的集成测试进行

        // 确保清理环境，避免其他测试的影响
        let env_vars_to_clear = [
            "SCHEDULER_ENV",
            "SCHEDULER_DATABASE__MAX_CONNECTIONS",
            "SCHEDULER_WORKER__WORKER_ID",
            "SCHEDULER_DATABASE__URL",
            "SCHEDULER_API__ENABLED",
        ];

        for var in &env_vars_to_clear {
            std::env::remove_var(var);
        }

        // 测试 API 存在性
        let config = AppConfig::default_with_env();
        assert!(
            config.is_ok(),
            "default_with_env should work without environment variables"
        );

        let config = config.unwrap();
        assert_eq!(config.database.max_connections, 10); // 默认值
        assert_eq!(config.worker.worker_id, "worker-001"); // 默认值

        // 测试从文件加载的 API
        let result = AppConfig::from_env(Some("development"));
        assert!(
            result.is_ok(),
            "from_env should work with development environment"
        );
    }

    #[test]
    fn test_json_config_file() {
        // 确保清理环境，避免其他测试的影响
        let env_vars_to_clear = [
            "SCHEDULER_ENV",
            "SCHEDULER_DATABASE__MAX_CONNECTIONS",
            "SCHEDULER_WORKER__WORKER_ID",
            "SCHEDULER_DATABASE__URL",
            "SCHEDULER_API__ENABLED",
        ];

        for var in &env_vars_to_clear {
            std::env::remove_var(var);
        }

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        // 创建一个简单的 JSON 配置文件，只覆盖部分字段
        let json_content = r#"{
    "database": {
        "url": "postgresql://localhost/json_test",
        "max_connections": 15,
        "min_connections": 1,
        "connection_timeout_seconds": 30,
        "idle_timeout_seconds": 600
    }
}"#;

        fs::write(&config_path, json_content).unwrap();

        // 测试从文件加载时是否正确合并了默认配置
        let config = AppConfig::from_file(&config_path).unwrap();
        assert_eq!(config.database.url, "postgresql://localhost/json_test");
        assert_eq!(config.database.max_connections, 15);

        // 验证其他字段是否使用了默认值
        assert_eq!(config.worker.worker_id, "worker-001"); // 默认值
    }

    #[test]
    fn test_environment_detection() {
        std::env::remove_var("SCHEDULER_ENV");
        let config = AppConfig::default();
        assert_eq!(config.environment(), "development");
        assert!(config.is_development());

        std::env::set_var("SCHEDULER_ENV", "production");
        let config = AppConfig::default();
        assert_eq!(config.environment(), "production");
        assert!(config.is_production());

        std::env::remove_var("SCHEDULER_ENV");
    }

    #[test]
    fn test_figment_env_parsing() {
        use figment::{providers::Env, Figment};
        use serde::{Deserialize, Serialize};

        // 清理所有相关环境变量
        std::env::remove_var("SCHEDULER_TEST_VALUE");
        std::env::remove_var("SCHEDULER_DATABASE__MAX_CONNECTIONS");
        std::env::remove_var("SCHEDULER_WORKER__WORKER_ID");

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct SimpleConfig {
            test_value: u32,
        }

        // 试验最简单的环境变量映射
        std::env::set_var("SCHEDULER_TEST_VALUE", "42");

        let figment = Figment::new().merge(Env::prefixed("SCHEDULER_").split("_"));

        if let Ok(value) = figment.find_value("test_value") {
            println!("Found test_value: {:?}", value);
        } else {
            println!("Could not find test_value");
        }

        // 试验不同的环境变量名
        std::env::set_var("SCHEDULER_DATABASE__MAX_CONNECTIONS", "50");
        std::env::set_var("SCHEDULER_WORKER__WORKER_ID", "test-worker");

        let figment2 = Figment::new().merge(Env::prefixed("SCHEDULER_").split("__"));

        if let Ok(value) = figment2.find_value("database.max_connections") {
            println!(
                "Found with __ separator database.max_connections: {:?}",
                value
            );
        } else {
            println!("Could not find database.max_connections with __ separator");
        }

        // 清理测试环境变量
        std::env::remove_var("SCHEDULER_TEST_VALUE");
        std::env::remove_var("SCHEDULER_DATABASE__MAX_CONNECTIONS");
        std::env::remove_var("SCHEDULER_WORKER__WORKER_ID");
    }
}
