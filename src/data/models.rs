use serde::{Deserialize, Serialize};

/// 外部資料來源
///
/// fetch.rs 內部做 normalization，對外統一輸出 RawCandle，
/// 上層模組不需感知來源差異。
/// serde rename 確保序列化結果為 "finmind" / "yfinance"，
/// 對應 API_CONTRACT.md 的 data_source 欄位格式。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// 主力來源：台股 (TWSE / TPEX)，走排程限流
    FinMind,
    /// 備用來源：補歷史資料用，禁止放在即時路徑
    YFinance,
}

/// 供日誌輸出與 tracing log 使用
impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSource::FinMind => write!(f, "finmind"),
            DataSource::YFinance => write!(f, "yfinance"),
        }
    }
}

/// 封裝向外部 API 抓取資料所需的參數
///
/// 由 fetch.rs 建立後傳入對應的資料來源 fetcher。
/// interval 合法值: "1m" / "5m" / "15m" / "1h" / "4h" / "1d"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchParams {
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub interval: String, // "1m" / "5m" / "15m" / "1h" / "4h" / "1d"
    pub source: DataSource,
}

/// 從外部 API 擷取後統一正規化的原始 K 線資料
///
/// 所有外部來源（FinMind / yfinance）在 fetch.rs 內部轉換為此格式，
/// 上層模組只需處理 RawCandle，不感知原始 API 差異。
/// 寫入 DB 前需通過 is_finite() 檢查，確保 open / high / low / close 不含 NaN 或 Inf。
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
