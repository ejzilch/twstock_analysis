use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ── 台股交易成本 ──────────────────────────────────────────────────────────────
pub const COMMISSION_RATE: f64 = 0.001425; // 單邊手續費 0.1425%
pub const TAX_RATE: f64 = 0.003; // 交易稅 0.3%（賣出時）

// ── Sharpe Ratio 無風險利率 ───────────────────────────────────────────────────
// 台灣年化約 1.875%（10 年期公債），換算為日化
// risk_free_daily = (1 + 0.01875)^(1/252) - 1 ≈ 0.0000740
pub const RISK_FREE_ANNUAL: f64 = 0.01875;

// ── 出場緩衝濾網 ──────────────────────────────────────────────────────────────
// 持倉中，訊號轉空時不立即出場；
// 必須額外確認「當日收盤相較昨日收盤跌幅 ≥ EXIT_FILTER_THRESHOLD」才出場。
// 預設 1.5%；可依策略特性在 BacktestRequest 中透過 exit_filter_pct 覆寫。
pub const DEFAULT_EXIT_FILTER_THRESHOLD: f64 = 0.015; // 1.5%

// ── 最短持倉天數 ──────────────────────────────────────────────────────────────
// 進場後至少持有 N 天才允許出場訊號生效。
// 預設 5 天；可透過 BacktestRequest.min_holding_days 覆寫。
pub const DEFAULT_MIN_HOLDING_DAYS: u32 = 5;

// ── 硬停損 ────────────────────────────────────────────────────────────────────
pub const HARD_STOP_LOSS_PCT: f64 = 0.03;

// ── DB 行對應結構 ─────────────────────────────────────────────────────────────

#[derive(Debug, FromRow, Clone)]
pub struct CandleRow {
    pub timestamp_ms: i64,
    pub close: f64,
}

// ── Request / Response ────────────────────────────────────────────────────────

/// POST /api/v1/backtest 的請求結構，對應 API_CONTRACT.md
#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub request_id: String,
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    /// 出場緩衝濾網：持倉中訊號轉空時，需跌破前收盤幾 % 才真正出場。
    /// 不傳時使用預設值 1.5%（DEFAULT_EXIT_FILTER_THRESHOLD）。
    /// 傳 0.0 則等同停用濾網（還原為原始行為）。
    #[serde(default)]
    pub exit_filter_pct: Option<f64>,
    /// 最短持倉天數：進場後至少持有幾天才允許出場訊號生效。
    /// 不傳時使用預設值 5 天（DEFAULT_MIN_HOLDING_DAYS）。
    /// 傳 0 則等同停用（任何時候都可出場）。
    #[serde(default)]
    pub min_holding_days: Option<u32>,
}

/// POST /api/v1/backtest 的回測指標
#[derive(Debug, Serialize)]
pub struct BacktestMetrics {
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub annual_return: f64,
    /// 回測期間最長連續虧損筆數
    pub max_consecutive_losses: u32,
    /// 最長連虧區間的累計虧損金額（TWD，絕對值）
    pub max_consecutive_loss_amount: f64,
    /// 所有連虧區間的平均筆數（無連虧時為 0.0）
    pub avg_consecutive_losses: f64,
    /// 回測期間最長連續獲利筆數
    pub max_consecutive_wins: u32,
}

/// 單筆交易記錄
#[derive(Debug, Serialize)]
pub struct TradeRecord {
    /// 進場時間戳（毫秒）
    pub entry_timestamp_ms: i64,
    /// 出場時間戳（毫秒）
    pub exit_timestamp_ms: i64,
    /// 進場價格
    pub entry_price: f64,
    /// 出場價格
    pub exit_price: f64,
    /// 損益金額
    pub net_pnl: f64,
    /// 是否獲利
    pub is_win: bool,
}

/// POST /api/v1/backtest 的完整回應
#[derive(Debug, Serialize)]
pub struct BacktestResponse {
    pub backtest_id: String,
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub metrics: BacktestMetrics,
    pub created_at_ms: i64,
    pub exit_filter_pct: f64,
    /// 每筆交易記錄，供前端 K 線圖標記使用
    pub trades: Vec<TradeRecord>,
}
