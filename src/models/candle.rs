use crate::models::Interval;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

/// 系統內部計算使用的 K 線資料 (domain model)
///
/// 職責：供 IndicatorFactory 與 SignalAggregator 內部使用。
/// 不直接序列化為 API response，對外格式由
/// src/api/models/response.rs 的 CandleResponse 負責。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub interval: Interval, // "1m" / "5m" / "15m" / "1h" / "4h" / "1d"
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64, // 成交量不可為負，對齊 RawCandle 定義

    /// 動態指標結果，key 為指標名稱（如 "ma20", "rsi14", "macd"）
    /// 預設為空 HashMap，由 IndicatorFactory 計算後填入
    #[serde(default)]
    pub indicators: HashMap<String, IndicatorValue>,
}

/// 指標值的允許形態
///
/// 使用 enum 明確限制型態，禁止使用 serde_json::Value，
/// 確保 Rust 型別系統在編譯期提供保護。
/// serde untagged 使序列化結果與 API_CONTRACT.md 格式一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndicatorValue {
    /// MA, RSI, Bollinger Bands 等單一數值指標
    Scalar(f64),
    /// MACD 三線結構
    Macd(MacdValue),
}

/// MACD 指標結構
///
/// 對應 API_CONTRACT.md 的 macd response 欄位格式：
/// { "macd_line": f64, "signal_line": f64, "histogram": f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdValue {
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
}

#[derive(Debug, FromRow, Clone)]
pub struct CandleRow {
    pub timestamp_ms: i64,
    pub close: f64,
}
