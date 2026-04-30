use serde::Serialize;

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
