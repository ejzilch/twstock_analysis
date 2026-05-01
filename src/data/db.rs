/// K 線批次寫入緩衝區（重構版）。
///
/// 架構改動：
///   BulkInsertBuffer 不再直接依賴 PgPool / redis::Connection，
///   改透過 DbWriter + CacheInvalidator trait 注入，
///   讓 unit test 可以不需要真實 DB / Redis。
///
/// 快取失效順序（固定，不可顛倒）：
///   1. Bulk INSERT 寫入 DB（DbWriter::write_batch）
///   2. COMMIT 事務
///   3. 統一 UNLINK affected symbols 的 redis keys（CacheInvalidator::invalidate）
use crate::constants::{BULK_INSERT_MAX_BATCH_SIZE, BULK_INSERT_MAX_WAIT_MS};
use crate::data::models::RawCandle;
use crate::data::traits::{CacheInvalidator, DbWriter};
use crate::domain::BridgeError;
use sqlx::PgPool;
use tracing::error;

use std::time::{Duration, Instant};
use tracing::info;

// ── BulkInsertBuffer ──────────────────────────────────────────────────────────

/// K 線批次寫入緩衝區。
///
/// 攢批策略：累積 BULK_INSERT_MAX_BATCH_SIZE 筆
///           或距上次刷入超過 BULK_INSERT_MAX_WAIT_MS ms，
///           擇一觸發 flush。
pub struct BulkInsertBuffer {
    pub(crate) buffer: Vec<RawCandle>,
    pub(crate) last_flush_at: Instant,
    total_inserted: i32,
    total_skipped: i32,
}

impl std::fmt::Debug for BulkInsertBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BulkInsertBuffer")
            .field("buffer_len", &self.buffer.len())
            .field("total_inserted", &self.total_inserted)
            .field("total_skipped", &self.total_skipped)
            .finish()
    }
}

impl BulkInsertBuffer {
    pub fn total_inserted(&self) -> i32 {
        self.total_inserted
    }

    pub fn total_skipped(&self) -> i32 {
        self.total_skipped
    }
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(BULK_INSERT_MAX_BATCH_SIZE),
            last_flush_at: Instant::now(),
            total_inserted: 0,
            total_skipped: 0,
        }
    }

    /// 推入一筆資料，達到觸發條件時自動 flush。
    pub async fn push(
        &mut self,
        candle: RawCandle,
        writer: &dyn DbWriter,
        invalidator: &mut dyn CacheInvalidator,
    ) -> Result<(), BridgeError> {
        self.buffer.push(candle);
        if self.should_flush() {
            self.flush(writer, invalidator).await?;
        }
        Ok(())
    }

    /// 將緩衝區內所有資料寫入 DB，並使對應的 Redis keys 失效。
    pub async fn flush(
        &mut self,
        writer: &dyn DbWriter,
        invalidator: &mut dyn CacheInvalidator,
    ) -> Result<(), BridgeError> {
        if self.is_empty() {
            return Ok(());
        }

        let batch = std::mem::take(&mut self.buffer);

        // 收集受影響的股票代號（用於快取失效）
        let affected_symbols: Vec<String> = batch
            .iter()
            .map(|c| c.symbol.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Step 1 & 2：寫入 DB + COMMIT（由 DbWriter 實作負責）
        let written = writer.write_batch(&batch).await?;

        // 紀錄實際寫入與跳過（總數 - 成功寫入 = 跳過）的數量
        self.total_inserted += written as i32;
        self.total_skipped += (batch.len() - written) as i32;

        // Step 3：快取失效（COMMIT 之後才執行）
        let _ = invalidator.invalidate(&affected_symbols);

        self.last_flush_at = Instant::now();

        info!(
            total   = batch.len(),
            written,
            skipped = batch.len() - written,
            symbols = ?affected_symbols,
            "Buffer flushed"
        );
        Ok(())
    }

    /// Graceful Shutdown 時呼叫，強制 flush 剩餘資料。
    pub async fn flush_and_close(
        &mut self,
        writer: &dyn DbWriter,
        invalidator: &mut dyn CacheInvalidator,
    ) -> Result<(), BridgeError> {
        info!(
            remaining_candles = self.len(),
            "Flushing remaining candles before shutdown"
        );
        self.flush(writer, invalidator).await
    }

    /// 目前緩衝區內的資料筆數。
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    // ── 內部方法 ──────────────────────────────────────────────────────────────

    /// 判斷是否應觸發 flush。
    /// 只依賴業務規則（constants），不依賴外部狀態。
    fn should_flush(&self) -> bool {
        self.buffer.len() >= BULK_INSERT_MAX_BATCH_SIZE
            || self.last_flush_at.elapsed() >= Duration::from_millis(BULK_INSERT_MAX_WAIT_MS)
    }
}

// ── SyncLogEntry ──────────────────────────────────────────────────────────────

/// 建立 sync_log 記錄所需的資料。
pub struct SyncLogEntry {
    pub sync_id: String,
    /// "manual" 或 "scheduled"
    pub sync_type: String,
    /// "ej" 或 "system"
    pub triggered_by: String,
    pub symbols: Vec<String>,
}

// ── sync_log CRUD ─────────────────────────────────────────────────────────────

/// 建立新的 sync_log 記錄（status = 'running'）。
pub async fn sync_log_create(
    db_pool: &PgPool,
    entry: &SyncLogEntry,
    started_at_ms: i64,
) -> Result<(), BridgeError> {
    sqlx::query!(
        r#"
        INSERT INTO sync_log (
            sync_id, sync_type, triggered_by, symbols,
            total_inserted, total_skipped, total_failed,
            started_at_ms, completed_at_ms, status
        )
        VALUES ($1, $2, $3, $4, 0, 0, 0, $5, NULL, 'running')
        "#,
        entry.sync_id,
        entry.sync_type,
        entry.triggered_by,
        &entry.symbols,
        started_at_ms,
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        error!(
            error   = %e,
            sync_id = %entry.sync_id,
            "Failed to create sync_log entry"
        );
        BridgeError::from_db("sync_log_create failed", e)
    })?;

    Ok(())
}

impl Default for BulkInsertBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// 累加 sync_log 統計數字（非覆蓋，使用 += 語意）。
pub async fn sync_log_update_counts(
    db_pool: &PgPool,
    sync_id: &str,
    inserted: i32,
    skipped: i32,
    failed: i32,
) -> Result<(), BridgeError> {
    sqlx::query!(
        r#"
        UPDATE sync_log
        SET
            total_inserted = total_inserted + $2,
            total_skipped  = total_skipped  + $3,
            total_failed   = total_failed   + $4
        WHERE sync_id = $1
        "#,
        sync_id,
        inserted,
        skipped,
        failed,
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        error!(error = %e, sync_id = %sync_id, "Failed to update sync_log counts");
        BridgeError::from_db("sync_log_update_counts failed", e)
    })?;

    Ok(())
}

/// 更新 sync_log 狀態與完成時間。
pub async fn sync_log_update_status(
    db_pool: &PgPool,
    sync_id: &str,
    status: &str,
    completed_at_ms: Option<i64>,
) -> Result<(), BridgeError> {
    sqlx::query!(
        r#"
        UPDATE sync_log
        SET status = $2, completed_at_ms = $3
        WHERE sync_id = $1
        "#,
        sync_id,
        status,
        completed_at_ms,
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        error!(
            error   = %e,
            sync_id = %sync_id,
            status  = %status,
            "Failed to update sync_log status"
        );
        BridgeError::from_db("sync_log_update_status failed", e)
    })?;

    Ok(())
}
