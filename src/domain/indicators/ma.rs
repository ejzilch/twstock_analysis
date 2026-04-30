use crate::domain::indicators::traits::IndicatorCalculator;
use crate::models::{Candle, IndicatorValue};
use std::collections::HashMap;

/// 簡單移動平均線（Simple Moving Average）
///
/// 計算最近 N 根 K 線收盤價的算術平均值。
/// 前 period-1 個位置因資料不足，以第一個可計算值向前填充（forward-fill 的反向）
/// 實際上回傳 IndicatorValue::Scalar(f64::NAN) 供上層過濾。
pub struct MovingAverage {
    /// 指標唯一 ID，格式為 "ma{period}"，如 "ma20"
    id: String,
    /// 計算週期，如 5、20、50、200
    period: usize,
}

impl MovingAverage {
    /// 建立新的移動平均線計算器
    ///
    /// # 參數
    /// - `period`: 計算週期，必須大於 0
    pub fn new(period: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(period > 0, "MA period must be greater than 0, got {period}");
        Ok(Self {
            id: format!("ma{period}"),
            period,
        })
    }
}

impl IndicatorCalculator for MovingAverage {
    fn id(&self) -> &str {
        &self.id
    }

    fn compute(
        &self,
        candles: &[Candle],
        _computed: &HashMap<String, Vec<IndicatorValue>>,
    ) -> anyhow::Result<Vec<IndicatorValue>> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let result = compute_sma(&closes, self.period);
        Ok(result.into_iter().map(IndicatorValue::Scalar).collect())
    }
}

/// SMA 純函數計算，回傳與輸入等長的 f64 陣列
///
/// 資料不足的位置回傳 f64::NAN，由上層決定如何處理。
pub fn compute_sma(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.is_empty() || period == 0 {
        return vec![];
    }

    closes.windows(period).enumerate().fold(
        vec![f64::NAN; closes.len()],
        |mut result, (i, window)| {
            let sum: f64 = window.iter().sum();
            result[i + period - 1] = sum / period as f64;
            result
        },
    )
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Interval;

    #[test]
    fn test_sma_basic_calculation() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = compute_sma(&closes, 3);

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 2.0).abs() < f64::EPSILON); // (1+2+3)/3
        assert!((result[3] - 3.0).abs() < f64::EPSILON); // (2+3+4)/3
        assert!((result[4] - 4.0).abs() < f64::EPSILON); // (3+4+5)/3
    }

    #[test]
    fn test_sma_period_equals_length() {
        let closes = vec![2.0, 4.0, 6.0];
        let result = compute_sma(&closes, 3);

        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sma_period_one_returns_original() {
        let closes = vec![1.0, 2.0, 3.0];
        let result = compute_sma(&closes, 1);

        assert!((result[0] - 1.0).abs() < f64::EPSILON);
        assert!((result[1] - 2.0).abs() < f64::EPSILON);
        assert!((result[2] - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sma_insufficient_data_returns_nan() {
        let closes = vec![1.0, 2.0];
        let result = compute_sma(&closes, 5);

        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn test_sma_empty_input() {
        let result = compute_sma(&[], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_moving_average_new_zero_period_returns_error() {
        let result = MovingAverage::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_moving_average_result_length_matches_input() {
        let ma = MovingAverage::new(3).unwrap();
        let candles = make_candles(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = ma.compute(&candles, &HashMap::new()).unwrap();
        assert_eq!(result.len(), candles.len());
    }

    // 測試輔助函數
    fn make_candles(closes: Vec<f64>) -> Vec<Candle> {
        closes
            .into_iter()
            .enumerate()
            .map(|(i, close)| Candle {
                symbol: "2330".to_string(),
                interval: Interval::OneHour,
                timestamp_ms: 1704067200000 + (i as i64 * 3_600_000),
                open: close,
                high: close,
                low: close,
                close,
                volume: 1_000_000,
                indicators: Default::default(),
            })
            .collect()
    }
}
