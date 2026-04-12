use serde::{Deserialize, Serialize};

// 外部資料來源定義
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataSource {
    #[serde(rename = "finmind")]
    FinMind,
    #[serde(rename = "yfinance")]
    YFinance,
}

// 供日誌輸出與字串轉換使用
impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSource::FinMind => write!(f, "finmind"),
            DataSource::YFinance => write!(f, "yfinance"),
        }
    }
}

// 封裝抓取資料所需的參數
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchParams {
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub interval: String,
    pub source: DataSource,
}

// 原始 K 線數據結構 (從 API 獲取後統一轉換為此格式)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCandle {
    pub symbol: String,
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub source: DataSource,
}
