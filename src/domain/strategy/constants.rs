// ── strategy 共用 ──────────────────────────────────────────────────────────────────────

/// 強制停利
pub const TAKE_PROFIT_RATIO: f64 = 0.3;

/// 連續虧損統計：損益序列為空時的預設平均值
pub const DEFAULT_AVG_CONSECUTIVE_LOSSES: f64 = 0.0;

// ── trend_follow ───────────────────────────────────────────────────────────────────────

/// trend_follow_v1：MA 計算週期
pub const TF_MA_SHORT: usize = 5;
pub const TF_MA_MID: usize = 20;
pub const TF_MA_LONG: usize = 50;

/// trend_follow_v1：RSI 過熱門檻，超過此值且轉弱才出場
pub const TF_RSI_OVERBOUGHT: f64 = 75.0;

/// trend_follow_v1：弱勢進場的倉位比例（相對於 position_size_percent）
pub const TF_WEAK_SIGNAL_POSITION_RATIO: f64 = 0.5;

// ── market_filter ──────────────────────────────────────────────────────────────────────

/// Layer 0 市場環境過濾：帶寬低於此 percentile 視為橫盤，完全不交易
pub const BANDWIDTH_LOW_PERCENTILE: f64 = 0.30; // 30th percentile

/// Layer 0 市場環境過濾：帶寬高於此 percentile 視為恐慌行情，完全不交易
pub const BANDWIDTH_HIGH_PERCENTILE: f64 = 0.95; // 95th percentile

/// Layer 0 帶寬 percentile 計算所需的最小歷史根數
pub const BANDWIDTH_LOOKBACK: usize = 20;
