/// trait 的生產環境實作。
/// 測試時不使用本模組，改用 mocks.rs。
use crate::data::models::RawCandle;
use crate::data::traits::{CacheInvalidator, DbWriter};
use crate::domain::BridgeError;

use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tracing::{error, info, warn};

// ── PostgresDbWriter ──────────────────────────────────────────────────────────

/// 生產環境：使用 sqlx PgPool 寫入 PostgreSQL。
pub struct PostgresDbWriter {
    pool: PgPool,
}

impl PostgresDbWriter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DbWriter for PostgresDbWriter {
    async fn write_batch(&self, batch: &[RawCandle]) -> Result<usize, BridgeError> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            error!(error = %e, "Failed to begin transaction");
            BridgeError::FinMindDataSourceError {
                context: format!("Failed to begin transaction: {}", e),
                source: None,
            }
        })?;

        let mut written = 0usize;

        for candle in batch {
            let result = sqlx::query!(
                r#"
                INSERT INTO candles (
                    symbol, timestamp_ms, interval,
                    open, high, low, close, volume,
                    source, created_at_ms
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (symbol, timestamp_ms, interval) DO NOTHING
                "#,
                candle.symbol,
                candle.timestamp_ms,
                candle.interval.as_str(),
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume as i64,
                candle.source.to_string(),
                candle.created_at_ms,
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!(
                    error        = %e,
                    symbol       = %candle.symbol,
                    timestamp_ms = candle.timestamp_ms,
                    "Failed to insert candle"
                );
                BridgeError::FinMindDataSourceError {
                    context: format!("Failed to insert candle: {}", e),
                    source: None,
                }
            })?;

            // rows_affected() == 0 → ON CONFLICT 跳過（不視為錯誤）
            if result.rows_affected() > 0 {
                written += 1;
            }
        }

        tx.commit().await.map_err(|e| {
            error!(error = %e, "Failed to commit candle batch");
            BridgeError::FinMindDataSourceError {
                context: format!("Failed to commit candle batch: {}", e),
                source: None,
            }
        })?;

        info!(
            total = batch.len(),
            written,
            skipped = batch.len() - written,
            "Candle batch committed"
        );
        Ok(written)
    }
}

// ── RedisInvalidator ──────────────────────────────────────────────────────────

/// 生產環境：使用 redis::Connection 使 keys 失效。
///
/// 使用 SCAN + UNLINK（非同步刪除，效能優於 DEL）。
/// 失敗時只記錄 warning，不中斷主流程。
pub struct RedisInvalidator {
    conn: MultiplexedConnection,
}

impl RedisInvalidator {
    pub fn new(conn: MultiplexedConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl CacheInvalidator for RedisInvalidator {
    async fn invalidate(&mut self, symbols: &[String]) {
        let keys: Vec<String> = symbols
            .iter()
            .flat_map(|s| {
                vec![
                    format!("stock:{}:latest", s),
                    format!("indicators:{}:*", s),
                    format!("signal:{}:*", s),
                ]
            })
            .collect();

        // UNLINK 為非同步刪除，不阻塞 Redis
        if let Err(e) = redis::cmd("UNLINK")
            .arg(&keys)
            .query_async::<()>(&mut self.conn)
            .await
        {
            warn!(
                error   = %e,
                symbols = ?symbols,
                "Redis UNLINK failed after candle insert; relying on TTL expiry"
            );
        }
    }
}
