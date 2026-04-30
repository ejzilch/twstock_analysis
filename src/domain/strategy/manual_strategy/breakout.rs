use crate::constants::BO_RSI_OVERBOUGHT;

/// 判斷是否符合進場條件
///
/// L2 進場訊號：
///   收盤站上 Bollinger 上軌（收盤確認，非刺穿）
///   MACD histogram > 0 且 > 前日 histogram（動能擴大）
///   RSI < BO_RSI_OVERBOUGHT（未過熱）
pub fn breakout_should_enter(
    close: f64,
    boll_upper: f64,
    macd_histogram: f64,
    macd_histogram_prev: f64,
    rsi: f64,
) -> bool {
    if !boll_upper.is_finite()
        || !macd_histogram.is_finite()
        || !macd_histogram_prev.is_finite()
        || !rsi.is_finite()
    {
        return false;
    }

    let above_upper_band = close > boll_upper;
    let macd_expanding = macd_histogram > 0.0 && macd_histogram > macd_histogram_prev;
    let not_overbought = rsi < BO_RSI_OVERBOUGHT;

    above_upper_band && macd_expanding && not_overbought
}

/// 判斷是否應該出場
///
/// 條件一：收盤跌回 Bollinger 上軌之下 → 突破失敗
/// 條件二：MACD histogram 轉負 → 動能耗盡
pub fn breakout_should_exit(close: f64, boll_upper: f64, macd_histogram: f64) -> bool {
    if !boll_upper.is_finite() || !macd_histogram.is_finite() {
        return false;
    }

    if close < boll_upper {
        return true; // 突破失敗
    }

    if macd_histogram < 0.0 {
        return true; // 動能耗盡
    }

    false
}
