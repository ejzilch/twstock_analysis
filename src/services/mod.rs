/// Service layer — 業務流程協調者
///
/// 每個 service 負責一個業務領域的完整流程：
///   - 驗證輸入
///   - 協調 domain 計算與 infrastructure 存取
///   - 組裝 response
///
/// Handler 只做 parse → call service → return response，不含任何業務邏輯。
pub mod admin_sync;
pub mod backtest;
pub mod candle;
pub mod signal;
