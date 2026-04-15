use crate::api::models::ErrorResponse;
use crate::constants;
use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

/// 單一 IP 的請求計數狀態
#[derive(Debug)]
pub struct IpRateState {
    count: u32,
    window_start: Instant,
}

impl IpRateState {
    fn new() -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
        }
    }

    /// 若視窗已過期（超過 60 秒），重置計數器
    fn reset_if_expired(&mut self) {
        if self.window_start.elapsed() >= Duration::from_secs(constants::RATE_LIMIT_WINDOW_SECS) {
            self.count = 0;
            self.window_start = Instant::now();
        }
    }
}

/// IP 層級的 Rate Limiter 狀態，在 handler 間共享
pub type RateLimiterState = Arc<Mutex<HashMap<String, IpRateState>>>;

/// 建立新的 RateLimiterState，在 main.rs 初始化後傳入 router
pub fn new_rate_limiter_state() -> RateLimiterState {
    Arc::new(Mutex::new(HashMap::new()))
}

/// IP 層級的 Rate Limiting 中介軟體
///
/// 每個 IP 每分鐘最多 60 次請求，超過時回傳 429 Too Many Requests。
/// 使用滑動視窗計數，60 秒後自動重置。
pub async fn rate_limit_middleware(
    state: axum::extract::State<RateLimiterState>,
    request: Request,
    next: Next,
) -> Response {
    // 從 ConnectInfo 取得 IP，若無法取得則跳過限流（內部呼叫）
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    {
        let mut limiter = state.lock().await;
        let ip_state = limiter.entry(ip.clone()).or_insert_with(IpRateState::new);

        ip_state.reset_if_expired();
        ip_state.count += 1;

        if ip_state.count > constants::RATE_LIMIT_MAX_REQUESTS_PER_MINUTE {
            tracing::warn!(
                ip = %ip,
                request_count = ip_state.count,
                limit = constants::RATE_LIMIT_MAX_REQUESTS_PER_MINUTE,
                "Rate limit exceeded"
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse::new(
                    "RATE_LIMIT_EXCEEDED",
                    "Too many requests. Please slow down and try again.",
                )),
            )
                .into_response();
        }
    }

    next.run(request).await
}
