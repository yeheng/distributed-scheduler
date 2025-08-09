use axum::{extract::Request, http::Method, middleware::Next, response::Response};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

pub async fn request_logging(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    info!("开始处理请求: {} {}", method, uri);

    let response = next.run(request).await;
    let duration = start.elapsed();

    info!(
        "完成请求处理: {} {} - 状态: {} - 耗时: {:?}",
        method,
        uri,
        response.status(),
        duration
    );

    response
}

pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any)
}

pub fn trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}

pub async fn health_check_middleware(request: Request, next: Next) -> Response {
    if request.uri().path() == "/health" {
        return axum::response::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                r#"{"status":"ok","timestamp":"#.to_string()
                    + &chrono::Utc::now().to_rfc3339()
                    + r#""}"#,
            ))
            .unwrap();
    }

    next.run(request).await
}
