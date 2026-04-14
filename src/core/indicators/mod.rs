/// 技術指標計算模組
///
/// 所有指標透過 IndicatorCalculator trait 統一介面，
/// 由 IndicatorFactory 依 DAG 拓撲排序後依序計算。
pub mod bollinger;
pub mod factory;
pub mod ma;
pub mod macd;
pub mod rsi;
pub mod traits;
