use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::time::interval;
use tracing::{error, info, warn};

/// 日志轮转配置
#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    /// 日志文件路径
    pub log_file_path: PathBuf,
    /// 最大文件大小（字节）
    pub max_file_size_bytes: u64,
    /// 保留的日志文件数量
    pub max_files: u32,
    /// 轮转检查间隔（秒）
    pub check_interval_seconds: u64,
    /// 是否启用压缩
    pub compress_rotated_files: bool,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            log_file_path: PathBuf::from("scheduler.log"),
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            max_files: 10,
            check_interval_seconds: 300, // 5分钟
            compress_rotated_files: false,
        }
    }
}

/// 日志轮转管理器
pub struct LogRotationManager {
    config: LogRotationConfig,
    current_writer: Arc<Mutex<Option<BufWriter<File>>>>,
    is_running: Arc<Mutex<bool>>,
}

impl LogRotationManager {
    /// 创建新的日志轮转管理器
    pub fn new(config: LogRotationConfig) -> Result<Self> {
        // 确保日志目录存在
        if let Some(parent) = config.log_file_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create log directory")?;
        }

        Ok(Self {
            config,
            current_writer: Arc::new(Mutex::new(None)),
            is_running: Arc::new(Mutex::new(false)),
        })
    }

    /// 启动日志轮转服务
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.lock().unwrap();
            if *running {
                return Ok(());
            }
            *running = true;
        }

        info!("Starting log rotation manager");

        // 初始化当前日志文件
        self.initialize_current_log_file()?;

        // 启动轮转检查任务
        let config = self.config.clone();
        let current_writer = Arc::clone(&self.current_writer);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.check_interval_seconds));

            loop {
                interval.tick().await;

                // 检查是否应该停止
                {
                    let running = is_running.lock().unwrap();
                    if !*running {
                        break;
                    }
                }

                // 检查是否需要轮转
                if let Err(e) = Self::check_and_rotate(&config, &current_writer).await {
                    error!("Log rotation check failed: {}", e);
                }
            }

            info!("Log rotation manager stopped");
        });

        Ok(())
    }

    /// 停止日志轮转服务
    pub fn stop(&self) -> Result<()> {
        {
            let mut running = self.is_running.lock().unwrap();
            *running = false;
        }

        // 关闭当前写入器
        {
            let mut writer = self.current_writer.lock().unwrap();
            if let Some(mut w) = writer.take() {
                w.flush().context("Failed to flush log writer")?;
            }
        }

        info!("Log rotation manager stopped");
        Ok(())
    }

    /// 初始化当前日志文件
    fn initialize_current_log_file(&self) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_file_path)
            .context("Failed to open log file")?;

        let writer = BufWriter::new(file);
        
        {
            let mut current_writer = self.current_writer.lock().unwrap();
            *current_writer = Some(writer);
        }

        info!("Initialized log file: {:?}", self.config.log_file_path);
        Ok(())
    }

    /// 检查并执行日志轮转
    async fn check_and_rotate(
        config: &LogRotationConfig,
        current_writer: &Arc<Mutex<Option<BufWriter<File>>>>,
    ) -> Result<()> {
        // 检查当前文件大小
        let file_size = match std::fs::metadata(&config.log_file_path) {
            Ok(metadata) => metadata.len(),
            Err(_) => return Ok(()), // 文件不存在，无需轮转
        };

        if file_size >= config.max_file_size_bytes {
            info!(
                "Log file size ({} bytes) exceeds limit ({} bytes), rotating",
                file_size, config.max_file_size_bytes
            );

            Self::rotate_log_file(config, current_writer).await?;
        }

        Ok(())
    }

    /// 执行日志文件轮转
    async fn rotate_log_file(
        config: &LogRotationConfig,
        current_writer: &Arc<Mutex<Option<BufWriter<File>>>>,
    ) -> Result<()> {
        // 关闭当前写入器
        {
            let mut writer = current_writer.lock().unwrap();
            if let Some(mut w) = writer.take() {
                w.flush().context("Failed to flush log writer before rotation")?;
            }
        }

        // 生成轮转文件名
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let rotated_file_name = format!(
            "{}.{}",
            config.log_file_path.display(),
            timestamp
        );
        let rotated_path = PathBuf::from(&rotated_file_name);

        // 移动当前日志文件
        std::fs::rename(&config.log_file_path, &rotated_path)
            .context("Failed to rotate log file")?;

        info!("Rotated log file to: {:?}", rotated_path);

        // 压缩轮转的文件（如果启用）
        if config.compress_rotated_files {
            if let Err(e) = Self::compress_file(&rotated_path).await {
                warn!("Failed to compress rotated log file: {}", e);
            }
        }

        // 清理旧的日志文件
        Self::cleanup_old_log_files(config).await?;

        // 重新初始化当前日志文件
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file_path)
            .context("Failed to create new log file after rotation")?;

        let writer = BufWriter::new(file);
        
        {
            let mut current_writer = current_writer.lock().unwrap();
            *current_writer = Some(writer);
        }

        info!("Created new log file after rotation");
        Ok(())
    }

    /// 压缩文件
    async fn compress_file(file_path: &Path) -> Result<()> {
        
        let input_file = File::open(file_path)
            .context("Failed to open file for compression")?;
        
        let compressed_path = file_path.with_extension("gz");
        let output_file = File::create(&compressed_path)
            .context("Failed to create compressed file")?;

        let mut encoder = flate2::write::GzEncoder::new(output_file, flate2::Compression::default());
        let mut input_reader = std::io::BufReader::new(input_file);
        
        std::io::copy(&mut input_reader, &mut encoder)
            .context("Failed to compress file")?;
        
        encoder.finish().context("Failed to finish compression")?;

        // 删除原始文件
        std::fs::remove_file(file_path)
            .context("Failed to remove original file after compression")?;

        info!("Compressed log file: {:?} -> {:?}", file_path, compressed_path);
        Ok(())
    }

    /// 清理旧的日志文件
    async fn cleanup_old_log_files(config: &LogRotationConfig) -> Result<()> {
        let log_dir = config.log_file_path.parent()
            .unwrap_or_else(|| Path::new("."));
        
        let log_file_name = config.log_file_path.file_name()
            .context("Invalid log file path")?
            .to_string_lossy();

        // 查找所有相关的日志文件
        let mut log_files = Vec::new();
        
        for entry in std::fs::read_dir(log_dir)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            if file_name.starts_with(&*log_file_name) && file_name != log_file_name {
                if let Ok(metadata) = entry.metadata() {
                    log_files.push((entry.path(), metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
                }
            }
        }

        // 按修改时间排序（最新的在前）
        log_files.sort_by(|a, b| b.1.cmp(&a.1));

        // 删除超过保留数量的文件
        if log_files.len() > config.max_files as usize {
            let files_to_remove = &log_files[config.max_files as usize..];
            
            for (file_path, _) in files_to_remove {
                if let Err(e) = std::fs::remove_file(file_path) {
                    warn!("Failed to remove old log file {:?}: {}", file_path, e);
                } else {
                    info!("Removed old log file: {:?}", file_path);
                }
            }
        }

        Ok(())
    }

    /// 写入日志消息
    pub fn write_log(&self, message: &str) -> Result<()> {
        let mut writer = self.current_writer.lock().unwrap();
        
        if let Some(ref mut w) = writer.as_mut() {
            writeln!(w, "{}", message)
                .context("Failed to write log message")?;
            w.flush().context("Failed to flush log writer")?;
        }

        Ok(())
    }

    /// 获取当前日志文件大小
    pub fn get_current_log_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.config.log_file_path)
            .context("Failed to get log file metadata")?;
        Ok(metadata.len())
    }

    /// 获取日志轮转统计信息
    pub fn get_rotation_stats(&self) -> Result<LogRotationStats> {
        let log_dir = self.config.log_file_path.parent()
            .unwrap_or_else(|| Path::new("."));
        
        let log_file_name = self.config.log_file_path.file_name()
            .context("Invalid log file path")?
            .to_string_lossy();

        let mut rotated_files = 0;
        let mut total_size = 0u64;

        for entry in std::fs::read_dir(log_dir)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            if file_name.starts_with(&*log_file_name) {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    if file_name != log_file_name {
                        rotated_files += 1;
                    }
                }
            }
        }

        Ok(LogRotationStats {
            current_file_size: self.get_current_log_size().unwrap_or(0),
            rotated_files_count: rotated_files,
            total_log_size: total_size,
            max_file_size: self.config.max_file_size_bytes,
            max_files: self.config.max_files,
        })
    }
}

/// 日志轮转统计信息
#[derive(Debug, Clone)]
pub struct LogRotationStats {
    pub current_file_size: u64,
    pub rotated_files_count: u32,
    pub total_log_size: u64,
    pub max_file_size: u64,
    pub max_files: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_rotation_config_default() {
        let config = LogRotationConfig::default();
        assert_eq!(config.log_file_path, PathBuf::from("scheduler.log"));
        assert_eq!(config.max_file_size_bytes, 100 * 1024 * 1024);
        assert_eq!(config.max_files, 10);
        assert_eq!(config.check_interval_seconds, 300);
        assert!(!config.compress_rotated_files);
    }

    #[tokio::test]
    async fn test_log_rotation_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = LogRotationConfig {
            log_file_path: log_path,
            max_file_size_bytes: 1024,
            max_files: 5,
            check_interval_seconds: 1,
            compress_rotated_files: false,
        };

        let manager = LogRotationManager::new(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_log_rotation_manager_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = LogRotationConfig {
            log_file_path: log_path,
            max_file_size_bytes: 1024,
            max_files: 5,
            check_interval_seconds: 1,
            compress_rotated_files: false,
        };

        let manager = LogRotationManager::new(config).unwrap();
        
        // 启动管理器
        assert!(manager.start().await.is_ok());
        
        // 停止管理器
        assert!(manager.stop().is_ok());
    }

    #[tokio::test]
    async fn test_write_log() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = LogRotationConfig {
            log_file_path: log_path.clone(),
            max_file_size_bytes: 1024,
            max_files: 5,
            check_interval_seconds: 1,
            compress_rotated_files: false,
        };

        let manager = LogRotationManager::new(config).unwrap();
        assert!(manager.start().await.is_ok());
        
        // 写入日志
        assert!(manager.write_log("Test log message").is_ok());
        
        // 检查文件是否存在
        assert!(log_path.exists());
        
        // 停止管理器
        assert!(manager.stop().is_ok());
    }

    #[tokio::test]
    async fn test_get_current_log_size() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = LogRotationConfig {
            log_file_path: log_path.clone(),
            max_file_size_bytes: 1024,
            max_files: 5,
            check_interval_seconds: 1,
            compress_rotated_files: false,
        };

        let manager = LogRotationManager::new(config).unwrap();
        assert!(manager.start().await.is_ok());
        
        // 写入一些数据
        assert!(manager.write_log("Test log message").is_ok());
        
        // 获取文件大小
        let size = manager.get_current_log_size();
        assert!(size.is_ok());
        assert!(size.unwrap() > 0);
        
        assert!(manager.stop().is_ok());
    }

    #[tokio::test]
    async fn test_get_rotation_stats() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = LogRotationConfig {
            log_file_path: log_path.clone(),
            max_file_size_bytes: 1024,
            max_files: 5,
            check_interval_seconds: 1,
            compress_rotated_files: false,
        };

        let manager = LogRotationManager::new(config).unwrap();
        assert!(manager.start().await.is_ok());
        
        // 写入一些数据
        assert!(manager.write_log("Test log message").is_ok());
        
        // 获取统计信息
        let stats = manager.get_rotation_stats();
        assert!(stats.is_ok());
        
        let stats = stats.unwrap();
        assert!(stats.current_file_size > 0);
        assert_eq!(stats.rotated_files_count, 0);
        assert_eq!(stats.max_files, 5);
        assert_eq!(stats.max_file_size, 1024);
        
        assert!(manager.stop().is_ok());
    }
}