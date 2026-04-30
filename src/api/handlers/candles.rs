use crate::api::middleware::ApiError;
use crate::api::models::enums::FetchSource;
use crate::api::models::request::CandlesQueryParams;
use crate::api::models::response::{CandleResponse, CandlesApiResponse};
use crate::app_state::AppState;
use crate::constants::CANDLES_MAX_PER_REQUEST;
use crate::domain::indicators::factory::IndicatorFactory;
use crate::models::indicators::{BollingerConfig, IndicatorConfig};
use crate::models::{Candle, IndicatorValue, Interval};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};

/// GET /api/v1/candles/{symbol}
///
/// 單次最多回傳 CANDLES_MAX_QUERY_LIMIT 根 K 線。
/// 超過範圍時回傳 400 QUERY_RANGE_TOO_LARGE，不自動截斷。
/// 使用 next_cursor 分頁取得後續資料。
pub async fn candles_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(params): Query<CandlesQueryParams>,
) -> Result<Json<CandlesApiResponse>, ApiError> {
    // 確認 symbol 存在
    validate_symbol_exists(&state, &symbol).await?;

    let interval = params.interval();

    // 先查詢總筆數，超過上限直接拒絕
    let total_available =
        count_candles(&state, &symbol, interval, params.from_ms, params.to_ms).await?;

    if total_available > CANDLES_MAX_PER_REQUEST {
        return Err(ApiError::QueryRangeTooLarge {
            requested: total_available,
            max: CANDLES_MAX_PER_REQUEST,
        });
    }

    // 嘗試從 Redis 取快取
    let (mut candles_response, source, cached) =
        fetch_candles_with_cache(&state, &symbol, interval, params.from_ms, params.to_ms).await?;

    // ── 指標計算: 直接使用既有的 IndicatorFactory ────────────────────────────────
    if let Some(indicator_str) = &params.indicators {
        if !indicator_str.trim().is_empty() {
            let indicator_request = parse_indicator_request(indicator_str);

            if !indicator_request.is_empty() {
                // 把 CandleResponse 轉成 factory 需要的 Candle 格式
                let candles_for_calc: Vec<Candle> = candles_response
                    .iter()
                    .map(|c| Candle {
                        symbol: symbol.clone(),
                        interval,
                        timestamp_ms: c.timestamp_ms,
                        open: c.open,
                        high: c.high,
                        low: c.low,
                        close: c.close,
                        volume: c.volume as u64,
                        indicators: Default::default(),
                    })
                    .collect();

                match IndicatorFactory::build_from_request(&indicator_request) {
                    Ok(factory) => {
                        match factory.compute_all(&candles_for_calc) {
                            Ok((computed, _)) => {
                                // 把計算結果填回 CandleResponse 的 indicators HashMap
                                apply_indicators_to_candles(&mut candles_response, &computed);
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Indicator computation failed, returning candles without indicators");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to build indicator factory");
                    }
                }
            }
        }
    }

    let count = candles_response.len();

    Ok(Json(CandlesApiResponse {
        symbol: symbol.clone(),
        interval: interval,
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        candles: candles_response,
        count,
        total_available,
        next_cursor: None, // cursor 分頁於此版本以 total_available 限制取代
        source: source,
        cached,
        computed_at_ms: Utc::now().timestamp_millis(),
    }))
}

// ── 私有查詢函數 ──────────────────────────────────────────────────────────────

async fn validate_symbol_exists(state: &AppState, symbol: &str) -> Result<(), ApiError> {
    let exists: Option<bool> = sqlx::query_scalar!(
        r#"
        SELECT is_active as "is_active!"
        FROM symbols
        WHERE symbol = $1
        "#,
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
        WHERE symbol = $1
          AND interval = $2
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

async fn fetch_candles_with_cache(
    state: &AppState,
    symbol: &str,
    interval: Interval,
    from_ms: i64,
    to_ms: i64,
) -> Result<(Vec<CandleResponse>, FetchSource, bool), ApiError> {
    // 嘗試 Redis 快取
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

    // 快取未命中，從 DB 查詢
    let candles = fetch_candles_from_db(state, symbol, interval, from_ms, to_ms).await?;
    Ok((candles, FetchSource::Database, false))
}

async fn fetch_candles_from_db(
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
        WHERE symbol = $1
          AND interval = $2
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

/// 把 "ma5,ma20,ma50,rsi,macd,bollinger" 解析成 IndicatorFactory 需要的 HashMap
fn parse_indicator_request(indicator_str: &str) -> HashMap<String, IndicatorConfig> {
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

fn apply_indicators_to_candles(
    candles: &mut Vec<CandleResponse>,
    computed: &HashMap<String, Vec<IndicatorValue>>,
) {
    for (key, values) in computed {
        // Bollinger 的 values 是 [upper0, mid0, lower0, upper1, mid1, lower1, ...]
        // 三個 Scalar 為一組，對應一根 K 棒
        let is_bollinger = key == "bollinger";

        if is_bollinger {
            let groups = values.chunks(3);
            for (i, chunk) in groups.enumerate() {
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
                    serde_json::json!({
                        "upper": u, "middle": m, "lower": l,
                    }),
                );
            }
            continue;
        }

        // MA、RSI：單一 Scalar
        // MACD：MacdValue
        for (i, value) in values.iter().enumerate() {
            if i >= candles.len() {
                break;
            }
            let json_val = match value {
                IndicatorValue::Scalar(v) if !v.is_nan() => serde_json::json!(v),
                IndicatorValue::Macd(m) if !m.macd_line.is_nan() => serde_json::json!({
                    "macd_line":   m.macd_line,
                    "signal_line": m.signal_line,
                    "histogram":   m.histogram,
                }),
                _ => continue,
            };
            candles[i].indicators.insert(key.clone(), json_val);
        }
    }
}
