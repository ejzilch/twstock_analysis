use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::constants::{
    BOLL_PERIOD, BOLL_STD_MULTIPLIER, BO_MACD_FAST, BO_MACD_SIGNAL, BO_MACD_SLOW,
    DEFAULT_AVG_CONSECUTIVE_LOSSES, RSI_PERIOD, TF_MA_LONG, TF_MA_MID, TF_MA_SHORT,
};
use crate::core::indicators::{
    bollinger::compute_bollinger, ma::compute_sma, macd::compute_macd, rsi::compute_rsi,
};
use crate::models::candle::MacdValue;
use axum::{extract::State, Json};
use chrono::Utc;
use std::sync::Arc;

use super::cost::{compute_annualized_return, compute_max_drawdown, compute_sharpe_ratio};
use super::types::{
    BacktestMetrics, BacktestRequest, BacktestResponse, CandleRow, TradeRecord, COMMISSION_RATE,
    DEFAULT_EXIT_FILTER_THRESHOLD, DEFAULT_MIN_HOLDING_DAYS, HARD_STOP_LOSS_PCT, RISK_FREE_ANNUAL,
    TAX_RATE,
};
use crate::api::handlers::backtest::market_filter::{
    compute_bandwidth_series, is_market_tradeable,
};
use crate::api::handlers::backtest::strategy::{
    breakout::{breakout_should_enter, breakout_should_exit},
    mean_reversion::{mean_reversion_should_enter, mean_reversion_should_exit},
    should_hold_position,
    trend_follow::{trend_follow_entry, trend_follow_should_exit},
};
/// POST /api/v1/backtest
pub async fn backtest_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
    // ── 基本參數驗證 ──────────────────────────────────────────────────────────
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

    // ── 取得 K 線資料 ─────────────────────────────────────────────────────────
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

    if candles.len() < 3 {
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "not enough candle data for backtest (need at least 3 daily candles)"
                .to_string(),
        });
    }

    // ── 解析請求參數 ──────────────────────────────────────────────────────────
    let position_fraction = request.position_size_percent / 100.0;

    let exit_filter_threshold = match request.exit_filter_pct {
        Some(v) if v < 0.0 => {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "exit_filter_pct must be >= 0.0".to_string(),
            });
        }
        Some(v) => v / 100.0,
        None => DEFAULT_EXIT_FILTER_THRESHOLD,
    };

    let min_holding_days = match request.min_holding_days {
        Some(v) => v,
        None => match request.strategy_name.as_str() {
            "mean_reversion_v1" => 2,
            _ => DEFAULT_MIN_HOLDING_DAYS,
        },
    };

    // ── 預算指標序列 ──────────────────────────────────────────────────────────
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let bandwidth_series = compute_bandwidth_series(&candles);
    let ma5_series = compute_sma(&closes, TF_MA_SHORT);
    let ma20_series = compute_sma(&closes, TF_MA_MID);
    let ma50_series = compute_sma(&closes, TF_MA_LONG);
    let rsi_series = compute_rsi(&closes, RSI_PERIOD);
    let boll_series: Vec<(f64, f64, f64)> =
        compute_bollinger(&closes, BOLL_PERIOD, BOLL_STD_MULTIPLIER);
    let macd_series: Vec<MacdValue> =
        compute_macd(&closes, BO_MACD_FAST, BO_MACD_SLOW, BO_MACD_SIGNAL);

    // ── 持倉狀態 ──────────────────────────────────────────────────────────────
    let mut equity: f64 = request.initial_capital;
    let mut equity_curve: Vec<f64> = vec![equity];
    let mut strategy_daily_returns: Vec<f64> = Vec::with_capacity(candles.len().saturating_sub(2));

    let mut in_position = false;
    let mut entry_price = 0.0_f64;
    let mut position_units = 0.0_f64;
    let mut position_cost = 0.0_f64;
    let mut holding_days = 0_u32;
    let mut entry_timestamp_ms = 0_i64;

    // ── 統計計數器 ────────────────────────────────────────────────────────────
    let mut winning_trades = 0_i32;
    let mut losing_trades = 0_i32;
    let mut gross_profit = 0.0_f64;
    let mut gross_loss = 0.0_f64;
    let mut consecutive_losses = 0_u32;
    let mut max_consecutive_losses = 0_u32;
    let mut max_consecutive_loss_amount = 0.0_f64;
    let mut current_loss_streak_amount = 0.0_f64;
    let mut consecutive_wins = 0_u32;
    let mut max_consecutive_wins = 0_u32;
    let mut loss_streaks: Vec<u32> = Vec::new();
    let mut trades: Vec<TradeRecord> = Vec::new();

    // ── 主迴圈：訊號在第 i 日收盤確認，第 i+1 日收盤成交 ────────────────────
    for i in 1..candles.len().saturating_sub(1) {
        // Layer 0：市場環境過濾
        if !is_market_tradeable(&bandwidth_series, i) {
            if in_position {
                holding_days += 1;
            }
            continue;
        }

        let exec_close = candles[i + 1].close;
        let prev_exec_close = candles[i].close;
        if exec_close <= 0.0 || prev_exec_close <= 0.0 {
            continue;
        }

        // ── 依策略取得進出場訊號 ──────────────────────────────────────────────
        let (should_hold, actual_position_fraction) = match request.strategy_name.as_str() {
            "trend_follow_v1" => trend_follow_entry(
                &ma5_series,
                &ma20_series,
                &ma50_series,
                &closes,
                position_fraction,
                i,
            ),
            "mean_reversion_v1" => {
                let close = candles[i].close;
                let (_, _, boll_lower) = boll_series[i];
                let hold =
                    mean_reversion_should_enter(close, ma50_series[i], rsi_series[i], boll_lower);
                (hold, position_fraction)
            }
            "breakout_v1" => {
                if i < 1 {
                    (false, 0.0)
                } else {
                    let close = candles[i].close;
                    let (boll_upper, _, _) = boll_series[i];
                    let macd_curr = &macd_series[i];
                    let macd_prev = &macd_series[i - 1];
                    if macd_curr.macd_line.is_nan() || macd_prev.macd_line.is_nan() {
                        (false, 0.0)
                    } else {
                        let hold = breakout_should_enter(
                            close,
                            boll_upper,
                            macd_curr.histogram,
                            macd_prev.histogram,
                            rsi_series[i],
                        );
                        (hold, position_fraction)
                    }
                }
            }
            _ => {
                let hold = should_hold_position(&request.strategy_name, &candles, i);
                (hold, position_fraction)
            }
        };

        // ── 出場訊號 ──────────────────────────────────────────────────────────
        let strategy_wants_exit = match request.strategy_name.as_str() {
            "trend_follow_v1" => {
                trend_follow_should_exit(&ma5_series, &ma20_series, &ma50_series, &rsi_series, i)
            }
            "mean_reversion_v1" => {
                let close = candles[i].close;
                let (_, boll_middle, _) = boll_series[i];
                mean_reversion_should_exit(close, ma50_series[i], boll_middle)
            }
            "breakout_v1" => {
                let macd_curr = &macd_series[i];
                if macd_curr.macd_line.is_nan() {
                    false
                } else {
                    let (boll_upper, _, _) = boll_series[i];
                    breakout_should_exit(candles[i].close, boll_upper, macd_curr.histogram)
                }
            }
            _ => !should_hold,
        };

        // ── 出場緩衝濾網 ──────────────────────────────────────────────────────
        let signal_close = candles[i].close;
        let signal_prev = candles[i.saturating_sub(1)].close;
        let drop_ratio = if signal_prev > 0.0 {
            (signal_prev - signal_close) / signal_prev
        } else {
            0.0
        };

        // ── 硬停損 ────────────────────────────────────────────────────────────
        let hard_stop_triggered = in_position
            && entry_price > 0.0
            && (exec_close - entry_price) / entry_price <= -HARD_STOP_LOSS_PCT;

        let can_exit = holding_days >= min_holding_days;

        let signal_exit = match request.strategy_name.as_str() {
            "mean_reversion_v1" | "breakout_v1" => strategy_wants_exit && can_exit,
            _ => strategy_wants_exit && drop_ratio >= exit_filter_threshold && can_exit,
        };
        let should_exit = (signal_exit || hard_stop_triggered) && in_position;

        // ── 進場 ──────────────────────────────────────────────────────────────
        if should_hold && !in_position {
            entry_timestamp_ms = candles[i + 1].timestamp_ms;
            let capital_deployed = equity * actual_position_fraction;
            let buy_fee = capital_deployed * COMMISSION_RATE;
            position_units = capital_deployed / exec_close;
            position_cost = capital_deployed;
            entry_price = exec_close;
            equity -= buy_fee;
            in_position = true;
            holding_days = 0;
        } else if should_exit {
            // ── 出場 ──────────────────────────────────────────────────────────
            let market_value = position_units * exec_close;
            let sell_fee = market_value * (COMMISSION_RATE + TAX_RATE);
            let cash_received = market_value - sell_fee;
            equity = equity - position_cost + cash_received;

            let buy_fee_paid = position_cost * COMMISSION_RATE;
            let gross_pnl = market_value - position_cost;
            let net_pnl = gross_pnl - buy_fee_paid - sell_fee;

            update_trade_stats(
                net_pnl,
                position_cost,
                &mut winning_trades,
                &mut losing_trades,
                &mut gross_profit,
                &mut gross_loss,
                &mut consecutive_wins,
                &mut max_consecutive_wins,
                &mut consecutive_losses,
                &mut max_consecutive_losses,
                &mut max_consecutive_loss_amount,
                &mut current_loss_streak_amount,
                &mut loss_streaks,
            );

            in_position = false;
            holding_days = 0;
            position_units = 0.0;
            position_cost = 0.0;

            if hard_stop_triggered {
                tracing::debug!(entry_price, exec_close, "Hard stop-loss triggered");
            }

            trades.push(TradeRecord {
                entry_timestamp_ms,
                exit_timestamp_ms: candles[i + 1].timestamp_ms,
                entry_price,
                exit_price: exec_close,
                net_pnl,
                is_win: net_pnl >= 0.0,
            });
        }

        // ── 每日報酬 ──────────────────────────────────────────────────────────
        let day_return = if in_position {
            holding_days += 1;
            let position_value_today = position_units * exec_close;
            let position_value_prev = position_units * prev_exec_close;
            (position_value_today - position_value_prev) / equity.max(f64::EPSILON)
        } else {
            0.0
        };

        equity *= 1.0 + day_return;
        strategy_daily_returns.push(day_return);
        equity_curve.push(equity);
    }

    // ── 末日強制平倉 ──────────────────────────────────────────────────────────
    if in_position {
        let last_close = candles.last().map(|c| c.close).unwrap_or(entry_price);
        let market_value = position_units * last_close;
        let sell_fee = market_value * (COMMISSION_RATE + TAX_RATE);
        let cash_received = market_value - sell_fee;
        equity = equity - position_cost + cash_received;

        let buy_fee_paid = position_cost * COMMISSION_RATE;
        let gross_pnl = market_value - position_cost;
        let net_pnl = gross_pnl - buy_fee_paid - sell_fee;

        update_trade_stats(
            net_pnl,
            position_cost,
            &mut winning_trades,
            &mut losing_trades,
            &mut gross_profit,
            &mut gross_loss,
            &mut consecutive_wins,
            &mut max_consecutive_wins,
            &mut consecutive_losses,
            &mut max_consecutive_losses,
            &mut max_consecutive_loss_amount,
            &mut current_loss_streak_amount,
            &mut loss_streaks,
        );

        trades.push(TradeRecord {
            entry_timestamp_ms,
            exit_timestamp_ms: candles.last().map(|c| c.timestamp_ms).unwrap_or(0),
            entry_price,
            exit_price: last_close,
            net_pnl,
            is_win: net_pnl >= 0.0,
        });
    }

    // 末日平倉後收尾連虧區間
    if consecutive_losses > 0 {
        loss_streaks.push(consecutive_losses);
    }

    // ── 彙整指標 ──────────────────────────────────────────────────────────────
    let avg_consecutive_losses = if loss_streaks.is_empty() {
        DEFAULT_AVG_CONSECUTIVE_LOSSES
    } else {
        loss_streaks.iter().sum::<u32>() as f64 / loss_streaks.len() as f64
    };

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
    let sharpe_ratio = compute_sharpe_ratio(&strategy_daily_returns, RISK_FREE_ANNUAL);
    let annual_return = compute_annualized_return(
        request.initial_capital,
        equity,
        strategy_daily_returns.len(),
    );

    Ok(Json(BacktestResponse {
        backtest_id: format!("bt-{}", uuid::Uuid::new_v4()),
        symbol: request.symbol,
        strategy_name: request.strategy_name,
        from_ms: request.from_ms,
        to_ms: request.to_ms,
        initial_capital: request.initial_capital,
        final_capital: equity,
        exit_filter_pct: exit_filter_threshold * 100.0,
        trades,
        metrics: BacktestMetrics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            profit_factor,
            max_drawdown,
            sharpe_ratio,
            annual_return,
            max_consecutive_losses,
            max_consecutive_loss_amount,
            avg_consecutive_losses,
            max_consecutive_wins,
        },
        created_at_ms: Utc::now().timestamp_millis(),
    }))
}

// ── 私有：統一的交易統計更新 ──────────────────────────────────────────────────
// 進出場時呼叫一次，主迴圈與末日平倉都走同一條路徑。
#[allow(clippy::too_many_arguments)]
fn update_trade_stats(
    net_pnl: f64,
    position_cost: f64,
    winning_trades: &mut i32,
    losing_trades: &mut i32,
    gross_profit: &mut f64,
    gross_loss: &mut f64,
    consecutive_wins: &mut u32,
    max_consecutive_wins: &mut u32,
    consecutive_losses: &mut u32,
    max_consecutive_losses: &mut u32,
    max_consecutive_loss_amount: &mut f64,
    current_loss_streak_amount: &mut f64,
    loss_streaks: &mut Vec<u32>,
) {
    if net_pnl >= 0.0 {
        *winning_trades += 1;
        *gross_profit += net_pnl / position_cost;
        *consecutive_wins += 1;
        if *consecutive_wins > *max_consecutive_wins {
            *max_consecutive_wins = *consecutive_wins;
        }
        if *consecutive_losses > 0 {
            loss_streaks.push(*consecutive_losses);
        }
        *consecutive_losses = 0;
        *current_loss_streak_amount = 0.0;
    } else {
        *losing_trades += 1;
        *gross_loss += net_pnl.abs() / position_cost;
        *consecutive_wins = 0;
        *consecutive_losses += 1;
        *current_loss_streak_amount = (*current_loss_streak_amount + net_pnl.abs()).min(f64::MAX);
        if *consecutive_losses > *max_consecutive_losses {
            *max_consecutive_losses = *consecutive_losses;
            *max_consecutive_loss_amount = *current_loss_streak_amount;
        }
    }
}
