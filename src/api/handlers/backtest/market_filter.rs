use crate::constants::{BANDWIDTH_HIGH_PERCENTILE, BANDWIDTH_LOOKBACK, BANDWIDTH_LOW_PERCENTILE};

use super::types::CandleRow;

/// 預算每根 K 線對應的 Bollinger 帶寬值
/// 帶寬定義：(upper - lower) / middle，跨股票可比較
/// 資料不足 period 根時回傳 None
pub fn compute_bandwidth_series(candles: &[CandleRow]) -> Vec<Option<f64>> {
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
pub fn is_market_tradeable(bandwidth_series: &[Option<f64>], idx: usize) -> bool {
    let current_bandwidth = match bandwidth_series[idx] {
        Some(bw) => bw,
        None => return true, // 帶寬無法計算時，不過濾
    };

    let (low_threshold, high_threshold) = match compute_bandwidth_percentiles(bandwidth_series, idx)
    {
        Some(thresholds) => thresholds,
        None => return true, // 歷史資料不足時，不過濾
    };

    current_bandwidth > low_threshold && current_bandwidth < high_threshold
}
