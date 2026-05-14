use crate::models::{DataSource, Interval};
use serde::{Deserialize, Serialize};

/// 從外部 API 取得的原始 K 線資料。
///
/// 寫入 DB 時使用 `INSERT ... ON CONFLICT DO NOTHING` 確保冪等性。
/// `created_at_ms` 記錄第一次寫入時間，重複寫入時不覆蓋。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCandle {
    /// 股票代號，例如 "2330"
    pub symbol: String,

    /// K 線開始時間，毫秒級 UTC timestamp
    pub timestamp_ms: i64,

    /// K 線時間粒度
    pub interval: Interval,

    /// 開盤價，保證 is_finite() == true
    pub open: f64,

    /// 最高價，保證 is_finite() == true
    pub high: f64,

    /// 最低價，保證 is_finite() == true
    pub low: f64,

    /// 收盤價，保證 is_finite() == true
    pub close: f64,

    /// 成交量，不可為負
    pub volume: u64,

    /// 資料來源
    pub source: DataSource,

    /// 第一次寫入 DB 的時間戳（毫秒）。
    /// 由呼叫端在寫入前填入，不從外部 API 取得。
    /// ON CONFLICT DO NOTHING 保證此值記錄第一次寫入時間。
    pub created_at_ms: i64,
}

impl RawCandle {
    /// 建立新的 RawCandle，自動填入 created_at_ms 為當前時間。
    pub fn new(
        symbol: impl Into<String>,
        timestamp_ms: i64,
        interval: Interval,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: u64,
        source: DataSource,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            timestamp_ms,
            interval,
            open,
            high,
            low,
            close,
            volume,
            source,
            created_at_ms: current_timestamp_ms(),
        }
    }
}

/// 傳回目前時間的毫秒級 UTC timestamp。
pub fn current_timestamp_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::{DataSource, Interval};

    fn sample_candle() -> RawCandle {
        RawCandle::new(
            "2330",
            1704067200000,
            Interval::OneHour,
            150.0,
            151.5,
            149.5,
            151.0,
            1_000_000,
            DataSource::FinMind,
        )
    }

    #[test]
    fn test_created_at_ms_is_set_on_new() {
        let before = current_timestamp_ms();
        let candle = sample_candle();
        let after = current_timestamp_ms();
        assert!(candle.created_at_ms >= before);
        assert!(candle.created_at_ms <= after);
    }
}
