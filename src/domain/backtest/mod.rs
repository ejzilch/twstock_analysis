/// 回測核心模組（Domain layer）
///
/// engine:  純計算回測引擎，零 I/O，可單元測試
/// metrics: 財務指標計算函數（max drawdown / Sharpe / annual return）
pub mod constants;
pub mod engine;
pub mod metrics;
