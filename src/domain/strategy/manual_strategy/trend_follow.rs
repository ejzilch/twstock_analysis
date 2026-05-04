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

/// 判斷趨勢跟隨策略的進場強度。
///
/// # 策略邏輯
///
/// ## Strong（強勢進場，全倉）
/// 條件：`MA5 > MA20 > MA50`
///
/// 三條均線呈多頭排列，短中長期趨勢一致向上。
/// 這是最理想的進場時機，代表趨勢已充分確立。
///
/// ## Weak（弱勢進場，半倉）
/// 條件：`MA20 > MA50` 且 `MA5 剛上穿 MA20`（今日穿越，昨日仍在下方）
///
/// 中長期趨勢向上（MA20 > MA50），但短期均線剛剛突破中期均線。
/// 這是趨勢初期的補捉訊號，代表動能開始轉強但尚未完全確立，
/// 因此採半倉進場以控制風險。
/// 注意：「剛上穿」定義為今日 MA5 > MA20 且昨日 MA5 <= MA20，
/// 避免趨勢持續期間重複觸發進場訊號。
///
/// ## None（不進場）
/// 以上條件均不符合，或資料不足（idx < TF_MA_LONG）、均線值無效（NaN/Inf）。
///
/// # 參數
/// - `ma5_series`  / `ma20_series` / `ma50_series`：各均線完整序列
/// - `idx`：當前 K 線索引
///
/// # 回傳
/// [`TrendSignalStrength`]：`Strong` / `Weak` / `None`
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

    // Strong：三均線順勢排列
    if ma5 > ma20 && ma20 > ma50 {
        return TrendSignalStrength::Strong;
    }

    // Weak：MA5 剛上穿 MA20（今天穿越，昨天還在下方），且 MA20 > MA50 趨勢向上
    if idx >= 1 {
        let ma5_prev = ma5_series[idx - 1];
        let ma20_prev = ma20_series[idx - 1];

        if ma20 > ma50
            && ma5 > ma20          // 今天 MA5 在 MA20 上方
            && ma5_prev <= ma20_prev
        // 昨天 MA5 還在 MA20 下方或相等
        {
            return TrendSignalStrength::Weak;
        }
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
    let rsi_exit =
        rsi.is_finite() && rsi_prev.is_finite() && rsi > TF_RSI_OVERBOUGHT && rsi < rsi_prev;

    // 條件三：趨勢破壞（最後防線）
    let trend_break = ma5 < ma20 && ma20 < ma50;

    momentum_exit || rsi_exit || trend_break
}
