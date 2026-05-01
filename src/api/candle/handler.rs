/// GET /api/v1/candles/{symbol} — 薄 handler
///
/// 職責：解析 path/query → 呼叫 CandleService → 回傳 response。
/// 指標計算、快取邏輯、DB 查詢全部移至 CandleService。
use super::dto::request::CandlesQueryParams;
use super::dto::response::CandlesApiResponse;
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::services::candle::CandleService;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

/// GET /api/v1/candles/{symbol}
pub async fn candles_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(params): Query<CandlesQueryParams>,
) -> Result<Json<CandlesApiResponse>, ApiError> {
    let response = CandleService::query(&state, &symbol, &params).await?;
    Ok(Json(response))
}
