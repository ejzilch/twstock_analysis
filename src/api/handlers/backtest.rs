use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── POST /api/v1/backtest ─────────────────────────────────────────────────────

/// POST /api/v1/backtest 的請求結構，對應 API_CONTRACT.md
#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub request_id: String,
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub position_size_percent: f64,
}

/// POST /api/v1/backtest 的回測指標
#[derive(Debug, Serialize)]
pub struct BacktestMetrics {
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub annual_return: f64,
}

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
}

/// POST /api/v1/backtest
///
/// Rust Gateway 轉發請求。
/// 回測指標計算依賴 POST /api/v1/indicators/compute，確保與實盤一致。
pub async fn backtest_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
    // request_id 帶入 tracing span，讓這次請求的所有 log 可以關聯
    tracing::info!(
        request_id = %request.request_id,
        symbol = %request.symbol,
        strategy = %request.strategy_name,
        state = ?state,
        "Backtest request received"
    );

    // 基本參數驗證
    if request.initial_capital <= 0.0 {
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "initial_capital must be greater than 0".to_string(),
        });
    }

    if !(1.0..=100.0).contains(&request.position_size_percent) {
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "position_size_percent must be between 1 and 100".to_string(),
        });
    }

    // TODO: 轉發至 Python 回測引擎
    // 目前回傳佔位回應，待 Codex 實作 Python 回測後替換
    let backtest_id = format!("bt-{}", uuid::Uuid::new_v4());

    Ok(Json(BacktestResponse {
        backtest_id,
        symbol: request.symbol,
        strategy_name: request.strategy_name,
        from_ms: request.from_ms,
        to_ms: request.to_ms,
        initial_capital: request.initial_capital,
        final_capital: request.initial_capital,
        metrics: BacktestMetrics {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: 0.0,
            profit_factor: 0.0,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            annual_return: 0.0,
        },
        created_at_ms: Utc::now().timestamp_millis(),
    }))
}
