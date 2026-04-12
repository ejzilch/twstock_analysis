use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::models::{DataSource, FetchParams, RawCandle};
use anyhow::Context;
use reqwest::Client;

pub struct DataFetcher {
    client: Client,
    limiter: FinMindRateLimiter,
}

impl DataFetcher {
    pub fn new(limiter: FinMindRateLimiter) -> Self {
        Self {
            client: Client::new(),
            limiter,
        }
    }

    /// 核心抓取函數：根據參數決定抓取來源
    pub async fn fetch_candles(&self, params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        match params.source {
            DataSource::FinMind => self.fetch_from_finmind(params).await,
            DataSource::YFinance => self.fetch_from_yfinance(params).await,
        }
    }

    /// 實作 FinMind 抓取邏輯
    async fn fetch_from_finmind(&self, params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        // 1. 獲取限流許可
        let _permit = self.limiter.acquire().await?;

        // 2. 構建 API 請求 (以 FinMind 官方格式為例)
        let url = format!(
            "https://api.finmindtrade.com/api/v4/data?dataset=TaiwanStockPrice&data_id={}&start_date={}",
            params.symbol,
            self.ms_to_date_string(params.from_ms)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to FinMind")?;

        // 3. 解析與轉換 (這裡假設 FinMind 返回的 JSON 結構)
        // 實作時需定義對應的內部分類結構體來接收原始 JSON
        let raw_data: FinMindResponse = response
            .json()
            .await
            .context("Failed to parse FinMind response")?;

        let candles = raw_data
            .data
            .into_iter()
            .map(|item| {
                RawCandle {
                    symbol: params.symbol.clone(),
                    timestamp_ms: item.date_to_ms(), // 需實作日期轉毫秒
                    open: item.open,
                    high: item.max,
                    low: item.min,
                    close: item.close,
                    volume: item.trading_volume as i64,
                    source: DataSource::FinMind,
                }
            })
            .collect();

        Ok(candles)
    }

    /// 備援來源：yfinance (僅限排程任務補資料)
    async fn fetch_from_yfinance(&self, _params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        // 依照 COLLAB_FRAMEWORK.md 規範，yfinance 禁止放在即時路徑
        todo!("yfinance implementation for historical backfill only")
    }

    fn ms_to_date_string(&self, ms: i64) -> String {
        // 實作毫秒轉 "YYYY-MM-DD" 格式
        "2026-01-01".to_string()
    }
}

// 內部使用的資料接收結構 (非對外公開)
#[derive(serde::Deserialize)]
struct FinMindResponse {
    data: Vec<FinMindItem>,
}

#[derive(serde::Deserialize, Debug)]
struct FinMindItem {
    pub date: String, // 交易日期，如 "2026-04-11"
    pub open: f64,    // 開盤價
    pub max: f64,     // 最高價
    pub min: f64,     // 最低價
    pub close: f64,   // 收盤價
    pub spread: f64,  // 漲跌價差

    // 使用 rename 對接 API 的不規則命名
    #[serde(rename = "Trading_Volume")]
    pub trading_volume: u64, // 成交股數
    #[serde(rename = "Trading_money")]
    pub trading_money: u64, // 成交金額
    #[serde(rename = "Trading_turnover")]
    pub trading_turnover: u64, // 成交筆數
}

impl FinMindItem {
    /// 將日期字串轉換為毫秒時間戳 (符合系統 13 位規範)
    fn date_to_ms(&self) -> i64 {
        // 使用 chrono 解析 "YYYY-MM-DD" 並轉為 UTC 毫秒
        // 可用 chrono::NaiveDate::parse_from_str
        1704067200000
    }
}
