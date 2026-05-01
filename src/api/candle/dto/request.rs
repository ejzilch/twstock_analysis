use crate::models::Interval;
use serde::Deserialize;

/// GET /api/v1/candles/{symbol} 的查詢參數
#[derive(Debug, Clone, Deserialize)]
pub struct CandlesQueryParams {
    /// 開始時間戳（毫秒）
    pub from_ms: i64,
    /// 結束時間戳（毫秒）
    pub to_ms: i64,
    /// K 線時間粒度，預設 "1h"
    pub interval: Option<Interval>,
    /// 逗號分隔的指標名稱，如 "ma20,rsi,macd"
    pub indicators: Option<String>,
    /// 分頁游標
    pub cursor: Option<String>,
}

impl CandlesQueryParams {
    /// 解析 interval，缺少時回傳預設值 "1h"
    pub fn interval(&self) -> Interval {
        self.interval.unwrap_or(Interval::OneHour)
    }

    /// 解析 indicators 字串為 Vec，如 "ma20,rsi" -> ["ma20", "rsi"]
    pub fn indicator_list(&self) -> Vec<String> {
        match &self.indicators {
            None => vec![],
            Some(s) if s.is_empty() => vec![],
            Some(s) => s.split(',').map(|i| i.trim().to_string()).collect(),
        }
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candles_query_interval_default() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: None,
            cursor: None,
        };
        assert_eq!(params.interval(), Interval::OneHour);
    }

    #[test]
    fn test_candles_query_indicator_list_parses_correctly() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: Some("ma20,rsi, macd".to_string()),
            cursor: None,
        };
        assert_eq!(params.indicator_list(), vec!["ma20", "rsi", "macd"]);
    }

    #[test]
    fn test_candles_query_indicator_list_empty_string() {
        let params = CandlesQueryParams {
            from_ms: 0,
            to_ms: 1,
            interval: None,
            indicators: Some(String::new()),
            cursor: None,
        };
        assert!(params.indicator_list().is_empty());
    }
}
