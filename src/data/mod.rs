/// 外部資料擷取、DB 寫入與快取失效模組
///
/// 子模組職責：
/// - models:             RawCandle、DataSource、FetchParams 定義
/// - fetch:              FinMind / yfinance K 線抓取與正規化
/// - fetch_rate_limiter: FinMind API 排程限流，預留付費升級介面
/// - db:                 Bulk Insert 緩衝區、Redis 快取失效
/// - symbol_sync:        動態 Symbol 清單每日同步
pub mod db;
pub mod fetch;
pub mod fetch_rate_limiter;
pub mod implementations;
pub mod manual_sync;
pub mod models;
pub mod symbol_sync;
pub mod traits;

// 補上這兩個
#[cfg(test)]
mod db_tests;
#[cfg(test)]
pub mod mocks;
