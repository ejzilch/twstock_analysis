/// GET /api/v1/signals/{symbol} — 薄 handler
///
/// 職責：解析 path/query → 呼叫 SignalService → 回傳 response。
/// AI 呼叫、fallback 邏輯、指標查詢全部移至 SignalService。
use crate::{api::middleware::ApiError, services::signal::Signals};

use crate::app_state::AppState;
use crate::domain::signal::aggregator::TradeSignal;
use crate::services::signal::SignalService;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub struct SignalsQueryParams {
    /// 開始時間戳（毫秒）
    pub from_ms: i64,
    /// 結束時間戳（毫秒）
    pub to_ms: i64,
}

/// GET /api/v1/signals/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct SignalsApiResponse {
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub signals: Vec<TradeSignal>,
    pub count: usize,
}

impl From<&Signals> for SignalsApiResponse {
    fn from(signals: &Signals) -> Self {
        SignalsApiResponse {
            symbol: signals.symbol.clone(),
            from_ms: signals.from_ms,
            to_ms: signals.to_ms,
            signals: signals.signals.clone(),
            count: signals.count,
        }
    }
}

/// GET /api/v1/signals/{symbol}
pub async fn signals_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(params): Query<SignalsQueryParams>,
) -> Result<Json<SignalsApiResponse>, ApiError> {
    let signals = SignalService::generate(&state, &symbol, params.from_ms, params.to_ms).await?;

    Ok(Json(SignalsApiResponse::from(&signals)))
}
