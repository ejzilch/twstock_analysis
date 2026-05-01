/// CandleService — K 線查詢與指標計算協調者（Service layer）
///
/// 職責：
///   1. 驗證 symbol 是否存在
///   2. 檢查查詢筆數上限
///   3. 快取優先取得 K 線
///   4. 依請求計算指標
///   5. 組裝 CandlesApiResponse
use std::collections::HashMap;

use chrono::Utc;

use crate::api::candle::dto::request::CandlesQueryParams;
use crate::api::candle::dto::response::{CandleResponse, CandlesApiResponse};
use crate::api::middleware::ApiError;
use crate::api::models::enums::FetchSource;
use crate::app_state::AppState;
use crate::constants::CANDLES_MAX_PER_REQUEST;
use crate::domain::indicators::factory::IndicatorFactory;
use crate::models::indicators::{BollingerConfig, IndicatorConfig};
use crate::models::{Candle, IndicatorValue, Interval};

pub struct CandleService;

impl CandleService {
    pub async fn query(
        state: &AppState,
        symbol: &str,
        params: &CandlesQueryParams,
    ) -> Result<CandlesApiResponse, ApiError> {
        // ── Step 1: 確認 symbol 存在 ──────────────────────────────────────────
        Self::validate_symbol(state, symbol).await?;

        let interval = params.interval();

        // ── Step 2: 查詢總筆數，超過上限直接拒絕 ─────────────────────────────
        let total_available =
            Self::count_candles(state, symbol, interval, params.from_ms, params.to_ms).await?;

        if total_available > CANDLES_MAX_PER_REQUEST {
            return Err(ApiError::QueryRangeTooLarge {
                requested: total_available,
                max: CANDLES_MAX_PER_REQUEST,
            });
        }

        // ── Step 3: 快取優先取得 K 線 ─────────────────────────────────────────
        let (mut candles, source, cached) =
            Self::fetch_with_cache(state, symbol, interval, params.from_ms, params.to_ms).await?;

        // ── Step 4: 計算指標（若有請求）──────────────────────────────────────
        if let Some(indicator_str) = &params.indicators {
            if !indicator_str.trim().is_empty() {
                Self::apply_indicators(state, symbol, interval, &mut candles, indicator_str);
            }
        }

        let count = candles.len();

        Ok(CandlesApiResponse {
            symbol: symbol.to_string(),
            interval,
            from_ms: params.from_ms,
            to_ms: params.to_ms,
            candles,
            count,
            total_available,
            next_cursor: None,
            source,
            cached,
            computed_at_ms: Utc::now().timestamp_millis(),
        })
    }

    // ── 私有方法 ──────────────────────────────────────────────────────────────

    async fn validate_symbol(state: &AppState, symbol: &str) -> Result<(), ApiError> {
        let exists: Option<bool> = sqlx::query_scalar!(
            r#"SELECT is_active as "is_active!" FROM symbols WHERE symbol = $1"#,
            symbol
        )
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, symbol = %symbol, "DB error checking symbol");
            ApiError::DataSourceInterrupted
        })?;

        match exists {
            Some(_) => Ok(()),
            None => Err(ApiError::SymbolNotFound {
                symbol: symbol.to_string(),
            }),
        }
    }

    async fn count_candles(
        state: &AppState,
        symbol: &str,
        interval: Interval,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<usize, ApiError> {
        let count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM candles
            WHERE symbol = $1 AND interval = $2
              AND timestamp_ms BETWEEN $3 AND $4
            "#,
            symbol,
            interval.as_str(),
            from_ms,
            to_ms
        )
        .fetch_one(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count candles");
            ApiError::DataSourceInterrupted
        })?;

        Ok(count as usize)
    }

    async fn fetch_with_cache(
        state: &AppState,
        symbol: &str,
        interval: Interval,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<(Vec<CandleResponse>, FetchSource, bool), ApiError> {
        let cache_key = format!("indicators:{symbol}:{interval}");
        let mut conn = state.redis_client.clone();

        let cached: redis::RedisResult<Option<String>> = redis::cmd("GET")
            .arg(&cache_key)
            .query_async(&mut conn)
            .await;

        if let Ok(Some(json_str)) = cached {
            if let Ok(candles) = serde_json::from_str::<Vec<CandleResponse>>(&json_str) {
                tracing::debug!(symbol = %symbol, "Cache hit for candles");
                return Ok((candles, FetchSource::Cache, true));
            }
        }

        let candles = Self::fetch_from_db(state, symbol, interval, from_ms, to_ms).await?;
        Ok((candles, FetchSource::Database, false))
    }

    async fn fetch_from_db(
        state: &AppState,
        symbol: &str,
        interval: Interval,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<Vec<CandleResponse>, ApiError> {
        struct CandleRow {
            timestamp_ms: i64,
            open: f64,
            high: f64,
            low: f64,
            close: f64,
            volume: i64,
        }

        let rows = sqlx::query_as!(
            CandleRow,
            r#"
            SELECT timestamp_ms, open, high, low, close, volume
            FROM candles
            WHERE symbol = $1 AND interval = $2
              AND timestamp_ms BETWEEN $3 AND $4
            ORDER BY timestamp_ms ASC
            "#,
            symbol,
            interval.as_str(),
            from_ms,
            to_ms
        )
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to fetch candles from DB");
            ApiError::DataSourceInterrupted
        })?;

        Ok(rows
            .into_iter()
            .map(|row| CandleResponse {
                timestamp_ms: row.timestamp_ms,
                open: row.open,
                high: row.high,
                low: row.low,
                close: row.close,
                volume: row.volume as u64,
                indicators: HashMap::new(),
            })
            .collect())
    }

    /// 解析指標字串並計算，結果填入 candles
    fn apply_indicators(
        _state: &AppState,
        symbol: &str,
        interval: Interval,
        candles: &mut Vec<CandleResponse>,
        indicator_str: &str,
    ) {
        let indicator_request = parse_indicator_request(indicator_str);
        if indicator_request.is_empty() {
            return;
        }

        let candles_for_calc: Vec<Candle> = candles
            .iter()
            .map(|c| Candle {
                symbol: symbol.to_string(),
                interval,
                timestamp_ms: c.timestamp_ms,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
                indicators: Default::default(),
            })
            .collect();

        match IndicatorFactory::build_from_request(&indicator_request) {
            Ok(factory) => match factory.compute_all(&candles_for_calc) {
                Ok((computed, _)) => fill_indicators(candles, &computed),
                Err(e) => {
                    tracing::warn!(error = %e, "Indicator computation failed, returning candles without indicators");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to build indicator factory");
            }
        }
    }
}

// ── 純函數：指標解析與填充 ────────────────────────────────────────────────────

/// "ma5,ma20,rsi,macd,bollinger" → IndicatorFactory 需要的 HashMap
pub fn parse_indicator_request(indicator_str: &str) -> HashMap<String, IndicatorConfig> {
    let mut map: HashMap<String, IndicatorConfig> = HashMap::new();
    let mut ma_periods: Vec<u32> = vec![];

    for key in indicator_str.split(',').map(|s| s.trim()) {
        match key {
            "ma5" => ma_periods.push(5),
            "ma20" => ma_periods.push(20),
            "ma50" => ma_periods.push(50),
            "rsi" => {
                map.insert("rsi".to_string(), IndicatorConfig::Periods(vec![14]));
            }
            "macd" => {
                map.insert(
                    "macd".to_string(),
                    IndicatorConfig::Periods(vec![12, 26, 9]),
                );
            }
            "bollinger" => {
                map.insert(
                    "bollinger".to_string(),
                    IndicatorConfig::Bollinger(BollingerConfig {
                        period: 20,
                        std_dev_multiplier: 2.0,
                    }),
                );
            }
            _ => {}
        }
    }

    if !ma_periods.is_empty() {
        map.insert("ma".to_string(), IndicatorConfig::Periods(ma_periods));
    }

    map
}

/// 計算結果填入 CandleResponse.indicators
pub fn fill_indicators(
    candles: &mut Vec<CandleResponse>,
    computed: &HashMap<String, Vec<IndicatorValue>>,
) {
    for (key, values) in computed {
        if key == "bollinger" {
            for (i, chunk) in values.chunks(3).enumerate() {
                if i >= candles.len() || chunk.len() < 3 {
                    break;
                }
                let (u, m, l) = match (&chunk[0], &chunk[1], &chunk[2]) {
                    (
                        IndicatorValue::Scalar(u),
                        IndicatorValue::Scalar(m),
                        IndicatorValue::Scalar(l),
                    ) if !u.is_nan() => (*u, *m, *l),
                    _ => continue,
                };
                candles[i].indicators.insert(
                    key.clone(),
                    serde_json::json!({ "upper": u, "middle": m, "lower": l }),
                );
            }
            continue;
        }

        for (i, value) in values.iter().enumerate() {
            if i >= candles.len() {
                break;
            }
            let json_val = match value {
                IndicatorValue::Scalar(v) if !v.is_nan() => serde_json::json!(v),
                IndicatorValue::Macd(m) if !m.macd_line.is_nan() => serde_json::json!({
                    "macd_line": m.macd_line,
                    "signal_line": m.signal_line,
                    "histogram": m.histogram,
                }),
                _ => continue,
            };
            candles[i].indicators.insert(key.clone(), json_val);
        }
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_indicator_request_ma() {
        let result = parse_indicator_request("ma5,ma20,ma50");
        assert!(result.contains_key("ma"));
        if let Some(IndicatorConfig::Periods(periods)) = result.get("ma") {
            assert!(periods.contains(&5));
            assert!(periods.contains(&20));
            assert!(periods.contains(&50));
        } else {
            panic!("Expected Periods config for ma");
        }
    }

    #[test]
    fn test_parse_indicator_request_rsi() {
        let result = parse_indicator_request("rsi");
        assert!(result.contains_key("rsi"));
    }

    #[test]
    fn test_parse_indicator_request_macd() {
        let result = parse_indicator_request("macd");
        if let Some(IndicatorConfig::Periods(periods)) = result.get("macd") {
            assert_eq!(*periods, vec![12, 26, 9]);
        } else {
            panic!("Expected Periods config for macd");
        }
    }

    #[test]
    fn test_parse_indicator_request_bollinger() {
        let result = parse_indicator_request("bollinger");
        assert!(matches!(
            result.get("bollinger"),
            Some(IndicatorConfig::Bollinger(_))
        ));
    }

    #[test]
    fn test_parse_indicator_request_ignores_unknown() {
        let result = parse_indicator_request("ma5,unknown_indicator");
        assert!(!result.contains_key("unknown_indicator"));
    }

    #[test]
    fn test_parse_indicator_request_empty_string() {
        let result = parse_indicator_request("");
        assert!(result.is_empty());
    }
}
