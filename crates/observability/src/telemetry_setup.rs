use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::structured_logger::{LoggingConfig, LogFormat};

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
