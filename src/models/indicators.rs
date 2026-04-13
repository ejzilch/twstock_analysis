use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::candle::{IndicatorValue, MacdValue};

/// POST /api/v1/indicators/compute 的請求結構
///
/// indicators 欄位支援兩種格式，對應 API_CONTRACT.md：
/// - 參數列表型：MA / RSI / MACD，如 "ma": [5, 10, 20]
/// - 具名參數型：Bollinger，如 "bollinger": { "period": 20, "std_dev_multiplier": 2.0 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeIndicatorsRequest {
    pub request_id: String,
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub interval: String,
    pub indicators: HashMap<String, IndicatorConfig>,
}

/// 指標請求的參數格式
///
/// serde untagged 使反序列化自動依值的形態判斷：
/// - 陣列   -> Periods（MA / RSI / MACD）
/// - 物件   -> Bollinger
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndicatorConfig {
    /// MA / RSI / MACD：參數列表，如 [5, 10, 20] 或 [12, 26, 9]
    Periods(Vec<u32>),
    /// Bollinger Bands：具名參數物件
    Bollinger(BollingerConfig),
}

/// Bollinger Bands 專用參數結構
///
/// 對應 API_CONTRACT.md 的 bollinger request 格式：
/// { "period": 20, "std_dev_multiplier": 2.0 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerConfig {
    pub period: u32,
    pub std_dev_multiplier: f64,
}

/// POST /api/v1/indicators/compute 的回應結構
///
/// indicators 值為時間序列陣列（Vec），對應 API_CONTRACT.md 格式：
/// "ma5": [150.0, 150.1], "rsi14": [55.0, 56.2]
/// 每個元素型態對齊 IndicatorValue，與 Candle.indicators 保持一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeIndicatorsResponse {
    pub symbol: String,
    pub interval: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub indicators: HashMap<String, Vec<IndicatorValue>>,
    pub computed_at_ms: i64,
    pub computation_time_ms: i64,
    pub cached: bool,
    pub dag_execution_order: Vec<String>,
}
