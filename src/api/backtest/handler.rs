/// POST /api/v1/backtest — 薄 handler
///
/// 職責：解析 request → 呼叫 BacktestService → 回傳 response。
/// 所有業務邏輯已移至 `crate::services::backtest::BacktestService`。
use crate::api::backtest::dto::{request::BacktestRequest, response::BacktestResponse};
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::services::backtest::BacktestService;
use axum::{extract::State, Json};
use std::sync::Arc;

/// POST /api/v1/backtest
pub async fn backtest_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
    let response = BacktestService::run(&state, &request).await?;
    Ok(Json(response))
}
