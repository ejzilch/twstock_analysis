use crate::api::models::FetchSource;
use crate::models::Interval;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GET /api/v1/candles/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct CandlesApiResponse {
    pub symbol: String,
    pub interval: Interval,
    pub from_ms: i64,
    pub to_ms: i64,
    pub candles: Vec<CandleResponse>,
    pub count: usize,
    pub total_available: usize,
    /// 分頁游標，None 表示已取完全部資料
    pub next_cursor: Option<String>,
    /// 資料來源："database" / "cache"
    pub source: FetchSource,
    pub cached: bool,
    pub computed_at_ms: i64,
}

/// GET /api/v1/candles/{symbol} 的單筆 K 線資料
///
/// 從 Candle domain model 轉換而來，不含 symbol 與 interval（由外層 response 提供）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleResponse {
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    /// 動態指標結果，key 為指標名稱（如 "ma20"、"rsi14"、"macd"）
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub indicators: HashMap<String, serde_json::Value>,
}
