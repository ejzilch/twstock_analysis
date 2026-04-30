/// src/data/traits.rs
///
/// 副作用隔離 trait 定義。
/// 讓 BulkInsertBuffer 不直接依賴 PgPool 與 redis::Connection，
/// 測試時可注入 mock，生產時注入真實實作。
use async_trait::async_trait;

use crate::data::models::RawCandle;
use crate::domain::BridgeError;

// ── DbWriter ──────────────────────────────────────────────────────────────────

/// K 線資料寫入介面。
///
/// 生產實作：PostgresDbWriter（使用 sqlx PgPool）
/// 測試 mock：InMemoryDbWriter（寫入 Vec，供驗收）
#[async_trait]
pub trait DbWriter: Send + Sync {
    /// 批次寫入 K 線，ON CONFLICT DO NOTHING 保證冪等性。
    /// 回傳實際寫入筆數（不含跳過）。
    async fn write_batch(&self, batch: &[RawCandle]) -> Result<usize, BridgeError>;
}

// ── CacheInvalidator ──────────────────────────────────────────────────────────

/// 快取失效介面。
///
/// 生產實作：RedisInvalidator（使用 redis::Connection）
/// 測試 mock：SpyCacheInvalidator（記錄呼叫，供驗收）
#[async_trait]
pub trait CacheInvalidator: Send + Sync {
    /// 使指定股票清單的所有相關 Redis keys 失效。
    /// 失敗時只記錄 warning，不中斷主流程。
    async fn invalidate(&mut self, symbols: &[String]);
}
