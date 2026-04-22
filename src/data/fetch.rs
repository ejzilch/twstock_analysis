/// src/data/fetch.rs
///
/// 外部 API 呼叫層。
///
/// 資料來源優先級：
///   主力：FinMind（台股即時與歷史資料）
///   備用：yfinance（僅限排程補歷史，禁止即時路徑）
///
/// 本次新增：fetch_range()，供 manual_sync.rs 的缺口補齊使用。
/// 現有函數（fetch_latest 等）不動，確保排程邏輯不受影響。
use crate::constants::{FINMIND_API_BASE_URL, FINMIND_API_TIMEOUT_SECS, FINMIND_DATE_FORMAT};
use crate::core::BridgeError;
use crate::data::models::{current_timestamp_ms, RawCandle};
use crate::models::enums::{DataSource, Interval};

use reqwest::Client;
use serde::Deserialize;
use serde::Deserializer;
use tracing::{error, info, warn};

// ── FinMind API 回應結構 ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FinMindResponse {
    #[serde(deserialize_with = "de_status_code")]
    status: u32,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Vec<FinMindCandle>,
}

#[derive(Debug, Deserialize)]
struct FinMindCandle {
    date: Option<String>, // "2026-01-02"
    open: Option<f64>,
    max: Option<f64>, // FinMind 用 max/min，非 high/low
    min: Option<f64>,
    close: Option<f64>,
    volume: Option<f64>,
}

// ── 現有函數（不動）──────────────────────────────────────────────────────────

/// 取得指定股票的最新 K 線（排程用）。
/// 此函數為現有排程邏輯使用，本次不修改。
pub async fn fetch_latest(
    client: &Client,
    symbol: &str,
    interval: Interval,
    api_token: &str,
) -> Result<Vec<RawCandle>, BridgeError> {
    // 取最近 7 天資料，排程每日只需補最新一筆
    let today = chrono::Utc::now().format(FINMIND_DATE_FORMAT).to_string();
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7))
        .format(FINMIND_DATE_FORMAT)
        .to_string();

    fetch_range(client, symbol, interval, &week_ago, &today, api_token).await
}

// ── 新增函數：fetch_range ─────────────────────────────────────────────────────

/// 取得指定股票、指定時間範圍的歷史 K 線（手動補資料用）。
///
/// 供 manual_sync.rs 的 fetch_and_insert_gap() 呼叫。
/// 每次呼叫消耗 FinMind API 1 次請求額度。
///
/// # Arguments
/// * `client`     - HTTP client（由呼叫端傳入，共用連線池）
/// * `symbol`     - 股票代號，例如 "2330"
/// * `interval`   - K 線粒度
/// * `from_date`  - 開始日期，格式 "YYYY-MM-DD"
/// * `to_date`    - 結束日期，格式 "YYYY-MM-DD"
/// * `api_token`  - FinMind API token
///
/// # Returns
/// 該時間範圍內的所有 RawCandle，可能為空（該範圍無資料）。
pub async fn fetch_range(
    client: &Client,
    symbol: &str,
    interval: Interval,
    from_date: &str,
    to_date: &str,
    api_token: &str,
) -> Result<Vec<RawCandle>, BridgeError> {
    let dataset = interval_to_finmind_dataset(interval);

    let base = std::env::var(FINMIND_API_BASE_URL).expect("FINMIND_API_BASE not set");

    let url = format!(
        "{base}/data?dataset={dataset}&data_id={symbol}&start_date={from_date}&end_date={to_date}&token={api_token}"
    );

    info!(
        symbol = %symbol,
        interval = %interval.as_str(),
        from_date = %from_date,
        to_date = %to_date,
        "Fetching range from FinMind"
    );

    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(FINMIND_API_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, symbol = %symbol, "FinMind request failed");
            BridgeError::FinMindDataSourceError {
                context: "FinMind request failed: ".into(),
                source: Some(Box::new(e)),
            }
        })?;

    let body = response.text().await.map_err(|e| {
        error!(error = %e, symbol = %symbol, "Failed to read FinMind response body");
        BridgeError::FinMindDataSourceError {
            context: "FinMind response body read failed: ".into(),
            source: Some(Box::new(e)),
        }
    })?;

    let finmind_resp: FinMindResponse = serde_json::from_str(&body).map_err(|e| {
        let preview: String = body.chars().take(240).collect();
        error!(
            error = %e,
            symbol = %symbol,
            body_preview = %preview,
            "FinMind response deserialization failed"
        );
        BridgeError::FinMindDataSourceError {
            context: format!("FinMind deserialization failed: body={preview}"),
            source: Some(Box::new(e)),
        }
    })?;

    if finmind_resp.status != 200 {
        error!(
            symbol = %symbol,
            status = finmind_resp.status,
            msg = %finmind_resp.msg,
            "FinMind returned non-200 status"
        );
        return Err(BridgeError::FinMindDataSourceError {
            context: format!(
                "FinMind error: status={}, msg={}",
                finmind_resp.status, finmind_resp.msg
            ),
            source: None,
        });
    }

    let created_at_ms = current_timestamp_ms();

    let candles: Vec<RawCandle> = finmind_resp
        .data
        .into_iter()
        .filter_map(|row| {
            let date = row.date?;
            let open = row.open?;
            let high = row.max?;
            let low = row.min?;
            let close = row.close?;
            let volume = row.volume?;

            // 驗證數值合法性
            if !open.is_finite() || !high.is_finite() || !low.is_finite() || !close.is_finite() {
                warn!(
                    symbol = %symbol,
                    date = %date,
                    "Skipping candle with non-finite values"
                );
                return None;
            }

            let timestamp_ms = date_str_to_ms(&date)?;

            Some(RawCandle {
                symbol: symbol.to_string(),
                timestamp_ms,
                interval,
                open,
                high,
                low,
                close,
                volume: volume as u64,
                source: DataSource::FinMind,
                created_at_ms,
            })
        })
        .collect();

    info!(
        symbol = %symbol,
        interval = %interval.as_str(),
        count = candles.len(),
        "FinMind range fetch complete"
    );

    Ok(candles)
}

fn de_status_code<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|v| v as u32)
            .ok_or_else(|| serde::de::Error::custom("invalid numeric status")),
        serde_json::Value::String(s) => s
            .parse::<u32>()
            .map_err(|_| serde::de::Error::custom("invalid string status")),
        _ => Err(serde::de::Error::custom("unsupported status type")),
    }
}

// ── 內部工具函數 ──────────────────────────────────────────────────────────────

/// Interval → FinMind dataset 名稱對應。
fn interval_to_finmind_dataset(interval: Interval) -> &'static str {
    match interval {
        Interval::OneMin => "1m",
        Interval::FiveMin => "5m",
        Interval::FifteenMin => "15m",
        Interval::OneHour => "1h",
        Interval::FourHours => "4h",
        Interval::OneDay => "1d",
    }
}

/// 日期字串（"YYYY-MM-DD"）轉為毫秒級 UTC timestamp。
fn date_str_to_ms(date_str: &str) -> Option<i64> {
    use chrono::NaiveDate;
    let date = NaiveDate::parse_from_str(date_str, FINMIND_DATE_FORMAT).ok()?;
    let datetime = date.and_hms_opt(0, 0, 0)?;
    Some(datetime.and_utc().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_str_to_ms_valid() {
        let ms = date_str_to_ms("2026-01-02");
        assert!(ms.is_some());
        assert!(ms.unwrap() > 0);
    }

    #[test]
    fn test_date_str_to_ms_invalid() {
        assert!(date_str_to_ms("not-a-date").is_none());
    }

    #[test]
    fn test_interval_to_finmind_dataset() {
        assert_eq!(
            interval_to_finmind_dataset(Interval::OneDay),
            "TaiwanStockPrice"
        );
        assert_eq!(
            interval_to_finmind_dataset(Interval::OneHour),
            "TaiwanStockPriceHour"
        );
    }
}
