use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// 供系統核心與 API 回傳使用的乾淨 K 線結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,

    // 預設為空 HashMap，完美支援動態指標 (如 MA, RSI, MACD 物件)
    #[serde(default)]
    pub indicators: HashMap<String, serde_json::Value>,
}
