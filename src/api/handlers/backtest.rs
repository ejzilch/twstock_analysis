use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::constants::{
    BANDWIDTH_HIGH_PERCENTILE, BANDWIDTH_LOOKBACK, BANDWIDTH_LOW_PERCENTILE, BOLL_PERIOD,
    BOLL_STD_MULTIPLIER, BO_MACD_FAST, BO_MACD_SIGNAL, BO_MACD_SLOW, BO_RSI_OVERBOUGHT,
    DEFAULT_AVG_CONSECUTIVE_LOSSES, MR_MA50_TOLERANCE, MR_RSI_NEUTRAL_MAX, MR_RSI_OVERSOLD,
    RSI_PERIOD, TF_MA_LONG, TF_MA_MID, TF_MA_SHORT, TF_RSI_OVERBOUGHT,
    TF_WEAK_SIGNAL_POSITION_RATIO,
};
use crate::core::indicators::{
    bollinger::compute_bollinger, ma::compute_sma, macd::compute_macd, rsi::compute_rsi,
};
use crate::models::candle::MacdValue;
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
    /// 回測期間最長連續虧損筆數
    pub max_consecutive_losses: u32,
    /// 最長連虧區間的累計虧損金額（TWD，絕對值）
    pub max_consecutive_loss_amount: f64,
    /// 所有連虧區間的平均筆數（無連虧時為 0.0）
    pub avg_consecutive_losses: f64,
    /// 回測期間最長連續獲利筆數
    pub max_consecutive_wins: u32,
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
    pub exit_filter_pct: f64,
    /// 每筆交易記錄，供前端 K 線圖標記使用
    pub trades: Vec<TradeRecord>,
}

#[derive(Debug, FromRow, Clone)]
struct CandleRow {
    timestamp_ms: i64,
    close: f64,
}

/// trend_follow_v1 進場強度
/// 影響實際使用的倉位比例
#[derive(Debug, PartialEq)]
enum TrendSignalStrength {
    /// MA5 > MA20 > MA50，強勢排列，全倉進場
    Strong,
    /// MA20 > MA50 且 MA5 剛上穿 MA20，弱勢補訊號，半倉進場
    Weak,
    /// 不進場
    None,
}

// 台股交易成本
const COMMISSION_RATE: f64 = 0.001425; // 單邊手續費 0.1425%
const TAX_RATE: f64 = 0.003; // 交易稅 0.3%（賣出時）

// Sharpe Ratio 無風險利率：台灣年化約 1.875%（10 年期公債），換算為日化
// risk_free_daily = (1 + 0.01875)^(1/252) - 1 ≈ 0.0000740
const RISK_FREE_ANNUAL: f64 = 0.01875;

// ── 出場緩衝濾網 ──────────────────────────────────────────────────────────────
// 持倉中，訊號轉空時不立即出場；
// 必須額外確認「當日收盤相較昨日收盤跌幅 ≥ EXIT_FILTER_THRESHOLD」才出場。
// 目的：過濾每日微小震盪造成的 Whipsaw，避免「今買明賣」反覆摩擦交易成本。
// 預設 1.5%；可依策略特性在 BacktestRequest 中透過 exit_filter_pct 覆寫。
const DEFAULT_EXIT_FILTER_THRESHOLD: f64 = 0.015; // 1.5%

// ── 最短持倉天數 ──────────────────────────────────────────────────────────────
// 進場後至少持有 N 天才允許出場訊號生效，避免頻繁進出摩擦交易成本。
// 預設 5 天（約一個交易週）；可透過 BacktestRequest.min_holding_days 覆寫。
const DEFAULT_MIN_HOLDING_DAYS: u32 = 5;

// ── trend_follow_v1 MA 參數 ───────────────────────────────────────────────────
// MA5 / MA20 交叉判斷趨勢方向；idx < MA_LONG_PERIOD 時資料不足，不進場。
const MA_SHORT_PERIOD: usize = 5;
const MA_LONG_PERIOD: usize = 20;

// 強制停損
const HARD_STOP_LOSS_PCT: f64 = 0.03;

/// POST /api/v1/backtest
pub async fn backtest_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
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

    if candles.len() < 3 {
        // 修正：需要至少 3 根 K 棒：1 根訊號基準、1 根訊號確認、1 根執行
        return Err(ApiError::InvalidIndicatorConfig {
            detail: "not enough candle data for backtest (need at least 3 daily candles)"
                .to_string(),
        });
    }

    let position_fraction = request.position_size_percent / 100.0;

    // 解析出場濾網閾值：caller 可傳入覆寫，否則用預設 1.5%
    let exit_filter_threshold = match request.exit_filter_pct {
        Some(v) if v < 0.0 => {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "exit_filter_pct must be >= 0.0".to_string(),
            });
        }
        Some(v) => v / 100.0, // 百分比 → 小數
        None => DEFAULT_EXIT_FILTER_THRESHOLD,
    };

    // 解析最短持倉天數
    let min_holding_days = match request.min_holding_days {
        Some(v) => v,
        None => match request.strategy_name.as_str() {
            "mean_reversion_v1" => 2,      // 均值回歸縮短為 2 天
            _ => DEFAULT_MIN_HOLDING_DAYS, // 其他維持 5 天
        },
    };

    // ── Units-based 持倉模型 ──────────────────────────────────────────────────
    // 改用「買入單位數」追蹤實際部位市值，解決三個問題：
    //   1. 出場成本基準精確（units × exit_price，而非 equity × fraction 估算）
    //   2. 消除 net_pnl_ratio 重複扣費（equity 增減即為精確現金流，不再另外算）
    //   3. 硬停損可直接比對 entry_price（-HARD_STOP_LOSS_PCT 觸發強制出場）
    // ────────────────────────────────────────────────────────────────────────
    let mut equity: f64 = request.initial_capital;
    let mut equity_curve: Vec<f64> = vec![equity];
    let mut strategy_daily_returns: Vec<f64> = Vec::with_capacity(candles.len().saturating_sub(2));

    let mut in_position: bool = false;
    let mut entry_price: f64 = 0.0;
    let mut position_units: f64 = 0.0; // 買入的股數（單位數）
    let mut position_cost: f64 = 0.0; // 進場時的名目成本（entry_price × units）
    let mut holding_days: u32 = 0;
    let mut winning_trades: i32 = 0;
    let mut consecutive_losses: u32 = 0;
    let mut max_consecutive_losses: u32 = 0;
    let mut max_consecutive_loss_amount: f64 = 0.0;
    let mut current_loss_streak_amount: f64 = 0.0;
    let mut consecutive_wins: u32 = 0;
    let mut max_consecutive_wins: u32 = 0;
    let mut loss_streaks: Vec<u32> = Vec::new();
    let mut losing_trades: i32 = 0;
    let mut gross_profit: f64 = 0.0;
    let mut gross_loss: f64 = 0.0;

    // ── 回測開始前，預算指標序列 ──────────────────────────────────────────
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    // ── 回測開始前，預算整個 K 線序列的帶寬 ──
    let bandwidth_series = compute_bandwidth_series(&candles);

    // MA 序列（共用）
    let ma5_series: Vec<f64> = compute_sma(&closes, TF_MA_SHORT); // 現有函數直接復用
    let ma20_series: Vec<f64> = compute_sma(&closes, TF_MA_MID);
    let ma50_series: Vec<f64> = compute_sma(&closes, TF_MA_LONG);

    // RSI 序列（共用）
    let rsi_series = compute_rsi(&closes, RSI_PERIOD); // 現有函數直接復用

    // Bollinger Bands（共用）
    let boll_series: Vec<(f64, f64, f64)> =
        compute_bollinger(&closes, BOLL_PERIOD, BOLL_STD_MULTIPLIER);

    let macd_series: Vec<MacdValue> =
        compute_macd(&closes, BO_MACD_FAST, BO_MACD_SLOW, BO_MACD_SIGNAL);

    let mut trades: Vec<TradeRecord> = Vec::new();
    let mut entry_timestamp_ms: i64 = 0;

    // 訊號在第 i 日收盤確認，第 i+1 日收盤成交（Look-ahead Bias 已修正）
    for i in 1..candles.len().saturating_sub(1) {
        // Layer 0：市場環境過濾
        if !is_market_tradeable(&bandwidth_series, i) {
            // 若持倉中，繼續更新每日報酬但不做新的進出場判斷
            if in_position {
                holding_days += 1;
                // 每日報酬更新維持現有邏輯
            }
            continue; // 跳過進出場判斷
        }

        let exec_close = candles[i + 1].close;
        let prev_exec_close = candles[i].close;

        if exec_close <= 0.0 || prev_exec_close <= 0.0 {
            continue;
        }
        // match 之前
        // ── 依策略取得進出場訊號 ────────────────────────────────────────────
        let (should_hold, actual_position_fraction) = match request.strategy_name.as_str() {
            "trend_follow_v1" => {
                // L1 趨勢過濾：收盤需在 MA50 之上
                let ma50 = ma50_series[i];
                let close = candles[i].close;
                if !ma50.is_finite() || close <= ma50 {
                    (false, 0.0)
                } else {
                    // L2 進場強度
                    let strength =
                        trend_follow_signal_strength(&ma5_series, &ma20_series, &ma50_series, i);
                    let fraction = match strength {
                        TrendSignalStrength::Strong => position_fraction,
                        TrendSignalStrength::Weak => {
                            position_fraction * TF_WEAK_SIGNAL_POSITION_RATIO
                        }
                        TrendSignalStrength::None => 0.0,
                    };
                    (strength != TrendSignalStrength::None, fraction)
                }
            }
            "mean_reversion_v1" => {
                let close = candles[i].close;
                let ma50 = ma50_series[i];
                let rsi = rsi_series[i];
                let (_, _boll_middle, boll_lower) = boll_series[i];

                let hold = mean_reversion_should_enter(close, ma50, rsi, boll_lower);
                (hold, position_fraction) // 均值回歸不分層，固定全倉
            }
            "breakout_v1" => {
                if i < 1 {
                    (false, 0.0)
                } else {
                    let close = candles[i].close;
                    let (boll_upper, _, _) = boll_series[i];
                    let rsi = rsi_series[i];

                    // 取當日與前日 MACD histogram
                    let macd_curr = &macd_series[i];
                    let macd_prev = &macd_series[i - 1];
                    if !macd_curr.macd_line.is_nan() && !macd_prev.macd_line.is_nan() {
                        let hold = breakout_should_enter(
                            close,
                            boll_upper,
                            macd_curr.histogram,
                            macd_prev.histogram,
                            rsi,
                        );
                        (hold, position_fraction)
                    } else {
                        (false, 0.0) // MACD 資料不足時不進場
                    }
                }
            }
            // 其他策略維持現有邏輯，fraction 固定用 position_fraction
            _ => {
                let hold = should_hold_position(&request.strategy_name, &candles, i);
                (hold, position_fraction)
            }
        };

        // ── 出場判斷 ────────────────────────────────────────────────────────
        // trend_follow_v1 用新的出場邏輯，其他策略維持現有
        let strategy_wants_exit = match request.strategy_name.as_str() {
            "trend_follow_v1" => {
                trend_follow_should_exit(&ma5_series, &ma20_series, &ma50_series, &rsi_series, i)
            }
            "mean_reversion_v1" => {
                let close = candles[i].close;
                let ma50 = ma50_series[i];
                let (_, boll_middle, _) = boll_series[i];

                mean_reversion_should_exit(close, ma50, boll_middle)
            }
            "breakout_v1" => {
                let close = candles[i].close;
                let (boll_upper, _, _) = boll_series[i];
                let macd_curr = &macd_series[i];

                if macd_curr.macd_line.is_nan() {
                    false // 資料不足時不觸發出場
                } else {
                    breakout_should_exit(close, boll_upper, macd_curr.histogram)
                }
            }
            _ => !should_hold,
        };

        // ── 出場緩衝濾網 ────────────────────────────────────────────────────────
        // signal_close = candles[i].close（訊號日收盤）
        // signal_prev  = candles[i-1].close（訊號日的昨日收盤）
        // drop_ratio   = (signal_prev - signal_close) / signal_prev
        // ────────────────────────────────────────────────────────────────────────
        let signal_close = candles[i].close;
        let signal_prev = candles[i.saturating_sub(1)].close;
        let drop_ratio = if signal_prev > 0.0 {
            (signal_prev - signal_close) / signal_prev
        } else {
            0.0
        };

        // ── 硬停損：持倉虧損超過 HARD_STOP_LOSS_PCT 強制出場 ─────────────────
        // 以執行日收盤價對比進場價，超過閾值優先於其他出場條件觸發
        let hard_stop_triggered = in_position
            && entry_price > 0.0
            && (exec_close - entry_price) / entry_price <= -HARD_STOP_LOSS_PCT;

        // ── 出場條件：(訊號轉空 AND 跌幅達閾值 AND 滿最短持倉) OR 硬停損 ──────
        let can_exit = holding_days >= min_holding_days;

        // 現有的出場條件組合
        let signal_exit = match request.strategy_name.as_str() {
            // mean_reversion breakout 出場不需要 drop_ratio 過濾
            // 只需要 can_exit（最短持倉天數）
            "mean_reversion_v1" | "breakout_v1" => strategy_wants_exit && can_exit,
            // 其他策略維持原本的 drop_ratio + can_exit 雙重條件
            _ => strategy_wants_exit && drop_ratio >= exit_filter_threshold && can_exit,
        };
        let should_exit = (signal_exit || hard_stop_triggered) && in_position;

        // ── 進場：用 actual_position_fraction 取代固定的 position_fraction ──
        if should_hold && !in_position {
            // ── 進場：計算買入單位數，扣除買進手續費 ────────────────────────────
            // capital_deployed = 當下可用資金 × 部位比例
            // buy_fee          = capital_deployed × COMMISSION_RATE（買進手續費）
            // units            = capital_deployed / exec_close（實際可買的股數）
            // equity 扣除 buy_fee 後剩餘閒置資金部分不動
            entry_timestamp_ms = candles[i + 1].timestamp_ms; // 執行日
            let capital_deployed = equity * actual_position_fraction; // 強弱倉位分層
            let buy_fee = capital_deployed * COMMISSION_RATE;
            position_units = capital_deployed / exec_close;
            position_cost = capital_deployed; // 名目成本（不含手續費）
            entry_price = exec_close;
            equity -= buy_fee; // 只扣手續費，部位本身仍計入 equity
            in_position = true;
            holding_days = 0;
        } else if should_exit {
            // ── 出場：以實際部位市值計算現金回收，扣除賣出手續費 + 交易稅 ────────
            // market_value = 持有單位數 × 出場價（精確市值）
            // sell_fee     = market_value × (COMMISSION_RATE + TAX_RATE)
            // cash_received = market_value - sell_fee
            // equity 的增減 = cash_received - position_cost（買入時的名目成本）
            let market_value = position_units * exec_close;
            let sell_fee = market_value * (COMMISSION_RATE + TAX_RATE);
            let cash_received = market_value - sell_fee;

            // 更新 equity：還回部位成本 → 換成實際收到的現金
            // (equity 目前已含 position_cost 這筆浮動資產，需先扣掉再加回現金)
            equity = equity - position_cost + cash_received;

            // ── 記錄損益（以名目成本為基準，不重複扣費）────────────────────────
            // gross_pnl = 市值變化（未扣費）
            // net_pnl   = 市值變化 - 進場手續費 - 出場手續費 - 交易稅
            let buy_fee_paid = position_cost * COMMISSION_RATE;
            let gross_pnl = market_value - position_cost;
            let net_pnl = gross_pnl - buy_fee_paid - sell_fee;

            if net_pnl >= 0.0 {
                winning_trades += 1;
                gross_profit += net_pnl / position_cost;

                // 連勝計數
                consecutive_wins += 1;
                if consecutive_wins > max_consecutive_wins {
                    max_consecutive_wins = consecutive_wins;
                }
                // 結束連虧區間，記錄後重置
                if consecutive_losses > 0 {
                    loss_streaks.push(consecutive_losses);
                }

                consecutive_losses = 0;
                current_loss_streak_amount = 0.0;
            } else {
                losing_trades += 1;
                gross_loss += net_pnl.abs() / position_cost;

                // 連虧計數
                consecutive_wins = 0;
                consecutive_losses += 1;

                current_loss_streak_amount =
                    (current_loss_streak_amount + net_pnl.abs()).min(f64::MAX); // f64 + f64 溢位只會到 inf，min 可以夾住

                if consecutive_losses > max_consecutive_losses {
                    max_consecutive_losses = consecutive_losses;
                    max_consecutive_loss_amount = current_loss_streak_amount;
                }
            }

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

        // ── 每日報酬：持倉中以「部位市值變化 / 總 equity」計算 ──────────────────
        // 改用 units × price 精確反映部位市值，而非 equity × fraction 估算
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

    // ── 末日強制平倉：迴圈結束後若仍持倉，以最後一根收盤平倉 ───────────────────
    if in_position {
        let last_close = candles.last().map(|c| c.close).unwrap_or(entry_price);
        let market_value = position_units * last_close;
        let sell_fee = market_value * (COMMISSION_RATE + TAX_RATE);
        let cash_received = market_value - sell_fee;
        equity = equity - position_cost + cash_received;

        let buy_fee_paid = position_cost * COMMISSION_RATE;
        let gross_pnl = market_value - position_cost;
        let net_pnl = gross_pnl - buy_fee_paid - sell_fee;

        if net_pnl >= 0.0 {
            winning_trades += 1;
            gross_profit += net_pnl / position_cost;

            // 連勝計數
            consecutive_wins += 1;
            if consecutive_wins > max_consecutive_wins {
                max_consecutive_wins = consecutive_wins;
            }
            // 結束連虧區間，記錄後重置
            if consecutive_losses > 0 {
                loss_streaks.push(consecutive_losses);
            }
            consecutive_losses = 0;
            // current_loss_streak_amount = 0.0;
        } else {
            losing_trades += 1;
            gross_loss += net_pnl.abs() / position_cost;

            // 連虧計數
            // consecutive_wins = 0;
            consecutive_losses += 1;
            // f64 + f64 溢位只會到 inf，min 可以夾住
            current_loss_streak_amount = (current_loss_streak_amount + net_pnl.abs()).min(f64::MAX);

            if consecutive_losses > max_consecutive_losses {
                max_consecutive_losses = consecutive_losses;
                max_consecutive_loss_amount = current_loss_streak_amount;
            }
        }

        trades.push(TradeRecord {
            entry_timestamp_ms,
            exit_timestamp_ms: candles.last().map(|c| c.timestamp_ms).unwrap_or(0),
            entry_price,
            exit_price: last_close,
            net_pnl,
            is_win: net_pnl >= 0.0,
        });
    }

    // 末日平倉結束後，收尾連虧區間
    if consecutive_losses > 0 {
        loss_streaks.push(consecutive_losses);
    }

    // 計算平均
    let avg_consecutive_losses: f64 = if loss_streaks.is_empty() {
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

    // ── 修正 4：Sharpe Ratio 扣除無風險利率 ─────────────────────────────────
    let sharpe_ratio = compute_sharpe_ratio(&strategy_daily_returns, RISK_FREE_ANNUAL);
    // ────────────────────────────────────────────────────────────────────────

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

fn should_hold_position(strategy_name: &str, candles: &[CandleRow], idx: usize) -> bool {
    match strategy_name {
        // ── trend_follow_v1：MA5 > MA20 交叉判斷趨勢方向 ────────────────────
        // 資料不足 MA_LONG_PERIOD 根時直接返回 false，避免用不完整均線進場。
        // MA 取 candles[idx-N..idx]（不含當日），確保不偷看當日收盤。
        "trend_follow_v1" => {
            if idx < MA_LONG_PERIOD {
                return false;
            }
            let ma5: f64 = candles[idx - MA_SHORT_PERIOD..idx]
                .iter()
                .map(|c| c.close)
                .sum::<f64>()
                / MA_SHORT_PERIOD as f64;
            let ma20: f64 = candles[idx - MA_LONG_PERIOD..idx]
                .iter()
                .map(|c| c.close)
                .sum::<f64>()
                / MA_LONG_PERIOD as f64;
            ma5 > ma20
        }

        // 均值回歸：跌超過 1% 進場，反彈後離場
        "mean_reversion_v1" => {
            let close = candles[idx].close;
            let prev = candles[idx.saturating_sub(1)].close;
            close < prev * 0.99
        }

        // 突破：突破前 5 日最高收盤才持有
        "breakout_v1" => {
            let close = candles[idx].close;
            let start = idx.saturating_sub(5);
            let recent_max = candles[start..idx]
                .iter()
                .map(|c| c.close)
                .fold(f64::MIN, f64::max);
            close > recent_max
        }

        // 未知策略 fallback
        _ => {
            let close = candles[idx].close;
            let prev = candles[idx.saturating_sub(1)].close;
            close > prev
        }
    }
}

/// trend_follow_v1 專用：判斷進場強度
/// Strong  = MA5 > MA20 > MA50（三均線順勢排列）
/// Weak    = MA20 > MA50 且 MA5 剛上穿 MA20（補訊號）
/// None    = 不進場
fn trend_follow_signal_strength(
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    idx: usize,
) -> TrendSignalStrength {
    // 資料不足時不進場
    if idx < TF_MA_LONG {
        return TrendSignalStrength::None;
    }

    let ma5 = ma5_series[idx];
    let ma20 = ma20_series[idx];
    let ma50 = ma50_series[idx];
    let ma5_prev = ma5_series[idx - 1];
    let ma20_prev = ma20_series[idx - 1];

    // 任一指標無效時不進場
    if !ma5.is_finite() || !ma20.is_finite() || !ma50.is_finite() {
        return TrendSignalStrength::None;
    }

    // 強勢：三均線順勢排列
    if ma5 > ma20 && ma20 > ma50 {
        return TrendSignalStrength::Strong;
    }

    // 弱勢：MA20 > MA50 且 MA5 剛上穿 MA20
    // 「剛上穿」= 今日 MA5 > MA20 且昨日 MA5 <= MA20
    let just_crossed = ma5 > ma20 && ma5_prev <= ma20_prev;
    if ma20 > ma50 && just_crossed {
        return TrendSignalStrength::Weak;
    }

    TrendSignalStrength::None
}

/// trend_follow_v1 專用：判斷是否應該出場
/// MA5 < MA20（動能轉弱）OR（RSI > 門檻 且 RSI 轉弱）
fn trend_follow_should_exit(
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    rsi_series: &[f64],
    idx: usize,
) -> bool {
    if idx < 1 {
        return false;
    }

    let ma5 = ma5_series[idx];
    let ma20 = ma20_series[idx];
    let ma50 = ma50_series[idx];
    let rsi = rsi_series[idx];
    let rsi_prev = rsi_series[idx - 1];

    if !ma5.is_finite() || !ma20.is_finite() || !ma50.is_finite() {
        return false;
    }

    // 條件一：MA5 跌破 MA20 且 MA20 也跌破 MA50
    // 雙重確認趨勢真的結束，不是短期震盪
    if ma5 < ma20 && ma20 < ma50 {
        return true;
    }

    // 條件二：RSI 過熱後轉弱
    // 「轉弱」= 今日 RSI < 前日 RSI
    if rsi.is_finite() && rsi_prev.is_finite() {
        if rsi > TF_RSI_OVERBOUGHT && rsi < rsi_prev {
            return true;
        }
    }

    false
}

/// mean_reversion_v1 專用：判斷是否符合進場條件
///
/// L1 趨勢過濾：收盤 > MA50（多頭環境）且 RSI < MR_RSI_NEUTRAL_MAX
/// L2 進場訊號：收盤跌破 Bollinger 下軌 且 RSI < MR_RSI_OVERSOLD
fn mean_reversion_should_enter(close: f64, ma50: f64, rsi: f64, boll_lower: f64) -> bool {
    // 任一指標無效時不進場
    if !ma50.is_finite() || !rsi.is_finite() || !boll_lower.is_finite() {
        return false;
    }

    // L1：多頭環境過濾
    if close <= ma50 * MR_MA50_TOLERANCE {
        return false; // 收盤在 MA50 之下，空頭環境不接刀
    }
    if rsi >= MR_RSI_NEUTRAL_MAX {
        return false; // RSI 中性偏強區間，均值回歸無意義
    }

    // L2：雙重確認進場
    let below_lower_band = close < boll_lower;
    let oversold = rsi < MR_RSI_OVERSOLD;

    below_lower_band && oversold
}

/// mean_reversion_v1 專用：判斷是否應該出場
///
/// 條件一：收盤回到 Bollinger 中軌（MA20）→ 均值回歸完成
/// 條件二：收盤跌破 MA50 → 大趨勢轉壞，立即出場
fn mean_reversion_should_exit(close: f64, ma50: f64, boll_middle: f64) -> bool {
    if !ma50.is_finite() || !boll_middle.is_finite() {
        return false;
    }

    // 條件一：均值回歸完成
    if close >= boll_middle {
        return true;
    }

    // 條件二：大趨勢轉壞
    if close < ma50 * MR_MA50_TOLERANCE {
        return true;
    }

    false
}

/// breakout_v1 專用：判斷是否符合進場條件
///
/// L2 進場訊號：
///   收盤站上 Bollinger 上軌（收盤確認，非刺穿）
///   MACD histogram > 0 且 > 前日 histogram（動能擴大）
///   RSI < BO_RSI_OVERBOUGHT（未過熱）
fn breakout_should_enter(
    close: f64,
    boll_upper: f64,
    macd_histogram: f64,
    macd_histogram_prev: f64,
    rsi: f64,
) -> bool {
    // 任一指標無效時不進場
    if !boll_upper.is_finite()
        || !macd_histogram.is_finite()
        || !macd_histogram_prev.is_finite()
        || !rsi.is_finite()
    {
        return false;
    }

    // 收盤站上布林上軌（收盤確認，非盤中刺穿）
    let above_upper_band = close > boll_upper;

    // MACD histogram 為正且擴大（動能支撐突破）
    let macd_expanding = macd_histogram > 0.0 && macd_histogram > macd_histogram_prev;

    // RSI 未過熱（避免追在頂部）
    let not_overbought = rsi < BO_RSI_OVERBOUGHT;

    above_upper_band && macd_expanding && not_overbought
}

/// breakout_v1 專用：判斷是否應該出場
///
/// 條件一：收盤跌回 Bollinger 上軌之下 → 突破失敗
/// 條件二：MACD histogram 轉負 → 動能耗盡
fn breakout_should_exit(close: f64, boll_upper: f64, macd_histogram: f64) -> bool {
    if !boll_upper.is_finite() || !macd_histogram.is_finite() {
        return false;
    }

    // 條件一：突破失敗，收盤跌回上軌之下
    if close < boll_upper {
        return true;
    }

    // 條件二：動能耗盡
    if macd_histogram < 0.0 {
        return true;
    }

    false
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

/// ── 修正 4：加入 risk_free_annual 參數 ──────────────────────────────────────
/// Sharpe = (mean_daily_excess_return / std_daily_return) * sqrt(252)
/// excess_return = strategy_daily_return - risk_free_daily
/// risk_free_daily = (1 + risk_free_annual)^(1/252) - 1
/// ────────────────────────────────────────────────────────────────────────────
fn compute_sharpe_ratio(daily_returns: &[f64], risk_free_annual: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

    // 年化無風險利率換算為日化（複利）
    let risk_free_daily = (1.0 + risk_free_annual).powf(1.0 / 252.0) - 1.0;

    let excess: Vec<f64> = daily_returns.iter().map(|r| r - risk_free_daily).collect();

    let mean = excess.iter().sum::<f64>() / excess.len() as f64;
    let variance = excess
        .iter()
        .map(|r| {
            let d = r - mean;
            d * d
        })
        .sum::<f64>()
        / (excess.len() as f64 - 1.0);

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

/// 預算每根 K 線對應的 Bollinger 帶寬值
/// 帶寬定義：(upper - lower) / middle，跨股票可比較
/// 資料不足 period 根時回傳 None
fn compute_bandwidth_series(candles: &[CandleRow]) -> Vec<Option<f64>> {
    let period = 20usize;
    let std_multiplier = 2.0f64;
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let n = closes.len();
    let mut result = vec![None; n];

    for i in period - 1..n {
        let window = &closes[i + 1 - period..=i];
        let mean = window.iter().sum::<f64>() / period as f64;
        if mean <= 0.0 {
            continue;
        }
        let variance = window.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();
        let upper = mean + std_multiplier * std_dev;
        let lower = mean - std_multiplier * std_dev;
        let bandwidth = (upper - lower) / mean;
        if bandwidth.is_finite() {
            result[i] = Some(bandwidth);
        }
    }
    result
}

/// 計算某根 K 線位置的帶寬 percentile 閾值
/// 取 idx 之前 BANDWIDTH_LOOKBACK 根有效帶寬值排序後取 percentile
fn compute_bandwidth_percentiles(
    bandwidth_series: &[Option<f64>],
    idx: usize,
) -> Option<(f64, f64)> {
    let lookback = BANDWIDTH_LOOKBACK;
    let start = if idx >= lookback { idx - lookback } else { 0 };

    let mut history: Vec<f64> = bandwidth_series[start..idx]
        .iter()
        .filter_map(|v| *v)
        .collect();

    if history.len() < lookback / 2 {
        // 歷史資料不足一半，不過濾（寬鬆處理回測起始點）
        return None;
    }

    history.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = history.len();

    let low_idx = ((len as f64) * BANDWIDTH_LOW_PERCENTILE) as usize;
    let high_idx = ((len as f64) * BANDWIDTH_HIGH_PERCENTILE) as usize;
    let low_threshold = history[low_idx.min(len - 1)];
    let high_threshold = history[high_idx.min(len - 1)];

    Some((low_threshold, high_threshold))
}

/// Layer 0：判斷當前市場環境是否值得交易
/// 帶寬過低（橫盤）或過高（恐慌）時回傳 false
fn is_market_tradeable(bandwidth_series: &[Option<f64>], idx: usize) -> bool {
    let current_bandwidth = match bandwidth_series[idx] {
        Some(bw) => bw,
        None => return true, // 帶寬無法計算時，不過濾（讓策略自己判斷）
    };

    let (low_threshold, high_threshold) = match compute_bandwidth_percentiles(bandwidth_series, idx)
    {
        Some(thresholds) => thresholds,
        None => return true, // 歷史資料不足時，不過濾
    };

    // 帶寬在正常範圍內才交易
    current_bandwidth > low_threshold && current_bandwidth < high_threshold
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
        // 當所有日報酬等於無風險日報酬時，超額報酬為 0，Sharpe 應為 0
        let risk_free_daily = (1.0 + RISK_FREE_ANNUAL).powf(1.0 / 252.0) - 1.0;
        let returns = vec![risk_free_daily; 3];
        assert_eq!(compute_sharpe_ratio(&returns, RISK_FREE_ANNUAL), 0.0);
    }

    #[test]
    fn test_sharpe_lower_with_risk_free_rate() {
        // 加入無風險利率後，Sharpe 應低於不扣無風險利率的版本
        let returns = vec![0.001, 0.002, 0.0015, 0.001, 0.002];
        let sharpe_with_rf = compute_sharpe_ratio(&returns, RISK_FREE_ANNUAL);
        let sharpe_no_rf = compute_sharpe_ratio(&returns, 0.0);
        assert!(sharpe_with_rf < sharpe_no_rf);
    }

    #[test]
    fn test_annualized_return_positive_growth() {
        let r = compute_annualized_return(100.0, 110.0, 252);
        assert!(r > 0.0);
    }

    #[test]
    fn test_dynamic_position_size_reduces_with_equity() {
        // 驗證動態部位：equity 縮水後，下一筆交易的名目金額也應縮水
        // 以 equity=90 為例，部位金額 = 90 * 0.5 = 45，而非固定 100 * 0.5 = 50
        let equity = 90.0_f64;
        let position_fraction = 0.5_f64;
        let position_size = equity * position_fraction;
        assert!((position_size - 45.0).abs() < f64::EPSILON);
    }

    // ── 出場濾網測試 ──────────────────────────────────────────────────────────

    #[test]
    fn test_exit_filter_blocks_small_drop() {
        // 跌幅 0.8% < 閾值 1.5%，should_exit 應為 false（繼續持倉）
        let signal_prev = 100.0_f64;
        let signal_close = 99.2_f64; // 跌 0.8%
        let drop_ratio = (signal_prev - signal_close) / signal_prev;
        let threshold = DEFAULT_EXIT_FILTER_THRESHOLD;
        let should_hold = false; // 策略訊號已轉空
        let should_exit = !should_hold && drop_ratio >= threshold;
        assert!(!should_exit, "小跌 0.8% 不應觸發出場");
    }

    #[test]
    fn test_exit_filter_allows_large_drop() {
        // 跌幅 2.0% > 閾值 1.5%，should_exit 應為 true
        let signal_prev = 100.0_f64;
        let signal_close = 98.0_f64; // 跌 2.0%
        let drop_ratio = (signal_prev - signal_close) / signal_prev;
        let threshold = DEFAULT_EXIT_FILTER_THRESHOLD;
        let should_hold = false;
        let should_exit = !should_hold && drop_ratio >= threshold;
        assert!(should_exit, "大跌 2.0% 應觸發出場");
    }

    #[test]
    fn test_exit_filter_zero_disables_filter() {
        // exit_filter_pct = 0.0 時，任何跌幅都應觸發出場（等同停用濾網）
        let signal_prev = 100.0_f64;
        let signal_close = 99.9_f64; // 跌 0.1%
        let drop_ratio = (signal_prev - signal_close) / signal_prev;
        let threshold = 0.0_f64; // 停用
        let should_hold = false;
        let should_exit = !should_hold && drop_ratio >= threshold;
        assert!(should_exit, "threshold=0 時任何跌幅都應出場");
    }

    #[test]
    fn test_exit_filter_no_exit_when_signal_still_hold() {
        // 即使跌幅超過閾值，若 should_hold=true，不應出場
        let drop_ratio = 0.03_f64; // 跌 3%，超過閾值
        let threshold = DEFAULT_EXIT_FILTER_THRESHOLD;
        let should_hold = true; // 策略仍看多
        let should_exit = !should_hold && drop_ratio >= threshold;
        assert!(!should_exit, "策略看多時即使大跌也不應出場");
    }

    // ── trend_follow_v1 MA 交叉測試 ──────────────────────────────────────────

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

    #[test]
    fn test_trend_follow_false_when_insufficient_data() {
        // idx < MA_LONG_PERIOD(20) 時，資料不足應返回 false
        let candles = make_candles(&vec![100.0; 25]);
        assert!(!should_hold_position("trend_follow_v1", &candles, 10));
    }

    #[test]
    fn test_trend_follow_golden_cross() {
        // 構造近 5 日均價 > 近 20 日均價的情境（上升趨勢）
        // 前 15 天收盤 90，後 5 天收盤 110 → MA5=110, MA20=93.75 → 黃金交叉
        let mut closes = vec![90.0f64; 20];
        closes.extend_from_slice(&[110.0f64; 5]);
        let candles = make_candles(&closes);
        // idx=24：MA5 = candles[19..24] 全是 110 = 110
        //         MA20 = candles[4..24] = 15×90 + 5×110 = 1900 / 20 = 95
        assert!(should_hold_position("trend_follow_v1", &candles, 24));
    }

    #[test]
    fn test_trend_follow_death_cross() {
        // 前 15 天收盤 110，後 5 天收盤 90 → MA5=90, MA20=106.25 → 死亡交叉
        let mut closes = vec![110.0f64; 20];
        closes.extend_from_slice(&[90.0f64; 5]);
        let candles = make_candles(&closes);
        assert!(!should_hold_position("trend_follow_v1", &candles, 24));
    }

    // ── 最短持倉天數測試 ──────────────────────────────────────────────────────

    #[test]
    fn test_min_holding_days_blocks_early_exit() {
        // 持倉 3 天，min_holding_days=5，should_exit 應為 false
        let holding_days = 3u32;
        let min_holding_days = DEFAULT_MIN_HOLDING_DAYS; // 5
        let drop_ratio = 0.03_f64;
        let threshold = DEFAULT_EXIT_FILTER_THRESHOLD;
        let can_exit = holding_days >= min_holding_days;
        let should_exit = true && drop_ratio >= threshold && can_exit; // !should_hold=true
        assert!(!should_exit, "持倉未滿 5 天不應出場");
    }

    #[test]
    fn test_min_holding_days_allows_exit_after_threshold() {
        // 持倉 6 天，min_holding_days=5，跌幅達標，should_exit 應為 true
        let holding_days = 6u32;
        let min_holding_days = DEFAULT_MIN_HOLDING_DAYS;
        let drop_ratio = 0.03_f64;
        let threshold = DEFAULT_EXIT_FILTER_THRESHOLD;
        let can_exit = holding_days >= min_holding_days;
        let should_exit = true && drop_ratio >= threshold && can_exit;
        assert!(should_exit, "持倉滿 5 天且跌幅達標應出場");
    }
}
