/// 回測財務指標計算（純數學函數，無副作用）
///
/// 從原 `api/handlers/backtest/cost.rs` 搬移至 `core/backtest/metrics.rs`，
/// 讓 domain 層可直接使用，不須跨越 API 層邊界。
/// 原 `cost.rs` 保留 re-export，維持向下相容。

/// 計算最大回撤（Max Drawdown）
///
/// 定義：從波峰到波谷的最大跌幅百分比。
/// 回傳值範圍 [0.0, 1.0]，0.0 表示無任何回撤。
pub fn compute_max_drawdown(equity_curve: &[f64]) -> f64 {
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

/// 計算年化 Sharpe Ratio
///
/// Sharpe = (mean_daily_excess_return / std_daily_return) * sqrt(252)
/// excess_return = strategy_daily_return - risk_free_daily
/// risk_free_daily = (1 + risk_free_annual)^(1/252) - 1
///
/// 標準差為 0 時回傳 0.0（避免 NaN）。
pub fn compute_sharpe_ratio(daily_returns: &[f64], risk_free_annual: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

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

/// 計算複利年化報酬率
///
/// 公式：(final / initial)^(1/years) - 1
/// 輸入無效時（負值、零值、零天數）回傳 0.0。
pub fn compute_annualized_return(initial: f64, final_capital: f64, trading_days: usize) -> f64 {
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

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::backtest::dto::types::RISK_FREE_ANNUAL;

    #[test]
    fn test_max_drawdown_non_zero() {
        let curve = vec![100.0, 120.0, 90.0, 110.0];
        let dd = compute_max_drawdown(&curve);
        assert!(dd > 0.0);
    }

    #[test]
    fn test_max_drawdown_monotone_up_is_zero() {
        let curve: Vec<f64> = (1..=10).map(|i| i as f64 * 10.0).collect();
        let dd = compute_max_drawdown(&curve);
        assert!(dd.abs() < f64::EPSILON);
    }

    #[test]
    fn test_max_drawdown_known_value() {
        // Peak=120, trough=90 → dd = (120-90)/120 = 0.25
        let curve = vec![100.0, 120.0, 90.0];
        let dd = compute_max_drawdown(&curve);
        assert!((dd - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_sharpe_zero_when_flat() {
        let risk_free_daily = (1.0 + RISK_FREE_ANNUAL).powf(1.0 / 252.0) - 1.0;
        let returns = vec![risk_free_daily; 3];
        assert_eq!(compute_sharpe_ratio(&returns, RISK_FREE_ANNUAL), 0.0);
    }

    #[test]
    fn test_sharpe_lower_with_risk_free_rate() {
        let returns = vec![0.001, 0.002, 0.0015, 0.001, 0.002];
        let sharpe_with_rf = compute_sharpe_ratio(&returns, RISK_FREE_ANNUAL);
        let sharpe_no_rf = compute_sharpe_ratio(&returns, 0.0);
        assert!(sharpe_with_rf < sharpe_no_rf);
    }

    #[test]
    fn test_sharpe_single_return_is_zero() {
        assert_eq!(compute_sharpe_ratio(&[0.01], RISK_FREE_ANNUAL), 0.0);
    }

    #[test]
    fn test_annualized_return_positive_growth() {
        let r = compute_annualized_return(100.0, 110.0, 252);
        assert!(r > 0.0);
    }

    #[test]
    fn test_annualized_return_zero_for_invalid_inputs() {
        assert_eq!(compute_annualized_return(0.0, 110.0, 252), 0.0);
        assert_eq!(compute_annualized_return(100.0, 0.0, 252), 0.0);
        assert_eq!(compute_annualized_return(100.0, 110.0, 0), 0.0);
    }

    #[test]
    fn test_annualized_return_known_value() {
        // 252 個交易日翻倍 → 年化 = 100%
        let r = compute_annualized_return(100.0, 200.0, 252);
        assert!((r - 1.0).abs() < 1e-10);
    }
}
