use crate::models::{Candle, IndicatorValue};
use std::collections::HashMap;

/// 指標計算器的統一介面
///
/// 所有技術指標（MA、RSI、MACD、Bollinger）都實作此 trait，
/// 由 IndicatorFactory 透過 DAG 拓撲排序後依序呼叫。
pub trait IndicatorCalculator: Send + Sync {
    /// 回傳此指標的唯一識別名稱，如 "ma20"、"rsi14"、"macd"
    fn id(&self) -> &str;

    /// 回傳此指標依賴的其他指標 ID 列表
    ///
    /// 大部分指標直接依賴原始 K 線資料，回傳空 Vec。
    /// 若某指標需要另一個指標的結果（如依賴 MA 計算的指標），
    /// 在此回傳依賴的指標 ID，讓 Factory 拓撲排序時正確處理。
    fn dependencies(&self) -> Vec<&str> {
        vec![]
    }

    /// 執行指標計算
    ///
    /// # 參數
    /// - `candles`: 原始 K 線資料切片，按時間升序排列
    /// - `computed`: 已計算完成的其他指標結果，供有依賴關係的指標使用
    ///
    /// # 回傳
    /// 時間序列的指標值陣列，長度與 candles 一致。
    /// 資料不足以計算時（如 MA20 但只有 5 根 K 線），對應位置回傳 None 的處理
    /// 由各指標實作自行決定（通常前 N-1 個位置為無效值）。
    fn compute(
        &self,
        candles: &[Candle],
        computed: &HashMap<String, Vec<IndicatorValue>>,
    ) -> anyhow::Result<Vec<IndicatorValue>>;
}
