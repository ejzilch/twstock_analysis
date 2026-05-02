/// GET /api/v1/candles/{symbol} — 薄 handler
///
/// 職責：解析 path/query → 呼叫 CandleService → 回傳 response。
/// 指標計算、快取邏輯、DB 查詢全部移至 CandleService。
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::models::{FetchSource, Interval};
use crate::services::candle::{CandleData, CandlesParams, CandlesService};

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// GET /api/v1/candles/{symbol} 的查詢參數
#[derive(Debug, Clone, Deserialize)]
pub struct CandlesQueryParams {
    /// 開始時間戳（毫秒）
    pub from_ms: i64,
    /// 結束時間戳（毫秒）
    pub to_ms: i64,
    /// K 線時間粒度，預設 "1h"
    pub interval: Option<Interval>,
    /// 逗號分隔的指標名稱，如 "ma20,rsi,macd"
    pub indicators: Option<String>,
    /// 分頁游標 (candles lazy loading 預留)
    pub cursor: Option<String>,
}

impl CandlesQueryParams {
    /// 解析 interval，缺少時回傳預設值 "1D"
    pub fn interval(&self) -> Interval {
        self.interval.unwrap_or(Interval::OneDay)
    }

    /// 解析 indicators 字串為 Vec，如 "ma20,rsi" -> ["ma20", "rsi"]
    pub fn indicator_list(&self) -> Vec<String> {
        match &self.indicators {
            None => vec![],
            Some(s) if s.is_empty() => vec![],
            Some(s) => s.split(',').map(|i| i.trim().to_string()).collect(),
        }
    }
}

impl From<&CandlesQueryParams> for CandlesParams {
    fn from(query_params: &CandlesQueryParams) -> Self {
        CandlesParams {
            from_ms: query_params.from_ms,
            to_ms: query_params.to_ms,
            interval: query_params.interval(),
            indicators: query_params.indicator_list(),
        }
    }
}

/// GET /api/v1/candles/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct CandlesApiResponse {
    pub symbol: String,
    pub interval: Interval,
    pub from_ms: i64,
    pub to_ms: i64,
    pub candles: Vec<CandleData>,
    pub count: usize,
    pub total_available: usize,
    /// 分頁游標，None 表示已取完全部資料
    pub next_cursor: Option<String>,
    /// 資料來源："database" / "cache"
    pub source: FetchSource,
    pub cached: bool,
    pub computed_at_ms: i64,
}

/// GET /api/v1/candles/{symbol}
pub async fn candles_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query_params): Query<CandlesQueryParams>,
) -> Result<Json<CandlesApiResponse>, ApiError> {
    let params = CandlesParams::from(&query_params);

    let response = CandlesService::query(&state, &symbol, &params).await?;

    Ok(Json(CandlesApiResponse {
        symbol: symbol,
        interval: query_params.interval(),
        from_ms: query_params.from_ms,
        to_ms: query_params.to_ms,
        candles: response.candles,
        count: response.count,
        total_available: response.total_available,
        next_cursor: response.next_cursor,
        source: response.source,
        cached: response.cached,
        computed_at_ms: response.computed_at_ms,
    }))
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candles_query_interval_default() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: None,
            cursor: None,
        };
        assert_eq!(params.interval(), Interval::OneDay);
    }

    #[test]
    fn test_candles_query_indicator_list_parses_correctly() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: Some("ma20,rsi, macd".to_string()),
            cursor: None,
        };
        assert_eq!(params.indicator_list(), vec!["ma20", "rsi", "macd"]);
    }

    #[test]
    fn test_candles_query_indicator_list_empty_string() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: Some(String::new()),
            cursor: None,
        };
        assert!(params.indicator_list().is_empty());
    }
}
