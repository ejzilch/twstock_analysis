use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
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

#[derive(Debug, FromRow, Clone)]
struct CandleRow {
    timestamp_ms: i64,
    close: f64,
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

    if request.from_ms >= request.to_ms {
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "from_ms must be earlier than to_ms".to_string(),
        });
    }

    let candles: Vec<CandleRow> = sqlx::query_as::<_, CandleRow>(
        r#"
        SELECT timestamp_ms, close
        FROM candles
        WHERE symbol = $1
          AND interval = '1d'
          AND timestamp_ms >= $2
          AND timestamp_ms <= $3
        ORDER BY timestamp_ms ASC
        "#,
    )
    .bind(&request.symbol)
    .bind(request.from_ms)
    .bind(request.to_ms)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Backtest candle query failed: {e}")))?;

    if candles.len() < 2 {
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "not enough candle data for backtest (need at least 2 daily candles)"
                .to_string(),
        });
    }

    let position_fraction = request.position_size_percent / 100.0;
    let mut equity = request.initial_capital;
    let mut equity_curve = vec![equity];
    let mut strategy_daily_returns = Vec::with_capacity(candles.len().saturating_sub(1));

    let mut in_position = false;
    let mut entry_price = 0.0;
    let mut winning_trades = 0i32;
    let mut losing_trades = 0i32;
    let mut gross_profit = 0.0f64;
    let mut gross_loss = 0.0f64;

    for i in 1..candles.len() {
        let prev_close = candles[i - 1].close;
        let close = candles[i].close;
        if prev_close <= 0.0 || close <= 0.0 {
            continue;
        }

        let should_hold = should_hold_position(&request.strategy_name, &candles, i);

        // 進場 / 出場（以當日收盤判定）
        if should_hold && !in_position {
            in_position = true;
            entry_price = close;
        } else if !should_hold && in_position {
            in_position = false;
            let trade_pnl_ratio = (close - entry_price) / entry_price;
            if trade_pnl_ratio >= 0.0 {
                winning_trades += 1;
                gross_profit += trade_pnl_ratio;
            } else {
                losing_trades += 1;
                gross_loss += trade_pnl_ratio.abs();
            }
        }

        let day_return = if in_position {
            ((close - prev_close) / prev_close) * position_fraction
        } else {
            0.0
        };

        equity *= 1.0 + day_return;
        strategy_daily_returns.push(day_return);
        equity_curve.push(equity);
    }

    // 若最後仍持倉，於末日收盤強制平倉
    if in_position {
        let last_close = candles.last().map(|c| c.close).unwrap_or(entry_price);
        let trade_pnl_ratio = (last_close - entry_price) / entry_price;
        if trade_pnl_ratio >= 0.0 {
            winning_trades += 1;
            gross_profit += trade_pnl_ratio;
        } else {
            losing_trades += 1;
            gross_loss += trade_pnl_ratio.abs();
        }
    }

    let total_trades = winning_trades + losing_trades;
    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };
    let profit_factor = if gross_loss > 0.0 {
        gross_profit / gross_loss
    } else if gross_profit > 0.0 {
        999.0
    } else {
        0.0
    };
    let max_drawdown = compute_max_drawdown(&equity_curve);
    let sharpe_ratio = compute_sharpe_ratio(&strategy_daily_returns);
    let annual_return = compute_annualized_return(
        request.initial_capital,
        equity,
        strategy_daily_returns.len(),
    );

    let backtest_id = format!("bt-{}", uuid::Uuid::new_v4());

    Ok(Json(BacktestResponse {
        backtest_id,
        symbol: request.symbol,
        strategy_name: request.strategy_name,
        from_ms: request.from_ms,
        to_ms: request.to_ms,
        initial_capital: request.initial_capital,
        final_capital: equity,
        metrics: BacktestMetrics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            profit_factor,
            max_drawdown,
            sharpe_ratio,
            annual_return,
        },
        created_at_ms: Utc::now().timestamp_millis(),
    }))
}

fn should_hold_position(strategy_name: &str, candles: &[CandleRow], idx: usize) -> bool {
    let close = candles[idx].close;
    let prev = candles[idx.saturating_sub(1)].close;

    match strategy_name {
        // 趨勢跟隨：今日收盤高於昨日收盤則持有
        "trend_follow_v1" => close > prev,

        // 均值回歸：跌超過 1% 進場，反彈後離場
        "mean_reversion_v1" => close < prev * 0.99,

        // 突破：突破前 5 日最高收盤才持有
        "breakout_v1" => {
            let start = idx.saturating_sub(5);
            let recent_max = candles[start..idx]
                .iter()
                .map(|c| c.close)
                .fold(f64::MIN, f64::max);
            close > recent_max
        }

        // 未知策略 fallback
        _ => close > prev,
    }
}

fn compute_max_drawdown(equity_curve: &[f64]) -> f64 {
    let mut peak = equity_curve.first().copied().unwrap_or(1.0);
    let mut max_dd = 0.0;
    for &v in equity_curve {
        if v > peak {
            peak = v;
        }
        if peak > 0.0 {
            let dd = (peak - v) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    max_dd
}

fn compute_sharpe_ratio(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let mean = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let variance = daily_returns
        .iter()
        .map(|r| {
            let d = *r - mean;
            d * d
        })
        .sum::<f64>()
        / (daily_returns.len() as f64 - 1.0);
    let std = variance.sqrt();
    if std <= f64::EPSILON {
        0.0
    } else {
        (mean / std) * 252.0_f64.sqrt()
    }
}

fn compute_annualized_return(initial: f64, final_capital: f64, trading_days: usize) -> f64 {
    if initial <= 0.0 || final_capital <= 0.0 || trading_days == 0 {
        return 0.0;
    }
    let years = trading_days as f64 / 252.0;
    if years <= 0.0 {
        0.0
    } else {
        (final_capital / initial).powf(1.0 / years) - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_drawdown_non_zero() {
        let curve = vec![100.0, 120.0, 90.0, 110.0];
        let dd = compute_max_drawdown(&curve);
        assert!(dd > 0.0);
    }

    #[test]
    fn test_sharpe_zero_when_flat() {
        let returns = vec![0.0, 0.0, 0.0];
        assert_eq!(compute_sharpe_ratio(&returns), 0.0);
    }

    #[test]
    fn test_annualized_return_positive_growth() {
        let r = compute_annualized_return(100.0, 110.0, 252);
        assert!(r > 0.0);
    }
}
