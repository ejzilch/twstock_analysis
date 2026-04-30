use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::domain::indicators::factory::IndicatorFactory;
use crate::models::indicators::{ComputeIndicatorsRequest, ComputeIndicatorsResponse};
use crate::models::Interval;
use axum::{extract::State, Json};
use chrono::Utc;
use std::sync::Arc;

/// POST /api/v1/indicators/compute
///
/// 動態參數化指標計算，計算順序由 IndicatorFactory 依 DAG 拓撲排序決定。
/// response 包含 dag_execution_order 供驗證計算順序。
pub async fn compute_indicators_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ComputeIndicatorsRequest>,
) -> Result<Json<ComputeIndicatorsResponse>, ApiError> {
    let started_at = Utc::now().timestamp_millis();

    // 從 DB 取得 K 線資料
    let candles = fetch_candles_for_compute(
        &state,
        &request.symbol,
        request.interval,
        request.from_ms,
        request.to_ms,
    )
    .await?;

    // 依請求建立 IndicatorFactory，validate 指標設定
    let factory = IndicatorFactory::build_from_request(&request.indicators).map_err(|e| {
        ApiError::InvalidIndicatorConfig {
            detail: e.to_string(),
        }
    })?;

    // 執行 DAG 拓撲排序並計算所有指標
    let (computed, dag_execution_order) = factory.compute_all(&candles).map_err(|e| {
        tracing::error!(
            symbol = %request.symbol,
            error  = %e,
            "Indicator computation failed"
        );
        ApiError::IndicatorComputeFailed {
            detail: e.to_string(),
        }
    })?;

    let computation_time_ms = Utc::now().timestamp_millis() - started_at;

    Ok(Json(ComputeIndicatorsResponse {
        symbol: request.symbol,
        interval: request.interval,
        from_ms: request.from_ms,
        to_ms: request.to_ms,
        indicators: computed,
        computed_at_ms: started_at,
        computation_time_ms,
        cached: false,
        dag_execution_order,
    }))
}

// ── 私有查詢函數 ──────────────────────────────────────────────────────────────

async fn fetch_candles_for_compute(
    state: &AppState,
    symbol: &str,
    interval: Interval,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<crate::models::Candle>, ApiError> {
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
        tracing::error!(error = %e, "Failed to fetch candles for indicator compute");
        ApiError::DataSourceInterrupted
    })?;

    let interval_enum = interval
        .as_str()
        .parse::<crate::models::enums::Interval>()
        .unwrap_or(crate::models::enums::Interval::OneHour);

    Ok(rows
        .into_iter()
        .map(|row| crate::models::Candle {
            symbol: symbol.to_string(),
            interval: interval_enum,
            timestamp_ms: row.timestamp_ms,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume as u64,
            indicators: Default::default(),
        })
        .collect())
}
