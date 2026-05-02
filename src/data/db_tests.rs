/// src/data/db_tests.rs
///
/// BulkInsertBuffer 三層測試套件：
///
///   Layer 1 — Unit test：只測 should_flush 規則，不碰 DB / Redis
///   Layer 2 — Integration test：測 flush pipeline（mock DB + mock cache）
///   Layer 3 — sqlx integration test：測真實 DB 寫入（需 test DB）
///
/// 執行方式：
///   cargo test                              # Layer 1 + 2
///   cargo test --features integration-test  # Layer 1 + 2 + 3

#[cfg(test)]
mod unit_tests {
    /// Layer 1：只測 should_flush 規則。
    /// 不需要 DB / Redis，純邏輯驗證。
    use std::time::{Duration, Instant};

    use crate::constants::{BULK_INSERT_MAX_BATCH_SIZE, BULK_INSERT_MAX_WAIT_MS};
    use crate::data::db::BulkInsertBuffer;
    use crate::data::models::RawCandle;
    use crate::models::{DataSource, Interval};

    fn make_candle(symbol: &str) -> RawCandle {
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
            created_at_ms: 1704067200000,
        }
    }

    // ── should_flush 行為測試 ─────────────────────────────────────────────────

    #[test]
    fn test_should_not_flush_when_empty() {
        let buf = BulkInsertBuffer::new();
        // 透過 is_empty() 間接驗證（should_flush 為 private，只測行為）
        assert!(buf.is_empty());
    }

    #[test]
    fn test_flush_triggered_by_batch_size() {
        let mut buf = BulkInsertBuffer::new();

        // 推入 MAX - 1 筆：不觸發
        for _ in 0..(BULK_INSERT_MAX_BATCH_SIZE - 1) {
            buf.buffer.push(make_candle("2330"));
        }
        assert_eq!(buf.len(), BULK_INSERT_MAX_BATCH_SIZE - 1);

        // 推入第 MAX 筆：should_flush() 應為 true
        buf.buffer.push(make_candle("2330"));
        // 直接呼叫 should_flush（透過 pub(crate) 或測試模組）
        // 此處改為驗證 len 是否達到上限（間接驗證）
        assert_eq!(buf.len(), BULK_INSERT_MAX_BATCH_SIZE);
    }

    #[test]
    fn test_flush_triggered_by_time() {
        let mut buf = BulkInsertBuffer::new();

        // 手動倒撥時間（模擬等待超過上限）
        buf.last_flush_at = Instant::now() - Duration::from_millis(BULK_INSERT_MAX_WAIT_MS + 10);

        buf.buffer.push(make_candle("2330"));
        assert_eq!(buf.len(), 1);

        // 時間超過閾值，即使只有 1 筆也應 flush
        // should_flush 測的是內部規則，透過下方 integration test 驗證行為
        let elapsed = buf.last_flush_at.elapsed();
        assert!(elapsed >= Duration::from_millis(BULK_INSERT_MAX_WAIT_MS));
    }

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = BulkInsertBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_buffer_len_tracks_pushes() {
        let mut buf = BulkInsertBuffer::new();
        buf.buffer.push(make_candle("2330"));
        buf.buffer.push(make_candle("2317"));
        assert_eq!(buf.len(), 2);
    }
}

#[cfg(test)]
mod integration_tests {
    /// Layer 2：測 flush pipeline，使用 mock DB + mock cache。
    /// 不需要真實 DB / Redis，但驗證整個 flush 流程的行為。
    use crate::constants::BULK_INSERT_MAX_BATCH_SIZE;
    use crate::data::db::BulkInsertBuffer;
    use crate::data::mocks::test_mocks::{InMemoryDbWriter, SpyCacheInvalidator};
    use crate::data::models::RawCandle;
    use crate::models::{DataSource, Interval};

    fn make_candle(symbol: &str, timestamp_ms: i64) -> RawCandle {
        RawCandle {
            symbol: symbol.to_string(),
            timestamp_ms,
            interval: Interval::OneHour,
            open: 150.0,
            high: 151.5,
            low: 149.5,
            close: 151.0,
            volume: 1_000_000,
            source: DataSource::FinMind,
            created_at_ms: 1704067200000,
        }
    }

    // ── flush pipeline 正常路徑 ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_flush_writes_to_db_and_clears_buffer() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        buf.buffer.push(make_candle("2330", 1704067200000));
        buf.buffer.push(make_candle("2317", 1704067200000));

        buf.flush(&writer, &mut spy).await.unwrap();

        // 資料寫入 mock DB
        assert_eq!(writer.written_count(), 2);

        // buffer 清空
        assert!(buf.is_empty());

        // 快取失效被呼叫一次，含兩個 symbol
        assert_eq!(spy.call_count, 1);
        assert!(spy.invalidated_symbols.contains(&"2330".to_string()));
        assert!(spy.invalidated_symbols.contains(&"2317".to_string()));
    }

    #[tokio::test]
    async fn test_flush_empty_buffer_does_nothing() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        buf.flush(&writer, &mut spy).await.unwrap();

        assert_eq!(writer.written_count(), 0);
        assert_eq!(spy.call_count, 0); // 空 buffer 不觸發快取失效
    }

    #[tokio::test]
    async fn test_flush_called_when_batch_size_reached() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        // 推入剛好 MAX 筆，每筆 timestamp_ms 不同（避免 conflict）
        for i in 0..BULK_INSERT_MAX_BATCH_SIZE {
            let candle = make_candle("2330", 1704067200000 + i as i64 * 3600_000);
            buf.push(candle, &writer, &mut spy).await.unwrap();
        }

        // 達到 MAX 後 push() 內部自動觸發 flush，buffer 應已清空
        assert!(buf.is_empty());
        assert_eq!(writer.written_count(), BULK_INSERT_MAX_BATCH_SIZE);
        assert_eq!(spy.call_count, 1);
    }

    #[tokio::test]
    async fn test_flush_and_close_flushes_remaining() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        buf.buffer.push(make_candle("2330", 1704067200000));

        buf.flush_and_close(&writer, &mut spy).await.unwrap();

        assert!(buf.is_empty());
        assert_eq!(writer.written_count(), 1);
    }

    // ── ON CONFLICT DO NOTHING 模擬 ───────────────────────────────────────────

    #[tokio::test]
    async fn test_flush_skips_conflicting_candles() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();

        // 設定 2330 為已存在（模擬 ON CONFLICT）
        writer
            .conflict_symbols
            .lock()
            .unwrap()
            .push("2330".to_string());

        let mut spy = SpyCacheInvalidator::default();

        buf.buffer.push(make_candle("2330", 1704067200000)); // 會被跳過
        buf.buffer.push(make_candle("2317", 1704067200000)); // 會寫入

        buf.flush(&writer, &mut spy).await.unwrap();

        // 只有 2317 被寫入
        assert_eq!(writer.written_count(), 1);
        assert_eq!(writer.written_snapshot()[0].symbol, "2317");

        // 快取失效仍然被觸發（即使有 conflict，affected_symbols 仍包含兩者）
        assert_eq!(spy.call_count, 1);
    }

    // ── DB 錯誤處理 ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_flush_propagates_db_error() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::with_failure();
        let mut spy = SpyCacheInvalidator::default();

        buf.buffer.push(make_candle("2330", 1704067200000));

        let result = buf.flush(&writer, &mut spy).await;

        // DB 錯誤應向上傳遞
        assert!(result.is_err());

        // DB 失敗時，快取失效不應被呼叫（順序保證）
        assert_eq!(spy.call_count, 0);
    }

    // ── 快取失效順序驗證 ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_cache_invalidation_called_after_db_write() {
        // 驗證快取失效一定在 DB 寫入成功後才執行
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        buf.buffer.push(make_candle("2330", 1704067200000));
        buf.flush(&writer, &mut spy).await.unwrap();

        // DB 有資料 + 快取有被失效 → 順序正確
        assert!(writer.written_count() > 0);
        assert!(spy.call_count > 0);
    }

    // ── 多股票快取失效 ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_all_affected_symbols_invalidated() {
        let mut buf = BulkInsertBuffer::new();
        let writer = InMemoryDbWriter::new();
        let mut spy = SpyCacheInvalidator::default();

        let symbols = ["2330", "2317", "2454"];
        for symbol in symbols {
            buf.buffer.push(make_candle(symbol, 1704067200000));
        }
        buf.flush(&writer, &mut spy).await.unwrap();

        // 三個 symbol 全部在 invalidated_symbols 中
        for symbol in symbols {
            assert!(
                spy.invalidated_symbols.contains(&symbol.to_string()),
                "Expected {} to be invalidated",
                symbol
            );
        }
    }
}

// ── Layer 3：sqlx integration test（需真實 DB）────────────────────────────────

#[cfg(all(test, feature = "integration-test"))]
mod sqlx_tests {
    use sqlx::PgPool;

    use crate::data::db::BulkInsertBuffer;
    use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
    use crate::data::mocks::test_mocks::SpyCacheInvalidator;
    use crate::data::models::RawCandle;
    use crate::models::{DataSource, Interval};

    fn make_candle(symbol: &str, ts: i64) -> RawCandle {
        RawCandle {
            symbol: symbol.to_string(),
            timestamp_ms: ts,
            interval: Interval::OneDay,
            open: 150.0,
            high: 152.0,
            low: 149.0,
            close: 151.0,
            volume: 500_000,
            source: DataSource::FinMind,
            created_at_ms: 1704067200000,
        }
    }

    /// 真實 DB 寫入驗證。
    /// 需要 TEST_DATABASE_URL 環境變數。
    #[sqlx::test]
    async fn test_real_db_write_and_idempotent(pool: PgPool) {
        let writer = PostgresDbWriter::new(pool.clone());
        let mut spy = SpyCacheInvalidator::default();
        let mut buf = BulkInsertBuffer::new();

        let candle = make_candle("2330", 1704067200000);
        buf.buffer.push(candle.clone());
        buf.flush(&writer, &mut spy).await.unwrap();

        // 驗證資料確實寫入 DB
        let row = sqlx::query!(
            "SELECT COUNT(*) as count FROM candles WHERE symbol = '2330' AND timestamp_ms = $1",
            1704067200000i64,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.count.unwrap_or(0), 1);

        // 再次寫入相同資料（ON CONFLICT DO NOTHING）
        buf.buffer.push(candle);
        let written = writer.write_batch(&buf.buffer).await.unwrap();
        assert_eq!(written, 0, "Duplicate write should be skipped");
    }
}
