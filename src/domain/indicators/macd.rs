use crate::domain::indicators::traits::IndicatorCalculator;
use crate::models::{candle::MacdValue, Candle, IndicatorValue};
use std::collections::HashMap;

/// MACD 指標（Moving Average Convergence Divergence）
///
/// 標準參數：fast=12, slow=26, signal=9。
/// 計算流程：
/// 1. fast_ema = EMA(close, fast_period)
/// 2. slow_ema = EMA(close, slow_period)
/// 3. macd_line = fast_ema - slow_ema
/// 4. signal_line = EMA(macd_line, signal_period)
/// 5. histogram = macd_line - signal_line
pub struct Macd {
    /// 指標唯一 ID，固定為 "macd"
    id: String,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
}

impl Macd {
    /// 建立新的 MACD 計算器
    ///
    /// # 參數
    /// - `fast_period`:   快線 EMA 週期，標準值 12
    /// - `slow_period`:   慢線 EMA 週期，標準值 26，必須大於 fast_period
    /// - `signal_period`: 信號線 EMA 週期，標準值 9
    pub fn new(
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            fast_period > 0 && slow_period > 0 && signal_period > 0,
            "MACD periods must all be greater than 0"
        );
        anyhow::ensure!(
            slow_period > fast_period,
            "MACD slow_period ({slow_period}) must be greater than fast_period ({fast_period})"
        );
        Ok(Self {
            id: "macd".to_string(),
            fast_period,
            slow_period,
            signal_period,
        })
    }
}

impl IndicatorCalculator for Macd {
    fn id(&self) -> &str {
        &self.id
    }

    fn compute(
        &self,
        candles: &[Candle],
        _computed: &HashMap<String, Vec<IndicatorValue>>,
    ) -> anyhow::Result<Vec<IndicatorValue>> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let result = compute_macd(
            &closes,
            self.fast_period,
            self.slow_period,
            self.signal_period,
        );
        Ok(result.into_iter().map(IndicatorValue::Macd).collect())
    }
}

/// MACD 純函數計算
///
/// 回傳與輸入等長的 MacdValue 陣列。
/// 資料不足的位置三個欄位均為 NAN。
pub fn compute_macd(
    closes: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> Vec<MacdValue> {
    let nan_value = MacdValue {
        macd_line: f64::NAN,
        signal_line: f64::NAN,
        histogram: f64::NAN,
    };

    if closes.len() < slow_period + signal_period {
        return vec![nan_value; closes.len()];
    }

    let fast_ema = compute_ema(closes, fast_period);
    let slow_ema = compute_ema(closes, slow_period);

    // MACD line = fast EMA - slow EMA
    let macd_line: Vec<f64> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(f, s)| {
            if f.is_nan() || s.is_nan() {
                f64::NAN
            } else {
                f - s
            }
        })
        .collect();

    // Signal line = EMA(macd_line, signal_period)，跳過 NAN 值
    let signal_line = compute_ema_skip_nan(&macd_line, signal_period);

    // Histogram = macd_line - signal_line
    macd_line
        .iter()
        .zip(signal_line.iter())
        .map(|(&macd, &signal)| {
            if macd.is_nan() || signal.is_nan() {
                nan_value.clone()
            } else {
                MacdValue {
                    macd_line: macd,
                    signal_line: signal,
                    histogram: macd - signal,
                }
            }
        })
        .collect()
}

/// EMA 純函數計算
///
/// 初始值使用前 period 個收盤價的 SMA。
/// multiplier = 2 / (period + 1)，標準 EMA 公式。
pub fn compute_ema(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.len() < period {
        return vec![f64::NAN; closes.len()];
    }

    let mut result = vec![f64::NAN; closes.len()];
    let multiplier = 2.0 / (period as f64 + 1.0);

    // 初始 EMA = 前 period 個收盤價的 SMA
    let initial_sma: f64 = closes[..period].iter().sum::<f64>() / period as f64;
    result[period - 1] = initial_sma;

    for i in period..closes.len() {
        result[i] = closes[i] * multiplier + result[i - 1] * (1.0 - multiplier);
    }

    result
}

/// EMA 計算，跳過前段 NAN 值後從第一個有效值開始計算
///
/// 用於計算 signal line（對 MACD line 做 EMA，MACD line 前段為 NAN）。
fn compute_ema_skip_nan(values: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; values.len()];

    // 找到第一個非 NAN 的位置
    let first_valid = match values.iter().position(|v| !v.is_nan()) {
        Some(pos) => pos,
        None => return result,
    };

    let valid_values = &values[first_valid..];
    if valid_values.len() < period {
        return result;
    }

    let multiplier = 2.0 / (period as f64 + 1.0);
    let initial_sma: f64 = valid_values[..period].iter().sum::<f64>() / period as f64;

    result[first_valid + period - 1] = initial_sma;

    for i in period..valid_values.len() {
        let prev = result[first_valid + i - 1];
        result[first_valid + i] = valid_values[i] * multiplier + prev * (1.0 - multiplier);
    }

    result
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macd_new_invalid_periods_returns_error() {
        assert!(Macd::new(26, 12, 9).is_err()); // slow < fast
        assert!(Macd::new(0, 26, 9).is_err()); // zero period
    }

    #[test]
    fn test_macd_insufficient_data_returns_nan() {
        let closes: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let result = compute_macd(&closes, 12, 26, 9);
        assert!(result.iter().all(|v| v.macd_line.is_nan()));
    }

    #[test]
    fn test_macd_result_length_matches_input() {
        let closes: Vec<f64> = (1..=50).map(|i| i as f64).collect();
        let result = compute_macd(&closes, 12, 26, 9);
        assert_eq!(result.len(), closes.len());
    }

    #[test]
    fn test_macd_valid_values_are_finite() {
        let closes: Vec<f64> = (1..=50).map(|i| i as f64).collect();
        let result = compute_macd(&closes, 12, 26, 9);
        for value in result.iter().filter(|v| !v.macd_line.is_nan()) {
            assert!(value.macd_line.is_finite());
            assert!(value.signal_line.is_finite());
            assert!(value.histogram.is_finite());
        }
    }

    #[test]
    fn test_ema_result_length_matches_input() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = compute_ema(&closes, 3);
        assert_eq!(result.len(), closes.len());
    }

    #[test]
    fn test_ema_initial_value_equals_sma() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = compute_ema(&closes, 3);
        // 初始 EMA = SMA(1, 2, 3) = 2.0
        assert!((result[2] - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_macd_histogram_equals_macd_minus_signal() {
        let closes: Vec<f64> = (1..=50).map(|i| i as f64 * 0.5 + 100.0).collect();
        let result = compute_macd(&closes, 12, 26, 9);
        for value in result.iter().filter(|v| !v.macd_line.is_nan()) {
            let expected_histogram = value.macd_line - value.signal_line;
            assert!((value.histogram - expected_histogram).abs() < 1e-10);
        }
    }
}
