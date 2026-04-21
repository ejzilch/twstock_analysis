use crate::api::middleware::ApiError;
use crate::api::models::enums::FetchSource;
use crate::api::models::request::CandlesQueryParams;
use crate::api::models::response::{CandleResponse, CandlesApiResponse};
use crate::app_state::AppState;
use crate::constants::CANDLES_MAX_PER_REQUEST;
use crate::models::Interval;
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
    let (candles, source, cached) =
        fetch_candles_with_cache(&state, &symbol, interval, params.from_ms, params.to_ms).await?;

    let count = candles.len();

    Ok(Json(CandlesApiResponse {
        symbol: symbol.clone(),
        interval: interval,
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        candles,
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
