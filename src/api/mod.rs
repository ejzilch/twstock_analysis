pub mod handlers;
pub mod middleware;
pub mod models;

use crate::api::{
    handlers::{
        backtest::backtest_handler,
        candles::candles_handler,
        health::{health_handler, integrity_handler},
        indicators::compute_indicators_handler,
        predict::predict_handler,
        signals::signals_handler,
        symbols::symbols_handler,
        AppState,
    },
    middleware::{
        auth::auth_middleware,
        rate_limit::{rate_limit_middleware, RateLimiterState},
    },
};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// 組裝完整的 Axum Router
///
/// 路由分為兩組：
/// - 健康檢查端點（不需認證，不套用 rate limit）
/// - API 端點（需 X-API-KEY 認證，套用 rate limit）
pub fn build_router(app_state: Arc<AppState>, rate_limiter: RateLimiterState) -> Router {
    // 健康檢查路由，掛在根路徑，不需認證
    let health_router = Router::new()
        .route("/health", get(health_handler))
        .route("/health/integrity", get(integrity_handler))
        .with_state(app_state.clone());

    // API 路由，掛在 /api/v1，需認證與 rate limit
    let api_router = Router::new()
        .route("/symbols", get(symbols_handler))
        .route("/candles/:symbol", get(candles_handler))
        .route("/indicators/compute", post(compute_indicators_handler))
        .route("/signals/:symbol", get(signals_handler))
        .route("/predict", post(predict_handler))
        .route("/backtest", post(backtest_handler))
        .layer(axum::middleware::from_fn(auth_middleware))
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            rate_limit_middleware,
        ))
        .with_state(app_state);

    Router::new()
        .merge(health_router)
        .nest("/api/v1", api_router)
}
