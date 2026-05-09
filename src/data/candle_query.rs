/// K 線查詢（唯讀）。
/// 寫入邏輯在 implementations.rs，此模組只負責查詢。
use crate::domain::BridgeError;
use crate::models::{candle::CandleRow, Interval};

use sqlx::PgPool;

pub async fn symbol_exists(db: &PgPool, symbol: &str) -> Result<bool, BridgeError> {
    let exists = sqlx::query_scalar!(
        r#"SELECT is_active as "is_active!" FROM symbols WHERE symbol = $1"#,
        symbol
    )
    .fetch_optional(db)
    .await
    .map_err(|e| BridgeError::from_db("symbol_exists failed", e))?;

    Ok(exists.is_some())
}

pub async fn count_candles(
    db: &PgPool,
    symbol: &str,
    interval: Interval,
    from_ms: i64,
    to_ms: i64,
) -> Result<usize, BridgeError> {
    let count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM candles
           WHERE symbol = $1 AND interval = $2
             AND timestamp_ms BETWEEN $3 AND $4"#,
        symbol,
        interval.as_str(),
        from_ms,
        to_ms
    )
    .fetch_one(db)
    .await
    .map_err(|e| BridgeError::from_db("count_candles failed", e))?;

    Ok(count as usize)
}

/// 完整欄位：candle service 用
pub struct FullCandleRow {
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

pub async fn fetch_candles_range(
    db: &PgPool,
    symbol: &str,
    interval: Interval,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<FullCandleRow>, BridgeError> {
    sqlx::query_as!(
        FullCandleRow,
        r#"SELECT timestamp_ms, open, high, low, close, volume
           FROM candles
           WHERE symbol = $1 AND interval = $2
             AND timestamp_ms BETWEEN $3 AND $4
           ORDER BY timestamp_ms ASC"#,
        symbol,
        interval.as_str(),
        from_ms,
        to_ms
    )
    .fetch_all(db)
    .await
    .map_err(|e| BridgeError::from_db("fetch_candles_range failed", e))
}

pub async fn fetch_candles_for_backtest(
    db: &PgPool,
    symbol: &str,
    interval: Interval,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<CandleRow>, BridgeError> {
    sqlx::query_as!(
        CandleRow,
        r#"SELECT timestamp_ms, close
           FROM candles
           WHERE symbol = $1 AND interval = $2
             AND timestamp_ms BETWEEN $3 AND $4
           ORDER BY timestamp_ms ASC"#,
        symbol,
        interval.as_str(),
        from_ms,
        to_ms
    )
    .fetch_all(db)
    .await
    .map_err(|e| BridgeError::from_db("fetch_candles_range failed", e))
}
