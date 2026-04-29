/// 計算最大回撤
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

/// Sharpe = (mean_daily_excess_return / std_daily_return) * sqrt(252)
/// excess_return = strategy_daily_return - risk_free_daily
/// risk_free_daily = (1 + risk_free_annual)^(1/252) - 1
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

/// 年化報酬率（複利）
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::backtest::types::RISK_FREE_ANNUAL;

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
}
