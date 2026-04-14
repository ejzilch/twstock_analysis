use crate::core::indicators::ma::compute_sma;
use crate::core::indicators::traits::IndicatorCalculator;
use crate::models::{Candle, IndicatorValue};
use std::collections::HashMap;

/// Bollinger Bands（布林通道）
///
/// 由三條線組成：
/// - upper  = SMA(period) + std_dev_multiplier * std_dev
/// - middle = SMA(period)
/// - lower  = SMA(period) - std_dev_multiplier * std_dev
///
/// 標準參數：period=20, std_dev_multiplier=2.0。
/// 前 period-1 個位置因 SMA 資料不足，三條線均回傳 NAN。
pub struct BollingerBands {
    /// 指標唯一 ID，固定為 "bollinger"
    id: String,
    /// SMA 計算週期，標準值 20
    period: usize,
    /// 標準差倍數，標準值 2.0
    std_dev_multiplier: f64,
}

impl BollingerBands {
    /// 建立新的 Bollinger Bands 計算器
    ///
    /// # 參數
    /// - `period`:             SMA 計算週期，必須大於 1
    /// - `std_dev_multiplier`: 標準差倍數，必須大於 0.0
    pub fn new(period: usize, std_dev_multiplier: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(
            period > 1,
            "Bollinger period must be greater than 1, got {period}"
        );
        anyhow::ensure!(
            std_dev_multiplier > 0.0 && std_dev_multiplier.is_finite(),
            "Bollinger std_dev_multiplier must be positive and finite, got {std_dev_multiplier}"
        );
        Ok(Self {
            id: "bollinger".to_string(),
            period,
            std_dev_multiplier,
        })
    }
}

impl IndicatorCalculator for BollingerBands {
    fn id(&self) -> &str {
        &self.id
    }

    fn compute(
        &self,
        candles: &[Candle],
        _computed: &HashMap<String, Vec<IndicatorValue>>,
    ) -> anyhow::Result<Vec<IndicatorValue>> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let result = compute_bollinger(&closes, self.period, self.std_dev_multiplier);

        // Bollinger Bands 回傳三個獨立的 Scalar 序列
        // 格式：依序為 upper, middle, lower，由 Factory 組裝為 response
        Ok(result
            .into_iter()
            .flat_map(|(upper, middle, lower)| {
                vec![
                    IndicatorValue::Scalar(upper),
                    IndicatorValue::Scalar(middle),
                    IndicatorValue::Scalar(lower),
                ]
            })
            .collect())
    }
}

/// Bollinger Bands 純函數計算
///
/// 回傳與輸入等長的 (upper, middle, lower) tuple 陣列。
/// 資料不足的位置三個值均為 NAN。
pub fn compute_bollinger(
    closes: &[f64],
    period: usize,
    std_dev_multiplier: f64,
) -> Vec<(f64, f64, f64)> {
    let nan_triple = (f64::NAN, f64::NAN, f64::NAN);

    if closes.len() < period {
        return vec![nan_triple; closes.len()];
    }

    let sma_values = compute_sma(closes, period);

    closes.windows(period).enumerate().fold(
        vec![nan_triple; closes.len()],
        |mut result, (i, window)| {
            let idx = i + period - 1;
            let middle = sma_values[idx];

            if middle.is_nan() {
                return result;
            }

            let variance: f64 = window
                .iter()
                .map(|&price| (price - middle).powi(2))
                .sum::<f64>()
                / period as f64;

            let std_dev = variance.sqrt();
            let band = std_dev_multiplier * std_dev;

            result[idx] = (middle + band, middle, middle - band);
            result
        },
    )
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bollinger_new_invalid_params_returns_error() {
        assert!(BollingerBands::new(0, 2.0).is_err());
        assert!(BollingerBands::new(1, 2.0).is_err());
        assert!(BollingerBands::new(20, 0.0).is_err());
        assert!(BollingerBands::new(20, -1.0).is_err());
        assert!(BollingerBands::new(20, f64::INFINITY).is_err());
    }

    #[test]
    fn test_bollinger_insufficient_data_returns_nan() {
        let closes = vec![1.0, 2.0, 3.0];
        let result = compute_bollinger(&closes, 20, 2.0);
        assert!(result
            .iter()
            .all(|(u, m, l)| u.is_nan() && m.is_nan() && l.is_nan()));
    }

    #[test]
    fn test_bollinger_result_length_matches_input() {
        let closes: Vec<f64> = (1..=30).map(|i| i as f64).collect();
        let result = compute_bollinger(&closes, 20, 2.0);
        assert_eq!(result.len(), closes.len());
    }

    #[test]
    fn test_bollinger_upper_greater_than_lower() {
        let closes: Vec<f64> = vec![
            100.0, 102.0, 98.0, 101.0, 99.0, 103.0, 97.0, 104.0, 96.0, 105.0, 100.0, 102.0, 98.0,
            101.0, 99.0, 103.0, 97.0, 104.0, 96.0, 105.0, 101.0, 103.0,
        ];
        let result = compute_bollinger(&closes, 20, 2.0);
        for (upper, middle, lower) in result.iter().filter(|(u, _, _)| !u.is_nan()) {
            assert!(upper > middle && middle > lower);
        }
    }

    #[test]
    fn test_bollinger_constant_price_has_zero_bandwidth() {
        // 價格不變時標準差為 0，上下軌等於中軌
        let closes = vec![100.0_f64; 25];
        let result = compute_bollinger(&closes, 20, 2.0);
        for (upper, middle, lower) in result.iter().filter(|(u, _, _)| !u.is_nan()) {
            assert!((upper - middle).abs() < 1e-10);
            assert!((lower - middle).abs() < 1e-10);
        }
    }

    #[test]
    fn test_bollinger_middle_equals_sma() {
        let closes: Vec<f64> = (1..=25).map(|i| i as f64).collect();
        let sma = compute_sma(&closes, 20);
        let result = compute_bollinger(&closes, 20, 2.0);

        for (i, (_, middle, _)) in result.iter().enumerate() {
            if !middle.is_nan() {
                assert!((middle - sma[i]).abs() < 1e-10);
            }
        }
    }
}
