/// API Handler 模組
///
/// 每個 handler 對應一組相關端點，職責單一。
/// 所有 handler 回傳 Result<Json<T>, ApiError>，
/// ApiError 透過 IntoResponse 自動轉換為正確的 HTTP response。
pub mod admin_sync;
pub mod backtest;
pub mod candles;
pub mod health;
pub mod indicators;
pub mod predict;
pub mod signals;
pub mod symbols;
pub mod sync_state;
