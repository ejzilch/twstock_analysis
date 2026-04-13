use crate::data::fetch_rate_limiter::{FinMindRateLimiter, RateLimitError};
use crate::data::models::{DataSource, FetchParams, RawCandle};
use anyhow::Context;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use reqwest::Client;
use std::time::Duration;

// FinMind API timeout，對應 ARCH_DESIGN.md 定義的 15s
const FINMIND_TIMEOUT_SECS: u64 = 15;

/// 外部 K 線資料抓取器
///
/// 主力使用 FinMind API（台股），備用使用 yfinance（補歷史資料）。
/// 當 FinMind 達到限流上限時，自動 fallback 至 yfinance 並記錄告警 log。
/// 所有來源的回傳資料統一正規化為 RawCandle，上層模組不感知來源差異。
pub struct DataFetcher {
    client: Client,
    limiter: FinMindRateLimiter,
}

impl DataFetcher {
    /// 建立新的 DataFetcher
    ///
    /// reqwest Client 設定 15s timeout，對應 FinMind API 的 timeout 規範。
    pub fn new(limiter: FinMindRateLimiter) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(FINMIND_TIMEOUT_SECS))
            .build()
            .expect("Failed to build reqwest Client");

        Self { client, limiter }
    }

    /// 依 FetchParams 決定資料來源並抓取 K 線
    ///
    /// FinMind 限流時自動 fallback 至 yfinance，並寫入 tracing warning log。
    /// yfinance 路徑禁止在即時路徑呼叫，僅限排程補資料任務使用。
    pub async fn fetch_candles(&self, params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        match params.source {
            DataSource::FinMind => self.fetch_from_finmind_with_fallback(params).await,
            DataSource::YFinance => self.fetch_from_yfinance(params).await,
        }
    }

    // ── 私有方法 ────────────────────────────────────────────────────────────

    /// FinMind 抓取，限流時自動 fallback 至 yfinance
    async fn fetch_from_finmind_with_fallback(
        &self,
        params: FetchParams,
    ) -> anyhow::Result<Vec<RawCandle>> {
        match self.limiter.acquire().await {
            Ok(()) => self.fetch_from_finmind(params).await,
            Err(rate_limit_error) => {
                tracing::warn!(
                    symbol        = %params.symbol,
                    rate_limit    = %rate_limit_error,
                    fallback      = "yfinance",
                    remaining_pct = self.limiter.daily_remaining_pct(),
                    "FinMind rate limit reached, falling back to yfinance"
                );
                self.fetch_from_yfinance(params).await
            }
        }
    }

    /// FinMind API 抓取實作
    async fn fetch_from_finmind(&self, params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        let start_date = ms_to_date_string(params.from_ms)
            .context("Failed to convert from_ms to date string")?;
        let end_date =
            ms_to_date_string(params.to_ms).context("Failed to convert to_ms to date string")?;

        let url = format!(
            "https://api.finmindtrade.com/api/v4/data\
             ?dataset=TaiwanStockPrice\
             &data_id={symbol}\
             &start_date={start_date}\
             &end_date={end_date}",
            symbol = params.symbol,
            start_date = start_date,
            end_date = end_date,
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to FinMind API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("FinMind API returned error: status={status}, body={body}");
        }

        let raw_data: FinMindResponse = response
            .json()
            .await
            .context("Failed to parse FinMind response JSON")?;

        raw_data
            .data
            .into_iter()
            .map(|item| finmind_item_to_raw_candle(item, &params.symbol))
            .collect()
    }

    /// yfinance 備用抓取（僅限排程補資料任務，禁止即時路徑使用）
    async fn fetch_from_yfinance(&self, params: FetchParams) -> anyhow::Result<Vec<RawCandle>> {
        tracing::info!(
            symbol = %params.symbol,
            source = "yfinance",
            "Fetching historical data via yfinance fallback"
        );
        // TODO: 實作 yfinance HTTP 呼叫
        // 注意: yfinance 為非官方 API，僅用於補歷史資料，不穩定
        anyhow::bail!("yfinance implementation pending")
    }
}

// ── 純函數工具 ────────────────────────────────────────────────────────────────

/// 毫秒時間戳轉換為 FinMind API 接受的日期字串 "YYYY-MM-DD"
fn ms_to_date_string(ms: i64) -> anyhow::Result<String> {
    let datetime: DateTime<Utc> = Utc
        .timestamp_millis_opt(ms)
        .single()
        .context("Invalid timestamp_ms value")?;
    Ok(datetime.format("%Y-%m-%d").to_string())
}

/// FinMind 日期字串轉換為毫秒時間戳（13 位 UTC）
///
/// FinMind 回傳格式為 "YYYY-MM-DD"，轉換為當日 00:00:00 UTC 毫秒。
fn date_string_to_ms(date_str: &str) -> anyhow::Result<i64> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .with_context(|| format!("Failed to parse date string: {date_str}"))?;

    let datetime = naive_date
        .and_hms_opt(0, 0, 0)
        .context("Failed to construct midnight datetime")?;

    Ok(Utc.from_utc_datetime(&datetime).timestamp_millis())
}

/// FinMindItem 轉換為 RawCandle，含 is_finite() 驗證
///
/// 依 COLLAB_FRAMEWORK.md E1 規範，外部 API 回傳的浮點數
/// 必須通過 is_finite() 檢查，拒絕 NaN 與 Inf。
fn finmind_item_to_raw_candle(item: FinMindItem, symbol: &str) -> anyhow::Result<RawCandle> {
    // 驗證所有浮點數值，拒絕 NaN 與 Inf
    for (field_name, value) in [
        ("open", item.open),
        ("high", item.max),
        ("low", item.min),
        ("close", item.close),
    ] {
        if !value.is_finite() {
            anyhow::bail!(
                "FinMind returned non-finite value: symbol={symbol}, field={field_name}, value={value}"
            );
        }
    }

    let timestamp_ms = date_string_to_ms(&item.date)
        .with_context(|| format!("Failed to convert date to ms: {}", item.date))?;

    Ok(RawCandle {
        symbol: symbol.to_string(),
        timestamp_ms,
        open: item.open,
        high: item.max,
        low: item.min,
        close: item.close,
        volume: item.trading_volume, // u64，不需要 cast
        source: DataSource::FinMind,
    })
}

// ── FinMind API 內部反序列化結構（非對外公開）─────────────────────────────────

#[derive(serde::Deserialize)]
struct FinMindResponse {
    data: Vec<FinMindItem>,
}

/// FinMind TaiwanStockPrice dataset 的單筆回應
///
/// 僅反序列化系統需要的欄位，其他欄位由 serde 忽略。
#[derive(serde::Deserialize, Debug)]
struct FinMindItem {
    pub date: String, // 交易日期，如 "2026-04-11"
    pub open: f64,
    pub max: f64, // FinMind 以 max 表示最高價
    pub min: f64, // FinMind 以 min 表示最低價
    pub close: f64,

    #[serde(rename = "Trading_Volume")]
    pub trading_volume: u64, // 成交股數，u64 避免大成交量截斷
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ms_to_date_string_converts_correctly() {
        // 2024-01-01 00:00:00 UTC = 1704067200000ms
        let result = ms_to_date_string(1704067200000).unwrap();
        assert_eq!(result, "2024-01-01");
    }

    #[test]
    fn test_date_string_to_ms_converts_correctly() {
        let result = date_string_to_ms("2024-01-01").unwrap();
        assert_eq!(result, 1704067200000);
    }

    #[test]
    fn test_date_string_to_ms_invalid_format_returns_error() {
        let result = date_string_to_ms("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_finmind_item_to_raw_candle_rejects_nan() {
        let item = FinMindItem {
            date: "2024-01-01".to_string(),
            open: f64::NAN,
            max: 151.0,
            min: 149.0,
            close: 150.0,
            trading_volume: 1_000_000,
        };
        let result = finmind_item_to_raw_candle(item, "2330");
        assert!(result.is_err());
    }

    #[test]
    fn test_finmind_item_to_raw_candle_rejects_inf() {
        let item = FinMindItem {
            date: "2024-01-01".to_string(),
            open: 150.0,
            max: f64::INFINITY,
            min: 149.0,
            close: 150.0,
            trading_volume: 1_000_000,
        };
        let result = finmind_item_to_raw_candle(item, "2330");
        assert!(result.is_err());
    }

    #[test]
    fn test_finmind_item_to_raw_candle_valid_data_succeeds() {
        let item = FinMindItem {
            date: "2024-01-01".to_string(),
            open: 150.0,
            max: 151.5,
            min: 149.5,
            close: 151.0,
            trading_volume: 1_000_000,
        };
        let result = finmind_item_to_raw_candle(item, "2330").unwrap();
        assert_eq!(result.symbol, "2330");
        assert_eq!(result.timestamp_ms, 1704067200000);
        assert_eq!(result.open, 150.0);
        assert_eq!(result.high, 151.5);
        assert_eq!(result.low, 149.5);
        assert_eq!(result.close, 151.0);
        assert_eq!(result.volume, 1_000_000);
    }
}
