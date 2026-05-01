/// POST /api/v1/backtest — 薄 handler
///
/// 職責：解析 request → 呼叫 BacktestService → 回傳 response。
/// 所有業務邏輯已移至 `crate::services::backtest::BacktestService`。
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::domain::backtest::engine::{BacktestMetrics, TradeRecord};
use crate::services::backtest::{BacktestParams, BacktestService};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Request ───────────────────────────────────────────────────────────────────

/// POST /api/v1/backtest 的請求結構，對應 API_CONTRACT.md
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BacktestRequest {
    pub request_id: String,
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    /// 出場緩衝濾網：持倉中訊號轉空時，需跌破前收盤幾 % 才真正出場。
    /// 不傳時使用預設值 1.5%（DEFAULT_EXIT_FILTER_THRESHOLD）。
    /// 傳 0.0 則等同停用濾網（還原為原始行為）。
    #[serde(default)]
    pub exit_filter_pct: Option<f64>,
    /// 最短持倉天數：進場後至少持有幾天才允許出場訊號生效。
    /// 不傳時使用預設值 5 天（DEFAULT_MIN_HOLDING_DAYS）。
    /// 傳 0 則等同停用（任何時候都可出場）。
    #[serde(default)]
    pub min_holding_days: Option<u32>,
}

impl From<BacktestRequest> for BacktestParams {
    fn from(request: BacktestRequest) -> Self {
        BacktestParams {
            symbol: request.symbol,
            strategy_name: request.strategy_name,
            from_ms: request.from_ms,
            to_ms: request.to_ms,
            initial_capital: request.initial_capital,
            position_size_percent: request.position_size_percent,
            exit_filter_pct: request.exit_filter_pct,
            min_holding_days: request.min_holding_days,
        }
    }
}

// ── Response ──────────────────────────────────────────────────────────────────

/// POST /api/v1/backtest 的完整回應
#[derive(Debug, Serialize)]
pub struct BacktestResponse {
    pub backtest_id: String,
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub metrics: BacktestMetrics,
    pub created_at_ms: i64,
    pub exit_filter_pct: f64,
    /// 每筆交易記錄，供前端 K 線圖標記使用
    pub trades: Vec<TradeRecord>,
}

/// POST /api/v1/backtest
pub async fn backtest_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
    let params = BacktestParams::from(request);
    let output = BacktestService::run(&state, &params).await.map_err(|e| {
        ApiError::InvalidIndicatorConfig {
            detail: e.to_string(),
        }
    })?;

    Ok(Json(BacktestResponse {
        backtest_id: format!("bt-{}", uuid::Uuid::new_v4()),
        symbol: params.symbol,
        strategy_name: params.strategy_name,
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        initial_capital: params.initial_capital,
        // 從 output 來的
        final_capital: output.final_capital,
        exit_filter_pct: output.exit_filter_pct,
        trades: output.trades,
        metrics: output.metrics,
        created_at_ms: output.created_at_ms,
    }))
}
