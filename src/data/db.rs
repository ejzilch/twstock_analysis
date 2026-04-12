use crate::data::models::RawCandle;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct DbClient {
    pool: PgPool,
    redis_client: redis::Client,
}

pub struct BulkInsertBuffer {
    buffer: Vec<RawCandle>,
    last_flush_at: Instant,
    max_batch_size: usize,
    max_wait_ms: u64,
}

impl BulkInsertBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(500),
            last_flush_at: Instant::now(),
            max_batch_size: 500,
            max_wait_ms: 1000,
        }
    }

    /// 加入數據並判斷是否需要執行寫入
    pub async fn push(&mut self, candle: RawCandle, db: &DbClient) -> anyhow::Result<()> {
        self.buffer.push(candle);

        if self.should_flush() {
            self.flush(db).await?;
        }
        Ok(())
    }

    fn should_flush(&self) -> bool {
        self.buffer.len() >= self.max_batch_size
            || self.last_flush_at.elapsed() >= Duration::from_millis(self.max_wait_ms)
    }

    /// 執行批次寫入與快取失效
    pub async fn flush(&mut self, db: &DbClient) -> anyhow::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let items_to_insert = std::mem::replace(&mut self.buffer, Vec::with_capacity(500));
        let affected_symbols: std::collections::HashSet<String> =
            items_to_insert.iter().map(|c| c.symbol.clone()).collect();

        // 1. 執行 Bulk INSERT
        self.execute_bulk_insert(&db.pool, &items_to_insert).await?;

        // 2. 批次刪除 Redis 中的舊快取 (指標與信號)
        self.invalidate_caches(db, affected_symbols).await?;

        self.last_flush_at = Instant::now();
        Ok(())
    }

    async fn execute_bulk_insert(&self, pool: &PgPool, items: &[RawCandle]) -> anyhow::Result<()> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO candles (symbol, timestamp_ms, open, high, low, close, volume, source) ",
        );

        query_builder.push_values(items, |mut b, item| {
            b.push_bind(&item.symbol)
                .push_bind(item.timestamp_ms)
                .push_bind(item.open)
                .push_bind(item.high)
                .push_bind(item.low)
                .push_bind(item.close)
                .push_bind(item.volume)
                .push_bind(item.source.to_string());
        });

        query_builder.push(" ON CONFLICT (symbol, timestamp_ms) DO NOTHING");

        let query = query_builder.build();
        query.execute(pool).await?;
        Ok(())
    }

    async fn invalidate_caches(
        &self,
        db: &DbClient,
        symbols: std::collections::HashSet<String>,
    ) -> anyhow::Result<()> {
        let mut conn = db.redis_client.get_async_connection().await?;

        for symbol in symbols {
            // 根據快取鍵設計：指標與信號都需要失效
            let keys = vec![
                format!("indicators:{}:*", symbol),
                format!("signal:{}:*", symbol),
            ];
            redis::cmd("DEL").arg(&keys).query_async(&mut conn).await?;
        }
        Ok(())
    }
}
