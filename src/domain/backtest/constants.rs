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

// ── 停利：布林上軌超漲濾網 ───────────────────────────────────────────────────
// 持倉中連續 TP_BOLL_CONSEC_DAYS 天收盤 > boll_upper × (1 + DEFAULT_TP_BOLL_PCT) 則停利出場
pub const DEFAULT_TP_BOLL_PCT: f64 = 0.015; // 5%
pub const TP_BOLL_CONSEC_DAYS: u32 = 2; // 連續 2 天

// ── 停利後冷卻天數 ────────────────────────────────────────────────────────────
// 停利出場後需等待此天數才重新允許進場（避免立刻追高）
pub const DEFAULT_COOLDOWN_DAYS: u32 = 6;
