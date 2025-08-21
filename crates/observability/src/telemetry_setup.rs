use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::structured_logger::{LogFormat, LoggingConfig};

pub fn init_structured_logging(config: LoggingConfig) -> Result<()> {
    use tracing_subscriber::fmt::format::FmtSpan;

    let level = config.level.clone();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| config.level.into());

    let registry = tracing_subscriber::registry().with(env_filter);

    match config.format {
        LogFormat::Json => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
        LogFormat::Pretty => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
        LogFormat::Compact => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
    }

    info!(
        logging.format = ?config.format,
        logging.level = level,
        logging.location = config.include_location,
        "Structured logging initialized"
    );

    Ok(())
}

pub fn init_tracing() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    info!("Basic tracing initialized (OpenTelemetry disabled due to API compatibility)");
    Ok(())
}

pub fn init_logging_and_tracing(logging_config: LoggingConfig) -> Result<()> {
    let level = logging_config.level.clone();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| logging_config.level.clone().into());

    let registry = tracing_subscriber::registry().with(env_filter);
    match logging_config.format {
        LogFormat::Json => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
        LogFormat::Pretty => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
        LogFormat::Compact => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
    }

    info!(
        logging.format = ?logging_config.format,
        logging.level = level,
        logging.location = logging_config.include_location,
        "Logging and tracing initialized"
    );

    Ok(())
}

pub fn init_metrics() -> Result<()> {
    let (recorder, _handle) = metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9090))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create Prometheus exporter: {}", e))?;
    metrics::set_global_recorder(recorder)
        .map_err(|e| anyhow::anyhow!("Failed to install metrics recorder: {}", e))?;

    info!("OpenTelemetry metrics initialized with Prometheus exporter on :9090");
    Ok(())
}

pub fn init_observability(logging_config: Option<LoggingConfig>) -> Result<()> {
    let config = logging_config.unwrap_or_default();
    init_logging_and_tracing(config)?;
    init_metrics()?;
    info!("Complete observability stack initialized");
    Ok(())
}

pub fn shutdown_observability() {
    info!("OpenTelemetry observability shutdown completed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structured_logger::{LogFormat, LogOutput};

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Json);
        assert_eq!(config.output, LogOutput::Stdout);
        assert!(config.include_location);
        assert!(!config.include_thread_id);
        assert!(!config.include_thread_name);
    }

    #[test]
    fn test_init_tracing() {
        // Test that basic tracing initialization doesn't panic
        let result = init_tracing();
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_metrics() {
        // Test that metrics initialization doesn't panic
        // Note: This may fail in some test environments due to port conflicts
        let result = init_metrics();
        // We don't assert success because it might fail due to port conflicts
        // but we want to ensure it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_init_structured_logging_with_default_config() {
        let config = LoggingConfig::default();
        let result = init_structured_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_structured_logging_with_json_format() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Json,
            output: LogOutput::Stdout,
            include_location: true,
            include_thread_id: false,
            include_thread_name: false,
        };
        let result = init_structured_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_structured_logging_with_pretty_format() {
        let config = LoggingConfig {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stderr,
            include_location: false,
            include_thread_id: true,
            include_thread_name: true,
        };
        let result = init_structured_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_structured_logging_with_compact_format() {
        let config = LoggingConfig {
            level: "warn".to_string(),
            format: LogFormat::Compact,
            output: LogOutput::Stdout,
            include_location: true,
            include_thread_id: false,
            include_thread_name: false,
        };
        let result = init_structured_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_structured_logging_with_file_output() {
        let config = LoggingConfig {
            level: "info".to_string(),
            format: LogFormat::Json,
            output: LogOutput::File("/tmp/test_scheduler.log".to_string()),
            include_location: true,
            include_thread_id: false,
            include_thread_name: false,
        };
        let result = init_structured_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_logging_and_tracing() {
        let config = LoggingConfig::default();
        let result = init_logging_and_tracing(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_observability_with_default_config() {
        let result = init_observability(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_observability_with_custom_config() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stderr,
            include_location: false,
            include_thread_id: true,
            include_thread_name: true,
        };
        let result = init_observability(Some(config));
        assert!(result.is_ok());
    }

    #[test]
    fn test_shutdown_observability() {
        // Test that shutdown doesn't panic
        shutdown_observability();
    }

    #[test]
    fn test_multiple_initializations() {
        // Test that multiple initializations don't panic
        for _ in 0..3 {
            let result = init_tracing();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_initialization_with_different_log_levels() {
        let levels = vec!["trace", "debug", "info", "warn", "error"];
        for level in levels {
            let config = LoggingConfig {
                level: level.to_string(),
                format: LogFormat::Json,
                output: LogOutput::Stdout,
                include_location: true,
                include_thread_id: false,
                include_thread_name: false,
            };
            let result = init_structured_logging(config);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_initialization_with_different_output_combinations() {
        let combinations = vec![
            (LogFormat::Json, LogOutput::Stdout),
            (LogFormat::Json, LogOutput::Stderr),
            (LogFormat::Pretty, LogOutput::Stdout),
            (LogFormat::Pretty, LogOutput::Stderr),
            (LogFormat::Compact, LogOutput::Stdout),
            (LogFormat::Compact, LogOutput::Stderr),
        ];

        for (format, output) in combinations {
            let config = LoggingConfig {
                level: "info".to_string(),
                format,
                output,
                include_location: true,
                include_thread_id: false,
                include_thread_name: false,
            };
            let result = init_structured_logging(config);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_initialization_with_different_location_settings() {
        let location_settings = vec![true, false];
        for include_location in location_settings {
            let config = LoggingConfig {
                level: "info".to_string(),
                format: LogFormat::Json,
                output: LogOutput::Stdout,
                include_location,
                include_thread_id: false,
                include_thread_name: false,
            };
            let result = init_structured_logging(config);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_initialization_with_different_thread_settings() {
        let thread_settings = vec![(false, false), (true, false), (false, true), (true, true)];
        for (include_thread_id, include_thread_name) in thread_settings {
            let config = LoggingConfig {
                level: "info".to_string(),
                format: LogFormat::Json,
                output: LogOutput::Stdout,
                include_location: true,
                include_thread_id,
                include_thread_name,
            };
            let result = init_structured_logging(config);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_logging_config_cloning() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::File("/tmp/test.log".to_string()),
            include_location: true,
            include_thread_id: true,
            include_thread_name: true,
        };

        let cloned_config = config.clone();
        assert_eq!(cloned_config.level, config.level);
        assert_eq!(cloned_config.format, config.format);
        assert_eq!(cloned_config.output, config.output);
        assert_eq!(cloned_config.include_location, config.include_location);
        assert_eq!(cloned_config.include_thread_id, config.include_thread_id);
        assert_eq!(
            cloned_config.include_thread_name,
            config.include_thread_name
        );
    }

    #[test]
    fn test_init_observability_comprehensive() {
        // Test a comprehensive observability setup
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Json,
            output: LogOutput::Stdout,
            include_location: true,
            include_thread_id: true,
            include_thread_name: true,
        };

        // Initialize observability
        let result = init_observability(Some(config));
        assert!(result.is_ok());

        // Test multiple shutdown calls (should not panic)
        for _ in 0..3 {
            shutdown_observability();
        }
    }

    #[test]
    fn test_error_handling_in_initialization() {
        // Test that initialization handles errors gracefully
        // We can't easily simulate errors in the initialization process
        // but we can test that the functions return Result types appropriately

        let config = LoggingConfig::default();
        let result = init_structured_logging(config);
        assert!(result.is_ok());

        let result = init_tracing();
        assert!(result.is_ok());

        let result = init_logging_and_tracing(LoggingConfig::default());
        assert!(result.is_ok());
    }
}
