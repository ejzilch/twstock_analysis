use reqwest::header::Entry;

use crate::domain::strategy::constants::{
    TF_MA_LONG, TF_RSI_OVERBOUGHT, TF_WEAK_SIGNAL_POSITION_RATIO,
};

/// trend_follow_v1 進場強度，影響實際使用的倉位比例
#[derive(Debug, PartialEq)]
pub enum TrendSignalStrength {
    /// MA5 > MA20 > MA50，強勢排列，全倉進場
    Strong,
    /// MA20 > MA50 且 MA5 剛上穿 MA20，弱勢補訊號，半倉進場
    Weak,
    /// 不進場
    None,
}

/// 判斷進場強度
/// Strong  = MA5 > MA20 > MA50（三均線順勢排列）
/// Weak    = MA20 > MA50 且 MA5 剛上穿 MA20（補訊號）
/// None    = 不進場
pub fn trend_follow_signal_strength(
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    idx: usize,
) -> TrendSignalStrength {
    if idx < TF_MA_LONG {
        return TrendSignalStrength::None;
    }

    let ma5 = ma5_series[idx];
    let ma20 = ma20_series[idx];
    let ma50 = ma50_series[idx];

    if !ma5.is_finite() || !ma20.is_finite() || !ma50.is_finite() {
        return TrendSignalStrength::None;
    }

    if ma5 > ma20 && ma20 > ma50 {
        return TrendSignalStrength::Strong;
    }

    // 「剛上穿」= 今日 MA5 > MA20 且昨日 MA5 <= MA20
    let just_crossed = ma5 > ma20;
    if ma20 > ma50 && just_crossed {
        return TrendSignalStrength::Weak;
    }

    TrendSignalStrength::None
}

/// 依進場強度解析實際倉位比例
/// 回傳 (should_hold, actual_position_fraction)
pub fn trend_follow_entry(
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    closes: &[f64],
    position_fraction: f64,
    idx: usize,
) -> (bool, f64) {
    let ma50 = ma50_series[idx];
    let close = closes[idx];

    // L1 趨勢過濾：收盤需在 MA50 之上
    if !ma50.is_finite() || close <= ma50 {
        return (false, 0.0);
    }

    let strength = trend_follow_signal_strength(ma5_series, ma20_series, ma50_series, idx);
    let fraction = match strength {
        TrendSignalStrength::Strong => position_fraction,
        TrendSignalStrength::Weak => position_fraction * TF_WEAK_SIGNAL_POSITION_RATIO,
        TrendSignalStrength::None => 0.0,
    };

    (strength != TrendSignalStrength::None, fraction)
}

/// 判斷是否應該出場
/// MA5 < MA20 且 MA20 < MA50（趨勢結束）OR（RSI 過熱後轉弱）
pub fn trend_follow_should_exit(
    ma5_series: &[f64],
    ma20_series: &[f64],
    ma50_series: &[f64],
    closes: &[f64],
    rsi_series: &[f64],
    idx: usize,
    entry_idx: usize,
) -> bool {
    if idx < 1 || idx <= entry_idx {
        return false;
    }

    let ma5 = ma5_series[idx];
    let ma5_prev = ma5_series[idx - 1];
    let ma20 = ma20_series[idx];
    let ma50 = ma50_series[idx];
    let close = closes[idx];
    let rsi = rsi_series[idx];
    let rsi_prev = rsi_series[idx - 1];

    if !ma5.is_finite() || !ma20.is_finite() || !ma50.is_finite() {
        return false;
    }

    // 條件一：動能轉弱（提前 exit）
    let momentum_exit = ma5 < ma5_prev && close < ma5;

    // 條件二：RSI 過熱後轉弱
    let rsi_exit = rsi.is_finite() && rsi_prev.is_finite() && rsi > 75.0 && rsi < rsi_prev;

    // 條件三：趨勢破壞（最後防線）
    let trend_break = ma5 < ma20 && ma20 < ma50;

    momentum_exit || rsi_exit || trend_break
}
