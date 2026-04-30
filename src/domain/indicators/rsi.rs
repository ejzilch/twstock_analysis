use crate::constants;
use crate::domain::indicators::traits::IndicatorCalculator;
use crate::models::{Candle, IndicatorValue};
use std::collections::HashMap;

/// 相對強弱指數（Relative Strength Index）
///
/// 使用 Wilder 平滑法（Exponential Moving Average）計算，
/// 與 TradingView 和主流交易平台的計算方式一致。
/// 數值範圍 0.0 ~ 100.0，前 period 個位置回傳 NAN。
pub struct Rsi {
    /// 指標唯一 ID，格式為 "rsi{period}"，如 "rsi14"
    id: String,
    /// 計算週期，標準為 14
    period: usize,
}

impl Rsi {
    /// 建立新的 RSI 計算器
    ///
    /// # 參數
    /// - `period`: 計算週期，必須大於 1（至少需要計算一次漲跌幅）
    pub fn new(period: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(
            period > 1,
            "RSI period must be greater than 1, got {period}"
        );
        Ok(Self {
            id: format!("rsi{period}"),
            period,
        })
    }
}

impl IndicatorCalculator for Rsi {
    fn id(&self) -> &str {
        &self.id
    }

    fn compute(
        &self,
        candles: &[Candle],
        _computed: &HashMap<String, Vec<IndicatorValue>>,
    ) -> anyhow::Result<Vec<IndicatorValue>> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let result = compute_rsi(&closes, self.period);
        Ok(result.into_iter().map(IndicatorValue::Scalar).collect())
    }
}

/// RSI 純函數計算（Wilder 平滑法）
///
/// 計算流程：
/// 1. 計算每日漲跌幅（price change）
/// 2. 前 period 個漲跌幅取算術平均，作為初始 avg_gain / avg_loss
/// 3. 之後使用 Wilder EMA：avg = (prev_avg * (period-1) + current) / period
/// 4. RSI = 100 - (100 / (1 + avg_gain / avg_loss))
pub fn compute_rsi(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.len() <= period {
        return vec![f64::NAN; closes.len()];
    }

    let mut result = vec![f64::NAN; closes.len()];

    // 計算每日漲跌幅
    let changes: Vec<f64> = closes.windows(2).map(|w| w[1] - w[0]).collect();

    // 初始 avg_gain / avg_loss：前 period 個漲跌幅的算術平均
    let initial_gains: f64 =
        changes[..period].iter().filter(|&&c| c > 0.0).sum::<f64>() / period as f64;

    let initial_losses: f64 = changes[..period]
        .iter()
        .filter(|&&c| c < 0.0)
        .map(|c| c.abs())
        .sum::<f64>()
        / period as f64;

    let mut avg_gain = initial_gains;
    let mut avg_loss = initial_losses;

    // 第一個可計算位置
    result[period] = rsi_value(avg_gain, avg_loss);

    // Wilder EMA 平滑
    let smoothing = period as f64;
    for i in period..changes.len() {
        let gain = if changes[i] > 0.0 { changes[i] } else { 0.0 };
        let loss = if changes[i] < 0.0 {
            changes[i].abs()
        } else {
            0.0
        };

        avg_gain = (avg_gain * (smoothing - 1.0) + gain) / smoothing;
        avg_loss = (avg_loss * (smoothing - 1.0) + loss) / smoothing;

        result[i + 1] = rsi_value(avg_gain, avg_loss);
    }

    result
}

/// 由 avg_gain / avg_loss 計算 RSI 值
///
/// avg_loss 為 0 時（全部上漲）回傳 100.0。
#[inline]
fn rsi_value(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        return constants::RSI_MAX_VALUE;
    }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsi_insufficient_data_returns_nan() {
        let closes = vec![1.0, 2.0, 3.0];
        let result = compute_rsi(&closes, 14);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn test_rsi_all_gains_returns_100() {
        // 全部上漲時 RSI 應為 100
        let closes: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let result = compute_rsi(&closes, 14);
        let valid: Vec<f64> = result.into_iter().filter(|v| !v.is_nan()).collect();
        assert!(!valid.is_empty());
        assert!((valid[0] - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rsi_all_losses_returns_0() {
        // 全部下跌時 RSI 應為 0
        let closes: Vec<f64> = (1..=20).rev().map(|i| i as f64).collect();
        let result = compute_rsi(&closes, 14);
        let valid: Vec<f64> = result.into_iter().filter(|v| !v.is_nan()).collect();
        assert!(!valid.is_empty());
        assert!(valid[0].abs() < f64::EPSILON);
    }

    #[test]
    fn test_rsi_value_in_valid_range() {
        let closes = vec![
            44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.15, 43.61, 44.33, 44.83, 45.10,
            45.15, 43.61, 44.33, 44.83,
        ];
        let result = compute_rsi(&closes, 14);
        for value in result.iter().filter(|v| !v.is_nan()) {
            assert!(*value >= 0.0 && *value <= 100.0);
        }
    }

    #[test]
    fn test_rsi_result_length_matches_input() {
        let closes: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let result = compute_rsi(&closes, 14);
        assert_eq!(result.len(), closes.len());
    }

    #[test]
    fn test_rsi_new_period_one_returns_error() {
        assert!(Rsi::new(1).is_err());
    }
}
