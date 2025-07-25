use crate::config::AppConfig;
use anyhow::{Context, Result};
use std::env;

/// 配置加载器，提供便捷的配置加载方法
pub struct ConfigLoader;

impl ConfigLoader {
    /// 根据环境加载配置
    ///
    /// 优先级：
    /// 1. 环境变量 SCHEDULER_CONFIG_PATH 指定的配置文件
    /// 2. 环境变量 SCHEDULER_ENV 指定的环境配置文件
    /// 3. 默认配置文件
    pub fn load() -> Result<AppConfig> {
        // 检查是否指定了配置文件路径
        if let Ok(config_path) = env::var("SCHEDULER_CONFIG_PATH") {
            return AppConfig::load(Some(&config_path))
                .with_context(|| format!("加载指定配置文件失败: {config_path}"));
        }

        // 检查环境变量
        let env_name = env::var("SCHEDULER_ENV").unwrap_or_else(|_| "development".to_string());
        let config_file = format!("config/{env_name}.toml");

        // 尝试加载环境特定的配置文件
        if std::path::Path::new(&config_file).exists() {
            AppConfig::load(Some(&config_file))
                .with_context(|| format!("加载环境配置文件失败: {config_file}"))
        } else {
            // 回退到默认配置
            AppConfig::load(None).context("加载默认配置失败")
        }
    }

    /// 加载指定环境的配置
    pub fn load_for_env(env: &str) -> Result<AppConfig> {
        let config_file = format!("config/{env}.toml");
        AppConfig::load(Some(&config_file)).with_context(|| format!("加载环境配置失败: {env}"))
    }

    /// 验证配置并返回组件特定的配置
    pub fn load_and_validate() -> Result<AppConfig> {
        let config = Self::load()?;
        config.validate().context("配置验证失败")?;
        Ok(config)
    }

    /// 获取数据库连接字符串，支持环境变量覆盖
    pub fn get_database_url(config: &AppConfig) -> String {
        env::var("DATABASE_URL").unwrap_or_else(|_| config.database.url.clone())
    }

    /// 获取消息队列连接字符串，支持环境变量覆盖
    pub fn get_message_queue_url(config: &AppConfig) -> String {
        env::var("RABBITMQ_URL")
            .or_else(|_| env::var("AMQP_URL"))
            .unwrap_or_else(|_| config.message_queue.url.clone())
    }

    /// 检查是否为开发环境
    pub fn is_development() -> bool {
        env::var("SCHEDULER_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase()
            == "development"
    }

    /// 检查是否为生产环境
    pub fn is_production() -> bool {
        env::var("SCHEDULER_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase()
            == "production"
    }

    /// 获取当前环境名称
    pub fn current_env() -> String {
        env::var("SCHEDULER_ENV").unwrap_or_else(|_| "development".to_string())
    }
}
