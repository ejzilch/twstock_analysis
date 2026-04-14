use crate::constants;
use crate::data::models::RawCandle;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashSet;
use std::time::{Duration, Instant};

// ── 公開型別 ─────────────────────────────────────────────────────────────────

/// PostgreSQL 與 Redis 的統一存取客戶端
///
/// 以 Arc 包裝後在 async task 間共享，不需複製。
pub struct DbClient {
    pub pool: PgPool,
    pub redis_client: redis::Client,
}

/// K 線批次寫入緩衝區
///
/// 攢批策略：累積 500 筆或距上次刷入超過 1000ms，擇一觸發。
/// Graceful Shutdown 時呼叫 flush_and_close() 強制刷入剩餘資料。
///
/// 快取失效順序（固定，不可顛倒）：
/// 1. Bulk INSERT 寫入 DB
/// 2. COMMIT 事務
/// 3. 統一 UNLINK affected symbols 的 redis keys
pub struct BulkInsertBuffer {
    buffer: Vec<RawCandle>,
    last_flush_at: Instant,
    max_batch_size: usize,
    max_wait_ms: u64,
}

// ── BulkInsertBuffer 實作 ─────────────────────────────────────────────────────

impl BulkInsertBuffer {
    /// 建立新的緩衝區，容量預先分配 500 筆
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(constants::BULK_INSERT_MAX_BATCH_SIZE),
            last_flush_at: Instant::now(),
            max_batch_size: constants::BULK_INSERT_MAX_BATCH_SIZE,
            max_wait_ms: constants::BULK_INSERT_MAX_WAIT_MS,
        }
    }

    /// 加入一筆 K 線，若達到刷入條件則自動執行批次寫入
    pub async fn push(&mut self, candle: RawCandle, db: &DbClient) -> anyhow::Result<()> {
        self.buffer.push(candle);
        if self.should_flush() {
            self.flush(db).await?;
        }
        Ok(())
    }

    /// 執行批次寫入與快取失效
    ///
    /// 寫入順序固定：Bulk INSERT -> COMMIT -> UNLINK redis keys。
    /// Redis UNLINK 失敗時記錄 warning log，不中斷主流程，依賴 TTL 自然過期。
    pub async fn flush(&mut self, db: &DbClient) -> anyhow::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let items_to_insert = std::mem::replace(&mut self.buffer, Vec::with_capacity(500));
        let affected_symbols: HashSet<String> =
            items_to_insert.iter().map(|c| c.symbol.clone()).collect();

        // 步驟 1 + 2: Bulk INSERT（sqlx 自動 COMMIT）
        execute_bulk_insert(&db.pool, &items_to_insert).await?;

        // 步驟 3: 統一 UNLINK redis keys（失敗不中斷主流程）
        if let Err(redis_error) = invalidate_caches(&db.redis_client, &affected_symbols).await {
            tracing::warn!(
                error          = %redis_error,
                symbol_count   = affected_symbols.len(),
                "Redis cache invalidation failed, relying on TTL expiry"
            );
        }

        self.last_flush_at = Instant::now();
        Ok(())
    }

    /// Graceful Shutdown 時呼叫，強制刷入剩餘資料後記錄完成 log
    ///
    /// 對應 ARCH_DESIGN.md Graceful Shutdown 第三步：Flush BulkInsertBuffer。
    pub async fn flush_and_close(&mut self, db: &DbClient) -> anyhow::Result<()> {
        let remaining = self.buffer.len();
        self.flush(db).await?;
        tracing::info!(
            flushed_count = remaining,
            "BulkInsertBuffer flushed and closed during graceful shutdown"
        );
        Ok(())
    }

    // ── 私有方法 ────────────────────────────────────────────────────────────

    fn should_flush(&self) -> bool {
        self.buffer.len() >= self.max_batch_size
            || self.last_flush_at.elapsed() >= Duration::from_millis(self.max_wait_ms)
    }
}

impl Default for BulkInsertBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ── 私有函數 ──────────────────────────────────────────────────────────────────

/// Bulk INSERT K 線資料，ON CONFLICT DO NOTHING 保證冪等性
///
/// 主鍵為 (symbol, timestamp_ms, interval)，對應 init_schema.sql 定義。
/// 同一筆資料重複寫入時靜默忽略，不回傳錯誤。
async fn execute_bulk_insert(pool: &PgPool, items: &[RawCandle]) -> anyhow::Result<()> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO candles \
         (symbol, timestamp_ms, interval, open, high, low, close, volume, source) ",
    );

    query_builder.push_values(items, |mut b, item| {
        b.push_bind(&item.symbol)
            .push_bind(item.timestamp_ms)
            .push_bind(item.interval.as_str())
            .push_bind(item.open)
            .push_bind(item.high)
            .push_bind(item.low)
            .push_bind(item.close)
            .push_bind(item.volume as i64) // sqlx PgPool 使用 i64 對應 BIGINT
            .push_bind(item.source.to_string());
    });

    // 主鍵為 (symbol, timestamp_ms, interval)，對應 init_schema.sql
    query_builder.push(" ON CONFLICT (symbol, timestamp_ms, interval) DO NOTHING");

    query_builder.build().execute(pool).await?;
    Ok(())
}

/// 統一 UNLINK affected symbols 的 redis keys
///
/// 使用 SCAN 找出符合 pattern 的 key，再批次 UNLINK（非同步刪除，效能優於 DEL）。
/// DEL / UNLINK 不支援萬用字元，必須先 SCAN 展開。
async fn invalidate_caches(
    redis_client: &redis::Client,
    symbols: &HashSet<String>,
) -> anyhow::Result<()> {
    let mut conn = redis_client.get_multiplexed_async_connection().await?;

    for symbol in symbols {
        let patterns = [
            format!("indicators:{symbol}:*"),
            format!("signal:{symbol}:*"),
        ];

        for pattern in &patterns {
            // KEYS 在生產環境會阻塞 Redis，改用 SCAN 迭代展開
            // SCAN 每次最多回傳 100 筆，迭代直到 cursor 回到 0
            let mut cursor: u64 = 0;
            let mut all_keys: Vec<String> = Vec::new();

            loop {
                let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(pattern)
                    .arg("COUNT")
                    .arg(constants::REDIS_SCAN_BATCH_SIZE)
                    .query_async(&mut conn)
                    .await?;

                all_keys.extend(batch);
                cursor = next_cursor;

                if cursor == 0 {
                    break;
                }
            }

            if !all_keys.is_empty() {
                // UNLINK 非同步刪除，效能優於 DEL，不阻塞 Redis
                let _: () = redis::cmd("UNLINK")
                    .arg(&all_keys)
                    .query_async(&mut conn)
                    .await?;

                tracing::debug!(
                    symbol  = %symbol,
                    pattern = %pattern,
                    count   = all_keys.len(),
                    "Redis keys invalidated via SCAN + UNLINK"
                );
            }
        }
    }

    Ok(())
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::models::DataSource;
    use crate::models::Interval;

    fn make_raw_candle(symbol: &str) -> RawCandle {
        RawCandle {
            symbol: symbol.to_string(),
            timestamp_ms: 1704067200000,
            interval: Interval::OneHour,
            open: 150.0,
            high: 151.5,
            low: 149.5,
            close: 151.0,
            volume: 1_000_000,
            source: DataSource::FinMind,
        }
    }

    #[test]
    fn test_should_flush_when_batch_size_reached() {
        let buffer = BulkInsertBuffer {
            buffer: vec![make_raw_candle("2330"); 500],
            last_flush_at: Instant::now(),
            max_batch_size: 500,
            max_wait_ms: 1000,
        };
        assert!(buffer.should_flush());
    }

    #[test]
    fn test_should_not_flush_when_below_threshold() {
        let buffer = BulkInsertBuffer {
            buffer: vec![make_raw_candle("2330"); 10],
            last_flush_at: Instant::now(),
            max_batch_size: 500,
            max_wait_ms: 1000,
        };
        assert!(!buffer.should_flush());
    }

    #[test]
    fn test_should_flush_when_time_elapsed() {
        let buffer = BulkInsertBuffer {
            buffer: vec![make_raw_candle("2330")],
            last_flush_at: Instant::now() - Duration::from_millis(1100),
            max_batch_size: 500,
            max_wait_ms: 1000,
        };
        assert!(buffer.should_flush());
    }

    #[test]
    fn test_buffer_clears_after_flush_preparation() {
        let mut buffer = BulkInsertBuffer::new();
        buffer.buffer.push(make_raw_candle("2330"));
        let items = std::mem::replace(&mut buffer.buffer, Vec::with_capacity(500));
        assert_eq!(items.len(), 1);
        assert!(buffer.buffer.is_empty());
    }
}
