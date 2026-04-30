/// GET /api/v1/signals/{symbol} — 薄 handler
///
/// 職責：解析 path/query → 呼叫 SignalService → 回傳 response。
/// AI 呼叫、fallback 邏輯、指標查詢全部移至 SignalService。
use crate::api::middleware::ApiError;
use crate::api::signal::dto::request::SignalsQueryParams;
use crate::api::signal::dto::response::SignalsApiResponse;
use crate::app_state::AppState;
use crate::services::signal::SignalService;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

/// GET /api/v1/signals/{symbol}
pub async fn signals_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(params): Query<SignalsQueryParams>,
) -> Result<Json<SignalsApiResponse>, ApiError> {
    let response = SignalService::generate(&state, &symbol, params.from_ms, params.to_ms).await?;
    Ok(Json(response))
}
