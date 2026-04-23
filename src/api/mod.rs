pub mod handlers;
pub mod middleware;
pub mod models;

use crate::api::handlers::admin_sync::{
    cancel_manual_sync, get_rate_limit_info, get_sync_status, get_sync_status_by_id,
    trigger_manual_sync,
};

use crate::api::{
    handlers::{
        backtest::backtest_handler,
        candles::candles_handler,
        health::{health_handler, integrity_handler},
        indicators::compute_indicators_handler,
        predict::predict_handler,
        signals::signals_handler,
        symbols::symbols_handler,
    },
    middleware::{
        auth::auth_middleware,
        rate_limit::{rate_limit_middleware, RateLimiterState},
    },
};
use crate::app_state::AppState;
use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

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

    dotenv().ok();

    let frontend_url =
        env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(frontend_url.parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // API 路由，掛在 /api/v1，需認證與 rate limit
    let api_router = Router::new()
        .route("/symbols", get(symbols_handler))
        .route("/candles/:symbol", get(candles_handler))
        .route("/indicators/compute", post(compute_indicators_handler))
        .route("/signals/:symbol", get(signals_handler))
        .route("/predict", post(predict_handler))
        .route("/backtest", post(backtest_handler))
        .route("/admin/sync", post(trigger_manual_sync))
        .route("/admin/sync/cancel/:sync_id", post(cancel_manual_sync))
        .route("/admin/sync/status", get(get_sync_status))
        .route("/admin/sync/rate-limit", get(get_rate_limit_info))
        .route("/admin/sync/status/:sync_id", get(get_sync_status_by_id))
        .layer(axum::middleware::from_fn(auth_middleware))
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            rate_limit_middleware,
        ))
        .with_state(app_state);

    Router::new()
        .merge(health_router)
        .nest("/api/v1", api_router)
        .layer(cors)
}
