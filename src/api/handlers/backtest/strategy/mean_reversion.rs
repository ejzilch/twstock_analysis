use crate::constants::{MR_MA50_TOLERANCE, MR_RSI_NEUTRAL_MAX, MR_RSI_OVERSOLD};

/// 判斷是否符合進場條件
///
/// L1 趨勢過濾：收盤 > MA50（多頭環境）且 RSI < MR_RSI_NEUTRAL_MAX
/// L2 進場訊號：收盤跌破 Bollinger 下軌 且 RSI < MR_RSI_OVERSOLD
pub fn mean_reversion_should_enter(close: f64, ma50: f64, rsi: f64, boll_lower: f64) -> bool {
    if !ma50.is_finite() || !rsi.is_finite() || !boll_lower.is_finite() {
        return false;
    }

    // L1：多頭環境過濾
    if close <= ma50 * MR_MA50_TOLERANCE {
        return false; // 收盤在 MA50 之下，空頭環境不接刀
    }
    if rsi >= MR_RSI_NEUTRAL_MAX {
        return false; // RSI 中性偏強區間，均值回歸無意義
    }

    // L2：雙重確認進場
    let below_lower_band = close < boll_lower;
    let oversold = rsi < MR_RSI_OVERSOLD;

    below_lower_band && oversold
}

/// 判斷是否應該出場
///
/// 條件一：收盤回到 Bollinger 中軌（MA20）→ 均值回歸完成
/// 條件二：收盤跌破 MA50 → 大趨勢轉壞，立即出場
pub fn mean_reversion_should_exit(close: f64, ma50: f64, boll_middle: f64) -> bool {
    if !ma50.is_finite() || !boll_middle.is_finite() {
        return false;
    }

    if close >= boll_middle {
        return true; // 均值回歸完成
    }

    if close < ma50 * MR_MA50_TOLERANCE {
        return true; // 大趨勢轉壞
    }

    false
}
