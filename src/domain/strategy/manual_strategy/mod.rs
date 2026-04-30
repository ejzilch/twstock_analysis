pub mod breakout;
pub mod market_filter;
pub mod mean_reversion;
pub mod trend_follow;

use crate::models::candle::CandleRow;

// ── MA 常數（trend_follow legacy fallback 用） ────────────────────────────────
const MA_SHORT_PERIOD: usize = 5;
const MA_LONG_PERIOD: usize = 20;

/// 舊版 should_hold_position（fallback 與測試用）。
/// trend_follow / mean_reversion / breakout 已各自有獨立模組；
/// 此函數保留給未知策略名稱的 fallback，以及單元測試驗證 MA 交叉邏輯。
pub fn should_hold_position(strategy_name: &str, candles: &[CandleRow], idx: usize) -> bool {
    match strategy_name {
        // ── trend_follow_v1：MA5 > MA20 交叉判斷趨勢方向 ────────────────────
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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

    #[test]
    fn test_trend_follow_false_when_insufficient_data() {
        let candles = make_candles(&vec![100.0; 25]);
        assert!(!should_hold_position("trend_follow_v1", &candles, 10));
    }

    #[test]
    fn test_trend_follow_golden_cross() {
        // 前 15 天收盤 90，後 5 天收盤 110 → MA5=110, MA20=93.75 → 黃金交叉
        let mut closes = vec![90.0f64; 20];
        closes.extend_from_slice(&[110.0f64; 5]);
        let candles = make_candles(&closes);
        assert!(should_hold_position("trend_follow_v1", &candles, 24));
    }

    #[test]
    fn test_trend_follow_death_cross() {
        // 前 15 天收盤 110，後 5 天收盤 90 → 死亡交叉
        let mut closes = vec![110.0f64; 20];
        closes.extend_from_slice(&[90.0f64; 5]);
        let candles = make_candles(&closes);
        assert!(!should_hold_position("trend_follow_v1", &candles, 24));
    }
}
