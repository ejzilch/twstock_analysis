/// 回測引擎核心（純計算，零 I/O）
///
/// 職責：接收 K 線資料與策略參數，執行模擬交易，回傳指標結果。
/// 不依賴任何資料庫、HTTP、Redis。可單元測試。
use super::constants::{
    COMMISSION_RATE, DEFAULT_COOLDOWN_DAYS, DEFAULT_EXIT_FILTER_THRESHOLD,
    DEFAULT_MIN_HOLDING_DAYS, DEFAULT_TP_BOLL_PCT, HARD_STOP_LOSS_PCT, RISK_FREE_ANNUAL, TAX_RATE,
    TP_BOLL_CONSEC_DAYS,
};
use super::metrics::{compute_annualized_return, compute_max_drawdown, compute_sharpe_ratio};
use crate::constants::{
    BOLL_PERIOD, BOLL_STD_MULTIPLIER, BO_MACD_FAST, BO_MACD_SIGNAL, BO_MACD_SLOW, RSI_PERIOD,
};
use crate::domain::indicators::{
    bollinger::compute_bollinger, ma::compute_sma, macd::compute_macd, rsi::compute_rsi,
};
use crate::domain::strategy::constants::{
    DEFAULT_AVG_CONSECUTIVE_LOSSES, TF_MA_LONG, TF_MA_MID, TF_MA_SHORT,
};
use crate::domain::strategy::manual_strategy::market_filter::{
    compute_bandwidth_series, is_market_tradeable,
};
use crate::domain::strategy::manual_strategy::{
    breakout::{breakout_should_enter, breakout_should_exit},
    mean_reversion::{mean_reversion_should_enter, mean_reversion_should_exit},
    should_hold_position,
    trend_follow::{trend_follow_entry, trend_follow_should_exit},
};

use crate::models::candle::{CandleRow, MacdValue};
use chrono::Utc;
use serde::Serialize;

// ── 輸入 / 輸出 ───────────────────────────────────────────────────────────────

/// 引擎輸入：K 線資料 + 策略設定
pub struct BacktestInput<'a> {
    pub candles: &'a [CandleRow],
    pub strategy_name: String,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    /// 出場緩衝濾網：持倉中訊號轉空時，需跌破前收盤幾 % 才真正出場。
    /// 不傳時使用預設值 1.5%（DEFAULT_EXIT_FILTER_THRESHOLD）。
    /// 傳 0.0 則等同停用濾網（還原為原始行為）。
    pub exit_filter_pct: Option<f64>,
    /// 最短持倉天數：進場後至少持有幾天才允許出場訊號生效。
    /// 不傳時使用預設值 5 天（DEFAULT_MIN_HOLDING_DAYS）。
    /// 傳 0 則等同停用（任何時候都可出場）。
    pub min_holding_days: Option<u32>,
    /// 停利濾網：收盤連續 2 天超出布林上軌幾 % 就停利出場。
    /// 不傳時使用預設值 5%（DEFAULT_TP_BOLL_PCT）。
    /// 傳 0.0 則停用停利機制。
    pub take_profit_boll_pct: Option<f64>,
}

/// 引擎輸出：完整回測結果（不含 symbol / from_ms 等 metadata，由 Service 補充）
pub struct BacktestOutput {
    pub final_capital: f64,
    pub exit_filter_pct: f64,
    pub trades: Vec<TradeRecord>,
    pub metrics: BacktestMetrics,
    pub created_at_ms: i64,
}

/// 單筆交易記錄
#[derive(Debug, Serialize)]
pub struct TradeRecord {
    /// 進場時間戳（毫秒）
    pub entry_timestamp_ms: i64,
    /// 出場時間戳（毫秒）
    pub exit_timestamp_ms: i64,
    /// 進場價格
    pub entry_price: f64,
    /// 出場價格
    pub exit_price: f64,
    /// 損益金額
    pub net_pnl: f64,
    /// 是否獲利
    pub is_win: bool,
    /// 出場原因："signal" | "hard_stop" | "take_profit" | "forced"
    pub exit_reason: String,
}

/// 回測指標
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
    /// 回測期間最長連續虧損筆數
    pub max_consecutive_losses: u32,
    /// 最長連虧區間的累計虧損金額（TWD，絕對值）
    pub max_consecutive_loss_amount: f64,
    /// 所有連虧區間的平均筆數（無連虧時為 0.0）
    pub avg_consecutive_losses: f64,
    /// 回測期間最長連續獲利筆數
    pub max_consecutive_wins: u32,
}

// ── 引擎入口 ──────────────────────────────────────────────────────────────────

/// 執行回測，回傳結果。
///
/// 所有錯誤以 `anyhow::Error` 回傳，由上層（Service）轉換為 ApiError。
pub fn run(input: &BacktestInput) -> anyhow::Result<BacktestOutput> {
    let candles = input.candles;

    // ── 解析請求參數 ──────────────────────────────────────────────────────────
    let position_fraction = input.position_size_percent / 100.0;

    let exit_filter_threshold = match input.exit_filter_pct {
        Some(v) if v < 0.0 => {
            anyhow::bail!("exit_filter_pct must be >= 0.0");
        }
        Some(v) => v / 100.0,
        None => DEFAULT_EXIT_FILTER_THRESHOLD,
    };

    let min_holding_days = match input.min_holding_days {
        Some(v) => v,
        None => match input.strategy_name.as_str() {
            "mean_reversion_v1" => 2,
            _ => DEFAULT_MIN_HOLDING_DAYS,
        },
    };

    let tp_boll_pct = match input.take_profit_boll_pct {
        Some(v) if v < 0.0 => anyhow::bail!("take_profit_boll_pct must be >= 0.0"),
        Some(v) => v / 100.0,
        None => DEFAULT_TP_BOLL_PCT,
    };

    // ── 預算指標序列 ──────────────────────────────────────────────────────────
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let bandwidth_series = compute_bandwidth_series(candles);
    let ma5_series = compute_sma(&closes, TF_MA_SHORT);
    let ma20_series = compute_sma(&closes, TF_MA_MID);
    let ma50_series = compute_sma(&closes, TF_MA_LONG);
    let rsi_series = compute_rsi(&closes, RSI_PERIOD);
    let boll_series: Vec<(f64, f64, f64)> =
        compute_bollinger(&closes, BOLL_PERIOD, BOLL_STD_MULTIPLIER);
    let macd_series: Vec<MacdValue> =
        compute_macd(&closes, BO_MACD_FAST, BO_MACD_SLOW, BO_MACD_SIGNAL);

    // ── 持倉狀態 ──────────────────────────────────────────────────────────────
    let mut equity: f64 = input.initial_capital;
    let mut equity_curve: Vec<f64> = vec![equity];
    let mut strategy_daily_returns: Vec<f64> = Vec::with_capacity(candles.len().saturating_sub(2));

    let mut in_position = false;
    let mut entry_price = 0.0_f64;
    let mut position_units = 0.0_f64;
    let mut position_cost = 0.0_f64;
    let mut holding_days = 0_u32;
    let mut entry_timestamp_ms = 0_i64;
    let mut entry_idx = 0_usize;

    // ── 停利 & 冷卻狀態 ───────────────────────────────────────────────────────
    let mut days_above_boll_high = 0_u32; // 連續超出布林上軌天數
    let mut cooldown_remaining = 0_u32; // 停利後冷卻剩餘天數

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

    // ── 主迴圈 ────────────────────────────────────────────────────────────────
    for i in 1..candles.len().saturating_sub(1) {
        // 停利計數
        if in_position {
            let (boll_upper, _, _) = boll_series[i];
            let tp_threshold = boll_upper * (1.0 + tp_boll_pct);
            if boll_upper > 0.0 && candles[i].close > tp_threshold {
                days_above_boll_high += 1;
            } else {
                days_above_boll_high = 0;
            }
        } else {
            days_above_boll_high = 0;
        }

        let take_profit_triggered =
            in_position && tp_boll_pct > 0.0 && days_above_boll_high >= TP_BOLL_CONSEC_DAYS;

        // 停利優先，不受 is_market_tradeable 限制
        if take_profit_triggered {
            let exec_close = candles[i].close; // 當天收盤出場，不等下一根
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

            cooldown_remaining = DEFAULT_COOLDOWN_DAYS;

            trades.push(TradeRecord {
                entry_timestamp_ms,
                exit_timestamp_ms: candles[i].timestamp_ms,
                entry_price,
                exit_price: exec_close,
                net_pnl,
                is_win: net_pnl >= 0.0,
                exit_reason: "take_profit".to_string(),
            });

            in_position = false;
            holding_days = 0;
            position_units = 0.0;
            position_cost = 0.0;
            days_above_boll_high = 0;
        }

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

        // ── 進出場訊號（原有邏輯） ────────────────────────────────────────────
        let (should_hold, actual_position_fraction) = resolve_entry_signal(
            input,
            &ma5_series,
            &ma20_series,
            &ma50_series,
            &closes,
            &boll_series,
            &rsi_series,
            &macd_series,
            candles,
            position_fraction,
            i,
        );

        let strategy_wants_exit = resolve_exit_signal(
            input,
            &ma5_series,
            &ma20_series,
            &ma50_series,
            &closes,
            &rsi_series,
            &boll_series,
            &macd_series,
            candles,
            should_hold,
            i,
            entry_idx,
        );

        let signal_close = candles[i].close;
        let signal_prev = candles[i.saturating_sub(1)].close;
        let drop_ratio = if signal_prev > 0.0 {
            (signal_prev - signal_close) / signal_prev
        } else {
            0.0
        };

        let hard_stop_triggered = in_position
            && entry_price > 0.0
            && (exec_close - entry_price) / entry_price <= -HARD_STOP_LOSS_PCT;

        let can_exit = holding_days >= min_holding_days;

        let signal_exit = match input.strategy_name.as_str() {
            "mean_reversion_v1" | "breakout_v1" => strategy_wants_exit && can_exit,
            _ => strategy_wants_exit && drop_ratio >= exit_filter_threshold && can_exit,
        };

        // 停利不受 min_holding_days 限制（已超漲，應立即保護獲利）
        let should_exit = (signal_exit || hard_stop_triggered) && in_position;

        // ── 進場：冷卻期間封鎖 ────────────────────────────────────────────────
        if should_hold && !in_position && cooldown_remaining == 0 {
            entry_timestamp_ms = candles[i + 1].timestamp_ms;
            entry_idx = i;
            let capital_deployed = equity * actual_position_fraction;
            let buy_fee = capital_deployed * COMMISSION_RATE;
            position_units = capital_deployed / exec_close;
            position_cost = capital_deployed;
            entry_price = exec_close;
            equity -= buy_fee;
            in_position = true;
            holding_days = 0;
            days_above_boll_high = 0;
        } else if should_exit {
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

            // 決定出場原因
            let exit_reason = if hard_stop_triggered {
                "hard_stop"
            } else {
                "signal"
            };

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
                exit_reason: exit_reason.to_string(),
            });

            in_position = false;
            holding_days = 0;
            position_units = 0.0;
            position_cost = 0.0;
        }

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

        // ── 冷卻倒數（每天無條件遞減） ────────────────────────────────────────
        if cooldown_remaining > 0 {
            cooldown_remaining -= 1;
        }
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
            exit_reason: "forced".to_string(),
        });
    }

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
    let annual_return =
        compute_annualized_return(input.initial_capital, equity, strategy_daily_returns.len());

    Ok(BacktestOutput {
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
    })
}

// ── 私有：進出場訊號解析 ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn resolve_entry_signal(
    input: &BacktestInput,
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    closes: &[f64],
    boll_series: &[(f64, f64, f64)],
    rsi_series: &[f64],
    macd_series: &[MacdValue],
    candles: &[CandleRow],
    position_fraction: f64,
    i: usize,
) -> (bool, f64) {
    match input.strategy_name.as_str() {
        "trend_follow_v1" => trend_follow_entry(
            ma5_series,
            ma20_series,
            ma50_series,
            closes,
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
            let hold = should_hold_position(&input.strategy_name, candles, i);
            (hold, position_fraction)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_exit_signal(
    input: &BacktestInput,
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    rsi_series: &[f64],
    closes: &[f64],
    boll_series: &[(f64, f64, f64)],
    macd_series: &[MacdValue],
    candles: &[CandleRow],
    should_hold: bool,
    idx: usize,
    entry_idx: usize,
) -> bool {
    match input.strategy_name.as_str() {
        "trend_follow_v1" => trend_follow_should_exit(
            ma5_series,
            ma20_series,
            ma50_series,
            closes,
            rsi_series,
            idx,
            entry_idx,
        ),
        "mean_reversion_v1" => {
            let close = candles[idx].close;
            let (_, boll_middle, _) = boll_series[idx];
            mean_reversion_should_exit(close, ma50_series[idx], boll_middle)
        }
        "breakout_v1" => {
            let macd_curr = &macd_series[idx];
            if macd_curr.macd_line.is_nan() {
                false
            } else {
                let (boll_upper, _, _) = boll_series[idx];
                breakout_should_exit(candles[idx].close, boll_upper, macd_curr.histogram)
            }
        }
        _ => !should_hold,
    }
}

// ── 私有：統一交易統計更新 ────────────────────────────────────────────────────

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

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candles(closes: &[f64]) -> Vec<CandleRow> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| CandleRow {
                timestamp_ms: i as i64 * 86_400_000,
                close: c,
            })
            .collect()
    }

    fn make_request<'a>(candles: &'a Vec<CandleRow>, strategy: &str) -> BacktestInput<'a> {
        BacktestInput {
            candles: candles,
            strategy_name: strategy.to_string(),
            initial_capital: 100_000.0,
            position_size_percent: 100.0,
            exit_filter_pct: None,
            min_holding_days: None,
            take_profit_boll_pct: Some(0.05),
        }
    }

    #[test]
    fn test_run_returns_output_with_correct_initial_capital() {
        // 需要足夠多的 K 線讓指標有效
        let closes: Vec<f64> = (1..=60).map(|i| 100.0 + i as f64).collect();
        let candles = make_candles(&closes);
        let input = make_request(&candles, "trend_follow_v1");

        let output = run(&input).unwrap();
        // 初始資本合理範圍：不可能翻 10 倍
        assert!(output.final_capital > 0.0);
        assert!(output.final_capital < input.initial_capital * 10.0);
    }

    #[test]
    fn test_run_with_insufficient_data_returns_zero_trades() {
        let closes = vec![100.0, 101.0, 102.0];
        let candles = make_candles(&closes);
        let input = make_request(&candles, "trend_follow_v1");

        let output = run(&input).unwrap();
        // K 線不足，指標全 NaN，不應有任何交易
        assert_eq!(output.trades.len(), 0);
    }

    #[test]
    fn test_run_exit_filter_pct_negative_returns_error() {
        let closes: Vec<f64> = (1..=60).map(|i| 100.0 + i as f64).collect();
        let candles = make_candles(&closes);
        let mut input = make_request(&candles, "trend_follow_v1");
        input.exit_filter_pct = Some(-1.0);

        assert!(run(&input).is_err());
    }

    #[test]
    fn test_metrics_win_rate_between_0_and_1() {
        let closes: Vec<f64> = (1..=80)
            .map(|i| if i % 5 == 0 { 90.0 } else { 100.0 + i as f64 })
            .collect();
        let candles = make_candles(&closes);
        let input = make_request(&candles, "mean_reversion_v1");

        let output = run(&input).unwrap();
        assert!(output.metrics.win_rate >= 0.0 && output.metrics.win_rate <= 1.0);
    }
}
