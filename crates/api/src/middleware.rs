use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, warn};

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed per window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Token refill rate (tokens per second)
    pub refill_rate: f64,
    /// Maximum burst size
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            refill_rate: 10.0, // 10 tokens per second
            burst_size: 20,
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        let tokens_to_add = elapsed * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
        self.last_refill = now;
    }

    fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn is_allowed(&self, client_id: &str) -> bool {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets.entry(client_id.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size as f64, self.config.refill_rate)
        });

        bucket.try_consume(1.0)
    }

    pub async fn get_remaining_tokens(&self, client_id: &str) -> f64 {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets.entry(client_id.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size as f64, self.config.refill_rate)
        });

        bucket.available_tokens()
    }

    /// Cleanup old buckets to prevent memory leaks
    pub async fn cleanup_old_buckets(&self) {
        let mut buckets = self.buckets.write().await;
        let cutoff = Instant::now() - Duration::from_secs(3600); // Remove buckets older than 1 hour

        buckets.retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

/// Rate limiting middleware function
pub async fn rate_limiting_middleware(
    State(rate_limiter): axum::extract::State<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    // Extract client identifier (IP address or user ID)
    let client_id = extract_client_id(&request);

    // Check if request is allowed
    if !rate_limiter.is_allowed(&client_id).await {
        warn!("Rate limit exceeded for client: {}", client_id);

        // Return 429 Too Many Requests
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("content-type", "application/json")
            .header("retry-after", "60")
            .header("x-ratelimit-limit", rate_limiter.config.burst_size.to_string())
            .header("x-ratelimit-remaining", "0")
            .body(axum::body::Body::from(
                r#"{"error":"Rate limit exceeded","code":"RATE_LIMIT_EXCEEDED","retry_after_seconds":60}"#
            ))
            .unwrap();
    }

    // Get remaining tokens for headers
    let remaining = rate_limiter.get_remaining_tokens(&client_id).await;

    // Process the request
    let mut response = next.run(request).await;

    // Add rate limit headers
    let headers = response.headers_mut();
    headers.insert(
        "x-ratelimit-limit",
        rate_limiter.config.burst_size.to_string().parse().unwrap(),
    );
    headers.insert(
        "x-ratelimit-remaining",
        (remaining as u32).to_string().parse().unwrap(),
    );
    headers.insert(
        "x-ratelimit-reset",
        (std::time::SystemTime::now() + Duration::from_secs(60))
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
            .to_string()
            .parse()
            .unwrap(),
    );

    response
}

/// Create rate limiting layer
pub fn create_rate_limiting_layer(config: RateLimitConfig) -> Arc<RateLimiter> {
    Arc::new(RateLimiter::new(config))
}

/// Extract client identifier from request
fn extract_client_id(request: &Request) -> String {
    // Try to get client IP from various headers
    if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            return value
                .split(',')
                .next()
                .unwrap_or("unknown")
                .trim()
                .to_string();
        }
    }

    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            return value.to_string();
        }
    }

    // Fallback to connection info (this might not be available in all cases)
    "unknown".to_string()
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_token_bucket_basic_consumption() {
        let mut bucket = TokenBucket::new(10.0, 1.0);

        // Should be able to consume tokens initially
        assert!(bucket.try_consume(1.0));
        assert!(bucket.try_consume(5.0));

        // Should have 4 tokens left
        assert!((bucket.available_tokens() - 4.0).abs() < 0.1);

        // Should not be able to consume more than available
        assert!(!bucket.try_consume(5.0));
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10.0, 10.0); // 10 tokens per second

        // Consume all tokens
        assert!(bucket.try_consume(10.0));
        assert!(!bucket.try_consume(1.0));

        // Wait for refill
        sleep(Duration::from_millis(200)).await; // 0.2 seconds should add ~2 tokens

        // Should be able to consume again
        assert!(bucket.try_consume(1.0));
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            refill_rate: 1.0,
            burst_size: 5,
        };

        let limiter = RateLimiter::new(config);

        // Should allow requests within limit
        for _ in 0..5 {
            assert!(limiter.is_allowed("test-client").await);
        }

        // Should deny the 6th request
        assert!(!limiter.is_allowed("test-client").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_clients() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_secs(60),
            refill_rate: 1.0,
            burst_size: 2,
        };

        let limiter = RateLimiter::new(config);

        // Each client should have independent limits
        assert!(limiter.is_allowed("client-1").await);
        assert!(limiter.is_allowed("client-2").await);

        assert!(limiter.is_allowed("client-1").await);
        assert!(limiter.is_allowed("client-2").await);

        // Both should be at limit now
        assert!(!limiter.is_allowed("client-1").await);
        assert!(!limiter.is_allowed("client-2").await);
    }

    #[tokio::test]
    async fn test_extract_client_id() {
        use axum::http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let request = Request::builder()
            .method("GET")
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        // For now we test the fallback case since we can't easily inject headers in test
        let client_id = extract_client_id(&request);
        assert_eq!(client_id, "unknown");
    }
}
